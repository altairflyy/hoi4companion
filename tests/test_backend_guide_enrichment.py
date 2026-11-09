"""Guide presentation enrichments never rewrite authored actions or evidence."""
from dataclasses import asdict
import unittest
from hoi4companion._guide_reference import enrich_guide
from hoi4companion.guide import normalise_guide
from hoi4companion.focus import Tree,Focus
from hoi4companion.backend_core import _request
from hoi4companion._action_resources_reference import enrich_resource_costs

class BackendGuideEnrichmentTests(unittest.TestCase):
    def test_glossary_uses_the_supplied_vocabulary_without_reusing_another_generation(self):
        from copy import deepcopy
        from hoi4companion.research_upgrades import guide_language
        vocabulary=deepcopy(guide_language())
        vocabulary['abbreviations']['construction']['ZX']='fixture structure'
        guide={'id':'g','tracks':{'operations':[{'key':'o','action':{'kind':'manual','params':{'category':'construction','instructions':['Build ZX']}}}]}}
        args=dict(guide=guide,trees={},technologies={},labels={},folders={},continuous={})
        expected=[{'code':'ZX','meaning':'fixture structure','domain':'construction'}]
        self.assertEqual(_request(0,'source_enrich',vocabulary=vocabulary,**args)['tracks']['operations'][0]['operational_glossary'],expected)
        self.assertEqual(_request(0,'source_enrich',**args)['tracks']['operations'][0]['operational_glossary'],[])
    def test_focus_criteria_continuous_and_technology_references_preserve_source(self):
        def focus(key,**fields):return Focus(key,key,'','',0,0,'',10,[],[],False,False,[],source='f',**fields)
        first=focus('first',criteria={'schema_version':1,'available':{'op':'const','value':True}});other=focus('other');other.mutually_exclusive=['first']
        child=focus('child');child.prerequisites=[['other']]
        trees={'tree':Tree('tree','f',focuses={'first':first,'other':other,'child':child})};continuous={'suppress':focus('suppress')}
        tech={'t':{'name':'Tools','start_year':1936,'folders':[{'id':'industry','layout':{'template':'small'}}],'unlocks':[]}}
        guide=normalise_guide({'id':'g','title':'Guide','applies_to':{'country':'HUN','tree_id':'tree'},'tracks':{
            'focus':[{'key':'f','action':{'kind':'select_focus','entity':{'kind':'focus','id':'first'}}},{'key':'c','action':{'kind':'select_focus','entity':{'kind':'focus','id':'suppress'}}}],
            'research':[{'key':'t','action':{'kind':'research','entity':{'kind':'technology','id':'t'}}}],
            'operations':[{'key':'p','action':{'kind':'special_project','entity':{'kind':'project','id':'p'},'params':{'instructions':['Build MIL and CIV'],'category':'construction'}}}]}})
        labels={'HUN_t':'National Tools','p':'Project'};folders={'industry':{'name':'Industry'}}
        expected=enrich_guide(guide,trees,tech,labels,folders,continuous_focuses=continuous)
        actual=_request(0,'source_enrich',guide=guide,trees={key:{'focuses':{k:asdict(v) for k,v in t.focuses.items()}} for key,t in trees.items()},technologies=tech,labels=labels,folders=folders,continuous={key:asdict(v) for key,v in continuous.items()})
        self.assertEqual(actual,expected)
        self.assertEqual(actual['tracks']['focus'][0]['focus_criteria']['conflicts'],['child','other'])

    def test_literal_game_project_costs_override_only_qualified_project_rows(self):
        guide=normalise_guide({'id':'g','tracks':{'operations':[{'key':'p','action':{'kind':'special_project','entity':{'kind':'project','id':'p'}},'resource_costs':{'projects.points.land':99}}]}})
        catalog=[{'kind':'project','id':'p','resource_costs':{'projects.points.land':1}}]
        expected=enrich_resource_costs(guide,catalog)
        self.assertEqual(_request(0,'source_project_costs',guide=guide,catalog=catalog),expected)
