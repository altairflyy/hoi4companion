"""English guide identity remains stable under installed translation mods."""
import copy
from pathlib import Path
from tempfile import TemporaryDirectory
from threading import RLock
from types import SimpleNamespace
import unittest

from hoi4companion import installation
from hoi4companion.api import Store
from hoi4companion.guide_importer import catalog_entities, preview
from hoi4companion.import_sources import acquire_source


def guide(src, query, kind='focus', country='POL'):
    return {'schema_version': 1, 'id': 'localised-guide', 'title': 'Guide',
            'applies_to': {'country': country}, 'sources': [{'id': src['id'], 'type': 'manual'}],
            'tracks': {'focus' if kind == 'focus' else 'research': [{
                'key': 'step', 'action': {'kind': 'select_focus' if kind == 'focus' else 'research',
                'entity': {'kind': kind, 'id': query}},
                'evidence': [{'source_id': src['id'], 'basis': 'explicit', 'quote': src['text']}]}]}}


class ImportLocalisationTests(unittest.TestCase):
    def test_translation_can_override_english_files_without_losing_reference(self):
        with TemporaryDirectory() as directory:
            game, mod = Path(directory)/'game', Path(directory)/'translation'
            for root in (game, mod):
                (root/'localisation').mkdir(parents=True)
            (game/'localisation'/'base_l_english.yml').write_text(
                'l_english:\n POL_plan:0 "The Four Year Plan"\n fallback:0 "Base name"\n', encoding='utf-8-sig')
            (mod/'localisation'/'translation_l_english.yml').write_text(
                'l_english:\n POL_plan:0 "Il piano quadriennale"\n MOD_new:0 "New mod focus"\n', encoding='utf-8-sig')
            (mod/'localisation'/'translation_l_italian.yml').write_text(
                'l_italian:\n MOD_new:0 "Nuovo focus mod"\n', encoding='utf-8-sig')
            labels, reference = installation.load_localisations(game, [mod], 'italian')
            self.assertEqual(reference['POL_plan'], 'The Four Year Plan')
            self.assertEqual(labels['POL_plan'], 'Il piano quadriennale')
            self.assertEqual(labels['fallback'], 'Base name')
            self.assertEqual(reference['MOD_new'], 'New mod focus')
            self.assertEqual(labels['MOD_new'], 'Nuovo focus mod')
            self.assertEqual(installation.load_localisation(game, [mod], 'italian'), labels)
            english, reference2 = installation.load_localisations(game, [mod], 'english')
            self.assertEqual(english['MOD_new'], 'New mod focus')
            self.assertEqual(reference2, reference)

    def catalog(self):
        trees = {
            'polish': SimpleNamespace(focuses={'POL_plan': SimpleNamespace(name='Il piano quadriennale')}, eligible_countries=['POL']),
            'german': SimpleNamespace(focuses={'GER_plan': SimpleNamespace(name='Il Piano Quattro Anni')}, eligible_countries=['GER']),
        }
        tech = {'basic_machine_tools': {'name': 'Macchine utensili di base', 'paths': []}}
        return catalog_entities(trees, tech,
            {'POL_plan': 'Il piano quadriennale', 'GER_plan': 'Il Piano Quattro Anni'},
            {'POL_plan': '$plan_name$', 'plan_name': 'The Four Year Plan',
             'GER_plan': 'The Four Year Plan', 'basic_machine_tools': 'Basic Machine Tools'})

    def test_catalog_separates_reference_display_and_country_without_mutation(self):
        catalog = self.catalog()
        focus = next(row for row in catalog if row['id'] == 'POL_plan')
        self.assertEqual(focus['name'], 'The Four Year Plan')
        self.assertEqual(focus['localized_name'], 'Il piano quadriennale')
        self.assertIn('Il piano quadriennale', focus['aliases'])
        self.assertEqual(focus['countries'], ['POL'])

    def test_english_focus_resolves_in_country_and_keeps_localised_display(self):
        src = acquire_source(text='The Four Year Plan')
        value = guide(src, src['text']); before = copy.deepcopy(value)
        report = preview(value, [src], self.catalog())
        self.assertTrue(report['valid'], report['errors'])
        step = report['guide']['tracks']['focus'][0]
        self.assertEqual(step['target'], 'POL_plan')
        self.assertEqual(step['action']['entity']['scope'], 'polish')
        self.assertEqual(step['name'], 'Il piano quadriennale')
        self.assertEqual(value, before)

    def test_unknown_country_does_not_arbitrarily_disambiguate(self):
        src = acquire_source(text='The Four Year Plan'); value = guide(src, src['text'])
        value['applies_to'] = {}
        self.assertFalse(preview(value, [src], self.catalog())['valid'])
        value['applies_to'] = {'tree_id': 'german'}
        self.assertTrue(preview(value, [src], self.catalog())['valid'])
        value['applies_to'] = {'country': 'POL'}
        value['tracks']['focus'][0]['action']['entity']['id'] = 'GER_plan'
        self.assertFalse(preview(value, [src], self.catalog())['valid'])

    def test_both_technology_languages_resolve_to_same_stable_id(self):
        src = acquire_source(text='Basic Machine Tools')
        for query in ('Basic Machine Tools', 'Macchine utensili di base', 'basic_machine_tools'):
            report = preview(guide(src, query, 'technology'), [src], self.catalog())
            self.assertTrue(report['valid'], report['errors'])
            step = report['guide']['tracks']['research'][0]
            self.assertEqual(step['target'], 'basic_machine_tools')
            self.assertEqual(step['name'], 'Macchine utensili di base')

    def test_english_reference_precedes_a_different_entity_translation_alias(self):
        src = acquire_source(text='Basic Machine Tools'); catalog = self.catalog()
        catalog.append({'kind': 'technology', 'id': 'mod_different', 'name': 'Different mod technology',
                        'localized_name': 'Basic Machine Tools', 'aliases': ['Basic Machine Tools']})
        report = preview(guide(src, src['text'], 'technology'), [src], catalog)
        self.assertTrue(report['valid'], report['errors'])
        self.assertEqual(report['guide']['tracks']['research'][0]['target'], 'basic_machine_tools')

    def test_country_equipment_names_and_whole_word_abbreviations(self):
        catalog = catalog_entities({}, {'infantry_at': {'name':'Fanteria anticarro', 'paths':[]},
            'infantry_weapons1': {'name':'ZB vz.24', 'paths':[],
                'unlocks':[{'kind':'Equipaggiamento','id':'infantry_equipment_1'}]}},
            {'POL_infantry_at':'Kb Ppanc wz. 35 Ur'},
            {'infantry_at':'Infantry Anti-Tank I','POL_infantry_at':'Kb Ppanc wz. 35 Ur',
             'infantry_equipment_1':'Infantry Equipment I'})
        src = acquire_source(text='Kb PPanc')
        report = preview(guide(src,src['text'],'technology'),[src],catalog)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['guide']['tracks']['research'][0]['target'],'infantry_at')
        self.assertEqual(next(e for e in catalog if e['id']=='infantry_weapons1')['name'],'Infantry Equipment I')

    def test_machine_gun_abbreviation_and_localised_name_share_identity(self):
        catalog = [{'kind':'technology','id':'aa_hmg','name':'Heavy MG',
                    'localized_name':'Mitragliatrice pesante','aliases':['Mitragliatrice pesante']}]
        src = acquire_source(text='Heavy MG')
        for query in ('Heavy MG','Heavy Machine Gun','Heavy Machinegun','Mitragliatrice pesante'):
            report=preview(guide(src,query,'technology'),[src],catalog)
            self.assertTrue(report['valid'],report['errors'])
            self.assertEqual(report['guide']['tracks']['research'][0]['target'],'aa_hmg')

    def test_store_exposes_reference_catalog_without_changing_display_models(self):
        store = Store.__new__(Store); store.lock = RLock()
        focus = SimpleNamespace(name='Il piano quadriennale')
        store.trees = {'polish': SimpleNamespace(focuses={'POL_plan': focus}, eligible_countries=['POL'])}
        store.technologies = {'technologies': {}}
        store.labels = {'POL_plan': 'Il piano quadriennale'}
        store.reference_labels = {'POL_plan': 'The Four Year Plan'}
        self.assertEqual(store.import_catalog()[0]['name'], 'The Four Year Plan')
        self.assertEqual(focus.name, 'Il piano quadriennale')
