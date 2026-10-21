from io import BytesIO
from pathlib import Path
import struct
import tempfile
import unittest

from PIL import Image

from hoi4companion.sprites import SpriteLibrary
from hoi4companion.clausewitz import parse
from hoi4companion.focus import _build_focus


class SpriteTest(unittest.TestCase):
    def make_texture(self, path, color, size=(8, 4), kind='DDS'):
        path.parent.mkdir(parents=True, exist_ok=True)
        with Image.new('RGBA', size, color) as image:
            image.save(path, format=kind)

    def definition(self, root, texture='gfx/icon.dds', frames=1):
        folder = root / 'interface'
        folder.mkdir(parents=True, exist_ok=True)
        (folder / 'goals.gfx').write_text('spriteTypes={SpriteType={name="GFX_test" textureFile="'+texture+'" noOfFrames='+str(frames)+'}}')

    def test_decode_dds_alpha_and_cropped_frame(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td)/'game'
            self.definition(root, frames=2)
            self.make_texture(root/'gfx/icon.dds', (240, 15, 5, 128))
            library=SpriteLibrary(root, [], Path(td)/'cache')
            sprite=library.resolve('GFX_test', 2)
            data=library.png(sprite['url'].split('/')[-1][:-4])
            with Image.open(BytesIO(data)) as image:
                self.assertEqual(image.size, (4, 4))
                self.assertEqual(image.getpixel((0, 0)), (240, 15, 5, 128))
            self.assertIsNone(library.resolve('GFX_test', 0))
            self.assertIsNone(library.resolve('GFX_test', 3))

    def test_texture_only_mod_override_wins_and_png_declared_dds_is_resolved(self):
        with tempfile.TemporaryDirectory() as td:
            game, mod=Path(td)/'game', Path(td)/'mod'
            self.definition(game, 'gfx/icon.png')
            self.make_texture(game/'gfx/icon.png', (255,0,0,255), kind='PNG')
            self.make_texture(mod/'gfx/icon.dds', (0,255,0,255))
            library=SpriteLibrary(game, [mod], Path(td)/'cache')
            sprite=library.resolve('GFX_test')
            self.assertTrue(sprite['override'])
            with Image.open(BytesIO(library.png(sprite['url'].split('/')[-1][:-4]))) as image:
                self.assertEqual(image.getpixel((0,0)), (0,255,0,255))

    def test_http_tokens_do_not_select_arbitrary_paths(self):
        with tempfile.TemporaryDirectory() as td:
            game=Path(td)/'game'
            self.make_texture(Path(td)/'secret.dds', (1,2,3,4))
            self.definition(game, '../secret.dds')
            library=SpriteLibrary(game, [], Path(td)/'cache')
            self.assertIsNone(library.resolve('GFX_test'))
            self.assertIsNone(library.png('../secret'))
            self.assertIsNone(library.png('f'*64))
            self.assertFalse((Path(td)/'cache').exists())

    def test_replace_path_prevents_fallback_to_base_texture(self):
        with tempfile.TemporaryDirectory() as td:
            game, mod=Path(td)/'game',Path(td)/'mod'
            self.definition(game)
            self.make_texture(game/'gfx/icon.dds',(1,2,3,4))
            mod.mkdir()
            (mod/'descriptor.mod').write_text('replace_path="gfx"')
            library=SpriteLibrary(game,[mod],Path(td)/'cache')
            self.assertIsNone(library.resolve('GFX_test'))
            self.make_texture(mod/'gfx/icon.dds',(4,3,2,1))
            library=SpriteLibrary(game,[mod],Path(td)/'cache')
            self.assertTrue(library.resolve('GFX_test')['override'])

    def test_dx10_bgra_srgb_decodes_without_changing_source(self):
        header=bytearray(148)
        header[:4]=b'DDS '
        struct.pack_into('<I', header, 4, 124)
        struct.pack_into('<III',header,12,1,2,8)
        header[84:88]=b'DX10'
        struct.pack_into('<IIII',header,128,91,3,0,1)
        raw=bytes(header)+bytes([3,2,1,128,6,5,4,255])
        with SpriteLibrary._decode(raw) as image:
            self.assertEqual(image.size,(2,1))
            self.assertEqual(list(image.getdata()),[(1,2,3,128),(4,5,6,255)])
        with self.assertRaises(ValueError):
            SpriteLibrary._decode(raw[:-2])

    def test_bad_frame_geometry_and_decode_failure_are_explicit(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td)/'game'
            self.definition(root, frames=3)
            self.make_texture(root/'gfx/icon.dds', (1,2,3,4))
            library=SpriteLibrary(root, [], Path(td)/'cache')
            token=library.resolve('GFX_test')['url'].split('/')[-1][:-4]
            self.assertIsNone(library.png(token))
            self.assertIn(token,library.failed)

    def test_inline_icon_frames_keep_zero_unresolved(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td)/'game'
            self.definition(root, frames=2)
            self.make_texture(root/'gfx/icon.dds',(1,2,3,4))
            library=SpriteLibrary(root, [], Path(td)/'cache')
            result=library.inline(['£test|2 £test|0 £test£'])
            self.assertEqual(set(result), {'£test|2','£test£'})
            self.assertEqual(result['£test|2']['frame'],2)

    def test_conditional_focus_icon_is_not_claimed_current(self):
        focus=_build_focus(parse('id=AUS_test icon={GFX_a={has_war=yes} GFX_b=yes}'),{},'fixture.txt')
        self.assertEqual(focus.icon,'')
        self.assertEqual(focus.icon_variants,['GFX_a','GFX_b'])


if __name__=='__main__':
    unittest.main()
