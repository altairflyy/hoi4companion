import json
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch
import unittest

from hoi4companion.cwtools_engine import normalize_report


def raw_report():
    return {'engine_revision':'test', 'documents':1, 'entities':1,
            'diagnostics':[], 'type_names':[], 'parser_errors':[
                {'file':'events/broken.txt', 'classification':'rejected_by_both',
                 'message':'Missing brace', 'jomini_error':'Unclosed container',
                 'source_sha256':'a'*64, 'line':3, 'column':1},
                {'file':'events/disagreement.txt', 'classification':'parser_disagreement',
                 'message':'Unexpected token', 'jomini_error':'',
                 'source_sha256':'b'*64, 'line':2, 'column':1}]}


class SourceErrorsTests(unittest.TestCase):
    def test_only_cross_checked_source_failures_are_set_aside(self):
        report=normalize_report(raw_report(), {}, [])
        self.assertEqual(report['coverage']['ignored_parser_errors'],1)
        self.assertEqual(len(report['ignored_source_errors']),1)
        self.assertEqual(report['coverage']['parser_errors'],2)
        self.assertEqual(len(report['file_warnings']),1)
        self.assertIn('disagreement.txt',report['file_warnings'][0])
        self.assertEqual(report['status'],'partial')

    def test_registry_retains_versions_and_reactivates_without_duplicates(self):
        from hoi4companion.source_errors import record_source_errors
        with TemporaryDirectory() as temp:
            path=Path(temp)/'logs'/'source-errors.json'
            files={'events/broken.txt':str(Path(temp)/'game/events/broken.txt')}
            raw=raw_report()
            def record():
                return record_source_errors(normalize_report(raw,files,[]),path)
            report=record();record()
            saved=json.loads(path.read_text(encoding='utf8'))
            self.assertEqual(len(saved['entries']),1)
            entry=saved['entries'][0]
            self.assertEqual(entry['source_path'],files['events/broken.txt'])
            self.assertEqual(entry['source_sha256'],'a'*64)
            self.assertTrue(entry['active'])
            self.assertEqual(entry['line'],3)
            self.assertEqual(report['source_error_log']['status'],'saved')
            self.assertFalse(report['runtime_evaluated'])
            raw['parser_errors'][0]['source_sha256']='c'*64
            record()
            entries=json.loads(path.read_text(encoding='utf8'))['entries']
            self.assertEqual(len(entries),2)
            self.assertEqual(sum(e['active'] for e in entries),1)
            raw['parser_errors']=[]
            record()
            self.assertFalse(any(e['active'] for e in json.loads(path.read_text())['entries']))
            self.assertEqual(len(json.loads(path.read_text())['entries']),2)

    def test_another_installation_does_not_clear_previous_installation(self):
        from hoi4companion.source_errors import record_source_errors
        with TemporaryDirectory() as temp:
            path=Path(temp)/'errors.json'
            first={'events/broken.txt':str(Path(temp)/'game/events/broken.txt')}
            record_source_errors(normalize_report(raw_report(),first,[]),path)
            record_source_errors(normalize_report({**raw_report(),'parser_errors':[]},
                                {'events/broken.txt':str(Path(temp)/'other/events/broken.txt')},[]),path)
            self.assertTrue(json.loads(path.read_text())['entries'][0]['active'])

    def test_unwritable_or_corrupt_registry_is_visible_and_preserved(self):
        from hoi4companion.source_errors import record_source_errors
        with TemporaryDirectory() as temp:
            path=Path(temp)/'errors.json';path.write_text('broken')
            report=record_source_errors(normalize_report(raw_report(),{},[]),path)
            self.assertEqual(path.read_text(),'broken')
            self.assertEqual(report['source_error_log']['status'],'error')
            self.assertTrue(any('registro' in e.lower() for e in report['file_warnings']))
            self.assertEqual(report['status'],'partial')

    def test_failed_atomic_write_preserves_existing_log(self):
        from hoi4companion.source_errors import record_source_errors
        with TemporaryDirectory() as temp:
            path=Path(temp)/'errors.json'
            report=normalize_report(raw_report(),{},[])
            record_source_errors(report,path)
            original=path.read_bytes()
            with patch('hoi4companion.source_errors.os.replace',side_effect=PermissionError('locked')):
                failed=record_source_errors(report,path)
            self.assertEqual(path.read_bytes(),original)
            self.assertEqual(failed['source_error_log']['status'],'error')
            self.assertEqual(list(Path(temp).glob('*.tmp')),[])


if __name__=='__main__':unittest.main()
