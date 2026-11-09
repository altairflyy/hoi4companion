from unittest import TestCase
from hoi4companion.backend_core import _request
from hoi4companion._source_coverage_reference import coverage_report,inventory_items
from hoi4companion._guide_sequence_reference import sequence_report

class NativeImportReportsTests(TestCase):
    def test_coverage_and_structural_barriers_match_legacy(self):
        guide={'tracks':{'operations':[{'key':'a','action':{'kind':'manual'}},{'key':'b','action':{'kind':'manual'},'depends_on_started':['a']}]},'source_inventory':{'schema_version':1,'items':[
          {'id':'one','source_id':'s','quote':'Upgrade both.','timestamp_seconds':12,'step_keys':['a']},
          {'id':'two','source_id':'s','quote':'Upgrade both.','timestamp_seconds':12.0,'step_keys':['b']}]}}
        sources={'s':{'text':'Upgrade both.','segments':[{'start':12,'text':'Upgrade both.'}]}}
        self.assertEqual(_request(0,'import_coverage',guide=guide,sources=sources),coverage_report(guide,sources))
        self.assertEqual(_request(0,'import_inventory',guide=guide),inventory_items(guide))
        self.assertEqual(_request(0,'import_sequence',guide=guide),sequence_report(guide))
        guide['source_inventory']['items'][1]['step_keys']=['a']
        actual=_request(0,'import_coverage',guide=guide,sources=sources)
        expected=coverage_report(guide,sources)
        # Legacy used an unordered set only for untracked-step diagnostics.
        self.assertEqual(sorted(actual['issues'],key=str),sorted(expected['issues'],key=str))
        for key in set(actual)-{'issues'}:self.assertEqual(actual[key],expected[key])
