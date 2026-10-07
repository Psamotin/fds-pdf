"""CI-only gate for the pinned Tesseract's model path before expensive Rust builds."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

from package import REPO, copy, digest, fetch, run


def main():
    if os.name != 'nt':
        raise RuntimeError('Native Windows preflight requires Windows')
    manifest = json.loads((REPO / 'packaging/fds/runtime-lock.json').read_text())
    cache = REPO / '.ocr-build-cache'
    sevenzip = shutil.which('7z')
    if not sevenzip:
        raise RuntimeError('7-Zip required on build runner')
    archive = fetch(manifest['tesseract'], cache)
    with tempfile.TemporaryDirectory(prefix='fds-ocr-preflight-') as temporary:
        root = Path(temporary)
        extracted = root / 'extracted'
        run(sevenzip, 'x', '-y', f'-o{extracted}', archive, stdout=subprocess.DEVNULL)
        tess = root / 'Проверка переносимости ФДС ПДФ/Tesseract-OCR'
        for item in manifest['tesseract']['files']:
            source = extracted / item['filename']
            if digest(source) != item['sha256']:
                raise RuntimeError(f'Runtime checksum mismatch: {source.name}')
            copy(source, tess / item['filename'])
        for item in manifest['models']:
            copy(fetch(item, cache / 'models'), tess / 'tessdata' / item['name'])
        env = dict(os.environ, PATH=os.pathsep.join([str(tess), str(Path(os.environ['SystemRoot']) / 'System32')]),
                   TESSDATA_PREFIX='tessdata')
        outputs = {}
        for argument in ['--version', '--list-langs']:
            result = subprocess.run([str(tess / 'tesseract.exe'), argument], cwd=tess, env=env,
                                    capture_output=True, text=True, encoding='utf-8', errors='replace',
                                    timeout=30, creationflags=subprocess.CREATE_NO_WINDOW)
            print(result.stdout + result.stderr, flush=True)
            result.check_returncode()
            outputs[argument] = result.stdout + result.stderr
        assert outputs['--version'].splitlines()[0].strip() == 'tesseract v' + manifest['tesseract']['version']
        assert {x.strip() for x in outputs['--list-langs'].splitlines()} >= {'rus', 'eng', 'osd'}
        print('TESSERACT_UNICODE_PREFLIGHT=PASS: exact hashes/version, isolated PATH, rus/eng/osd')


if __name__ == '__main__':
    main()
