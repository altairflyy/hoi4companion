import unittest
from hoi4companion.scripted_names import expand_names


class ScriptedNamesTests(unittest.TestCase):
    def test_indexed_pass_preserves_all_labels_without_scanning_localisation_for_each_name(self):
        from hoi4companion.scripted_names import NameExpander
        class Labels(dict):
            scans=0
            def items(self):self.scans+=1;return super().items()
        labels=Labels({'AUS_ADJ':'Austrian','AUS_monarchy_ADJ':'Austro-Hungarian',
                       'GER_ADJ':'German','road':'Road','STATE_339':'Izmir'})
        names=NameExpander({'GetRoad':['road']},labels)
        queries=['An [Root.GetAdjective] Future','An [GER.GetAdjective] Future','[Root.GetRoad] in [339.GetName]']
        indexed=[names.expand(q,countries=['AUS']) for q in queries]
        self.assertEqual(labels.scans,1)
        expected=[expand_names(q,{'GetRoad':['road']},dict(labels),countries=['AUS']) for q in queries]
        self.assertEqual(indexed,expected)
    def test_all_defined_alternatives_resolve_identity_without_asserting_live_choice(self):
        definitions={'GetRoad':['road_a','road_b'],'GetNested':['nested']}
        labels={'road_a':'Royal Road','road_b':'National Road','nested':'[GetRoad] East'}
        self.assertEqual(expand_names('Build [GetRoad]',definitions,labels),['Build National Road','Build Royal Road'])
        self.assertEqual(expand_names('[GetNested]',definitions,labels),['National Road East','Royal Road East'])
        self.assertEqual(len(expand_names('[GetRoad] / [GetRoad]',definitions,labels)),4)

    def test_cycles_missing_definitions_and_unknown_scopes_never_invent_names(self):
        self.assertEqual(expand_names('[GetLoop]',{'GetLoop':['loop']},{'loop':'[GetLoop]'}),[])
        self.assertEqual(expand_names('[ROOT.GetName]',{},{}),[])
        self.assertEqual(expand_names('[Missing]',{},{}),[])
        self.assertEqual(expand_names('Fixed name',{},{}),[])

    def test_scoped_defined_functions_keep_all_scripted_alternatives(self):
        definitions={'GetName':['royal','ordinary']}
        labels={'royal':'Royal Factory','ordinary':'Ordnance Factory'}
        expected=['Ordnance Factory','Royal Factory']
        self.assertEqual(expand_names('[Root.GetName]',definitions,labels),expected)
        self.assertEqual(expand_names('[GER.GetName]',definitions,labels),expected)
        self.assertEqual(expand_names('[PREV.GetName]',definitions,labels),[])
        self.assertEqual(expand_names('[Unknown.GetName]',definitions,labels),[])

    def test_adjectives_and_fixed_state_names_use_only_explicit_identity(self):
        labels={'AUS_ADJ':'Austrian','AUS_empire_ADJ':'Austro-Hungarian',
                'GER_ADJ':'German','STATE_339':'Izmir'}
        self.assertEqual(expand_names('An [AUS.GetAdjective] Future',{},labels),
                         ['An Austrian Future','An Austro-Hungarian Future'])
        self.assertEqual(expand_names('An [Root.GetAdjective] Future',{},labels,countries=['AUS']),
                         ['An Austrian Future','An Austro-Hungarian Future'])
        self.assertEqual(expand_names('[ROOT.GetAdjective] Future',{},labels),[])
        self.assertEqual(expand_names('Officers to [339.GetName]',{},labels),['Officers to Izmir'])
        self.assertEqual(expand_names('[123.GetName]',{},labels),[])

    def test_scoped_cycles_and_dynamic_country_names_remain_unknown(self):
        self.assertEqual(expand_names('[GER.GetLoop]',{'GetLoop':['loop']},
                                     {'loop':'[Root.GetLoop]'}),[])
        self.assertEqual(expand_names('[GER.GetName]',{}, {'GER':'Germany'}),[])
        self.assertEqual(expand_names('[Root.Capital.GetName]',{}, {'STATE_339':'Izmir'}),[])
