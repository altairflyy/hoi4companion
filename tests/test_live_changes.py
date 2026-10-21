"""Cached definitions must never hide changes to mutable game-state fields."""
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from test_native_fixture import Fixture
from hoi4companion.live import NativeProvider, LiveReadError, compile_watch_reads, watch_changed


class LiveChangesTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.executable = Path(self.tmp.name) / 'hoi4.exe'
        self.executable.write_bytes(b'fixture')
        self.mem = Fixture()
        self.mem.close = lambda: None
        self.now = 100.
        self.provider = NativeProvider(self.executable, clock=lambda: self.now,
                                       profile_factory=lambda p: self.mem.profile,
                                       process_factory=lambda p: self.mem)

    def tearDown(self):
        self.provider.close()
        self.tmp.cleanup()

    def test_unchanged_or_tick_only_uses_probe_without_redecoding(self):
        self.provider.collect()
        self.mem.put(self.mem.world + 0x468, '<i', 60806041)
        result = self.provider.collect()
        self.assertEqual(result['native_tick'], 60806041)
        self.assertEqual(self.provider.metrics['full_reads'], 1)
        self.assertEqual(self.provider.metrics['probes'], 1)

    def test_focus_change_while_paused_triggers_full_validated_read(self):
        first = self.provider.collect()
        self.mem.put(self.mem.focus_state + 0x10, '<Q', 0)
        self.mem.put(self.mem.focus_state + 0x18, '<Q', self.mem.continuous)
        result = self.provider.collect()
        self.assertEqual(result['continuous_focus'], 'continuous_test')
        self.assertEqual(result['native_tick'], first['native_tick'])
        self.assertEqual(self.provider.metrics['full_reads'], 2)

    def test_research_slot_change_and_completion_are_not_hidden(self):
        self.provider.collect()
        self.mem.put(self.mem.slot + 0x18, '<Q', 0)
        result = self.provider.collect()
        self.assertIsNone(result['research_slots'][0]['technology_id'])
        self.mem.put(self.mem.model + 0x174, '<i', 1)
        self.mem.array(self.mem.tech_state + 0x40, [self.mem.model])
        result = self.provider.collect()
        self.assertEqual(result['researched_technologies'], ['construction4'])
        self.assertEqual(self.provider.metrics['changes'], 2)

    def test_partial_repeatable_level_changes_without_completed_list_change(self):
        self.mem.put(self.mem.definition + 0x3DC, '<i', 5)
        self.mem.put(self.mem.slot + 0x18, '<Q', 0)
        self.provider.collect()
        self.mem.put(self.mem.model + 0x174, '<i', 1)
        self.assertEqual(self.provider.collect()['researched_technologies'], ['construction4'])

    def test_foreign_slot_owner_invalidates_cache_and_handle(self):
        self.provider.collect()
        self.mem.put(self.mem.slot + 0x10, '<Q', self.mem.world)
        with self.assertRaises(LiveReadError):
            self.provider.collect()
        self.assertIsNone(self.provider.cached)
        self.assertIsNone(self.provider.process)

    def test_periodic_full_audit_catches_changed_definition(self):
        self.provider.collect()
        self.mem.string(self.mem.definition + 0x10, 'changed_id')
        # Definition strings aren't assumed to be mutation notifications.
        self.assertEqual(self.provider.collect()['technology_ids'], ['construction4'])
        self.now += 5
        self.assertEqual(self.provider.collect()['technology_ids'], ['changed_id'])
        self.assertEqual(self.provider.metrics['full_reads'], 2)

    def test_focus_completed_list_payload_change_is_detected(self):
        self.mem.put(self.mem.focus_state + 0x10, '<Q', 0)
        self.provider.collect()
        self.mem.array(self.mem.focus_state + 0x40, [self.mem.focus])
        self.assertEqual(self.provider.collect()['completed_focuses'], ['ROM_test'])

    def test_probe_torn_after_initial_read_rejects_cached_state(self):
        self.provider.collect()
        original = self.mem.read
        calls = 0
        def read(address, size):
            nonlocal calls
            target = self.mem.world + 0x520
            if address <= target < address + size:
                calls += 1
                if calls > 1:
                    value = bytearray(original(address, size))
                    value[target - address:target - address + 4] = b'\2\0\0\0'
                    return bytes(value)
            return original(address, size)
        self.mem.read = read
        with self.assertRaises(LiveReadError):
            self.provider.collect()
        self.assertIsNone(self.provider.cached)

    def test_close_discards_pointer_cache_and_rebuilds_next_collection(self):
        self.provider.collect()
        self.provider.close()
        self.assertFalse(self.provider.watch_plan)
        self.provider.collect()
        self.assertEqual(self.provider.metrics['full_reads'], 2)

    def test_batched_watch_ignores_mutations_in_unselected_gap(self):
        reads = compile_watch_reads({(0x200000000, 1): b'a', (0x200000010, 1): b'b'})
        mem = SimpleNamespace(read=lambda address, size: b'a' + b'changed padding' + b'b')
        self.assertFalse(watch_changed(mem, reads))
        mem.read = lambda address, size: b'x' + b'changed padding' + b'b'
        self.assertTrue(watch_changed(mem, reads))

    def test_batched_watch_never_crosses_a_page_boundary(self):
        reads = compile_watch_reads({(0x200000FFE, 4): b'abcd'})
        self.assertEqual([(address, size) for address, size, _ in reads],
                         [(0x200000FFE, 2), (0x200001000, 2)])

    def test_probe_batching_cannot_expand_gaps_beyond_read_budget(self):
        ranges = {}
        for index in range(513):
            address = 0x200000000 + index * 4096
            ranges[(address, 1)] = b'a'
            ranges[(address + 4095, 1)] = b'b'
        with self.assertRaises(LiveReadError):
            compile_watch_reads(ranges)
