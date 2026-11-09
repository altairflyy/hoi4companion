"""Script and DLC expansion retains bounded unknown predicates and budgets."""
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.clausewitz import parse
from hoi4companion.script_document import encode
from hoi4companion._focus_criteria_reference import compile_rules,expand_rules
from hoi4companion.dlc import DlcInventory

class BackendRuleExpansionTests(unittest.TestCase):
    def test_batch_matches_independent_source_compiler_with_cycles_and_unknowns(self):
        scripts={'treaty':parse('NOT={has_idea=old_treaty} political_power > 1_00'),
                 'cyclic':parse('cyclic=yes'),'both':parse('treaty=yes has_dlc=Pack')}
        inventory=DlcInventory(items=[{'name':'Pack','status':'enabled'}])
        rules={key:compile_rules(parse(text)) for key,text in {
            'one':'available={treaty=yes has_war_support > 0.6}',
            'two':'allow_branch={both=no} available={cyclic=yes runtime_predicate=yes}',
            'three':'available={IF={limit={has_dlc=Pack} treaty=yes}}'}.items()}
        expected={key:expand_rules(value,scripts,inventory) for key,value in rules.items()}
        actual=_request(0,'source_expand_rules',rules=rules,scripts={key:encode(node) for key,node in scripts.items()},dlcs={'items':inventory.items,'complete':inventory.complete})
        self.assertEqual(actual,expected)
