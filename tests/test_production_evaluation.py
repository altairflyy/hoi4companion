"""Production evaluation must be snapshot-local and identical in both runtimes."""
import copy
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

from hoi4companion import design_matching
from hoi4companion.guide_runtime import evaluate
from hoi4companion.production_progress import production_context
from test_production_remaining import guide, state, line
import test_production_sequence


def repeated_layouts():
    value = guide()
    first = value['tracks']['operations'][0]
    for target in first['action']['params']['production_targets']:
        target['production_priority'] = 'first'
    value['tracks']['operations'] = [dict(copy.deepcopy(first), key='layout-' + str(i)) for i in range(24)]
    return value


class ProductionEvaluationTests(unittest.TestCase):
    def test_each_distinct_product_is_matched_once_per_snapshot(self):
        g = repeated_layouts()
        from hoi4companion.backend_core import _request
        native=_request(0,'production_context',kind='production_stats',guide=g,
            state=state([line('1','sup',1,1),line('2','inf',19,19)]),baselines={})
        self.assertEqual(native['distinct_product_comparisons'],2)
        script = """
const fs=require('fs'),designs=require(process.argv[1]),p=require(process.argv[2]);
const x=JSON.parse(fs.readFileSync(0,'utf8')),original=designs.matching;let calls=0;
designs.matching=(...args)=>{calls++;return original(...args)};
p.context(x.guide,x.state,{});process.stdout.write(JSON.stringify(calls));
"""
        result = subprocess.run(['node', '-e', script,
            str(Path('src/hoi4companion/web/design-matching.js').resolve()),
            str(Path('src/hoi4companion/web/production-progress.js').resolve())],
            input=json.dumps({'guide': g, 'state': state([line('1', 'sup', 1), line('2', 'inf', 19)])}),
            text=True, capture_output=True, check=True)
        self.assertEqual(json.loads(result.stdout), 2)

    def test_new_snapshots_and_guide_recipes_never_reuse_old_matches(self):
        g = guide()
        params = g['tracks']['operations'][0]['action']['params']['production_targets'][1]
        params['design_reference'] = {'modules': {'gun': 'cannon'}, 'upgrades': {}}
        values = [line('1', 'sup', 1, 1), {**line('2', 'inf', 19, 19),
                  'design': {'status': 'observed', 'modules': {'gun': 'cannon'}, 'upgrades': {}}}]
        inputs = []
        def capture():
            inputs.append({'guide': copy.deepcopy(g), 'state': state(copy.deepcopy(values))})
        capture()
        values[1]['design'] = {'status': 'unavailable'}; capture()
        values[1]['design'] = {'status': 'observed', 'modules': {'gun': 'other'}, 'upgrades': {}}; capture()
        params['design_reference']['modules']['gun'] = 'other'; capture()
        stale = copy.deepcopy(inputs[-1]); stale['state']['usable'] = False; inputs.append(stale)
        results = [evaluate(x['guide'], x['state']) for x in inputs]
        checks = [x['steps'][0]['production']['layout'][1] for x in results]
        self.assertEqual([x['requested'] for x in checks], [19, None, 0, 19, None])
        script = "const fs=require('fs'),r=require(process.argv[1]);process.stdout.write(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(x=>r.evaluate(x.guide,x.state))));"
        run = subprocess.run(['node', '-e', script, str(Path('src/hoi4companion/web/guide-runtime.js').resolve())],
            input=json.dumps(inputs), text=True, capture_output=True, check=True)
        self.assertEqual(json.loads(run.stdout), results)

    def test_recipe_property_order_does_not_keep_an_obsolete_production_target(self):
        helper = test_production_sequence.ProductionSequenceTests()
        g = helper.guide()
        for row, modules in zip(g['tracks']['operations'],
                                ({'gun': 'cannon', 'turret': 'basic'}, {'turret': 'basic', 'gun': 'cannon'})):
            row['action']['params']['production_targets'][0]['design_reference'] = {
                'modules': modules, 'upgrades': {}}
        s = helper.live(['phase'])
        s['facts']['production.lines']['value'][0]['design'] = {
            'status': 'observed', 'modules': {'gun': 'cannon', 'turret': 'basic'}, 'upgrades': {}}
        expected = evaluate(g, s)
        self.assertEqual(expected['steps'][0]['reason'], 'production_superseded')
        script = "const fs=require('fs'),r=require(process.argv[1]),x=JSON.parse(fs.readFileSync(0,'utf8'));process.stdout.write(JSON.stringify(r.evaluate(x.guide,x.state)));"
        run = subprocess.run(['node', '-e', script, str(Path('src/hoi4companion/web/guide-runtime.js').resolve())],
            input=json.dumps({'guide': g, 'state': s}), text=True, capture_output=True, check=True)
        self.assertEqual(json.loads(run.stdout), expected)

    def test_recipe_labels_do_not_split_equivalent_revisions_but_different_recipes_do(self):
        helper = test_production_sequence.ProductionSequenceTests()
        g = helper.guide()
        first, later = [row['action']['params']['production_targets'][0]
                        for row in g['tracks']['operations'][:2]]
        for p, name in ((first, 'Old model label'), (later, 'New model label')):
            p['production_selector']['variant_names'] = [name]
            p['design_reference'] = {'modules': {'gun': 'cannon'}, 'upgrades': {}}
        s = helper.live(['phase'])
        s['facts']['production.lines']['value'][0]['design'] = {
            'status': 'observed', 'modules': {'gun': 'cannon'}, 'upgrades': {}}
        inputs = [{'guide': copy.deepcopy(g), 'state': s}]
        later['design_reference']['modules']['gun'] = 'other'; inputs.append({'guide': copy.deepcopy(g), 'state': s})
        first.pop('design_reference'); later.pop('design_reference'); inputs.append({'guide': g, 'state': s})
        expected = [evaluate(x['guide'], x['state']) for x in inputs]
        self.assertEqual(expected[0]['steps'][0]['reason'], 'production_superseded')
        for value in expected[1:]:
            self.assertNotEqual(value['steps'][0]['reason'], 'production_superseded')
            self.assertNotIn('superseded_by', value['steps'][0]['production']['layout'][0])
        script = "const fs=require('fs'),r=require(process.argv[1]);process.stdout.write(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(x=>r.evaluate(x.guide,x.state))));"
        run = subprocess.run(['node', '-e', script, str(Path('src/hoi4companion/web/guide-runtime.js').resolve())],
            input=json.dumps(inputs), text=True, capture_output=True, check=True)
        self.assertEqual(json.loads(run.stdout), expected)

    def test_cached_comparison_explanations_remain_independent_per_step(self):
        g = repeated_layouts()
        for row in g['tracks']['operations']:
            row['action']['params']['production_targets'][0]['design_reference'] = {
                'modules': {'gun': 'cannon'}, 'upgrades': {}}
        s = state([{**line('1', 'sup', 1), 'design': {
            'status': 'observed', 'modules': {'gun': 'other'}, 'upgrades': {}}}])
        before = copy.deepcopy((g, s))
        _, details = production_context(g, s, {})
        details['layout-0']['layout'][0]['design_check']['candidates'][0]['differences'].clear()
        self.assertEqual(len(details['layout-1']['layout'][0]['design_check']['candidates'][0]['differences']), 1)
        self.assertEqual((g, s), before)
