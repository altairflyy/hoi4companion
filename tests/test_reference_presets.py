import copy
import os
import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path

from hoi4companion.reference_presets import PresetInventory, annotate, recipes


RAW = b'''version=0 tank_designs={category=land
 equipment={name="Required" folder="delete" target_variant={type=medium modules={gun=howitzer} upgrades={armor=4}}}
 equipment={name="Required" folder="Armor" target_variant={type=medium modules={gun=howitzer} upgrades={armor=4}}}
 equipment={name="Other" target_variant={type=medium modules={gun=cannon}}}}
'''


class PresetInventoryTests(unittest.TestCase):
    def fixture(self, directory):
        path = Path(directory) / 'session_persistence/equipment_designs/equipment_designs.txt'
        path.parent.mkdir(parents=True); path.write_bytes(RAW)
        design = {k: v for k, v in recipes(RAW)[0].items() if k != 'folder'}
        return path, {**design, 'id': 'reference'}

    def test_inventory_counts_every_entry_and_preserves_saved_folders(self):
        with tempfile.TemporaryDirectory() as temp:
            path, design = self.fixture(temp)
            value = PresetInventory().inspect(temp, [design])
            self.assertEqual(value['status'], 'file_observed')
            self.assertEqual((value['count'], value['distinct_recipes']), (3, 2))
            self.assertEqual(value['designs']['reference'], {'status': 'present', 'folders': ['Armor', 'delete']})
            self.assertEqual(path.read_bytes(), RAW)
            self.assertNotIn('loaded', value)

    def test_same_name_different_recipe_is_not_installed_or_proven_by_chassis(self):
        with tempfile.TemporaryDirectory() as temp:
            _, design = self.fixture(temp)
            design['modules'] = {'gun': 'different'}
            self.assertEqual(PresetInventory().inspect(temp, [design])['designs']['reference']['status'], 'different_recipe')
            design['name'] = 'Missing'
            self.assertEqual(PresetInventory().inspect(temp, [design])['designs']['reference']['status'], 'missing')

    def test_corrupt_or_unsupported_file_never_claims_missing_designs(self):
        for raw in (b'version=1', b'version=0 tanks={category=unknown}',
                    b'version=0 tanks={category=land equipment={name=x}}',
                    b'version=0 tanks={category=land equipment={name=x target_variant={type=m modules={gun=a gun=b}}}}'):
            with tempfile.TemporaryDirectory() as temp:
                path, design = self.fixture(temp); path.write_bytes(raw)
                value = PresetInventory().inspect(temp, [design])
                self.assertEqual(value['status'], 'unavailable'); self.assertEqual(value['designs'], {})

    def test_cache_follows_file_changes_and_absence_without_reusing_old_locations(self):
        with tempfile.TemporaryDirectory() as temp:
            path, design = self.fixture(temp); reader = PresetInventory()
            self.assertEqual(reader.inspect(temp, [design])['status'], 'file_observed')
            path.write_bytes(RAW.replace(b'delete', b'New group'))
            self.assertEqual(reader.inspect(temp, [design])['designs']['reference']['folders'], ['Armor', 'New group'])
            path.unlink(); self.assertEqual(reader.inspect(temp, [design]), {'status': 'file_missing', 'designs': {}})

    def test_access_time_change_from_read_is_not_a_content_change(self):
        with tempfile.TemporaryDirectory() as temp:
            path, design = self.fixture(temp)
            read = Path.read_bytes
            def access_only(target):
                value = read(target); stat = target.stat()
                os.utime(target, ns=(stat.st_atime_ns + 1_000_000_000, stat.st_mtime_ns))
                return value
            with patch.object(Path, 'read_bytes', access_only):
                self.assertEqual(PresetInventory().inspect(temp, [design])['status'], 'file_observed')

    def test_annotations_do_not_change_live_completion_counts_or_other_guide_steps(self):
        guide = {'tracks': {'operations': [{'key': 'step', 'action': {'params': {
            'production_targets': [{'design_reference': {'id': 'reference'}}]}}}]}}
        proposal = {'key': 'step.target-1', 'panel': 'production', 'goal_satisfied': None, 'requested': 0}
        inventory = {'status': 'file_observed', 'designs': {'reference': {'status': 'present', 'folders': ['delete']}}}
        value = annotate([copy.deepcopy(proposal)], guide, inventory)[0]
        self.assertEqual(value['production_saved_design']['folders'], ['delete'])
        self.assertEqual({k: v for k, v in value.items() if k != 'production_saved_design'}, proposal)
        self.assertEqual(annotate([copy.deepcopy(proposal)], {'tracks': {}}, inventory), [proposal])
