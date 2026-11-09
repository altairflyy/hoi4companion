import copy
import json
from pathlib import Path
import unittest

from hoi4companion.clausewitz import parse
from hoi4companion.focus_criteria import compile_condition,assess,validate_rules
from hoi4companion.guide_runtime import evaluate
from hoi4companion.contracts import normalise_state


def rules(text='',parents=(),mutex=(),conflicts=()):
    return {'schema_version':1,'available':compile_condition(parse(text)),
            'branch':{'op':'const','value':True},'prerequisites':[list(g) for g in parents],
            'mutex':list(mutex),'conflicts':list(conflicts)}


def observed(value):return {'status':'observed','source':'fixture','value':value}


def state(support=.7,done=('A',),active=()):
    return {'schema_version':1,'usable':True,'facts':{'country':observed('HUN'),'focus.tree':observed('t'),
            'focus.completed':observed(list(done)),'focus.active':observed(list(active)),
            'country.metrics.war_support':observed(support)}}


def guide():
    return {'id':'criteria','title':'Guide','dependency_policy':'explicit','applies_to':{'country':'HUN','tree_id':'t'},
            'tracks':{'focus':[{'key':key,'action':{'kind':'select_focus','entity':{'kind':'focus','id':identity}},
                               'focus_criteria':criteria} for key,identity,criteria in [
                ('main','M',rules('has_war_support > 0.84',[['A']])),
                ('dependent','D',rules('',[['M']])),
                ('independent','R',rules('',[['A']]))]]}}


class FocusCriteriaTests(unittest.TestCase):
    def test_strict_threshold_and_actual_value(self):
        r=rules('has_war_support > 0.84')
        for support,expected in ((.84,False),(.84001,True),(.7,False)):
            result=assess(r,state(support));self.assertIs(result['value'],expected)
            self.assertEqual(result['checks'][0]['actual'],support)

    def test_declared_default_numeric_operator_is_not_guessed(self):
        result=assess(rules('has_war_support = 0.84'),state(.9))
        self.assertIsNone(result['value'])

    def test_unknown_and_or_not_and_script_expansion(self):
        from hoi4companion.focus_criteria import expand_rules
        scripts={'treaty_gone':parse('NOT = { has_idea = treaty }')}
        r=rules('OR = { has_war_support > 0.84 has_war = yes }')
        s=state(.7);self.assertIsNone(assess(r,s)['value'])
        s['facts']['country.diplomacy.at_war']=observed(True)
        self.assertTrue(assess(r,s)['value'])
        r=expand_rules(rules('treaty_gone = yes'),scripts,None)
        self.assertIsNone(assess(r,s)['value'])
        s['facts']['politics.ideas']=observed([]);self.assertTrue(assess(r,s)['value'])
        s['facts']['politics.ideas']=observed(['treaty']);self.assertFalse(assess(r,s)['value'])

    def test_alternative_is_independent_later_and_does_not_change_the_main_order(self):
        g=guide();before=copy.deepcopy(g);r=evaluate(g,state())
        by={x['key']:x for x in r['steps']}
        self.assertEqual(by['main']['status'],'waiting');self.assertEqual(by['dependent']['status'],'waiting')
        self.assertEqual(r['parallel']['focus']['steps'],[])
        self.assertEqual(r['parallel']['focus']['alternative_steps'],['independent'])
        self.assertEqual(r['tracks']['focus']['next'],'main');self.assertEqual(g,before)

    def test_unknown_funds_or_criteria_and_active_work_never_admit_an_alternative(self):
        s=state();s['facts'].pop('country.metrics.war_support')
        self.assertEqual(evaluate(guide(),s)['focus_plan']['steps'],[])
        self.assertEqual(evaluate(guide(),state(active=['outside']))['focus_plan']['steps'],[])
        g=guide();g['tracks']['focus'][2]['focus_criteria']=rules('unobserved_trigger = yes',[['A']])
        self.assertEqual(evaluate(g,state())['focus_plan']['steps'],[])

    def test_declared_dependencies_and_future_mutual_exclusions_are_respected(self):
        g=guide();g['tracks']['focus'][2]['depends_on']=['main']
        self.assertEqual(evaluate(g,state())['focus_plan']['steps'],[])
        g=guide();g['tracks']['focus'][2]['focus_criteria']['conflicts']=['M']
        self.assertEqual(evaluate(g,state())['focus_plan']['steps'],[])
        g=guide();g['tracks']['focus'][2]['when']={'op':'fact','key':'country','test':'eq','value':'GER'}
        self.assertEqual(evaluate(g,state())['focus_plan']['steps'],[])

    def test_primary_returns_when_eligible_and_active_work_is_not_cancelled(self):
        r=evaluate(guide(),state(.9,done=['A','R']))
        self.assertEqual(r['tracks']['focus']['next'],'main')
        self.assertEqual(r['parallel']['focus']['steps'],['main']);self.assertEqual(r['focus_plan']['steps'],[])
        r=evaluate(guide(),state(.5,active=['M']))
        self.assertEqual(next(x for x in r['steps'] if x['key']=='main')['status'],'active')
        self.assertEqual(r['parallel']['focus']['alternative_steps'],[])

    def test_wrong_nation_and_stale_state_do_not_propose_alternatives(self):
        s=state();s['facts']['country']=observed('GER')
        self.assertEqual(evaluate(guide(),s)['parallel']['focus']['steps'],[])
        s=state();s['usable']=False
        self.assertEqual(evaluate(guide(),s)['focus_plan']['steps'],[])

    def test_metrics_are_calculated_bounded_and_qualified_for_the_selected_country(self):
        for country,status,value,known in [('HUN','observed',.87043,True),('GER','observed',.87,False),
                                         ('HUN','partial',.87,False),('HUN','observed',87,False)]:
            s={'usable':True,'sample':{'country':'HUN','country_overview':{'country':country,'metrics':{'status':status,'war_support':value}}}}
            fact=normalise_state(s)['facts']['country.metrics.war_support']
            self.assertEqual(fact['status'],'calculated' if known else 'unknown')

    def test_malformed_conditions_are_rejected(self):
        for change in [lambda r:r.update(schema_version=True),
                       lambda r:r['available'].update(op='eval'),
                       lambda r:r['available']['args'][0].update(value='84')]:
            r=rules('has_war_support > 0.84');change(r)
            with self.assertRaises(ValueError):validate_rules(r)

    def test_malformed_or_oversized_game_conditions_remain_unknown(self):
        from hoi4companion.clausewitz import Node
        self.assertEqual(compile_condition('not a block')['op'],'unknown')
        self.assertEqual(compile_condition(Node([('', 'bare')]))['args'][0]['op'],'unknown')
        huge=compile_condition(Node([('always','yes')]*2000))
        r=rules();r['available']=huge
        validate_rules(r);self.assertIsNone(assess(r,state())['value'])

    def test_one_sided_exclusions_block_alternatives_and_their_descendants(self):
        from hoi4companion.focus import Focus,Tree
        from hoi4companion.guide import enrich_guide
        def f(key,parents=(),mutex=()):
            return Focus(key,key,'','',0,0,'',5,[list(g) for g in parents],list(mutex),False,False,[],'fixture',criteria={'schema_version':1,'available':{'op':'const','value':True},'branch':{'op':'const','value':True}})
        t=Tree('t','fixture',focuses={'M':f('M',mutex=['R']),'R':f('R'),'D':f('D',[['M']])})
        enriched=enrich_guide(guide(),{'t':t},{},{},{})
        row=enriched['tracks']['focus'][2]
        self.assertEqual(row['focus_criteria']['mutex'],['M'])
        self.assertEqual(row['focus_criteria']['conflicts'],['D','M'])

    def test_shared_fixture_matches_python_runtime(self):
        fixtures=json.loads((Path(__file__).parent/'fixtures'/'focus-criteria.json').read_text())
        for item in fixtures:
            with self.subTest(item['name']):self.assertEqual(evaluate(item['guide'],item['state']),item['expected'])

    def test_alternative_hud_uses_existing_purple_choice_tone(self):
        from hoi4companion.guide import normalise_guide
        from hoi4companion.screen_overlay import targets
        g=normalise_guide(guide());r=evaluate(g,state())
        catalog=[{'kind':'focus','id':x,'scope':'t','name':x*5} for x in ['M','D','R']]
        proposals=targets(g,r,catalog,{'usable':True,'sample':{'country':'HUN'}})
        self.assertEqual([(x['identity'],x.get('tone')) for x in proposals],[('R','choice')])
        self.assertIn('Alternative',proposals[0]['title'])
