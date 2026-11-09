"""Pinned CWTools source descriptions and static report parity, independent of live IO."""
from copy import deepcopy
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from hoi4companion.backend_core import _request
from hoi4companion._script_analysis_reference import RuleBook,RULE_ROOT
from hoi4companion._cwtools_engine_reference import normalize_report,installation_files
from test_source_errors import raw_report

class NativeScriptAnalysisTests(unittest.TestCase):
    def test_all_pinned_rule_variants_match_original_rule_book(self):
        expected=RuleBook.load()
        got=_request(0,'script_rulebook',root=str(RULE_ROOT))
        self.assertEqual(got['rules'],expected.rules)
        self.assertEqual(got['links'],expected.links)
        self.assertEqual({k:set(v) for k,v in got['enums'].items()},expected.enums)
        self.assertEqual(got['load_errors'],expected.load_errors)
        self.assertEqual(got['provenance'],expected.provenance)

    def test_report_keeps_unknown_metrics_positions_counts_and_source_recoveries(self):
        raw=raw_report();raw.update(schema_version=1,engine='cwtools',diagnostics=[{'code':'CW250','severity':'error','file':'events/a.txt','message':'Missing ref','line':3,'column':2,'end_line':3,'end_column':6,'category':'references'}])
        files={'events/broken.txt':'C:/fixture/events/broken.txt'}
        for supplied in (raw,{**raw,'diagnostic_counts':{'total':5,'error':5,'references':5}}, {**raw,'source_recoveries':[{'file':'a','evidence':{}}]}):
            before=deepcopy(supplied)
            got=_request(0,'script_normalize_report',raw=supplied,files=files,warnings=['fixture warning'],root=str(RULE_ROOT),limit=1)
            self.assertEqual(got,normalize_report(supplied,files,['fixture warning'],1));self.assertEqual(supplied,before)

    def test_manifest_honors_mod_overlay_and_replacement_without_writing_game(self):
        with TemporaryDirectory() as folder:
            game,mod=Path(folder)/'game',Path(folder)/'mod'
            for root in (game,mod):(root/'events').mkdir(parents=True)
            (game/'events/a.txt').write_text('old=yes');(game/'events/b.txt').write_text('old=yes')
            (mod/'descriptor.mod').write_text('replace_path="events"');(mod/'events/a.txt').write_text('new=yes')
            files,warnings=installation_files(game,[mod])
            got=_request(0,'script_analysis_files',game=str(game),mods=[str(mod)])
            self.assertEqual(got,{'files':files,'warnings':warnings})

    def test_tampered_provenance_rejects_changed_rules(self):
        import hashlib,json
        with TemporaryDirectory() as folder:
            root=Path(folder);(root/'rules.cwt').write_text('a=yes')
            (root/'provenance.json').write_text(json.dumps({'files':{'rules.cwt':'0'*64}}))
            with self.assertRaisesRegex(ValueError,'modificat|modified'):_request(0,'script_rulebook',root=str(root))

    def test_source_error_history_retains_versions_and_observes_successful_retry(self):
        import json
        raw=raw_report();files={'events/broken.txt':'C:/fixture/events/broken.txt'}
        now='2026-11-09T12:00:00+00:00'
        with TemporaryDirectory() as folder:
            path=Path(folder)/'logs/source-errors.json'
            def record():
                report=normalize_report(raw,files,[])
                return _request(0,'script_error_log',report=report,path=str(path),now=now)
            result=record();record();self.assertEqual(result['source_error_log']['status'],'saved')
            entries=json.loads(path.read_text(encoding='utf8'))['entries'];self.assertEqual(len(entries),1)
            self.assertTrue(entries[0]['active']);self.assertEqual(entries[0]['source_path'],files['events/broken.txt'])
            self.assertEqual(entries[0]['first_seen'],now)
            raw['parser_errors'][0]['source_sha256']='c'*64;record()
            entries=json.loads(path.read_text(encoding='utf8'))['entries'];self.assertEqual(len(entries),2)
            self.assertEqual(sum(e['active'] for e in entries),1)
            raw['parser_errors']=[];record()
            self.assertFalse(any(e['active'] for e in json.loads(path.read_text(encoding='utf8'))['entries']))
            path.write_text('broken',encoding='utf8');result=record()
            self.assertEqual(result['source_error_log']['status'],'error');self.assertEqual(path.read_text(encoding='utf8'),'broken')
            self.assertEqual(list(path.parent.glob('*.tmp')),[])

    def test_actual_upstream_host_matches_native_normalization_without_resident_python(self):
        from hoi4companion.cwtools_engine import analyze,normalize_report,HOST_ROOT
        root=Path(__file__).resolve().parents[1]
        with TemporaryDirectory() as folder:
            file=Path(folder)/'test.txt'
            file.write_text('focus_tree={id=T country={factor=1} focus={id=A x=0 y=0 cost=10 prerequisite={focus=MISSING} available={always=perhaps}}}',encoding='utf8')
            files={'common/national_focus/test.txt':str(file)}
            expected=normalize_report(analyze(files),files,[])
            actual=_request(0,'script_analysis_run',manifest={'files':files,'warnings':[]},rules=str(RULE_ROOT),host=str(HOST_ROOT),parser=str(root/'src/hoi4companion/_native/hoi4companion_parser.dll'))
            # Host timings are measurements of separate executions, not semantic output.
            self.assertIsInstance(actual.pop('elapsed_ms'),(int,float));expected.pop('elapsed_ms')
            self.assertEqual(actual,expected);self.assertFalse(actual['runtime_evaluated'])
