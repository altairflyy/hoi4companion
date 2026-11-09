"""Source name variants preserve unknown scopes and recursive bounds."""
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.scripted_names import expand_names

class BackendScriptedNamesTests(unittest.TestCase):
    def test_bulk_expansion_matches_scoped_adjectives_states_cycles_and_repeated_names(self):
        definitions={'GetRoad':['a','b'],'GetNested':['nested'],'GetLoop':['loop'],'GetName':['a']}
        labels={'a':'Royal Road','b':'National Road','nested':'[GetRoad] East','loop':'[Root.GetLoop]','HUN_ADJ':'Hungarian','HUN_monarchy_ADJ':'Austro-Hungarian','GER_ADJ':'German','STATE_339':'Izmir'}
        queries=[{'text':text,'countries':['HUN']} for text in ['Fixed','Build [GetRoad]','[GetRoad] / [GetRoad]','[GetNested]','[GetLoop]','[Root.GetAdjective] Future','[GER.GetAdjective] Future','[339.GetName]','[123.GetName]','[Root.Capital.GetName]','[PREV.GetName]','[GER.GetName]','[Unknown.GetName]','[Missing]']]
        expected=[expand_names(q['text'],definitions,labels,countries=q['countries']) for q in queries]
        self.assertEqual(_request(0,'source_name_variants',definitions=definitions,labels=labels,queries=queries),expected)
