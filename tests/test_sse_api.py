import json
import threading
import unittest
from http.server import ThreadingHTTPServer
from types import SimpleNamespace
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from hoi4companion.api import make_handler
from hoi4companion.stream import LiveEventHub


def frame(response):
    result = {}
    while True:
        line = response.readline().decode().strip()
        if not line:
            if 'event' in result:
                return result
            continue
        key, _, value = line.partition(':')
        result[key] = value.lstrip()


class SseApiTest(unittest.TestCase):
    def setUp(self):
        self.state = {'usable': True, 'status': 'observed', 'sample': {'country': 'ROM', 'current_focus': 'a'}}
        self.hub = LiveEventHub(lambda: self.state, interval=.01)
        self.server = ThreadingHTTPServer(('127.0.0.1', 0), make_handler(SimpleNamespace(live_hub=self.hub)))
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.url = f'http://127.0.0.1:{self.server.server_port}/api/state/events'

    def tearDown(self):
        self.hub.close()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=1)

    def test_sse_initial_state_change_disconnect_and_recovery(self):
        with urlopen(self.url, timeout=2) as response:
            self.assertIn('text/event-stream', response.headers['Content-Type'])
            first = frame(response)
            self.assertEqual(first['event'], 'state')
            self.assertEqual(json.loads(first['data'])['sample']['current_focus'], 'a')
            self.state = {**self.state, 'sample': {'country': 'ROM', 'current_focus': 'b'}}
            second = frame(response)
            self.assertGreater(int(second['id']), int(first['id']))
            self.assertEqual(json.loads(second['data'])['sample']['current_focus'], 'b')
            self.state = {'usable': False, 'status': 'stale', 'sample': self.state['sample']}
            self.assertFalse(json.loads(frame(response)['data'])['usable'])
            self.state = {**self.state, 'usable': True, 'status': 'observed'}
            self.assertTrue(json.loads(frame(response)['data'])['usable'])

    def test_stream_rejects_cross_origin_before_subscribing(self):
        request = Request(self.url, headers={'Origin': 'https://example.invalid'})
        with self.assertRaises(HTTPError) as error:
            urlopen(request, timeout=2)
        self.assertEqual(error.exception.code, 403)
        self.assertFalse(self.hub._clients)

    def test_subscriber_limit_produces_503(self):
        clients = [self.hub.subscribe() for _ in range(8)]
        try:
            with self.assertRaises(HTTPError) as error:
                urlopen(self.url, timeout=2)
            self.assertEqual(error.exception.code, 503)
        finally:
            for client in clients:
                client.close()
