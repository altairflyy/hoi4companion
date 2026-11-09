"""Native scheduling parity; never launches HOI4 or writes campaign progress."""
import copy
import json
from pathlib import Path
import unittest

from hoi4companion.guide_core import evaluate, engine_info, clear_compiled
from hoi4companion.guide_runtime import evaluate_python


class GuideCoreTests(unittest.TestCase):
    def test_adjacent_float_thresholds_are_not_collapsed_by_json_transport(self):
        import math
        for value in (121.69338496112731,2093.4889316403746):
            cost=math.nextafter(value,math.inf)
            guide={'id':'threshold','tracks':{'operations':[{'key':'law',
                'action':{'kind':'manual','params':{'category':'politics'}},
                'resource_costs':{'country.politics.political_power':cost}}]}}
            state={'schema_version':1,'usable':True,'facts':{'country.politics.political_power':{
                'status':'observed','value':value,'source':'fixture'}}}
            self.assertEqual(evaluate(guide,state),evaluate_python(guide,state))

    def test_installed_engine_and_every_shared_runtime_fixture(self):
        self.assertEqual(engine_info()['engine'], 'rust-guide-core')
        fixtures=json.loads((Path(__file__).parent/'fixtures/guide-runtime.json').read_text(encoding='utf-8'))
        for fixture in fixtures:
            with self.subTest(case=fixture['name']):
                before=copy.deepcopy(fixture)
                self.assertEqual(evaluate(fixture['guide'],fixture['state'],fixture.get('progress')),
                                 evaluate_python(fixture['guide'],fixture['state'],fixture.get('progress')))
                self.assertEqual(fixture,before)

    def test_guide_compilation_lifetime_does_not_retain_observed_state(self):
        case=json.loads((Path(__file__).parent/'fixtures/guide-runtime.json').read_text(encoding='utf-8'))[0]
        first=evaluate(case['guide'],case['state'],case.get('progress'))
        clear_compiled()
        self.assertEqual(evaluate(case['guide'],case['state'],case.get('progress')),first)
        unknown={'schema_version':1,'usable':False,'facts':{}}
        self.assertEqual(evaluate(case['guide'],unknown),evaluate_python(case['guide'],unknown))

    def test_multiple_resource_costs_keep_authored_order_and_strict_scalar_types(self):
        guide={'id':'costs','tracks':{'operations':[{'key':'project','action':{'kind':'special_project','entity':{'kind':'project','id':'p'}},
            'resource_costs':{'projects.points.specialization_nuclear':2,'projects.points.specialization_air':1}}]}}
        state={'schema_version':1,'usable':True,'facts':{}}
        self.assertEqual(evaluate(guide,state),evaluate_python(guide,state))

