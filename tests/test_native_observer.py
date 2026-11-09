"""Native revalidation reads only memory allocated by this test process."""
import ctypes as c
from ctypes import wintypes as w
import os
import random
import struct
import subprocess
import sys
import unittest
from unittest.mock import patch, Mock

from hoi4companion.native_observer import Observer, encode_sample, encode_watch, optional_observer
from hoi4companion.live import NativeSample, WindowsGame, LiveReadError, watch_changed
from hoi4companion.focus_ui_native import GuiSample


@unittest.skipUnless(os.name == 'nt', 'Windows read-only observer')
class NativeObserverTests(unittest.TestCase):
    def adapter_process(self):
        # Exercise the public sample/watch adapters over owned memory. Process
        # discovery/qualification now lives in Rust and has its own owned-process
        # integration test; a __new__ object is not a qualified native handle.
        process=WindowsGame.__new__(WindowsGame)
        process.read=lambda address,size:c.string_at(address,size)
        def compare(packet):
            try:return self.engine.changed(self.handle,packet,passes=2)
            except (ValueError,RuntimeError) as exc:raise LiveReadError(str(exc)) from exc
        process._compare=compare
        return process

    def setUp(self):
        self.engine = Observer()
        self.kernel = c.WinDLL('kernel32', use_last_error=True)
        self.kernel.GetCurrentProcess.restype = w.HANDLE
        self.handle = self.kernel.GetCurrentProcess()
        self.buffer = c.create_string_buffer(b'abcdefgh' + bytes([8, 0]))
        self.address = c.addressof(self.buffer)

    def test_exact_and_masked_witnesses(self):
        packet = encode_sample({(self.address, 8): b'abcdefgh'}, {self.address + 8: 8})
        self.assertFalse(self.engine.changed(self.handle, packet))
        self.buffer[8] = 24  # Unrelated flags must not invalidate visibility.
        self.assertFalse(self.engine.changed(self.handle, packet))
        self.buffer[8] = 16
        self.assertTrue(self.engine.changed(self.handle, packet))

    def test_full_byte_witness_is_not_weakened_by_mask(self):
        packet = encode_sample({(self.address + 8, 1): bytes([8])}, {self.address + 8: 8})
        self.buffer[8] = 24
        self.assertTrue(self.engine.changed(self.handle, packet))

    def test_watch_compares_only_parts_not_gaps(self):
        packet = encode_watch([(self.address, 8, [(0, b'ab'), (6, b'gh')])])
        self.buffer[3] = b'X'
        self.assertFalse(self.engine.changed(self.handle, packet, passes=2))
        self.buffer[7] = b'X'
        self.assertTrue(self.engine.changed(self.handle, packet, passes=2))

    def test_empty_observation_and_unreadable_are_distinct(self):
        self.assertFalse(self.engine.changed(self.handle, b''))
        with self.assertRaisesRegex(RuntimeError, 'read'):
            self.engine.changed(self.handle, encode_sample({(0x10000, 8): b'abcdefgh'}))
        with self.assertRaises(RuntimeError):
            self.engine.changed(0, encode_sample({(self.address, 8): b'abcdefgh'}))

    def test_rejects_malformed_and_out_of_bounds_packets(self):
        good = encode_sample({(self.address, 8): b'abcdefgh'})
        for packet in (good[:-1], good + b'X', struct.pack('<QII', self.address, 65537, 0),
                       struct.pack('<QIIIII', self.address, 8, 1, 8, 1, 255) + b'a'):
            with self.subTest(packet=packet), self.assertRaises(RuntimeError):
                self.engine.changed(self.handle, packet)
        with self.assertRaises(ValueError):
            self.engine.changed(self.handle, good, passes=3)

    def test_native_sample_integration_preserves_consistency_errors(self):
        process = self.adapter_process()
        sample = GuiSample(process)
        try:
            sample.read(self.address, 8)
            sample.shown(self.address + 8)
            sample.verify()
            self.buffer[8] = 24
            sample.verify()
            self.buffer[0] = b'X'
            with self.assertRaises(LiveReadError):
                sample.verify()
            with self.assertRaises(LiveReadError):
                process.verify_sample({(0x10000, 8): b'abcdefgh'})
        finally:
            process.handle = None  # Do not close the process pseudo-handle.

    def test_native_watch_plan_is_immutable_and_replaced_with_new_evidence(self):
        process = self.adapter_process()
        reads = [(self.address, 8, [(0, b'ab')])]
        original = process.prepare_watch(reads)
        self.assertFalse(process.probe_watch(original))
        reads[0] = (self.address, 8, [(0, b'XX')])
        self.assertFalse(process.probe_watch(original))
        self.assertTrue(process.probe_watch(process.prepare_watch(reads)))

    def test_native_and_python_watch_agree_on_generated_ranges(self):
        rng = random.Random(20261103)
        for case in range(100):
            ranges = []
            for index in range(4):
                offset = rng.randrange(0, 6)
                size = rng.randrange(1, 9 - offset)
                expected = bytes(self.buffer[offset:offset + size])
                if case % 2 and index == 2:
                    expected = bytes([expected[0] ^ 1]) + expected[1:]
                ranges.append((self.address, 8, [(offset, expected)]))
            class Process:
                def read(_, address, size):
                    return c.string_at(address, size)
            self.assertEqual(self.engine.changed(self.handle, encode_watch(ranges), passes=2),
                             watch_changed(Process(), ranges))

    def test_external_process_with_query_read_handle(self):
        child = subprocess.Popen([sys.executable, '-u', '-c',
            "import ctypes as c; b=c.create_string_buffer(b'external'); "
            "print(c.addressof(b),flush=True); input(); b[0]=b'X'; "
            "print('changed',flush=True); input()"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, creationflags=subprocess.CREATE_NO_WINDOW)
        self.kernel.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
        self.kernel.OpenProcess.restype = w.HANDLE
        self.kernel.CloseHandle.argtypes = [w.HANDLE]
        handle = None
        try:
            address = int(child.stdout.readline())
            handle = self.kernel.OpenProcess(0x410, False, child.pid)
            self.assertTrue(handle)
            packet = encode_sample({(address, 8): b'external'})
            self.assertFalse(self.engine.changed(handle, packet, passes=2))
            child.stdin.write('change\n'); child.stdin.flush()
            self.assertEqual(child.stdout.readline().strip(), 'changed')
            self.assertTrue(self.engine.changed(handle, packet, passes=2))
            child.communicate('quit\n', timeout=5)
            self.assertEqual(child.returncode, 0)
        finally:
            if handle:
                self.kernel.CloseHandle(handle)
            if child.poll() is None:
                child.kill()
            child.communicate(timeout=5)


class ObserverEncodingTests(unittest.TestCase):
    def test_absent_library_preserves_legacy_backend(self):
        with patch('hoi4companion.native_observer._library_path', return_value=None):
            self.assertIsNone(optional_observer())

    def test_wrong_abi_is_rejected_instead_of_silent_fallback(self):
        library = Mock()
        library.hoi4_observer_abi_version.return_value = 2
        with patch('hoi4companion.native_observer.ctypes.CDLL', return_value=library):
            with self.assertRaisesRegex(ValueError, 'ABI'):
                Observer(__file__)

    def test_rejects_invalid_fixture_shapes(self):
        for witnesses in ({(True, 1): b'a'}, {(0x10000, 2): b'a'}, {(0x10000, 1): 'a'}):
            with self.subTest(witnesses=witnesses), self.assertRaises(ValueError):
                encode_sample(witnesses)
        with self.assertRaises(ValueError):
            encode_watch([(0x10000, 4, [(3, b'aa')])])

    def test_fixture_process_uses_original_reader(self):
        class Process:
            def read(self, address, size):
                return b'a' * size
        sample = NativeSample(Process())
        sample.read(0x10000, 4)
        sample.verify()


if __name__ == '__main__':
    unittest.main()
