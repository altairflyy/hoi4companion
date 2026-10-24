import hashlib
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from hoi4companion.jomini_parser import parse,parse_file
from hoi4companion.cwtools_engine import analyze,normalize_report
from hoi4companion.source_errors import record_source_errors

FIXTURE=Path(__file__).parents[1]/'native/parser/tests/fixtures/recovery-empty.txt'


class ReviewedRecoveryTests(unittest.TestCase):
    def test_shared_root_includes_descendants_without_unrelated_branches(self):
        from hoi4companion.focus import load_trees,evaluate
        with TemporaryDirectory() as temp:
            file=Path(temp)/'focus.txt';file.write_text('''
                joint_focus={id=A x=0 y=0 cost=1}
                joint_focus={id=B x=1 y=1 cost=1 prerequisite={focus=A} available={always=no}}
                joint_focus={id=C x=1 y=2 cost=1 prerequisite={focus=B}}
                joint_focus={id=UNRELATED x=9 y=9 cost=1}
                focus_tree={id=t shared_focus=A}
            ''',encoding='utf8')
            trees,warnings=load_trees([file],{})
            self.assertEqual(set(trees['t'].focuses),{'A','B','C'})
            self.assertEqual(warnings,[])
            self.assertEqual(evaluate(trees['t'],{'A'},None)['B'],'verify')

    def test_file_recovery_is_content_pinned_and_strings_remain_strict(self):
        source=FIXTURE.read_bytes()
        with self.assertRaises(ValueError):parse(source.decode())
        with TemporaryDirectory() as temp:
            file=Path(temp)/'renamed.gfx';file.write_bytes(source)
            root=parse_file(file)
            self.assertEqual(root.blocks('objectTypes')[0].blocks('pdxmesh')[0].scalar('name'),'empty_mesh')
            self.assertEqual(root.source_recovery['status'],'recovered_static_definition')
            self.assertFalse(root.source_recovery['omitted_properties'])
            self.assertEqual(file.read_bytes(),source)
            file.write_bytes(source+b' # changed')
            with self.assertRaises(ValueError):parse_file(file)

    def test_required_ancestor_does_not_pull_in_unselected_sibling_branch(self):
        from hoi4companion.focus import load_trees
        with TemporaryDirectory() as temp:
            file=Path(temp)/'focus.txt';file.write_text('''
                shared_focus={id=PARENT}
                shared_focus={id=SELECTED prerequisite={focus=PARENT}}
                shared_focus={id=CHILD prerequisite={focus=SELECTED}}
                shared_focus={id=SIBLING prerequisite={focus=PARENT}}
                focus_tree={id=t shared_focus=SELECTED}
            ''',encoding='utf8')
            trees,_=load_trees([file],{})
            self.assertEqual(set(trees['t'].focuses),{'PARENT','SELECTED','CHILD'})

    def test_both_parsers_consume_the_same_recovery_and_registry_preserves_error_history(self):
        with TemporaryDirectory() as temp:
            file=Path(temp)/'fixture.gfx';file.write_bytes(FIXTURE.read_bytes())
            files={'gfx/entities/empty.gfx':str(file)}
            raw=analyze(files)
            self.assertEqual(raw['parser_errors'],[])
            self.assertEqual(len(raw['source_recoveries']),1)
            item=raw['source_recoveries'][0]
            self.assertEqual(item['source_sha256'],hashlib.sha256(file.read_bytes()).hexdigest())
            evidence=parse_file(file).source_recovery
            self.assertEqual(item['evidence'],evidence)
            report=normalize_report(raw,files,[])
            self.assertEqual(report['status'],'partial')
            self.assertEqual(report['coverage']['recovered_sources'],1)
            self.assertEqual(report['coverage']['parser_errors'],0)
            registry=Path(temp)/'source-errors.json'
            original={'file':item['file'],'source_sha256':item['source_sha256'],
                      'message':'Original CWTools error','jomini_error':'Unclosed block',
                      'classification':'rejected_by_both','line':8,'column':1}
            record_source_errors({**report,'source_recoveries':[],'ignored_source_errors':[original]},registry)
            record_source_errors(report,registry)
            entries=json.loads(registry.read_text())['entries']
            self.assertEqual(len(entries),1)
            self.assertEqual(entries[0]['disposition'],'recovered_in_memory')
            self.assertEqual(entries[0]['message'],'Original CWTools error')
            self.assertEqual(entries[0]['line'],8)
            self.assertTrue(entries[0]['active'])
            self.assertEqual(entries[0]['recovery'],evidence)

    def test_recovery_does_not_invent_values_or_repair_unknown_missing_properties(self):
        with TemporaryDirectory() as temp:
            file=Path(temp)/'fixture.txt';file.write_text('particle={force=}',encoding='utf8')
            with self.assertRaises(ValueError):parse_file(file)
            raw=analyze({'gfx/particles/fixture.txt':str(file)})
            self.assertTrue(raw['parser_errors'])
            self.assertEqual(raw['source_recoveries'],[])


if __name__=='__main__':unittest.main()
