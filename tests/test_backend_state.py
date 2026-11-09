"""Pure normalization parity, independent of DOM and the acquisition transport."""
import json
from pathlib import Path
import unittest
from hoi4companion.backend_core import _request
from hoi4companion._contracts_reference import normalise_state as reference


class BackendStateTests(unittest.TestCase):
    def test_json_transport_preserves_exact_threshold_numbers(self):
        for number in (2093.4889316403746,121.69338496112731):
            value={'schema_version':1,'usable':True,'facts':{'threshold':{
                'status':'calculated','value':number,'source':'fixture'}}}
            self.assertEqual(_request(0,'state_normalize',value=value),value|{'session_id':None})

    def test_all_shared_fixture_states_match_field_values_and_provenance(self):
        count=0
        def walk(value,path):
            nonlocal count
            if isinstance(value,dict):
                if 'usable' in value and ('sample' in value or 'facts' in value):
                    with self.subTest(path=path):
                        self.assertEqual(_request(0,'state_normalize',value=value),reference(value))
                    count+=1
                for key,child in value.items():walk(child,path+'/'+str(key))
            elif isinstance(value,list):
                for index,child in enumerate(value):walk(child,path+'/'+str(index))
        for file in sorted((Path(__file__).parent/'fixtures').glob('*.json')):
            walk(json.loads(file.read_text(encoding='utf-8')),file.name)
        self.assertGreater(count,50)

    def test_canonical_bad_types_unknowns_and_raw_absence_do_not_become_completions(self):
        values=[{},None,{'usable':False,'sample':{'country':'HUN','completed_focuses':['a']}},
                {'usable':True,'sample':{'current_focus':42}},
                {'schema_version':True,'usable':True,'facts':{}},
                {'schema_version':1,'usable':True,'facts':{
                    'focus.active':{'status':'observed','value':42,'source':'fixture'},
                    'missing':{'status':'unknown','value':12,'source':''},
                    'zero':{'status':'observed','value':0,'source':'fixture'},
                    'false':{'status':'calculated','value':False,'source':'fixture'}}}]
        for value in values:
            with self.subTest(value=value):self.assertEqual(_request(0,'state_normalize',value=value),reference(value))
