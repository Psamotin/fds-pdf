"""Regression checks for source archives used by portable Windows packaging."""
import gzip
import io
from pathlib import Path
import tarfile
import tempfile
import unittest

from package import notices_from_archive


class SourceNotices(unittest.TestCase):
    def test_gzip_header_and_license_directories(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / 'pypdfium2.tar.gz'
            # The gzip inner filename must not control licence discovery.
            with archive.open('wb') as output:
                with gzip.GzipFile(filename='unexpected-name', mode='wb', fileobj=output) as compressed:
                    with tarfile.open(fileobj=compressed, mode='w|') as tar:
                        for name in ['src/LICENSES/BSD-3-Clause.txt', 'src/BUILD_LICENSES/pdfium.txt',
                                     'src/leptonica-license.txt', 'src/code.py']:
                            content = name.encode()
                            member = tarfile.TarInfo(name)
                            member.size = len(content)
                            tar.addfile(member, io.BytesIO(content))
            destination = root / 'notices'
            notices_from_archive(archive, destination)
            self.assertEqual(sorted(str(p.relative_to(destination)) for p in destination.rglob('*') if p.is_file()),
                             ['src/BUILD_LICENSES/pdfium.txt', 'src/LICENSES/BSD-3-Clause.txt',
                              'src/leptonica-license.txt'])
            self.assertEqual((destination / 'src/BUILD_LICENSES/pdfium.txt').read_bytes(),
                             b'src/BUILD_LICENSES/pdfium.txt')

    def test_empty_archive_cannot_pass_with_stale_destination(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / 'empty.tar.gz'
            with tarfile.open(archive, 'w:gz'):
                pass
            destination = root / 'old'
            destination.mkdir()
            (destination / 'LICENSE').write_text('old')
            with self.assertRaisesRegex(RuntimeError, 'No licence/copyright notice'):
                notices_from_archive(archive, destination)


if __name__ == '__main__':
    unittest.main()
