"""Composition tests: real HTTP, deterministic game fixtures, no native reader."""
import json
import threading
import unittest
from http.server import ThreadingHTTPServer
from threading import RLock
from urllib.error import HTTPError
from urllib.request import Request, urlopen
from unittest.mock import patch

from hoi4companion.api import Store, make_handler
from hoi4companion.guide import normalise_guide


class RuntimeApiTests(unittest.TestCase):
    def test_runs_runtime_legacy_checks_and_http_guards(self):
        guides = {key: normalise_guide({'id': key, 'tracks': {'focus': [
            {'key': 'a', 'action': {'kind': 'manual'}, 'completion': {'op': 'check', 'key': 'done'}},
            {'key': 'b', 'action': {'kind': 'manual'}, 'depends_on': ['a']}], 'research': []}})
            for key in ['one', 'two']}
        store = Store.__new__(Store)
        store.lock = RLock(); store.default_guide_id = 'one'
        store.guide_checks = {}; store.guide_states = {}; store.guide_revision = 0
        store.trees = {}; store.labels = {}; store.settings = {'country': 'GER'}; store.demo = False
        store.technologies = {'technologies': {}, 'folders': {}}
        store.live_cache = {'usable': True, 'sample': {'country': 'GER', 'session_id': 's1'}}
        store.live_state = lambda: store.live_cache
        with patch('hoi4companion.api.load_guide', side_effect=lambda key=None: guides[key or 'one']):
            with ThreadingHTTPServer(('127.0.0.1', 0), make_handler(store)) as server:
                thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
                base = f'http://127.0.0.1:{server.server_port}'
                def call(path, body=None, headers=None):
                    request = Request(base + path, data=None if body is None else json.dumps(body).encode(),
                                      headers=headers or {'Content-Type': 'application/json'})
                    with urlopen(request, timeout=2) as response:
                        return json.load(response)
                try:
                    first = call('/api/guide/progress')
                    self.assertEqual(first['session_id'], 's1')
                    first_id = first['run_id']
                    saved = call('/api/guide/progress', {'run_id': first_id, 'checks': {'done': False}})
                    self.assertEqual(saved['checks'], {'done': False})
                    value = call('/api/guide/runtime')
                    self.assertEqual(value['steps'][0]['completion']['value'], False)
                    self.assertEqual(value['steps'][1]['status'], 'waiting')
                    self.assertEqual(call('/api/guide/checks')['checks'], {'done': False})
                    call('/api/guide/checks', {'key': 'done', 'checked': True})
                    self.assertEqual(call('/api/guide/runtime')['steps'][0]['status'], 'completed')
                    self.assertEqual(call('/api/guide/progress?id=two')['checks'], {})
                    second = call('/api/guide/progress', {})
                    self.assertNotEqual(second['run_id'], first_id)
                    self.assertEqual(second['checks'], {})
                    self.assertEqual(call('/api/guide/progress?run=' + first_id)['checks'], {'done': True})
                    self.assertEqual(call('/api/guide/runtime?run=' + first_id)['steps'][0]['status'], 'completed')
                    self.assertEqual(call('/api/guide/progress')['run_id'], second['run_id'])
                    for body in [{'run_id': first_id, 'checks': {'done': True}}, {'checks': {'undeclared': True}}]:
                        with self.assertRaises(HTTPError) as error:
                            call('/api/guide/progress', body)
                        self.assertEqual(error.exception.code, 400)
                    self.assertEqual(call('/api/guide/progress', {'run_id': first_id})['checks'], {'done': True})
                    for path in ['/api/guide/progress', '/api/guide/checks']:
                        with self.assertRaises(HTTPError) as error:
                            call(path, {}, {'Content-Type': 'application/json', 'Origin': 'https://example.invalid'})
                        self.assertEqual(error.exception.code, 403)
                    with urlopen(base + '/guide-runtime.js') as response:
                        self.assertIn(b'HoiGuideRuntime', response.read())
                finally:
                    server.shutdown(); thread.join(timeout=2)

    def test_session_rotation_preserves_history_and_ignores_stale_cache(self):
        store = Store.__new__(Store)
        store.lock = RLock(); store.default_guide_id = 'g'
        store.guide_checks = {}; store.guide_states = {}; store.guide_revision = 0
        guide = normalise_guide({'id': 'g', 'tracks': {'focus': [{'key': 'check', 'kind': 'gate'}]}})
        with patch('hoi4companion.api.load_guide', return_value=guide):
            store.live_cache = {'usable': True, 'sample': {'session_id': 'one'}}
            first = store.walkthrough_progress()
            store.walkthrough_checks({'key': 'check', 'checked': True})
            store.live_cache = {'usable': False, 'sample': {'session_id': 'two'}}
            self.assertEqual(store.walkthrough_progress()['run_id'], first['run_id'])
            store.live_cache = {'usable': True, 'sample': {'session_id': 'two'}}
            second = store.walkthrough_progress()
            self.assertNotEqual(first['run_id'], second['run_id'])
            self.assertEqual(store.guide_checks, {})
            self.assertEqual(store.walkthrough_progress(run_id=first['run_id'])['checks'], {'check': True})


if __name__ == '__main__':
    unittest.main()
