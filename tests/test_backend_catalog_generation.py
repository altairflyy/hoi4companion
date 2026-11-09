"""A native catalog generation owns coherent settings, source data and assets."""
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from hoi4companion.backend_core import _request

class BackendCatalogGenerationTests(unittest.TestCase):
    def test_generation_respects_dlc_overlays_branches_roles_and_technology_country(self):
        with TemporaryDirectory() as folder:
            root=Path(folder);game=root/'game';docs=root/'docs';docs.mkdir()
            sources={
                'common/national_focus/f.txt':'focus_tree={id=generic default=yes country={factor=1} focus={id=start x=0 y=0} focus={id=dlc x=1 y=1 allow_branch={has_dlc=Unavailable} prerequisite={focus=start}} focus={id=child x=1 y=2 prerequisite={focus=dlc}}}',
                'common/technology_tags/t.txt':'technology_folders={infantry_folder={ledger=army}}',
                'common/technologies/t.txt':'technologies={tech={folder={name=infantry_folder position={x=0 y=0}} start_year=1936}}',
                'common/scripted_localisation/n.txt':'defined_text={name=GetTitle text={localization_key=TITLE} text={localization_key=TITLE}}',
                'common/ideas/l.txt':'ideas={economy={law=yes civilian={}}}',
                'localisation/base_l_english.yml':'l_english:\n start:0 "Start"\n tech:0 "Base Tech"\n TITLE:0 "A Title"\n',
            }
            for name,text in sources.items():
                path=game/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf8')
            (docs/'dlc_load.json').write_text('{"enabled_mods":[],"disabled_dlcs":[]}')
            handle=_request(0,'catalog_open',settings={'game_dir':str(game),'docs_dir':str(docs),'country':'HUN','language':'english','mods':'auto'},cache=str(root/'cache'))
            self.addCleanup(lambda:_request(0,'catalog_release',handle=handle))
            value=_request(0,'catalog_sources',handle=handle)
            self.assertFalse(value['demo'])
            self.assertEqual(value['scripted_names'],{'GetTitle':['TITLE']})
            self.assertEqual([v['id'] for v in value['excluded_focuses']],['child','dlc'])
            self.assertEqual(value['trees']['generic']['focuses']['start']['name'],'Start')
            self.assertEqual(value['technologies']['technologies']['tech']['name'],'Base Tech')
            self.assertEqual(value['operation_entities'][0]['kind'],'law')
            self.assertEqual(value['mod_inventory']['roots'],[])
            self.assertEqual(value['all_trees']['generic'],value['trees']['generic'])
            identities=_request(0,'catalog_entities',handle=handle)
            self.assertTrue(any(row['id']=='civilian' for row in identities['entries']))
            self.assertFalse(any(row['id']=='dlc' for row in identities['entries']))
            self.assertEqual(identities,_request(0,'catalog_entities',handle=handle))
            guide={'id':'dlc','title':'Unavailable content','applies_to':{'country':'HUN','tree_id':'generic'},'tracks':{'focus':[{'key':'dlc','action':{'kind':'select_focus','entity':{'kind':'focus','id':'dlc','scope':'generic'}}}]}}
            compatibility=_request(0,'catalog_compatibility',handle=handle,guide=guide)
            self.assertTrue(compatibility['blocked'])
            self.assertEqual(compatibility['requirements'],[{'enable':['Unavailable'],'disable':[]}])
            from test_word_guides import word
            import zipfile
            archive=root/'guides.zip'
            with zipfile.ZipFile(archive,'w') as output:output.writestr('Country Guides/Example.docx',word([[],[],['dlc']]))
            inventory=_request(0,'catalog_word_inventory',handle=handle,path=str(archive))
            self.assertTrue(inventory['compatibility']['Country Guides/Example.docx']['blocked'])

    def test_demo_has_no_installed_or_live_identity_assumptions(self):
        root=Path(__file__).resolve().parents[1]/'src/hoi4companion/examples'
        handle=_request(0,'catalog_open',settings={'game_dir':'','country':'AUS'},demo_file=str(root/'demo_focus.txt'),demo_labels=str(root/'demo_labels.json'))
        self.addCleanup(lambda:_request(0,'catalog_release',handle=handle))
        value=_request(0,'catalog_sources',handle=handle)
        self.assertTrue(value['demo']);self.assertIsNone(value['dlcs']);self.assertEqual(value['operation_entities'],[])
        self.assertTrue(value['trees']);self.assertFalse(value['mod_roots'])
