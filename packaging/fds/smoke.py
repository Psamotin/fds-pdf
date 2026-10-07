"""Windows bundle acceptance gate: exact versions, models, relocation and synthetic OCR."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

bundle = Path(sys.argv[1]).resolve()
python = bundle / 'OCR/Python/python.exe'
tess = bundle / 'OCR/Tesseract-OCR/tesseract.exe'
assert Path(sys.executable).resolve() == python, 'Smoke must use bundled interpreter'
assert sys.version_info[:3] == (3, 13, 15), sys.version
assert sys.flags.isolated, 'Python must be isolated'
import _winapi
assert getattr(_winapi, '_fds_hidden_children', False), 'FDS child-process policy was not loaded'
import ocrmypdf
assert ocrmypdf.__version__ == '17.4.0', ocrmypdf.__version__
manifest = json.loads((Path(__file__).parent / 'runtime-lock.json').read_text())
for item in manifest['tesseract']['files']:
    path = tess.parent / item['filename']
    assert hashlib.sha256(path.read_bytes()).hexdigest() == item['sha256'], path.name
for item in manifest['models']:
    path = tess.parent / 'tessdata' / item['name']
    assert hashlib.sha256(path.read_bytes()).hexdigest() == item['sha256'], path.name
# Deliberately exclude host Python/Tesseract; keep only Windows OS dependencies.
env = {k: v for k, v in os.environ.items() if k.upper() not in {'PATH', 'PYTHONPATH', 'PYTHONHOME', 'TESSDATA_PREFIX'}}
env.update(PATH=os.pathsep.join([str(tess.parent), str(python.parent), str(python.parent / 'Scripts'),
                                str(Path(os.environ['SystemRoot']) / 'System32')]),
           TESSDATA_PREFIX=str(tess.parent / 'tessdata'), PYTHONNOUSERSITE='1', PYTHONUTF8='1')
flags = subprocess.CREATE_NO_WINDOW

def run(*args):
    result = subprocess.run([str(a) for a in args], env=env, capture_output=True, text=True, encoding='utf-8',
                            errors='replace', timeout=240, creationflags=flags)
    if result.returncode:
        raise RuntimeError(f'Runtime smoke failed ({result.returncode}): {result.stderr}')
    return result.stdout + result.stderr

# A console-subsystem Python child is launched with default flags here. The
# runtime policy must still give it no console (including nested subprocesses).
console_probe = "import ctypes,subprocess,sys; assert not ctypes.windll.kernel32.GetConsoleWindow(); subprocess.run([sys.executable,'-I','-c','import ctypes; assert not ctypes.windll.kernel32.GetConsoleWindow()'],check=True)"
probe = subprocess.run([str(python), '-I', '-c', console_probe], env=env, capture_output=True,
                       timeout=30, creationflags=0)
assert probe.returncode == 0, f'Hidden child policy failed: {probe.stderr!r}'
assert '3.13.15' in run(python, '--version')
assert '17.4.0' in run(python, '-I', '-m', 'ocrmypdf', '--version')
tesseract_banner = run(tess, '--version')
print(tesseract_banner, flush=True)
assert tesseract_banner.splitlines()[0].strip() == 'tesseract v' + manifest['tesseract']['version'], repr(tesseract_banner)
assert {x.strip() for x in run(tess, '--list-langs').splitlines()} >= {'rus', 'eng', 'osd'}
import ctypes
leptonica = ctypes.CDLL(str(tess.parent / 'libleptonica-6.dll'))
leptonica.getLeptonicaVersion.restype = ctypes.c_char_p
assert leptonica.getLeptonicaVersion().decode().startswith('leptonica-1.87.0'), 'Corresponding Leptonica source mismatch'
import pi_heif
assert pi_heif.libheif_version() == '1.23.0', pi_heif.libheif_version()
import ctypes
libde265_path = next((python.parent / 'Lib/site-packages').glob('libde265-*.dll'))
libde265 = ctypes.CDLL(str(libde265_path))
libde265.de265_get_version.restype = ctypes.c_char_p
assert libde265.de265_get_version().decode() == '1.1.1', 'Corresponding libde265 source version mismatch'
from PIL import Image, ImageDraw, ImageFont
import img2pdf
import pypdfium2
with tempfile.TemporaryDirectory(prefix='ФДС OCR проверка ') as temporary:
    folder = Path(temporary)
    image = Image.new('RGB', (1800, 1200), 'white')
    draw = ImageDraw.Draw(image)
    font = ImageFont.load_default(size=42)
    for row in range(14):
        draw.text((65, 50 + row * 75), 'FDS portable document recognition acceptance test', fill='black', font=font)
    image.save(folder / 'scan.png', dpi=(200, 200))
    source = folder / 'исходный файл.pdf'
    source.write_bytes(img2pdf.convert(str(folder / 'scan.png')))
    before = hashlib.sha256(source.read_bytes()).hexdigest()
    output = folder / 'исходный файл_OCR.pdf'
    run(python, '-I', '-m', 'ocrmypdf', '--language', 'rus+eng', '--mode', 'skip', '--rotate-pages', '--deskew',
        '--rasterizer', 'pypdfium', '--output-type', 'pdf', '--optimize', '0', source, output)
    assert hashlib.sha256(source.read_bytes()).hexdigest() == before, 'Source PDF changed'
    with pypdfium2.PdfDocument(output) as pdf:
        page = pdf[0]
        text_page = page.get_textpage()
        text = text_page.get_text_range()
        assert 'portable' in text.lower(), 'OCR output has no expected searchable text'
        text_page.close()
        page.close()
print('RUNTIME_SMOKE=PASS: exact versions; rus/eng/osd; isolated relative paths; searchable PDF; unchanged input')
