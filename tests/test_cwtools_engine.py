import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch


class CwtoolsEngineTests(unittest.TestCase):
    def test_effective_manifest_respects_load_order_and_replace_path(self):
        from hoi4companion.cwtools_engine import installation_files
        with TemporaryDirectory() as temp:
            game, mod = Path(temp)/'game', Path(temp)/'mod'
            for root in (game, mod):
                (root/'events').mkdir(parents=True)
            (game/'events/a.txt').write_text('old=yes')
            (game/'events/b.txt').write_text('old=yes')
            (mod/'descriptor.mod').write_text('replace_path="events"')
            (mod/'events/a.txt').write_text('new=yes')
            files, warnings = installation_files(game, [mod])
            self.assertEqual(files, {'events/a.txt':str(mod/'events/a.txt')})
            self.assertEqual(warnings, [])

    def test_installation_uses_rust_engine_and_never_silently_falls_back(self):
        from hoi4companion.script_analysis import analyze_installation
        with TemporaryDirectory() as temp:
            with patch('hoi4companion.cwtools_engine.analyze', side_effect=RuntimeError('missing host')):
                with self.assertRaisesRegex(RuntimeError, 'missing host'):
                    analyze_installation(Path(temp), [], {}, {})

    def test_unknown_metrics_are_not_fabricated_and_positions_are_preserved(self):
        from hoi4companion.cwtools_engine import normalize_report
        raw={'schema_version':1,'engine':'cwtools','engine_revision':'revision',
             'documents':2,'entities':4,'diagnostics':[
                 {'code':'CW250','severity':'error','file':'events/test.txt','message':'Missing reference',
                  'line':3,'column':5,'end_line':3,'end_column':10,'category':'references'}],
             'parser_errors':[], 'type_names':['focus']}
        report=normalize_report(raw, {}, [], 2)
        self.assertIsNone(report['coverage']['unsupported'])
        self.assertIsNone(report['coverage']['unknown_scope'])
        self.assertEqual(report['summary']['references'],1)
        self.assertEqual(report['diagnostics'][0]['position']['line'],3)
        self.assertFalse(report['runtime_evaluated'])
        raw['diagnostic_counts']={'total':5,'error':5,'references':5}
        report=normalize_report(raw,{},[],2)
        self.assertEqual(report['summary']['references'],5)
        self.assertTrue(report['truncated'])


if __name__=='__main__':unittest.main()
