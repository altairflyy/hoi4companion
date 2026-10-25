from pathlib import Path
import struct
import tempfile
import unittest

from hoi4companion.focus_visuals import load_focus_visuals
from hoi4companion.sprites import SpriteLibrary


GUI = '''guiTypes={
 positionType={name=focus_spacing position={x=96 y=130}}
 positionType={name=link_begin position={x=80 y=64}}
 positionType={name=link_end position={x=80 y=0}}
 containerWindowType={name=national_focus_item size={width=165 height=128}
   buttonType={name=symbol position={x=5 y=-44} centerposition=yes Orientation=CENTER}
   buttonType={name=bg position={x=5 y=40}}
   instantTextboxType={name=name position={x=15 y=58} maxWidth=147 maxHeight=20 font=hoi_16mbs}}
 containerWindowType={name=national_focus_exclusive_item position={x=-5 y=28}
   iconType={name=mid spriteType=GFX_focus_link_exclusive frame=1}}
}'''


class FocusVisualsTests(unittest.TestCase):
    def test_gui_geometry_and_center_anchor_are_preserved_without_gameplay_inference(self):
        with tempfile.TemporaryDirectory() as tmp:
            file=Path(tmp)/'focus.gui';file.write_text(GUI)
            data=load_focus_visuals([file])
            self.assertEqual(data['status'],'observed')
            self.assertEqual(data['positions']['focus_spacing']['y'],130)
            item=data['templates']['national_focus_item']
            self.assertEqual(item['size'],{'width':165,'height':128})
            self.assertEqual(item['children']['symbol']['orientation'],'CENTER')
            self.assertTrue(item['children']['symbol']['center'])
            self.assertEqual(item['children']['name']['width'],147)
            self.assertEqual(data['states'],{})
            self.assertNotIn('available',data)

    def test_custom_mod_filename_overrides_positions_and_links_without_replacing_other_fields(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);base=root/'base.gui';mod=root/'my_mod.gui'
            base.write_text(GUI)
            mod.write_text('''guiTypes={positionType={name=focus_spacing position={x=110 y=150}}
              positionType={name=link_end position={x=90 y=-8}}}''')
            result=load_focus_visuals([base,mod])
            self.assertEqual(result['positions']['focus_spacing']['x'],110)
            self.assertEqual(result['positions']['link_end']['y'],-8)
            self.assertEqual(result['positions']['link_begin']['x'],80)
            self.assertEqual(result['templates']['national_focus_item']['size']['width'],165)

    def test_invalid_override_does_not_reuse_previous_geometry_as_current(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);base=root/'base.gui';mod=root/'mod.gui'
            base.write_text(GUI);mod.write_text('guiTypes={positionType={name=focus_spacing position={x=oops y=130}}}')
            result=load_focus_visuals([base,mod])
            self.assertEqual(result['status'],'unavailable')
            self.assertNotIn('focus_spacing',result['positions'])

    def test_parse_failure_and_unrelated_documents_are_distinguished(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);bad=root/'bad.gui';other=root/'other.gui'
            bad.write_text('guiTypes={containerWindowType={name=national_focus_item')
            other.write_text('irrelevant broken file={')
            result=load_focus_visuals([bad,other])
            self.assertEqual(len(result['warnings']),1)
            self.assertEqual(result['status'],'unavailable')

    def test_original_atlas_frames_and_texture_dimensions_use_existing_mod_resolution(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);(root/'interface').mkdir();(root/'gfx').mkdir()
            gui=root/'interface/focus.gui';gui.write_text(GUI)
            header=bytearray(128);header[:4]=b'DDS ';struct.pack_into('<II',header,12,24,72)
            (root/'gfx/exclusive.dds').write_bytes(header)
            (root/'interface/focus.gfx').write_text('''spriteTypes={spriteType={
              name=GFX_focus_link_exclusive textureFile="gfx/exclusive.dds" noOfFrames=3}}''')
            line=bytearray(128);line[:4]=b'DDS ';struct.pack_into('<II',line,12,16,64)
            (root/'gfx/line.dds').write_bytes(line)
            with (root/'interface/focus.gfx').open('a') as stream:
                stream.write('spriteTypes={spriteType={name=GFX_focus_link_up_down textureFile="gfx/line.dds" noOfFrames=4}}')
            library=SpriteLibrary(root,[],root/'cache')
            data=load_focus_visuals([gui],library)
            asset=data['templates']['national_focus_exclusive_item']['children']['mid']['sprite']
            self.assertEqual((asset['width'],asset['height'],asset['frame']),(24,24,1))
            self.assertTrue(asset['url'].startswith('/api/sprite/'))
            self.assertEqual(data['links']['up_down']['frame'],3)
            self.assertEqual(data['link_states']['completed']['up_down']['frame'],1)
            self.assertEqual(data['link_states']['alternative']['up_down']['frame'],4)
            self.assertNotEqual(data['links']['up_down']['url'],data['link_states']['completed']['up_down']['url'])
            self.assertFalse((root/'cache').exists())  # No complete texture conversion here.
