import io
import json
import tempfile
from pathlib import Path
from unittest import TestCase
from zipfile import ZipFile

from PIL import Image
from hoi4companion.backend_core import _request
from hoi4companion._reference_designs_reference import ReferenceLibrary
from test_reference_designs import archive
from hoi4companion.reference_presets import PresetInventory
import test_reference_presets as presets


class NativeReferenceLibraryTests(TestCase):
    def test_saved_recipe_inventory_is_read_only_and_rejects_corrupt_recipes(self):
        with tempfile.TemporaryDirectory() as directory:
            path, design = presets.PresetInventoryTests().fixture(directory)
            for raw in (presets.RAW, presets.RAW.replace(b'delete', b'New group'), b'version=1',
                        b'version=0 t={category=land equipment={name=x target_variant={type=m modules={gun=a gun=b}}}}'):
                path.write_bytes(raw)
                self.assertEqual(_request(0, 'reference_library_presets', docs_dir=directory, designs=[design]),
                                 PresetInventory().inspect(directory, [design]))
                self.assertEqual(path.read_bytes(), raw)
            path.unlink()
            self.assertEqual(_request(0, 'reference_library_presets', docs_dir=directory, designs=[design]),
                             {'status': 'file_missing', 'designs': {}})
    def test_archive_recipes_image_categories_and_persistence_match(self):
        for raw in (archive(), self.unicode_jpeg_archive()):
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                source = root / 'source.zip'; source.write_bytes(raw)
                expected = ReferenceLibrary(root / 'reference').import_archive(io.BytesIO(raw))
                actual = _request(0, 'reference_library_import', path=str(source), root=str(root / 'native'))
                self.assertEqual(actual, expected)
                self.assertEqual(_request(0, 'reference_library_manifest', root=str(root / 'native')), expected)
                for image in expected['images']:
                    result = _request(0, 'reference_library_image', root=str(root / 'native'), id=image['id'])
                    self.assertEqual(bytes(result['data']), ReferenceLibrary(root / 'reference').image(image['id'])[0])
                    self.assertEqual(result['mime'], image['mime'])

    @staticmethod
    def unicode_jpeg_archive():
        raw = io.BytesIO(); picture = io.BytesIO(); Image.new('RGB', (4, 3)).save(picture, format='JPEG')
        with ZipFile(raw, 'w') as z:
            z.writestr('models.txt', 'designs={category=land equipment={name="Café 🚂" target_variant={type=tank modules={gun=cannon} upgrades={engine=2}}}}'.encode('utf8'))
            z.writestr('ARMOR DESIGNS/SITUATIONAL/Café.jpg', picture.getvalue())
        return raw.getvalue()

    def test_rejected_archive_never_replaces_existing_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); source = root / 'source.zip'; source.write_bytes(archive())
            before = _request(0, 'reference_library_import', root=str(root / 'library'), path=str(source))
            for entries in ([('bad.png', b'bad')], [('empty.txt', b'hello')],
                            [('same.txt', b'a'), ('same.txt', b'b')]):
                with ZipFile(source, 'w') as z:
                    for name, data in entries: z.writestr(name, data)
                with self.assertRaises(ValueError):
                    _request(0, 'reference_library_import', root=str(root / 'library'), path=str(source))
                self.assertEqual(json.loads((root / 'library/manifest.json').read_text(encoding='utf8')), before)
            with self.assertRaises(ValueError):
                _request(0, 'reference_library_image', root=str(root / 'library'), id='../manifest.json')
