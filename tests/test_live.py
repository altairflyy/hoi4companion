import unittest
from hoi4companion.live import NativeSample, LiveReadError, LiveMonitor, read_native_state


class Memory:
    def __init__(self):
        self.data = bytearray(4096)
    def read(self, address, size):
        if address < 0x10000 or address + size > 0x11000:
            raise LiveReadError('unreadable')
        return bytes(self.data[address - 0x10000:address - 0x10000 + size])


class LiveTest(unittest.TestCase):
    def test_no_loaded_world_is_rejected(self):
        mem = Memory()
        mem.base = 0x10000 - 0x332F260
        with self.assertRaises(LiveReadError):
            read_native_state(mem)

    def test_changed_witness_rejected(self):
        mem = Memory()
        sample = NativeSample(mem)
        sample.read(0x10000, 4)
        mem.data[0] = 1
        with self.assertRaises(LiveReadError):
            sample.verify()

    def test_empty_and_malformed_array(self):
        import struct
        mem = Memory()
        sample = NativeSample(mem)
        self.assertEqual(sample.array(0x10000, 8, 20), [])
        struct.pack_into('<Qii', mem.data, 0, 0x10100, 2, 3)
        with self.assertRaises(LiveReadError):
            sample.array(0x10000, 8, 20)

    def test_read_budget_and_invalid_string(self):
        sample = NativeSample(Memory(), budget=16)
        with self.assertRaises(LiveReadError):
            sample.read(0x10000, 32)
        with self.assertRaises(LiveReadError):
            NativeSample(Memory()).string(0x10000)

    def test_missing_process_is_not_a_completed_empty_sample(self):
        monitor = LiveMonitor(lambda: (_ for _ in ()).throw(LiveReadError('Gioco non avviato')))
        value = monitor.poll()
        self.assertEqual(value['status'], 'disconnected')
        self.assertIsNone(value['sample'])

    def test_disconnect_preserves_last_sample_as_stale(self):
        values = iter([{'session_id': 'one', 'country': 'ROM', 'completed_focuses': ['a']}, LiveReadError('stopped')])
        def collect():
            value = next(values)
            if isinstance(value, Exception):
                raise value
            return value
        monitor = LiveMonitor(collect)
        self.assertEqual(monitor.poll()['status'], 'observed')
        state = monitor.poll()
        self.assertEqual(state['status'], 'stale')
        self.assertEqual(state['sample']['completed_focuses'], ['a'])
        self.assertFalse(state['usable'])

    def test_failed_new_sample_never_merges_with_previous_session(self):
        values = iter([{'session_id': 'one', 'country': 'ROM', 'completed_focuses': ['a']},
                       {'session_id': 'two', 'country': 'AUS', 'completed_focuses': []}])
        monitor = LiveMonitor(lambda: next(values))
        monitor.poll()
        state = monitor.poll()
        self.assertEqual(state['sample']['completed_focuses'], [])
        self.assertEqual(state['sample']['session_id'], 'two')

    def test_store_matches_native_focus_membership_without_using_snapshot(self):
        from hoi4companion.api import Store
        from unittest.mock import Mock, patch
        from threading import RLock
        store = Store.__new__(Store)
        store.lock = RLock()
        store.settings = {'game_dir': 'fixture'}
        store.trees = {'rom': Mock(focuses={'a': object(), 'b': object()})}
        monitor = Mock()
        monitor.poll.return_value = {'usable': True, 'sample': {'focus_ids': ['a', 'b'], 'country': 'ROM'}}
        store.live_monitor = monitor
        store.live_provider_path = 'fixture'
        store.live_cache = None
        store.live_cache_time = 0
        with patch('hoi4companion.api.read_snapshot', side_effect=AssertionError('No save reads')):
            value = store.live_state()
        self.assertEqual(value['sample']['focus_tree_id'], 'rom')

    def test_live_tree_request_bypasses_even_a_configured_save(self):
        from hoi4companion.api import Store
        from unittest.mock import Mock, patch
        from threading import RLock
        store = Store.__new__(Store)
        store.lock = RLock()
        store.settings = {'tree_id': 'rom', 'snapshot_path': 'do-not-read.hoi4'}
        tree = Mock()
        tree.to_json.return_value = {'id': 'rom', 'focuses': []}
        store.trees, store.sprites, store.demo = {'rom': tree}, None, False
        with patch('hoi4companion.api.read_snapshot', side_effect=AssertionError('No save reads')):
            self.assertFalse(store.tree('rom', include_snapshot=False)['has_snapshot'])

    def test_http_live_route_and_save_bypass_are_wired(self):
        from hoi4companion.api import make_handler
        from unittest.mock import Mock
        from threading import Thread
        from http.server import ThreadingHTTPServer
        from urllib.request import urlopen
        import json
        store = Mock()
        store.live_state.return_value = {'usable': False, 'status': 'disconnected'}
        store.tree.return_value = {'id': 'rom'}
        with ThreadingHTTPServer(('127.0.0.1', 0), make_handler(store)) as server:
            worker = Thread(target=server.serve_forever, daemon=True)
            worker.start()
            try:
                base = f'http://127.0.0.1:{server.server_port}'
                with urlopen(base + '/api/state') as response:
                    self.assertEqual(json.load(response)['status'], 'disconnected')
                with urlopen(base + '/api/tree?id=rom&source=live') as response:
                    self.assertEqual(json.load(response)['id'], 'rom')
                store.tree.assert_called_once_with('rom', include_snapshot=False)
            finally:
                server.shutdown()
                worker.join()
