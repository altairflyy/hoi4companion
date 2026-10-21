import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from http.server import ThreadingHTTPServer
from threading import Thread
from urllib.request import Request, urlopen
from urllib.error import HTTPError
from unittest.mock import Mock

from hoi4companion.clausewitz import parse
from hoi4companion.dlc import DlcInventory, discover_dlcs
from hoi4companion.eligibility import condition, filter_trees, load_scripted_triggers
from hoi4companion.focus import Tree, load_trees
from hoi4companion.api import Store, make_handler


def inventory(*names):
    return DlcInventory(items=[{'name': name, 'status': 'enabled'} for name in names])


def tree(tree_id, country, default=False):
    return Tree(id=tree_id, source='fixture.txt', country_selector=parse(country), default=default)


class DlcDiscoveryTest(unittest.TestCase):
    def test_payload_disabled_and_integrated(self):
        with tempfile.TemporaryDirectory() as td:
            game, docs = Path(td) / 'game', Path(td) / 'docs'
            docs.mkdir()
            for group, name, payload in [('dlc', 'Expansion', True), ('dlc', 'Missing', False),
                                         ('integrated_dlc', 'Built In', False)]:
                folder = game / group / name
                folder.mkdir(parents=True)
                (folder / 'pack.dlc').write_text(f'name="{name}" path="dlc/{name}/data" steam_id="123"')
                if payload:
                    (folder / 'data').mkdir()
            (docs / 'dlc_load.json').write_text(json.dumps({'disabled_dlcs': ['dlc/Expansion/pack.dlc']}))
            result = discover_dlcs(game, docs)
            self.assertFalse(result.enabled('Expansion'))
            self.assertFalse(result.enabled('Missing'))
            self.assertTrue(result.enabled('Built In'))
            self.assertFalse(result.enabled('Not Installed'))
            self.assertEqual({x['status'] for x in result.items}, {'disabled', 'missing', 'integrated'})

    def test_archive_and_recheck_launcher(self):
        with tempfile.TemporaryDirectory() as td:
            game, docs = Path(td) / 'game', Path(td) / 'docs'
            folder = game / 'dlc' / 'pack'
            folder.mkdir(parents=True)
            docs.mkdir()
            (folder / 'pack.dlc').write_text('name="Pack" archive="dlc/pack/payload.zip"')
            (folder / 'payload.zip').write_bytes(b'fixture')
            config = docs / 'dlc_load.json'
            config.write_text('{"disabled_dlcs":[]}')
            self.assertTrue(discover_dlcs(game, docs).enabled('Pack'))
            config.write_text('{"disabled_dlcs":["dlc\\\\pack\\\\pack.dlc"]}')
            self.assertFalse(discover_dlcs(game, docs).enabled('Pack'))

    def test_invalid_launcher_and_descriptor_are_unknown(self):
        with tempfile.TemporaryDirectory() as td:
            game, docs = Path(td) / 'game', Path(td) / 'docs'
            folder = game / 'dlc' / 'pack'
            folder.mkdir(parents=True)
            docs.mkdir()
            (folder / 'pack.dlc').write_text('name="Pack" path="dlc/pack"')
            (docs / 'dlc_load.json').write_text('{broken')
            result = discover_dlcs(game, docs)
            self.assertIsNone(result.enabled('Pack'))
            self.assertIsNone(result.enabled('Absent'))
            (docs / 'dlc_load.json').write_text('{"disabled_dlcs":[]}')
            (folder / 'broken.dlc').write_text('name = {')
            result = discover_dlcs(game, docs)
            self.assertTrue(result.enabled('Pack'))
            self.assertIsNone(result.enabled('Absent'))


class EligibilityTest(unittest.TestCase):
    def test_boolean_dlc_conditions_preserve_unknown(self):
        dlcs = inventory('Old')
        self.assertTrue(condition(parse('OR={has_dlc="Missing" has_dlc="Old"}'), 'AST', dlcs, {}))
        self.assertTrue(condition(parse('has_dlc="Old" NOT={has_dlc="New"}'), 'AST', dlcs, {}))
        self.assertFalse(condition(parse('has_dlc="Missing" runtime_flag=yes'), 'AST', dlcs, {}))
        self.assertIsNone(condition(parse('has_dlc="Old" runtime_flag=yes'), 'AST', dlcs, {}))
        self.assertIsNone(condition(parse('NOT={has_dlc="New"}'), 'AST', DlcInventory(complete=False), {}))

    def test_alternative_priority_and_disabled_fallback(self):
        old = tree('old', 'factor=0 modifier={add=10 tag=AST has_dlc="Old"}')
        new = tree('new', 'factor=0 modifier={add=1000 tag=AST has_dlc="New"}')
        generic = tree('generic', 'factor=1', True)
        trees = {t.id: t for t in (old, new, generic)}
        visible, excluded = filter_trees(trees, inventory('Old'), {})
        self.assertEqual(set(visible), {'old', 'generic'})
        self.assertEqual(old.eligible_countries, ['AST'])
        visible, excluded = filter_trees(trees, inventory('Old', 'New'), {})
        self.assertEqual(set(visible), {'new', 'generic'})
        self.assertEqual(next(x for x in excluded if x['id'] == 'old')['category'], 'alternative')

    def test_shared_country_tree_survives_if_one_country_still_uses_it(self):
        shared = tree('shared', 'factor=0 modifier={add=10 OR={tag=CHI tag=RNG} has_dlc="Old"}')
        new = tree('new', 'factor=0 modifier={add=1000 tag=CHI has_dlc="New"}')
        visible, _ = filter_trees({t.id: t for t in (shared, new)}, inventory('Old', 'New'), {})
        self.assertEqual(set(visible), {'shared', 'new'})
        self.assertEqual(shared.eligible_countries, ['RNG'])

    def test_scripted_selector_and_cycle(self):
        nor = tree('norway', 'factor=0 modifier={add=10 NOR_AAT=yes}')
        scripts = {'NOR_AAT': parse('original_tag=NOR has_dlc="AAT"')}
        visible, _ = filter_trees({'norway': nor}, inventory('AAT'), scripts)
        self.assertIn('norway', visible)
        visible, _ = filter_trees({'norway': nor}, inventory(), scripts)
        self.assertNotIn('norway', visible)
        self.assertIsNone(condition(parse('A=yes'), 'NOR', inventory(), {'A': parse('B=yes'), 'B': parse('A=yes')}))

    def test_runtime_uncertainty_and_equal_weights_are_not_claimed_available(self):
        a = tree('a', 'factor=0 modifier={add=10 tag=AUS has_dlc="DLC" runtime_flag=yes}')
        b = tree('b', 'factor=0 modifier={add=5 tag=AUS}')
        visible, excluded = filter_trees({'a': a, 'b': b}, inventory('DLC'), {})
        self.assertFalse(visible)
        self.assertEqual(next(x for x in excluded if x['id'] == 'a')['category'], 'unknown')
        a = tree('a', 'factor=0 modifier={add=5 tag=AUS}')
        visible, _ = filter_trees({'a': a, 'b': b}, inventory(), {})
        self.assertFalse(visible)

    def test_generic_without_dlc_and_event_tree(self):
        generic = tree('generic', 'factor=1', True)
        event = tree('event', 'factor=0')
        visible, _ = filter_trees({'generic': generic, 'event': event}, inventory(), {})
        self.assertEqual(set(visible), {'generic'})

    def test_generic_priority_and_generic_dlc_requirements_are_respected(self):
        generic = tree('generic', 'factor=1', True)
        weak = tree('weak', 'factor=0 modifier={add=0.5 tag=AUS}')
        visible, _ = filter_trees({'generic': generic, 'weak': weak}, inventory(), {})
        self.assertEqual(set(visible), {'generic'})
        locked = tree('locked', 'factor=0 modifier={add=1 has_dlc="Missing"}', True)
        visible, _ = filter_trees({'locked': locked}, inventory(), {})
        self.assertFalse(visible)

    def test_country_selector_is_preserved_when_loading(self):
        with tempfile.TemporaryDirectory() as td:
            file = Path(td) / 'focus.txt'
            file.write_text('focus_tree={id=test country={factor=0 modifier={add=10 tag=AUS has_dlc="DLC"}}}')
            trees, warnings = load_trees([file], {})
            self.assertFalse(warnings)
            self.assertNotIn('test', filter_trees(trees, inventory(), {})[0])
            self.assertIn('test', filter_trees(trees, inventory('DLC'), {})[0])


class StoreFilterTest(unittest.TestCase):
    def test_refresh_http_checks_origin_and_does_not_save_settings(self):
        store = Mock()
        store.overview.return_value = {'trees': []}
        with ThreadingHTTPServer(('127.0.0.1', 0), make_handler(store)) as server:
            thread = Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                url = f'http://127.0.0.1:{server.server_port}/api/refresh'
                request = Request(url, data=b'{}', headers={'Content-Type': 'application/json', 'Origin': 'http://example.invalid'})
                with self.assertRaises(HTTPError) as error:
                    urlopen(request)
                self.assertEqual(error.exception.code, 403)
                store.refresh.assert_not_called()
                request = Request(url, data=b'{"mods":"off"}', headers={'Content-Type': 'application/json'})
                with self.assertRaises(HTTPError) as error:
                    urlopen(request)
                self.assertEqual(error.exception.code, 400)
                store.refresh.assert_not_called()
                request = Request(url, data=b'{}', headers={'Content-Type': 'application/json'})
                with urlopen(request) as response:
                    self.assertEqual(json.load(response), {'trees': []})
                store.refresh.assert_called_once()
                store.set_settings.assert_not_called()
            finally:
                server.shutdown()
                thread.join()

    def test_refresh_rechecks_dlc_and_rejects_hidden_tree(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            game, docs = root / 'game', root / 'docs'
            focus = game / 'common' / 'national_focus'
            focus.mkdir(parents=True)
            docs.mkdir()
            pack = game / 'dlc' / 'pack'
            pack.mkdir(parents=True)
            (pack / 'pack.dlc').write_text('name="DLC" path="dlc/pack"')
            (focus / 'base.txt').write_text('focus_tree={id=generic default=yes country={factor=1}} focus_tree={id=aus country={factor=0 modifier={add=10 tag=AUS has_dlc="DLC"}}}')
            config = docs / 'dlc_load.json'
            config.write_text('{"disabled_dlcs":[]}')
            with patch('hoi4companion.api.SETTINGS', root / 'settings.json'):
                store = Store({'game_dir': str(game), 'docs_dir': str(docs), 'mods': 'off', 'tree_id': 'aus'})
                self.assertIn('aus', store.trees)
                config.write_text('{"disabled_dlcs":["dlc/pack/pack.dlc"]}')
                store.refresh()
                self.assertNotIn('aus', store.trees)
                self.assertEqual(store.settings['tree_id'], 'generic')
                self.assertIn('error', store.tree('aus'))
                self.assertEqual(len(store.overview()['excluded_trees']), 1)


if __name__ == '__main__':
    unittest.main()
