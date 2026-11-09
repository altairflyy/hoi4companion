import copy
import json
from pathlib import Path
import subprocess
import unittest
import threading
from http.server import ThreadingHTTPServer
from types import SimpleNamespace
from urllib.request import urlopen

from hoi4companion.design_matching import compare, matching, valid_live_design
from hoi4companion.production_progress import selector_for, relative_spec
from hoi4companion.guide_runtime import evaluate
from hoi4companion.screen_overlay import targets
from hoi4companion.production_navigation import matches as navigate
from hoi4companion.contracts import normalise_state
from test_production_remaining import guide, state, line, CATALOG


RECIPE = {'modules': {'turret': 'three_man', 'gun': 'howitzer', 'special1': 'radio', 'special2': 'smoke'},
          'upgrades': {'engine': '0', 'armor': '4'}}


def design(**changes):
    return {'status': 'observed', 'modules': copy.deepcopy(RECIPE['modules']),
            'upgrades': {'engine': 0, 'armor': 4}, **changes}


def fixture():
    value = guide(); row = value['tracks']['operations'][0]
    params = row['action']['params']['production_targets'][1]
    params['design_reference'] = {'id': 'recipe', 'name': 'Required model', **copy.deepcopy(RECIPE)}
    params['variant_name'] = 'Required model'
    params['production_selector']['variant_names'] = ['Required model']
    values = [line('1', 'sup', 1, 1), {**line('2', 'inf', 19, 19), 'name': 'Different national name', 'design': design()}]
    return value, row, params, values


class DesignMatchingTests(unittest.TestCase):
    def test_comparison_script_is_served_before_browser_contracts(self):
        from hoi4companion.api import make_handler
        with ThreadingHTTPServer(('127.0.0.1', 0), make_handler(SimpleNamespace())) as server:
            thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
            try:
                base = f'http://127.0.0.1:{server.server_port}'
                with urlopen(base + '/design-matching.js') as response:
                    self.assertEqual(response.status, 200)
                    self.assertIn(b'HoiDesignMatching', response.read())
                with urlopen(base + '/walkthrough') as response:
                    html = response.read().decode()
                    self.assertLess(html.index('/design-matching.js'), html.index('/contracts.js'))
            finally:
                server.shutdown(); thread.join()

    def test_exact_configuration_ignores_name_but_not_slots_levels_or_extra_modules(self):
        g, row, params, rows = fixture()
        runtime = evaluate(g, state(rows)); result = runtime['steps'][0]
        self.assertEqual(result['status'], 'completed')
        detail = result['production']['layout'][1]
        self.assertEqual(detail['design_check']['status'], 'match')
        hud = targets(g, runtime, CATALOG, {**state(rows), 'sample': {'country': 'TST'}})[1]
        self.assertTrue(hud['goal_satisfied']); self.assertEqual(hud['production_line_ids'], ['2'])
        control = {'line_id': '2', 'equipment': 'inf', 'variant_name': 'Different national name', 'visible': True, 'rect': [0, 0, 100, 30]}
        raw = {'country': 'TST', 'production_ui': {'status': 'observed', 'visible': True, 'lines': [control]}}
        self.assertEqual(navigate(raw, [hud], lambda c, p: c['rect'])[0]['navigation_stage'], 'line')
        for changed in (design(modules={**RECIPE['modules'], 'special2': 'hmg'}),
                        design(modules={**RECIPE['modules'], 'extra': 'hmg'}),
                        design(upgrades={'armor': 5}), design(upgrades={'armor': 3})):
            rows[1].update(name='Required model', design=changed)
            failed = evaluate(g, state(rows))['steps'][0]
            self.assertNotEqual(failed['status'], 'completed')
            self.assertEqual(failed['production']['layout'][1]['requested'], 0)
            self.assertEqual(failed['production']['layout'][1]['design_check']['status'], 'different')

    def test_unreadable_candidate_never_counts_even_with_the_correct_name(self):
        g, _, _, rows = fixture(); rows[1]['name'] = 'Required model'
        for configuration in (None, {'status': 'unavailable', 'reason': 'unqualified'}):
            if configuration is None: rows[1].pop('design', None)
            else: rows[1]['design'] = configuration
            value = evaluate(g, state(rows))['steps'][0]['production']['layout'][1]
            self.assertIsNone(value['requested']); self.assertEqual(value['design_check']['status'], 'unknown')
        rows[1]['design'] = design(); rows.append({**line('3', 'inf', 2), 'design': {'status': 'unavailable'}})
        result = evaluate(g, state(rows))['steps'][0]['production']['layout'][1]
        self.assertIsNone(result['requested'])

    def test_empty_slots_and_zero_upgrades_are_canonical_only_for_complete_designs(self):
        self.assertEqual(compare(RECIPE, design(upgrades={'armor': 4}))['status'], 'match')
        expected = {**RECIPE, 'modules': {**RECIPE['modules'], 'unused': 'empty'}}
        self.assertEqual(compare(expected, design())['status'], 'match')
        for observed in ({'status': 'partial', **RECIPE}, {'status': 'observed', 'modules': RECIPE['modules']},
                         design(upgrades={'armor': '4'}), design(upgrades={'armor': True}), design(upgrades={'armor': 4.5})):
            self.assertEqual(compare(RECIPE, observed)['status'], 'unknown')
        self.assertTrue(valid_live_design(design(upgrades={'armor': 4.0})))

    def test_picker_uses_complete_recipe_and_ignores_display_name(self):
        g, _, _, rows = fixture(); rows[1]['design']['modules']['gun'] = 'wrong'
        r = evaluate(g, state(rows)); hud = targets(g, r, CATALOG, {**state(rows), 'sample': {'country': 'TST'}})[1]
        self.assertEqual(hud['production_design_check']['status'], 'different')
        raw = {'country': 'TST', 'production_ui': {'status': 'observed', 'visible': True, 'picker_open': True, 'picker_count': 1,
            'equipment': [{'equipment': 'inf', 'variant_name': 'National name', 'design': design(), 'visible': True, 'rect': [0, 0, 100, 30]}]}}
        self.assertEqual(navigate(raw, [hud], lambda c, p: c['rect'])[0]['navigation_stage'], 'equipment')
        raw['production_ui']['equipment'][0]['design'] = {'status': 'unavailable'}
        self.assertEqual(navigate(raw, [hud], lambda c, p: c['rect']), [])

    def test_recipe_changes_baseline_identity_and_no_recipe_preserves_generic_matching(self):
        _, _, params, _ = fixture(); p = copy.deepcopy(params); p.update(quantity_mode='additional',requested_factories=2)
        step = {'key': 'item', 'action': {'params': p}, 'completion': {'op': 'fact', 'key': 'guide.production.item.added', 'test': 'gte', 'value': 2}}
        before = relative_spec(step)['fact_key']; p['design_reference']['upgrades']['armor'] = '5'
        self.assertNotEqual(before, relative_spec(step)['fact_key'])
        p.pop('design_reference'); self.assertNotIn('design', selector_for(step))
        self.assertFalse(valid_live_design({'status': 'unavailable', 'modules': {}, 'upgrades': {}}))

    def test_python_javascript_matching_runtime_and_contract_parity(self):
        g, _, params, rows = fixture(); inputs = []
        for cfg in (design(), design(modules={'gun': 'wrong'}), {'status': 'unavailable'}, None):
            values = copy.deepcopy(rows)
            if cfg is None: values[1].pop('design')
            else: values[1]['design'] = cfg
            inputs.append({'guide': g, 'state': state(values)})
        script = "const fs=require('fs'),r=require(process.argv[1]);process.stdout.write(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(x=>r.evaluate(x.guide,x.state))));"
        run = subprocess.run(['node', '-e', script, str(Path('src/hoi4companion/web/guide-runtime.js').resolve())], input=json.dumps(inputs), text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(run.stdout), [evaluate(x['guide'], x['state']) for x in inputs])
        # The adapters preserve qualified structure, reject malformed observations.
        states = [{'usable': True, 'sample': {'country': 'TST', 'country_overview': {'country': 'TST', 'production': {'status': 'observed', 'lines': [row]}}}}
                  for row in ({**rows[1], 'design': design(upgrades={'armor': 4.0})}, {**rows[1], 'design': design(upgrades={'armor': '4'})})]
        adapter = "const fs=require('fs'),c=require(process.argv[1]);process.stdout.write(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(c.state)));"
        run = subprocess.run(['node', '-e', adapter, str(Path('src/hoi4companion/web/contracts.js').resolve())], input=json.dumps(states), text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(run.stdout), [normalise_state(s) for s in states])
