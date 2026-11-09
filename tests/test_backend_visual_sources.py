"""GUI source composition uses the same owned sprite generation as HTTP."""
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
import struct
from hoi4companion.backend_core import _request
from hoi4companion._focus_visuals_reference import load_focus_visuals
from test_focus_visuals import GUI
from hoi4companion.sprites import SpriteLibrary
from hoi4companion._technology_visuals_reference import load_technology_visuals

class BackendVisualSourcesTests(unittest.TestCase):
    def test_technology_textures_are_measured_only_when_gui_referenced(self):
        with TemporaryDirectory() as folder:
            root=Path(folder);(root/'interface').mkdir();(root/'gfx').mkdir()
            texture=root/'gfx/used.dds';header=bytearray(128);header[:4]=b'DDS ';struct.pack_into('<II',header,12,72,144);texture.write_bytes(header)
            gfx=root/'interface/view.gfx';gfx.write_text('spriteTypes={spriteType={name=unused textureFile="gfx/unread.dds"} frameAnimatedSpriteType={name=GFX_technology_x_folder_unavailable_item_bg textureFile="gfx/used.dds" noOfFrames=2}}')
            gui=root/'interface/view.gui';gui.write_text('guiTypes={containerWindowType={name=techtree_x_folder_item iconType={name=Icon position={x=30 y=20}}}}')
            expected=load_technology_visuals([gui],[gfx])
            result=_request(0,'source_technology_visuals',gui_files=[str(gui)],gfx_files=[str(gfx)])
            self.assertEqual(result['value'],expected)
            self.assertEqual(result['header_reads'],1)

    def test_focus_links_use_qualified_atlas_frames_without_decoding_textures(self):
        with TemporaryDirectory() as folder:
            root=Path(folder);(root/'interface').mkdir();(root/'gfx').mkdir()
            path=root/'interface/focus.gui';path.write_text(GUI)
            header=bytearray(128);header[:4]=b'DDS ';struct.pack_into('<II',header,12,24,96)
            (root/'gfx/link.dds').write_bytes(header)
            (root/'interface/focus.gfx').write_text('spriteTypes={spriteType={name=GFX_focus_link_up_down texturefile="gfx/link.dds" noOfFrames=4} spriteType={name=GFX_focus_link_exclusive texturefile="gfx/link.dds" noOfFrames=4}}')
            library=SpriteLibrary(root,[],root/'cache')
            expected=load_focus_visuals([path],library)
            value=_request(0,'source_focus_visuals',files=[str(path)],assets_handle=library._handle)
            self.assertEqual(value,expected)
            self.assertFalse((root/'cache').exists())

    def test_focus_geometry_templates_and_unknown_metrics_match_source(self):
        with TemporaryDirectory() as folder:
            path=Path(folder)/'focus.gui';path.write_text(GUI)
            expected=load_focus_visuals([path])
            value=_request(0,'source_focus_visuals',files=[str(path)])
            self.assertEqual(value,expected)
