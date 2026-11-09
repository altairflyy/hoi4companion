"""Shared-context parity: no acquisition, strategy generation or user data I/O."""
import json
from pathlib import Path
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.guide import normalise_guide
from hoi4companion.contracts import normalise_state
from hoi4companion._construction_progress_reference import construction_context as construction_reference
from hoi4companion._campaign_conditions_reference import context as campaign_reference
from hoi4companion._production_progress_reference import production_context as production_reference
from hoi4companion._guide_core_prepare_reference import prepare as prepare_reference


class BackendContextTests(unittest.TestCase):
    def test_model_label_normalization_retains_unicode_and_plus(self):
        from hoi4companion._production_progress_reference import _name
        for value in ('T-I 1 (0/4/SL)','Turán I','Straẞe','ΟΣ','Σ Σ','Árvíztűrő','Heavy Tank+','A\u0301BC','ＡＢＣ１２３'):
            self.assertEqual(_request(0,'production_context',kind='name',value=value),_name(value))

    def test_shared_guide_states_preserve_context_facts_and_operation_details(self):
        count=0
        def walk(value,path):
            nonlocal count
            if isinstance(value,dict):
                if isinstance(value.get('guide'),dict) and isinstance(value.get('state'),dict):
                    try:guide=normalise_guide(value['guide'])
                    except ValueError:pass
                    else:
                        state=normalise_state(value['state'])
                        baselines=(value.get('progress') or {}).get('baselines',{})
                        with self.subTest(path=path):
                            self.assertEqual(_request(0,'guide_prepare',guide=guide,state=value['state'],progress=value.get('progress')),list(prepare_reference(guide,value['state'],value.get('progress'))))
                            expected=construction_reference(guide,state,baselines)
                            self.assertEqual(_request(0,'domain_context',kind='construction',guide=guide,state=state,baselines=baselines),list(expected))
                            self.assertEqual(_request(0,'production_context',kind='production',guide=guide,state=state,baselines=baselines),list(production_reference(guide,state,baselines)))
                            for observations in (None,{}, {'country':'HUN','war_seen':True}):
                                self.assertEqual(_request(0,'domain_context',kind='campaign',state=state,observations=observations),campaign_reference(state,observations))
                        count+=1
                for key,child in value.items():walk(child,path+'/'+str(key))
            elif isinstance(value,list):
                for index,child in enumerate(value):walk(child,path+'/'+str(index))
        for file in sorted((Path(__file__).parent/'fixtures').glob('*.json')):
            walk(json.loads(file.read_text(encoding='utf-8')),file.name)
        self.assertGreater(count,25)

    def test_invalid_baselines_and_campaign_observations_are_not_admitted(self):
        for observations in (None,[],{'country':'HUN'}, {'country':'HUN','war_seen':False},
                             {'country':'HUN','war_seen':True,'milestones':{}}, {'country':'wrong','war_seen':True}):
            with self.assertRaises(ValueError):_request(0,'domain_context',kind='validate_campaign',observations=observations)
        for baselines in (None,[],{'step':{'value':5}}):
            with self.assertRaises(ValueError):_request(0,'domain_context',kind='validate_baselines',baselines=baselines)
