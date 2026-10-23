"""The migration gate must not excuse unrelated data loss as a syntax upgrade."""
import importlib.util
from pathlib import Path
import unittest

from hoi4companion.clausewitz import Node

spec = importlib.util.spec_from_file_location(
    'parser_comparison', Path(__file__).resolve().parents[1] / 'scripts' / 'compare-parsers.py')
comparison = importlib.util.module_from_spec(spec)
spec.loader.exec_module(comparison)


class ParserComparisonTests(unittest.TestCase):
    def test_opaque_question_expression_requires_exact_adjacent_fragments(self):
        old = Node([('cost', 'var'), ('', '?'), ('', '50')])
        self.assertIsNotNone(comparison.syntax_upgrades(old, Node([('cost', 'var?50')])))
        self.assertIsNone(comparison.syntax_upgrades(old, Node([('cost', 'var?60')])))

    def test_known_syntax_difference_does_not_hide_other_field_loss(self):
        old = Node([('cost', 'var'), ('', '?'), ('', '50'), ('required', 'yes')])
        self.assertIsNone(comparison.syntax_upgrades(old, Node([('cost', 'var?50')])))

    def test_nested_duplicate_order_and_semicolon_value_are_checked(self):
        old = Node([('x', Node([('tag', '001;'), ('tag', '002')]))])
        new = Node([('x', Node([('tag', '001'), ('tag', '002')]))])
        self.assertIsNotNone(comparison.syntax_upgrades(old, new))
        reordered = Node([('x', Node([('tag', '002'), ('tag', '001')]))])
        self.assertIsNone(comparison.syntax_upgrades(old, reordered))
