import json
from pathlib import Path
from tempfile import TemporaryDirectory
import threading
from threading import RLock
import unittest
from http.server import ThreadingHTTPServer
from urllib.request import urlopen, Request
from urllib.error import HTTPError
from unittest.mock import patch
from hoi4companion.api import Store, make_handler
from hoi4companion.guide_importer import Importer
from test_guide_importer import ENTITIES, definition


class ImporterApiTests(unittest.TestCase):
    def test_acquire_preview_install_existing_api_and_http_guards(self):
        with TemporaryDirectory() as directory, patch('hoi4companion.guide.LOCAL_GUIDES', Path(directory)):
            store = Store.__new__(Store); store.lock = RLock()
            store._importer = Importer(lambda: ENTITIES, Path(directory))
            store.trees = {}; store.labels = {}; store.settings = {'country': 'GER'}; store.demo = True
            store.technologies = {'technologies': {'basic_machine_tools': {'name': 'Tools'}}, 'folders': {}}
            with ThreadingHTTPServer(('127.0.0.1', 0), make_handler(store)) as server:
                thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
                base = f'http://127.0.0.1:{server.server_port}'
                def call(path, data=None, headers=None):
                    req = Request(base + path, data=None if data is None else json.dumps(data).encode(),
                                  headers=headers or {'Content-Type': 'application/json'})
                    with urlopen(req, timeout=3) as response: return json.load(response)
                try:
                    prompt = call('/api/import/prompt')['prompt']
                    self.assertIn('hoi4-guide-import', prompt)
                    src = call('/api/import/source', {'text': 'Research Basic Machine Tools.'})
                    packet = call('/api/import/packet', {'sources': [src]})
                    self.assertEqual(packet['import_schema_version'], 1)
                    guide = definition(src)
                    portable = json.loads(json.dumps({'format':'hoi4-guide-import','import_schema_version':1,'sources':[src],'guide':guide}))
                    portable['sources'][0].pop('content_sha256')
                    portable['sources'][0]['id'] = portable['guide']['sources'][0]['id'] = portable['guide']['tracks']['research'][0]['evidence'][0]['source_id'] = 'source-1'
                    normalized = call('/api/import/document', {'document': portable})
                    self.assertEqual(normalized['sources'][0]['id'], src['id'])
                    self.assertTrue(call('/api/import/preview', {k:normalized[k] for k in ('guide','sources')})['valid'])
                    report = call('/api/import/preview', {'guide': guide, 'sources': [src]})
                    self.assertTrue(report['valid'])
                    result = call('/api/import/install', {'guide': report['guide'], 'sources': [src], 'reviewed': True})
                    self.assertIn('guide=import-test', result['href'])
                    self.assertIn('import-test', {row['id'] for row in call('/api/guides')['guides']})
                    self.assertEqual(call('/api/guide?id=import-test')['tracks']['research'][0]['target'], 'basic_machine_tools')
                    with self.assertRaises(HTTPError) as error:
                        call('/api/import/source', {'text': 'x'}, {'Content-Type': 'application/json', 'Origin': 'https://evil.test'})
                    self.assertEqual(error.exception.code, 403)
                    for data in ([], {'guide': None}, {'sources': [], 'guide': guide, 'reviewed': True}):
                        with self.assertRaises(HTTPError) as error: call('/api/import/install', data)
                        self.assertEqual(error.exception.code, 400)
                    with urlopen(base + '/importer') as response:
                        self.assertIn(b'Guide Importer', response.read())
                    with urlopen(base + '/importer.js') as response: self.assertEqual(response.status, 200)
                finally:
                    server.shutdown(); thread.join(timeout=3)
