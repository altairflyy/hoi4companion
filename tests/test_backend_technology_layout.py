"""Native grids match source geometry without inventing ambiguous placement."""
import copy
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.technologies import load_technologies
from hoi4companion._technology_layout_reference import apply_technology_layout
from hoi4companion.dlc import DlcInventory

class BackendTechnologyLayoutTests(unittest.TestCase):
    def test_templates_parent_percentages_paths_and_shared_descendants(self):
        with TemporaryDirectory() as folder:
            source,gui=(Path(folder)/name for name in ('source.txt','layout.gui'))
            source.write_text('''technology_folders={test={}} technologies={
              A={enable_equipments={rifle} folder={name=test position={x=0 y=0}} path={leads_to_tech=B}}
              B={folder={name=test position={x=2 y=3}}}
              C={folder={name=test position={x=1 y=1}} path={leads_to_tech=B ignore_for_layout=yes}}}''')
            gui.write_text('''guiTypes={
              containerWindowType={name=techtree_test_item position={x=-56 y=-7} size={width=183 height=84} instantTextBoxType={name=Name}}
              containerWindowType={name=folder_tabs buttonType={name=test_tab position={x=111 y=0} quadTextureSprite=GFX_tab}}
              containerWindowType={name=test margin={left=10 right=20}
                containerWindowType={position={x=10% y=20} size={width=100% height=80%}
                  instantTextBoxType={name=test_subtitle text=TITLE position={x=10 y=40}}}
                gridboxtype={name=A_tree position={x=100 y=200} slotsize={width=70 height=60} format=LEFT}
                gridboxtype={name=C_tree position={x=800 y=50} slotsize={width=70 height=60} format=UP}}}''')
            labels={'TITLE':'Section'}
            catalog=load_technologies([source],[source],labels,DlcInventory(),{})
            expected=copy.deepcopy(catalog);apply_technology_layout(expected,[gui],labels)
            result=_request(0,'technology_layout',catalog=catalog,files=[str(gui)],labels=labels)
            self.assertEqual(result,expected)
