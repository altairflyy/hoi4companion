"""Portable source identities are verified and remapped by the backend."""
from copy import deepcopy
import unittest
from hoi4companion.backend_core import _request
from hoi4companion._guide_importer_reference import Importer,validate_sources

class BackendImportDocumentTests(unittest.TestCase):
    def test_external_source_ids_are_replaced_everywhere_without_changing_evidence(self):
        raw={'format':'hoi4-guide-authoring','authoring_schema_version':1,'sources':[{'id':'s','type':'manual','text':'Research Tools.'}],
             'guide':{'id':'universal','title':'Guide','steps':[{'key':'research','type':'research','target':'Tools','evidence':[{'source_id':'s','quote':'Research Tools.','basis':'explicit'}]}],
             'source_inventory':{'schema_version':1,'items':[{'id':'line','source_id':'s','quote':'Research Tools.','step_keys':['research']}]}}}
        expected=Importer(lambda:[]).document(raw)
        actual=_request(0,'import_document',document=raw)
        self.assertEqual(actual,expected)
        self.assertEqual(_request(0,'import_document',document=actual),actual)
        self.assertEqual(_request(0,'import_validate_sources',sources=actual['sources']),validate_sources(actual['sources']))
        bad=deepcopy(actual);bad['sources'][0]['text']='Changed text'
        with self.assertRaisesRegex(ValueError,'checksum'):_request(0,'import_document',document=bad)

    def test_duplicate_sources_unknown_evidence_and_nonfinite_fields_are_rejected(self):
        raw={'import_schema_version':1,'sources':[{'id':'s','type':'manual','text':'Text'}],
             'guide':{'id':'g','sources':[{'id':'missing'}],'tracks':{}}}
        with self.assertRaises(ValueError) as expected:Importer(lambda:[]).document(raw)
        with self.assertRaises(ValueError) as actual:_request(0,'import_document',document=raw)
        self.assertEqual(str(actual.exception),str(expected.exception))
        raw['sources'].append(deepcopy(raw['sources'][0]))
        with self.assertRaises(ValueError):_request(0,'import_document',document=raw)

    def test_generated_prompt_remains_identical_to_public_instructions(self):
        from hoi4companion._guide_authoring_reference import action_reference
        from pathlib import Path
        import hoi4companion
        table=['| type | Type-specific fields | Required |','| --- | --- | --- |']
        for row in action_reference():
            required=', '.join(row['required']) or ('equipment + factories or lines' if row['type']=='production' else 'person, advisor or trait + role' if row['type']=='appoint' else '—')
            table.append('| '+row['type']+' | '+(', '.join(row['fields']) or '—')+' | '+required+' |')
        expected=Path(hoi4companion.__file__).with_name('guide-import-prompt.md').read_text(encoding='utf8').replace('<!-- tipi-authoring-v1 -->','\n'.join(table))
        self.assertEqual(_request(0,'authoring_prompt'),expected)
