from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from hoi4companion.api import Store
from hoi4companion.dlc import DlcInventory
from hoi4companion.technologies import load_technologies


class TechnologiesTest(unittest.TestCase):
    def load(self, source, tags, dlcs=None):
        with tempfile.TemporaryDirectory() as td:
            tech, tag = Path(td) / 'technology.txt', Path(td) / 'folders.txt'
            tech.write_text(source, encoding='utf-8')
            tag.write_text(tags, encoding='utf-8')
            return load_technologies([tech], [tag], {'A': 'Localized A'}, dlcs or DlcInventory(), {})

    def test_folder_gates_and_shared_memberships(self):
        tags = '''technology_folders={old={available={not={has_dlc=Pack}}}
                  new={available={has_dlc=Pack}} shared={} doctrine={doctrine=yes}}'''
        source = '''technologies={A={folder={name=old position={x=0 y=0}}
                  folder={name=new position={x=1 y=2}} folder={name=shared}}
                  hidden={allow_branch={has_dlc=Missing} folder={name=shared}}}'''
        catalog = self.load(source, tags, DlcInventory(items=[{'name': 'Pack', 'status': 'enabled'}]))
        self.assertEqual(set(catalog['folders']), {'new', 'shared'})
        self.assertEqual(catalog['folders']['new']['technologies'], ['A'])
        self.assertNotIn('hidden', catalog['technologies'])
        self.assertEqual(catalog['technologies']['A']['name'], 'Localized A')

    def test_unknown_dlc_is_not_asserted_available(self):
        c = self.load('technologies={A={folder={name=test}}}',
                      'technology_folders={test={available={has_dlc=Pack}}}', DlcInventory(complete=False))
        self.assertFalse(c['folders'])
        self.assertEqual(c['excluded_folders'][0]['reason'], 'Disponibilità non verificata')

    def test_import_candidates_exclude_only_proven_inactive_dlc_folders(self):
        from hoi4companion.guide_importer import catalog_entities
        source='''technologies={
            landing_craft={folder={name=old} path={leads_to_tech=tank_landing_craft}}
            tank_landing_craft={folder={name=old}}
            mtg_landing_craft={folder={name=new} path={leads_to_tech=mtg_tank_landing_craft}}
            mtg_tank_landing_craft={folder={name=new}}
            multi={folder={name=old} folder={name=new}}
            child={} missing={folder={name=unlisted}}
        }'''
        tags='''technology_folders={old={available={NOT={has_dlc=Pack}}}
                                  new={available={has_dlc=Pack}}}'''
        for status,expected in [('enabled','mtg_landing_craft'),('integrated','mtg_landing_craft'),
                                ('disabled','landing_craft')]:
            with self.subTest(status=status):
                catalog=self.load(source,tags,DlcInventory(items=[{'name':'Pack','status':status}]))
                rows=catalog_entities({},catalog['technologies'],{},
                    {'landing_craft':'Landing Craft','mtg_landing_craft':'Landing Craft'})
                self.assertEqual([r['id'] for r in rows if r['name']=='Landing Craft'],[expected])
                ids={r['id'] for r in rows}
                self.assertTrue({'child','missing','multi'}<=ids)
                # Raw definitions remain available to dependency/diagnostic views.
                self.assertIn('landing_craft',catalog['technologies'])
        unknown=self.load(source,tags,DlcInventory(complete=False))
        rows=catalog_entities({},unknown['technologies'],{},
            {'landing_craft':'Landing Craft','mtg_landing_craft':'Landing Craft'})
        self.assertEqual(len([r for r in rows if r['name']=='Landing Craft']),2)

    def test_upgrades_do_not_cross_into_known_inactive_folder(self):
        from hoi4companion.guide_importer import catalog_entities
        catalog=self.load('''technologies={A={folder={name=active}
            path={leads_to_tech=OLD} path={leads_to_tech=B}}
            OLD={folder={name=inactive}} B={folder={name=active}}}''',
            'technology_folders={active={} inactive={available={always=no}}}')
        rows=catalog_entities({},catalog['technologies'],{})
        self.assertEqual(next(r for r in rows if r['id']=='A')['upgrade_successors'],['B'])

    def test_variables_cycles_xor_dependencies_subtechnologies(self):
        c = self.load('''technologies={@year=4 @alias=@year
          A={research_cost=1.5 start_year=1936 folder={name=test position={x=2 y=@alias}}
             path={leads_to_tech=B research_cost_coeff=0.9} XOR={C} sub_technologies={child}
             dependencies={outside=1} allow={ROOT={has_tech=outside}}
             enable_equipments={rifle} enable_equipment_modules={scope}}
          B={path={leads_to_tech=A} xor={A} folder={name=test}}
          child={research_cost=1 is_special_project_tech=yes sub_technologies={A}}
        }''', 'technology_folders={test={}}')
        a = c['technologies']['A']
        self.assertEqual(a['folders'][0]['y'], 4)
        self.assertEqual(a['research_cost'], 1.5)
        self.assertEqual(a['paths'], [{'target': 'B', 'coefficient': 0.9}])
        self.assertEqual(a['exclusions'], ['C'])
        self.assertEqual(c['technologies']['B']['exclusions'], ['A'])
        self.assertEqual(a['dependencies'], [{'id': 'outside', 'value': '1'}])
        self.assertTrue(a['has_conditions'])
        self.assertEqual(len(a['unlocks']), 2)
        store = Store.__new__(Store)
        from threading import RLock
        store.lock, store.technologies, store.sprites = RLock(), c, None
        store.settings = {'country': 'AUS', 'snapshot_path': 'must-not-be-read.json'}
        with patch('hoi4companion.api.read_snapshot', side_effect=AssertionError('Research must not read focus snapshot')):
            result = store.technology_tree('test')
        self.assertEqual(len(result['technologies']), 3)
        child = next(t for t in result['technologies'] if t['id'] == 'child')
        self.assertFalse(child['in_folder'])
        self.assertTrue(child['special_project'])
        self.assertNotIn('has_snapshot', result)

    def test_malformed_file_is_reported(self):
        c = self.load('technologies={A={', 'technology_folders={test={}}')
        self.assertEqual(len(c['warnings']), 1)

    def test_equipment_localisation_fallback(self):
        with tempfile.TemporaryDirectory() as td:
            file = Path(td) / 'fixture.txt'
            file.write_text('technology_folders={test={}} technologies={rifle_tech={enable_equipments={rifle} folder={name=test}}}')
            c = load_technologies([file], [file], {'AUS_rifle': 'Austrian rifle', 'rifle_desc': 'Equipment description'}, DlcInventory(), {})
        self.assertEqual(c['technologies']['rifle_tech']['name'], 'Austrian rifle')
        self.assertEqual(c['technologies']['rifle_tech']['description'], 'Equipment description')

    def test_national_equipment_name_precedes_generic_technology_label(self):
        with tempfile.TemporaryDirectory() as td:
            file = Path(td) / 'fixture.txt'
            file.write_text('technology_folders={test={}} technologies={rifle_tech={enable_equipments={rifle} folder={name=test}}}')
            c = load_technologies([file], [file], {'rifle_tech': 'Infantry weapons', 'ROM_rifle': 'Mannlicher M1893'}, DlcInventory(), {}, country='ROM')
        self.assertEqual(c['technologies']['rifle_tech']['name'], 'Mannlicher M1893')


if __name__ == '__main__':
    unittest.main()
