import copy
import unittest

from hoi4companion.guide_events import compile_event_plan


class GuideEventPlanTests(unittest.TestCase):
    def setUp(self):
        self.catalog=[{'kind':'event','id':'e.1','name':'An invitation'},
                      {'kind':'event_option','id':'e.1.a','event_id':'e.1',
                       'name':'An excellent idea! We shall send a liaison.',
                       'localized_name':'Ottima idea! Invieremo un rappresentante.'}]
    def guide(self,params):
        return {'id':'test','tracks':{'operations':[{'key':'step','title':'Invitation',
            'action':{'kind':'manual','params':{'category':'politics',**params}}}]}}
    def test_compiles_resolved_ids_without_mutating_guide(self):
        g=self.guide({'event_id':'e.1','event_option_id':'e.1.a'});before=copy.deepcopy(g)
        plan=compile_event_plan(g,self.catalog)
        self.assertEqual(g,before)
        self.assertEqual(plan['bindings']['step']['identity'],'e.1:e.1.a')
        self.assertIn('Ottima idea!',plan['bindings']['step']['labels'][0])
        self.assertEqual(plan['unresolved'],[])
    def test_unique_explicit_quote_fragment_is_supported_but_not_short_or_ambiguous_text(self):
        p={'event_option':'We shall send a liaison'}
        self.assertEqual(compile_event_plan(self.guide(p),self.catalog)['bindings']['step']['event_id'],'e.1')
        ambiguous=self.catalog+[dict(self.catalog[1],id='e.2.a',event_id='e.2')]
        self.assertFalse(compile_event_plan(self.guide(p),ambiguous)['bindings'])
        self.assertFalse(compile_event_plan(self.guide({'event_option':'liaison'}),self.catalog)['bindings'])
    def test_explicit_wrong_id_never_falls_back_to_a_valid_label(self):
        plan=compile_event_plan(self.guide({'event_option_id':'bad','event_option':'We shall send a liaison'}),self.catalog)
        self.assertFalse(plan['bindings']);self.assertEqual(len(plan['unresolved']),1)
        plan=compile_event_plan(self.guide({'event_id':'wrong','event_option_id':'e.1.a'}),self.catalog)
        self.assertFalse(plan['bindings'])
    def test_event_text_missing_is_reported_and_normal_non_event_step_is_ignored(self):
        self.assertEqual(compile_event_plan(self.guide({'instructions':['Build a factory']}),self.catalog)['unresolved'],[])
        plan=compile_event_plan(self.guide({'event_name':None,'event_option':None}),self.catalog)
        self.assertFalse(plan['bindings']);self.assertEqual(plan['unresolved'][0]['key'],'step')


if __name__=='__main__':unittest.main()
