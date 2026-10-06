#!/usr/bin/env python3
"""Build-time Windows packaging only; never shipped as a runtime launcher."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import urllib.request
import zipfile

REPO = Path(__file__).resolve().parents[2]
VERSION = '0.2.0-fds.1-rc1'
NAME = f'FDS-PDF-{VERSION}-windows-x64'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def fetch(item, cache):
    target = cache / item.get('filename', item.get('name'))
    target.parent.mkdir(parents=True, exist_ok=True)
    if not target.exists() or digest(target) != item['sha256']:
        pending = target.with_suffix(target.suffix + '.download')
        urls = [item['url'], *item.get('fallback_urls', [])]
        for url in urls:
            try:
                with urllib.request.urlopen(url, timeout=120) as response, pending.open('wb') as output:
                    shutil.copyfileobj(response, output)
                if digest(pending) != item['sha256']:
                    raise RuntimeError(f'Checksum mismatch: {target.name}')
                pending.replace(target)
                break
            except Exception:
                pending.unlink(missing_ok=True)
                if url == urls[-1]:
                    raise
    if digest(target) != item['sha256']:
        raise RuntimeError(f'Checksum mismatch: {target.name}')
    return target


def run(*args, **kwargs):
    subprocess.run([str(arg) for arg in args], check=True, **kwargs)


def copy(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)


def notices_from_archive(archive, destination, sevenzip):
    """Sources are preserved intact; also expose their licence texts without unpacking code in the app."""
    with tempfile.TemporaryDirectory(prefix='fds-source-notices-') as temporary:
        root = Path(temporary)
        run(sevenzip, 'x', '-y', f'-o{root}', archive, stdout=subprocess.DEVNULL)
        # 7-Zip decompresses tar.gz/xz/lz to a tar, then extracts the tar.
        for tar in list(root.glob('*.tar')):
            run(sevenzip, 'x', '-y', f'-o{root / "source"}', tar, stdout=subprocess.DEVNULL)
        for path in root.rglob('*'):
            if path.is_file() and path.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'COPYRIGHT', 'NOTICE', 'AUTHORS')):
                copy(path, destination / path.relative_to(root))
        if not destination.exists():
            raise RuntimeError(f'No licence/copyright notice found in {archive.name}')


def package(args):
    if os.name != 'nt':
        raise RuntimeError('Windows packaging and portability smoke require a Windows runner')
    manifest = json.loads((REPO / 'packaging/fds/runtime-lock.json').read_text(encoding='utf-8'))
    cache = args.cache.resolve()
    cache.mkdir(parents=True, exist_ok=True)
    dist = args.dist.resolve()
    bundle = dist / NAME
    if bundle.exists():
        raise RuntimeError(f'Refusing to overwrite an existing bundle: {bundle}')
    bundle.mkdir(parents=True)
    sevenzip = shutil.which('7z')
    if not sevenzip:
        raise RuntimeError('7-Zip required on build runner')
    python = bundle / 'OCR/Python'
    tesseract = bundle / 'OCR/Tesseract-OCR'
    licenses = bundle / 'LICENSES'
    sources = licenses / 'SOURCES'
    sources.mkdir(parents=True)
    shutil.copytree(REPO / 'packaging/fds/licenses/win32job', licenses / 'Rust/win32job-2.0.3')
    copy(REPO / 'target/release/printcraft.exe', bundle / 'FDS-PDF.exe')
    copy(REPO / 'assets/fds-pdf/fds-pdf.ico', bundle / 'FDS-PDF.ico')
    for name in ['LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE', 'ATTRIBUTION.md', 'ATTRIBUTION.toml']:
        copy(REPO / name, bundle / name)
    import tomllib
    attribution = tomllib.loads((REPO / 'ATTRIBUTION.toml').read_text(encoding='utf-8'))
    # Preserve licence_file paths exactly, including the source-icon contributor licence.
    def collect(value):
        if isinstance(value, dict):
            for key, item in value.items():
                if key == 'licence_file':
                    copy(REPO / item, bundle / item)
                else:
                    collect(item)
        elif isinstance(value, list):
            for item in value:
                collect(item)
    collect(attribution)
    with tempfile.TemporaryDirectory(prefix='fds-ocr-build-') as temporary:
        work = Path(temporary)
        archive = fetch(manifest['python'], cache)
        with zipfile.ZipFile(archive) as zipped:
            zipped.extractall(work / 'python-installer')
        installers = list((work / 'python-installer').glob('python-*.exe'))
        if len(installers) != 1:
            raise RuntimeError('Expected one official Python installer')
        # CI-only installation into the portable bundle; no registration/PATH/shortcuts/launcher.
        run(installers[0], '/quiet', 'InstallAllUsers=0', f'TargetDir={python}', 'Include_launcher=0',
            'Include_pip=1', 'Include_test=0', 'Include_tcltk=0', 'PrependPath=0', 'Shortcuts=0',
            'AssociateFiles=0', 'Include_doc=0', 'Include_dev=0', 'Include_tools=0')
        (python / 'python313._pth').write_text('.\nLib\nDLLs\nLib/site-packages\nimport site\n', encoding='ascii')
        for name in ['python.exe', 'python3.dll', 'python313.dll', 'vcruntime140.dll', 'vcruntime140_1.dll', 'LICENSE.txt']:
            if not (python / name).is_file():
                raise RuntimeError(f'Missing portable Python component: {name}')
        copy(python / 'LICENSE.txt', licenses / 'Python/LICENSE.txt')
        wheels = cache / 'wheels'
        wheels.mkdir(exist_ok=True)
        for wheel in manifest['wheels']:
            archive = fetch(wheel, wheels)
            with zipfile.ZipFile(archive) as zipped:
                for name in zipped.namelist():
                    base = Path(name).name.upper()
                    if not name.endswith('/') and ('/licenses/' in name or base.startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'AUTHORS')) or base == 'METADATA'):
                        target = licenses / 'Python-packages' / wheel['name'] / name
                        target.parent.mkdir(parents=True, exist_ok=True)
                        target.write_bytes(zipped.read(name))
        run(python / 'python.exe', '-I', '-m', 'pip', 'install', '--no-index', '--find-links', wheels,
            '--require-hashes', '--only-binary=:all:', '--no-deps', '--disable-pip-version-check',
            '-r', REPO / 'packaging/fds/requirements-windows.lock')
        run(python / 'python.exe', '-I', '-m', 'pip', 'check')
        archive = fetch(manifest['tesseract'], cache)
        extracted = work / 'tesseract'
        run(sevenzip, 'x', '-y', f'-o{extracted}', archive, stdout=subprocess.DEVNULL)
        tesseract.mkdir(parents=True)
        for item in manifest['tesseract']['files']:
            path = extracted / item['filename']
            if not path.is_file() or digest(path) != item['sha256']:
                raise RuntimeError(f'Tesseract runtime component mismatch: {item["filename"]}')
            copy(path, tesseract / path.name)
        for name in ['configs', 'tessconfigs']:
            shutil.copytree(extracted / 'tessdata' / name, tesseract / 'tessdata' / name)
        if digest(extracted / 'tessdata/pdf.ttf') != manifest['tesseract']['pdf_font']['sha256']:
            raise RuntimeError('Tesseract glyphless font checksum mismatch')
        copy(extracted / 'tessdata/pdf.ttf', tesseract / 'tessdata/pdf.ttf')
        # Unmodified runtime font, Apache-2.0, part of the pinned upstream Tesseract distribution.
        shutil.copytree(extracted / 'doc', licenses / 'Tesseract-upstream')
        for item in manifest['models']:
            copy(fetch(item, cache / 'models'), tesseract / 'tessdata' / item['name'])
        for item in [*manifest['python_sources'], *manifest['native_sources']]:
            archive = fetch(item, cache / 'sources')
            copy(archive, sources / item['filename'])
            notices_from_archive(archive, licenses / 'Source-notices' / item['name'], sevenzip)
    for item in manifest['extra_notices']:
        copy(fetch(item, cache / 'notices'), licenses / 'Native-notices' / item['filename'])
    for name in ['runtime-lock.json', 'requirements-windows.lock']:
        copy(REPO / 'packaging/fds' / name, licenses / name)
    copy(REPO / 'packaging/fds/README.txt', bundle / 'README.txt')
    copy(REPO / 'packaging/fds/THIRD-PARTY-NOTICES.txt', bundle / 'THIRD-PARTY-NOTICES.txt')
    # Move away from the install/build path before running any final checks.
    moved = dist / 'Проверка переносимости ФДС ПДФ' / NAME
    moved.parent.mkdir(parents=True, exist_ok=True)
    shutil.move(bundle, moved)
    run(moved / 'OCR/Python/python.exe', '-I', REPO / 'packaging/fds/smoke.py', moved)
    shutil.move(moved, bundle)
    moved.parent.rmdir()
    inventory = {str(path.relative_to(bundle)).replace('\\', '/'): {'bytes': path.stat().st_size, 'sha256': digest(path)}
                 for path in sorted(bundle.rglob('*')) if path.is_file()}
    (bundle / 'SHA256SUMS.json').write_text(json.dumps(inventory, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    zip_path = dist / f'{NAME}.zip'
    with zipfile.ZipFile(zip_path, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as zipped:
        for path in sorted(bundle.rglob('*')):
            if path.is_file():
                zipped.write(path, str(path.relative_to(dist)))
    report = {'artifact': zip_path.name, 'bytes': zip_path.stat().st_size, 'sha256': digest(zip_path),
              'python': manifest['python']['version'], 'ocrmypdf': '17.4.0', 'tesseract': manifest['tesseract']['version'],
              'languages': ['rus', 'eng', 'osd'], 'runtime_smoke': 'PASS'}
    (dist / 'build-report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report, indent=2))
    if summary := os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(summary, 'a', encoding='utf-8') as stream:
            stream.write('```json\n' + json.dumps(report, indent=2) + '\n```\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dist', type=Path, default=REPO / 'dist')
    parser.add_argument('--cache', type=Path, default=REPO / '.ocr-build-cache')
    package(parser.parse_args())
