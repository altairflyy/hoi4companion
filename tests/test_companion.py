import json
import tempfile
import unittest
from pathlib import Path
from hoi4companion.clausewitz import parse
from hoi4companion.focus import load_trees, evaluate, prerequisite_closure
from hoi4companion.installation import collect_files
from hoi4companion.state import read_snapshot

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests' / 'fixtures' / 'mini_national_focus.txt'


class ClausewitzTest(unittest.TestCase):
    def test_strings_comments_and_nested_blocks(self):
        obj = parse('id = "a # b" # comment\n block={a=1 a=2 text="escaped \\" quote"} other >= 3')
        self.assertEqual(obj.scalar('id'), 'a # b')
        self.assertEqual(obj.first('block').all('a'), ['1', '2'])
        self.assertEqual(obj.first('block').scalar('text'), 'escaped " quote')
        self.assertEqual(obj.scalar('other >='), '3')

    def test_errors(self):
        for source in ('x = { y = 1', 'x = }', 'x = "unterminated'):
            with self.subTest(source=source), self.assertRaises(ValueError):
                parse(source)


class FocusTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.trees, cls.warnings = load_trees([FIXTURE], {'AUS_start': 'Inizio'})
        cls.tree = cls.trees['AUS_test_tree']

    def test_prerequisite_groups_are_and_of_or(self):
        groups = self.tree.focuses['AUS_goal'].prerequisites
        self.assertEqual(groups, [['AUS_one', 'AUS_two'], ['SHARED_seed']])
        self.assertEqual(self.tree.focuses['AUS_start'].name, 'Inizio')
        self.assertEqual(self.tree.focuses['AUS_start'].to_json()['estimated_days'], 35)
        self.assertEqual(len(self.tree.focuses), 5)

    def test_relative_position(self):
        self.assertEqual((self.tree.focuses['AUS_one'].x, self.tree.focuses['AUS_one'].y), (4, 1))

    def test_evaluation(self):
        self.assertEqual(evaluate(self.tree, set())['AUS_goal'], 'locked')
        states = evaluate(self.tree, {'AUS_start', 'AUS_one', 'SHARED_seed'})
        self.assertEqual(states['AUS_two'], 'excluded')
        self.assertEqual(states['AUS_goal'], 'verify')
        self.assertEqual(evaluate(self.tree, {'AUS_start'}, 'AUS_one')['AUS_one'], 'active')

    def test_closure(self):
        expected={'AUS_goal','AUS_one','AUS_two','AUS_start','SHARED_seed'}
        self.assertEqual(prerequisite_closure(self.tree,'AUS_goal'),expected)


class LayerTest(unittest.TestCase):
    def test_mod_file_overrides_base(self):
        with tempfile.TemporaryDirectory() as td:
            base,mod=Path(td)/'vanilla',Path(td)/'mod'
            for root in (base,mod):
                (root/'common'/'national_focus').mkdir(parents=True)
            (base/'common'/'national_focus'/'same.txt').write_text('vanilla')
            (base/'common'/'national_focus'/'other.txt').write_text('other')
            (mod/'common'/'national_focus'/'same.txt').write_text('mod')
            files,warnings=collect_files(base,[mod],'common/national_focus','.txt')
            self.assertFalse(warnings)
            self.assertEqual(len(files),2)
            self.assertEqual(next(f for f in files if f.name=='same.txt').read_text(),'mod')


class SnapshotTest(unittest.TestCase):
    def test_json(self):
        snap=read_snapshot(ROOT/'examples'/'demo_snapshot.json')
        self.assertIn('DEMO_industry',snap.completed)
        self.assertEqual(snap.current,'DEMO_economy')
        self.assertEqual(snap.progress,.35)

    def test_text_save(self):
        with tempfile.TemporaryDirectory() as td:
            save=Path(td)/'example.hoi4'
            save.write_bytes(b'HOI4txt\ncountries = { AUS = { completed_focuses = { DEMO_a DEMO_b } focus = { current = DEMO_c progress = 14 } } }')
            snap=read_snapshot(save)
            self.assertEqual(snap.completed,['DEMO_a','DEMO_b'])
            self.assertEqual(snap.current,'DEMO_c')

    def test_bin_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            save=Path(td)/'example.hoi4'
            save.write_bytes(b'HOI4bin\x00\x01')
            with self.assertRaisesRegex(ValueError,'binario'):
                read_snapshot(save)


if __name__=='__main__':
    unittest.main()
