import copy
import unittest
from hoi4companion.guide_importer import Importer
from test_guide_authoring import document, CATALOG


class CoverageTests(unittest.TestCase):
    def load(self, inventory=None):
        raw = document({'type': 'research', 'target': 'Tools'})
        if inventory is not None:
            raw['guide']['source_inventory'] = inventory
        importer = Importer(lambda: CATALOG)
        loaded = importer.document(raw)
        return loaded, importer.preview(loaded['guide'], loaded['sources'])

    def inventory(self, **patch):
        return {'schema_version': 1, 'items': [{'id': 'i1', 'source_id': 's',
            'quote': 'Research tools.', 'step_keys': ['s0'], **patch}]}

    def test_legacy_is_not_claimed_complete(self):
        _, report = self.load()
        self.assertTrue(report['valid'])
        self.assertEqual(report['coverage']['status'], 'unavailable')

    def test_inventory_survives_compilation_and_source_remapping(self):
        loaded, report = self.load(self.inventory())
        self.assertTrue(report['valid'], report['errors'])
        self.assertEqual(report['coverage']['status'], 'accounted')
        self.assertEqual(report['coverage']['linked'], 1)
        self.assertEqual(loaded['guide']['source_inventory']['items'][0]['source_id'], loaded['sources'][0]['id'])
        self.assertEqual(Importer(lambda: CATALOG).document(loaded), loaded)

    def test_missing_and_invalid_links_block_installation(self):
        for patch, code in [({'step_keys': []}, 'unmapped'), ({'step_keys': ['absent']}, 'unknown_step'),
                            ({'quote': 'invented'}, 'unsupported_quote')]:
            _, report = self.load(self.inventory(**patch))
            self.assertFalse(report['valid'])
            self.assertIn(code, [x['code'] for x in report['coverage']['issues']])

    def test_exclusions_require_reason_and_do_not_cover_steps(self):
        _, report = self.load(self.inventory(step_keys=[], excluded_reason='Duplicate clip'))
        self.assertEqual(report['coverage']['excluded'], 1)
        self.assertIn('untracked_step', [x['code'] for x in report['coverage']['issues']])
        _, report = self.load(self.inventory(excluded_reason='Cannot be both'))
        self.assertFalse(report['valid'])

    def test_duplicate_observation_reported_without_dropping_steps(self):
        inventory = self.inventory()
        inventory['items'].append({**inventory['items'][0], 'id': 'i2'})
        before = copy.deepcopy(inventory)
        _, report = self.load(inventory)
        self.assertIn('duplicate_observation', [x['code'] for x in report['coverage']['issues']])
        self.assertEqual(inventory, before)
        self.assertEqual(len(report['guide']['tracks']['research']), 1)

    def test_invalid_inventory_shapes_fail_and_valid_coverage_does_not_change_runtime(self):
        from hoi4companion.guide_runtime import evaluate
        from hoi4companion.source_coverage import inventory_items
        for value in [[], {'schema_version':True,'items':[]}, {'schema_version':1,'items':[]}]:
            with self.assertRaises(ValueError):inventory_items({'source_inventory':value})
        for patch in [{'timestamp_seconds':True},{'timestamp_seconds':-1},{'excluded_reason':''},{'step_keys':['s0','s0']}]:
            with self.assertRaises(ValueError):self.load(self.inventory(**patch))
        _, old = self.load()
        _, new = self.load(self.inventory())
        # Editorial metadata is not interpreted as a condition or progress.
        old_guide=copy.deepcopy(old['guide']);new_guide=copy.deepcopy(new['guide'])
        new_guide.pop('source_inventory')
        self.assertEqual(old_guide,new_guide)

    def test_install_revalidates_coverage_even_when_reviewed(self):
        from tempfile import TemporaryDirectory
        from pathlib import Path
        loaded, report = self.load(self.inventory(step_keys=[]))
        with TemporaryDirectory() as root:
            importer=Importer(lambda:CATALOG,Path(root))
            with self.assertRaisesRegex(ValueError,'Copertura'):
                importer.install(loaded['guide'],loaded['sources'],reviewed=True)
            self.assertEqual(list(Path(root).iterdir()),[])
