"""Native Guide Schema migration must preserve normalized data and rejection."""
import json
from pathlib import Path
import unittest
from hoi4companion.backend_core import _request
from hoi4companion._guide_normalize_reference import normalise_guide as reference


class BackendGuideSchemaTests(unittest.TestCase):
    def test_shared_guides_are_normalized_or_rejected_like_independent_schema(self):
        count=0
        def check(guide,path):
            nonlocal count
            try:expected=reference(guide)
            except (ValueError,KeyError,TypeError):
                with self.subTest(path=path),self.assertRaises(ValueError):_request(0,'guide_normalize',guide=guide)
            else:
                with self.subTest(path=path):
                    actual=_request(0,'guide_normalize',guide=guide)
                    self.assertEqual(actual,expected)
                    self.assertEqual(_request(0,'guide_normalize',guide=actual),expected)
            count+=1
        def walk(value,path):
            if isinstance(value,dict):
                if 'guide' in value:check(value['guide'],path)
                for key,child in value.items():walk(child,path+'/'+str(key))
            elif isinstance(value,list):
                for index,child in enumerate(value):walk(child,path+'/'+str(index))
        for file in sorted((Path(__file__).parent/'fixtures').glob('*.json')):
            walk(json.loads(file.read_text(encoding='utf-8')),file.name)
        self.assertGreater(count,100)

    def test_cycles_missing_ids_and_misqualified_evidence_are_rejected(self):
        for guide in (
            {'tracks':{'a':[{'key':'a','kind':'gate','depends_on':['a']}]}},
            {'tracks':{'a':[{'key':'a','kind':'gate','depends_on':['missing']}]}},
            {'tracks':{'a':[{'kind':'focus','target':''}]}},
            {'tracks':{'a':[{'kind':'gate','evidence':[{'source_id':'missing','basis':'explicit'}]}]}},
        ):
            with self.subTest(guide=guide),self.assertRaises(ValueError):_request(0,'guide_normalize',guide=guide)
