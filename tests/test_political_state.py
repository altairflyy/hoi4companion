import copy
import unittest
from hoi4companion.contracts import normalise_state


def sample():
    return {'usable':True,'sample':{'country':'TST','session_id':'s','country_overview':{
        'country':'TST','politics':{'status':'observed',
            'ideas':{'status':'observed','items':[{'id':'war_economy','group':'economy','category':'law','is_law':True}]},
            'advisors':{'status':'observed','items':[{'id':'TST_advisor','character_id':'TST_person','slot':1}]}},
        'events':{'status':'observed','visible':[{'instance_id':'1','event_id':'example.1','option_ids':['example.1.a']}]}
    }}}


class PoliticalStateTests(unittest.TestCase):
    def test_qualified_memberships_and_explicit_empty(self):
        value=sample(); facts=normalise_state(value)['facts']
        self.assertEqual(facts['politics.ideas']['value'],['war_economy'])
        self.assertEqual(facts['politics.advisors']['value'],['TST_advisor'])
        self.assertEqual(facts['events.visible_options']['value'],['example.1:example.1.a'])
        value['sample']['country_overview']['politics']['advisors']['items']=[]
        self.assertEqual(normalise_state(value)['facts']['politics.advisors']['value'],[])

    def test_missing_partial_foreign_and_stale_never_mean_empty_or_completed(self):
        variants=[]
        for mutate in [lambda v:v.update(usable=False),
                       lambda v:v['sample']['country_overview'].update(country='OTH'),
                       lambda v:v['sample']['country_overview']['politics']['ideas'].update(status='unavailable'),
                       lambda v:v['sample']['country_overview']['politics']['ideas']['items'].append({'id':'war_economy'})]:
            value=sample(); mutate(value); variants.append(value)
        for value in variants: self.assertEqual(normalise_state(value)['facts']['politics.ideas']['status'],'unknown')
        value=sample(); del value['sample']['country_overview']['events']['visible'][0]['option_ids']
        facts=normalise_state(value)['facts']
        self.assertEqual(facts['events.visible']['value'],['example.1'])
        self.assertEqual(facts['events.visible_options']['status'],'unknown')

    def test_canonical_memberships_validate_identity_lists(self):
        for key in ['politics.ideas','politics.advisors','events.visible','events.visible_options']:
            value={'schema_version':1,'usable':True,'facts':{key:{'status':'observed','value':[None],'source':'fixture'}}}
            self.assertEqual(normalise_state(value)['facts'][key]['status'],'unknown')
