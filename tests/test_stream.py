import threading
import time
import unittest

from hoi4companion.stream import LiveEventHub, SubscriberLimitError, state_signature


class StreamTests(unittest.TestCase):
    def hub(self, source, **kwargs):
        hub = LiveEventHub(source, interval=.01, **kwargs)
        self.addCleanup(hub.close)
        return hub

    def test_ignores_timing_but_not_gameplay_or_validity(self):
        first = {'usable': True, 'sample': {'native_tick': 1, 'current_focus': 'a'}, 'metrics': {'bytes': 1}}
        second = {'usable': True, 'sample': {'native_tick': 2, 'current_focus': 'a'}, 'metrics': {'bytes': 2}}
        self.assertEqual(state_signature(first), state_signature(second))
        second['sample']['current_focus'] = 'b'
        self.assertNotEqual(state_signature(first), state_signature(second))
        second = {**first, 'usable': False}
        self.assertNotEqual(state_signature(first), state_signature(second))

    def test_shared_producer_and_unchanged_state_suppressed(self):
        count, threads = [], set()
        def source():
            count.append(1)
            threads.add(threading.get_ident())
            return {'usable': True, 'observed_at': len(count), 'sample': {'native_tick': len(count)}}
        hub = self.hub(source)
        with hub.subscribe() as a, hub.subscribe() as b:
            self.assertEqual(a.wait(1).revision, b.wait(1).revision)
            self.assertIsNone(a.wait(.05))
            self.assertGreater(len(count), 2)
            self.assertEqual(len(threads), 1)
            self.assertTrue(hub.heartbeat()['usable'])

    def test_stale_recovery_and_slow_client_coalescing(self):
        state = {'usable': True, 'status': 'observed', 'sample': {'current_focus': 'a'}}
        hub = self.hub(lambda: dict(state))
        with hub.subscribe() as fast, hub.subscribe() as slow:
            baseline = fast.wait(1)
            state.update(usable=False, status='stale')
            stale = fast.wait(1)
            self.assertFalse(stale.value['usable'])
            state.update(usable=True, status='observed', sample={'current_focus': 'b'})
            recovered = fast.wait(1)
            self.assertGreater(recovered.revision, stale.revision)
            self.assertGreater(recovered.revision, baseline.revision)
            latest = slow.wait(1)
            self.assertEqual(latest.revision, recovered.revision)
            self.assertEqual(latest.value['sample']['current_focus'], 'b')
            latest.value['sample']['current_focus'] = 'client mutation'
            self.assertEqual(hub._event.value['sample']['current_focus'], 'b')

    def test_subscriber_limit_cleanup_and_fresh_reconnect(self):
        calls = []
        hub = self.hub(lambda: calls.append(1) or {'usable': True, 'value': len(calls)}, max_subscribers=1)
        first = hub.subscribe()
        old = first.wait(1)
        with self.assertRaises(SubscriberLimitError):
            hub.subscribe()
        first.close()
        with hub._condition:
            self.assertTrue(hub._condition.wait_for(lambda: hub._worker is None, 1))
        stopped_count = len(calls)
        time.sleep(.03)
        self.assertEqual(stopped_count, len(calls))
        with hub.subscribe() as second:
            fresh = second.wait(1)
            self.assertGreater(fresh.revision, old.revision)
            self.assertGreater(fresh.value['value'], old.value['value'])

    def test_source_error_is_unusable_and_close_wakes_waiter(self):
        hub = self.hub(lambda: (_ for _ in ()).throw(ValueError('failed')))
        sub = hub.subscribe()
        self.assertFalse(sub.wait(1).value['usable'])
        hub.close()
        self.assertIsNone(sub.wait(1))
        with self.assertRaises(RuntimeError):
            hub.subscribe()

    def test_subscribe_during_idle_cleanup_reuses_single_worker(self):
        cleanup_started, release_cleanup = threading.Event(), threading.Event()
        threads = set()
        def source():
            threads.add(threading.get_ident())
            return {'usable': True}
        def idle():
            cleanup_started.set()
            release_cleanup.wait(1)
        hub = self.hub(source, on_idle=idle)
        first = hub.subscribe()
        first.wait(1)
        first.close()
        self.assertTrue(cleanup_started.wait(1))
        with hub.subscribe() as second:
            self.assertIsNone(second.wait(.02))
            release_cleanup.set()
            self.assertTrue(second.wait(1).value['usable'])
            self.assertEqual(len(threads), 1)

    def test_hung_reader_never_replays_usable_state_to_new_subscriber(self):
        reading, release = threading.Event(), threading.Event()
        count = []
        def source():
            count.append(1)
            if len(count) > 1:
                reading.set()
                release.wait(1)
            return {'usable': True, 'status': 'observed', 'sample': {'focus': 'a'}}
        hub = self.hub(source)
        self.addCleanup(release.set)
        with hub.subscribe() as first:
            first.wait(1)
            self.assertTrue(reading.wait(1))
            with hub._condition:
                hub._last_check -= 6
            with hub.subscribe() as second:
                stale = second.wait(0)
                self.assertFalse(stale.value['usable'])
                self.assertEqual(stale.value['status'], 'stale')
                self.assertFalse(first.wait(0).value['usable'])
                release.set()
                recovered = first.wait(1)
                self.assertTrue(recovered.value['usable'])
                self.assertGreater(recovered.revision, stale.revision)

    def test_heartbeat_expiration_and_identical_source_recovery(self):
        reading, release = threading.Event(), threading.Event()
        count = []
        def source():
            count.append(1)
            if len(count) > 1:
                reading.set()
                release.wait(1)
            return {'usable': True, 'status': 'observed', 'sample': {'focus': 'unchanged'}}
        hub = self.hub(source)
        self.addCleanup(release.set)
        with hub.subscribe() as client:
            original = client.wait(1)
            self.assertTrue(reading.wait(1))
            with hub._condition:
                hub._last_check -= 6
            beat = hub.heartbeat()
            self.assertFalse(beat['usable'])
            expired = client.wait(0)
            self.assertFalse(expired.value['usable'])
            release.set()
            recovered = client.wait(1)
            self.assertTrue(recovered.value['usable'])
            self.assertEqual(recovered.value['sample'], original.value['sample'])

    def test_no_baseline_does_not_emit_artificial_stale_state(self):
        reading, release = threading.Event(), threading.Event()
        def source():
            reading.set()
            release.wait(1)
            return {'usable': True}
        hub = self.hub(source)
        self.addCleanup(release.set)
        with hub.subscribe() as client:
            self.assertTrue(reading.wait(1))
            self.assertFalse(hub.heartbeat()['usable'])
            self.assertIsNone(client.wait(0))
            release.set()
            self.assertTrue(client.wait(1).value['usable'])


if __name__ == '__main__':
    unittest.main()
