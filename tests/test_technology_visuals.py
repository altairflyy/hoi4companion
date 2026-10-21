from pathlib import Path
import struct
import tempfile
import unittest

from hoi4companion.technology_visuals import load_technology_visuals


class TechnologyVisualsTest(unittest.TestCase):
    def test_native_texture_geometry_overrides_placeholder_gui_size(self):
        with tempfile.TemporaryDirectory() as tmp:
            game = Path(tmp)
            interface = game / 'interface'
            interface.mkdir()
            textures = game / 'gfx'
            textures.mkdir()
            for name, width, height in [('navy', 332, 69), ('square', 72, 72), ('active', 648, 72), ('backdrop', 1000, 400)]:
                header = bytearray(128)
                header[:4] = b'DDS '
                struct.pack_into('<II', header, 12, height, width)
                (textures / f'{name}.dds').write_bytes(header)
            gfx = interface / 'view.gfx'
            gfx.write_text('''spriteTypes={
              spriteType={name=GFX_technology_mtgnavalfolder_unavailable_item_bg textureFile="gfx/navy.dds"}
              spriteType={name=GFX_technology_industry_folder_unavailable_item_bg textureFile="gfx/square.dds"}
              frameAnimatedSpriteType={name=GFX_technology_industry_folder_currently_researching_item_bg texturefile="gfx/active.dds" noOfFrames=9}
              spriteType={name=GFX_industry_techtree_bg textureFile="gfx/backdrop.dds"}
            }''', encoding='utf-8')
            gui = interface / 'view.gui'
            gui.write_text('''guiTypes={
              containerWindowType={name=techtree_mtgnavalfolder_item position={x=-131 y=1} size={width=204 height=72}
                iconType={name=Icon position={x=167 y=34} centerposition=yes}
                instantTextBoxType={name=Name position={x=3 y=-1} maxWidth=270 maxHeight=20}}
              containerWindowType={name=techtree_industry_folder_item size={width=204 height=72}
                iconType={name=Icon position={x=34 y=35} centerposition=yes}}
              containerWindowType={name=industry_folder position={x=0 y=47}
                containerWindowType={name=size_filler position={x=5 y=8}
                  iconType={name=bg spriteType=GFX_industry_techtree_bg position={x=7 y=9}}}}
            }''', encoding='utf-8')
            data = load_technology_visuals([gui], [gfx])
            navy = data['templates']['techtree_mtgnavalfolder_item']
            self.assertEqual(navy['size'], {'width': 332, 'height': 69})
            self.assertEqual(navy['declared_size']['width'], 204)
            self.assertEqual(navy['variant'], 'equipment')
            industry = data['templates']['techtree_industry_folder_item']
            self.assertEqual(industry['size']['width'], 72)
            self.assertEqual(industry['variant'], 'small')
            self.assertEqual(industry['states']['currently_researching']['width'], 72)
            self.assertEqual(industry['states']['currently_researching']['frame'], 1)
            layer = data['folders']['industry_folder']['layers'][0]
            self.assertEqual((layer['x'], layer['y']), (12, 17))
            self.assertEqual((layer['width'], layer['height']), (1000, 400))

    def test_missing_invalid_resources_do_not_invent_dimensions_or_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)
            (path / 'interface').mkdir()
            gfx, gui = path / 'interface/view.gfx', path / 'interface/view.gui'
            gfx.write_text('spriteTypes={spriteType={name=GFX_technology_x_folder_unavailable_item_bg textureFile="../outside.dds"}}')
            gui.write_text('guiTypes={containerWindowType={name=techtree_x_folder_item size={width=50 height=60} iconType={name=Icon}}}')
            data = load_technology_visuals([gui], [gfx])
            template = data['templates']['techtree_x_folder_item']
            self.assertEqual(template['size'], {'width': 50, 'height': 60})
            self.assertNotIn('width', template['states']['unavailable'])
            self.assertNotIn('available', template['states'])

    def test_parse_errors_are_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'bad.gui'
            path.write_text('guiTypes={')
            result = load_technology_visuals([path], [])
            self.assertTrue(result['warnings'])
            self.assertFalse(result['templates'])


if __name__ == '__main__':
    unittest.main()
