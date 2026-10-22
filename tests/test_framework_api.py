"""Real HTTP boundary with fake catalogs; never reads native memory."""
import json
import threading
import unittest
from http.server import ThreadingHTTPServer
from threading import RLock
from urllib.error import HTTPError
from urllib.request import Request, urlopen
from unittest.mock import patch
from pathlib import Path
from tempfile import TemporaryDirectory

from hoi4companion.api import Store, make_handler
from hoi4companion.guide import normalise_guide, load_guide


class FrameworkApiTests(unittest.TestCase):
    def test_schema_metadata_is_preserved_and_invalid_file_returns_json_error(self):
        cases = json.loads((Path(__file__).parent / 'fixtures' / 'guide-schema.json').read_text(encoding='utf-8'))
        valid = next(c['guide'] for c in cases if c['name'] == 'extensions')
        invalid = next(c['guide'] for c in cases if c['name'] == 'huge time')
        store = Store.__new__(Store)
        store.lock = RLock(); store.trees = {}; store.labels = {}; store.demo = False
        store.settings = {'country': 'AUS'}; store.technologies = {'technologies': {}, 'folders': {}}
        with TemporaryDirectory() as folder:
            file = Path(folder) / 'guide.json'
            file.write_text(json.dumps(valid), encoding='utf-8')
            with patch('hoi4companion.api.load_guide', side_effect=lambda key=None: load_guide(key, directory=Path(folder))):
                with ThreadingHTTPServer(('127.0.0.1', 0), make_handler(store)) as server:
                    thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
                    url = f'http://127.0.0.1:{server.server_port}/api/guide?id=universal'
                    try:
                        with urlopen(url, timeout=2) as response:
                            data = json.load(response)
                        self.assertEqual(data['sources'], valid['sources'])
                        self.assertEqual(data['phases'], valid['phases'])
                        self.assertEqual(data['tracks']['research'][0]['completion'], valid['tracks']['research'][0]['completion'])
                        self.assertEqual(data['tracks']['focus'][0]['evidence'], valid['tracks']['focus'][0]['evidence'])
                        file.write_text(json.dumps(invalid), encoding='utf-8')
                        with self.assertRaises(HTTPError) as error:
                            urlopen(url, timeout=2)
                        self.assertEqual(error.exception.code, 400)
                        self.assertIn('error', json.load(error.exception))
                    finally:
                        server.shutdown(); thread.join(timeout=2)

    def test_versioned_routes_selected_guides_and_isolated_manual_checks(self):
        guides = {key: normalise_guide({'id': key, 'country': country, 'tracks': {
            'focus': [{'key': 'shared', 'kind': 'gate'}], 'research': []}})
            for key, country in [('one', 'AUS'), ('two', 'GER')]}
        def select(key=None):
            if (key or 'one') not in guides:
                raise ValueError('Guida non trovata')
            return guides[key or 'one']
        store = Store.__new__(Store)
        store.lock = RLock(); store.guide_checks = {}; store.guide_states = {}; store.guide_revision = 0
        store.default_guide_id = 'one'; store.trees = {}; store.labels = {}
        store.settings = {'country': 'AUS'}; store.demo = False
        store.technologies = {'technologies': {}, 'folders': {}}
        store.live_state = lambda: {'usable': True, 'source': 'process_memory', 'sample': {'country': 'GER'}}
        with patch('hoi4companion.api.load_guide', side_effect=select), patch('hoi4companion.api.list_guides', return_value=[{'id': key} for key in guides]):
            with ThreadingHTTPServer(('127.0.0.1', 0), make_handler(store)) as server:
                thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
                base = f'http://127.0.0.1:{server.server_port}'
                def get(path):
                    with urlopen(base + path, timeout=2) as response:
                        return json.load(response)
                try:
                    self.assertEqual(get('/api/guides')['schema_version'], 1)
                    self.assertEqual(get('/api/guide')['id'], 'one')
                    self.assertEqual(get('/api/guide?id=two')['applies_to']['country'], 'GER')
                    request = Request(base + '/api/guide/checks?id=two', data=b'{"key":"shared","checked":true}', headers={'Content-Type': 'application/json'})
                    with urlopen(request, timeout=2) as response:
                        self.assertEqual(json.load(response)['checks'], {'shared': True})
                    self.assertEqual(get('/api/guide/checks')['checks'], {})
                    self.assertEqual(get('/api/guide/checks?id=two')['checks'], {'shared': True})
                    state = get('/api/framework/state')
                    self.assertEqual(state['facts']['country']['status'], 'observed')
                    self.assertEqual(state['facts']['focus.completed']['status'], 'unknown')
                    with self.assertRaises(HTTPError) as error:
                        get('/api/guide?id=..%2Ftwo')
                    self.assertEqual(error.exception.code, 400)
                    with urlopen(base + '/walkthrough', timeout=2) as response:
                        page = response.read()
                        self.assertIn(b'/guide-session.js', page)
                        self.assertIn(b'operationCards', page)
                        self.assertNotIn(b'id="focusSteps"', page)
                        self.assertNotIn(b'id="researchSteps"', page)
                    for path in ['/contracts.js', '/guide-presentation.js', '/guide-session.js', '/guide-assist.js']:
                        with urlopen(base + path, timeout=2) as response:
                            self.assertIn('javascript', response.headers['Content-Type'])
                    with urlopen(base + '/guide-assist.css', timeout=2) as response:
                        self.assertIn(b'.guide-next', response.read())
                finally:
                    server.shutdown(); thread.join(timeout=2)
