"""Technology source projections preserve folders, paths, variables and DLCs."""
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from hoi4companion.backend_core import _request
from hoi4companion._technologies_reference import load_technologies
from hoi4companion.dlc import DlcInventory

class BackendTechnologySourceTests(unittest.TestCase):
    def test_folders_variables_equipment_names_unknown_and_disabled_branches(self):
        with TemporaryDirectory() as folder:
            source,tag=(Path(folder)/name for name in ('technology.txt','folders.txt'))
            tag.write_text('technology_folders={old={available={NOT={has_dlc=Pack}}} new={available={has_dlc=Pack}} shared={} doctrine={doctrine=yes}}')
            source.write_text('''@year=1936 technologies={@cost=1.5 @alias=@year
            tech={research_cost=@cost start_year=@alias folder={name=old position={x=1 y=2}} folder={name=new} folder={name=shared} path={leads_to_tech=next research_cost_coeff=0.9} XOR={next} sub_technologies={child} dependencies={outside=1} enable_equipments={rifle} enable_equipment_modules={scope} allow={runtime=yes}}
            next={folder={name=shared}} hidden={allow_branch={has_dlc=Missing}}
            child={is_special_project_tech=yes}}''')
            labels={'GER_rifle':'German rifle','rifle_desc':'Description','new':'Naval'}
            for inventory in (DlcInventory(items=[{'name':'Pack','status':'enabled'}]),DlcInventory(complete=False),None):
                expected=load_technologies([source],[tag],labels,inventory,{},'GER')
                native=_request(0,'technology_sources',files=[str(source)],tag_files=[str(tag)],labels=labels,
                    dlcs={'items':inventory.items,'complete':inventory.complete} if inventory else None,scripts={},country='GER')
                self.assertEqual(native['catalog'],expected)
