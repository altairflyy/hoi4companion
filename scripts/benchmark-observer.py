"""Paired observer benchmark on owned Windows buffers; not game/HUD latency."""
import argparse
import ctypes as c
from ctypes import wintypes as w
import json
import statistics
import time
from pathlib import Path

from hoi4companion.focus_ui_native import GuiSample
from hoi4companion.live import WindowsGame, compile_watch_reads, watch_changed
from hoi4companion.native_observer import Observer


def run(iterations=30, batches=11, count=1024):
    kernel = c.WinDLL('kernel32', use_last_error=True)
    kernel.GetCurrentProcess.restype = w.HANDLE
    kernel.ReadProcessMemory.argtypes = [w.HANDLE, c.c_void_p, c.c_void_p, c.c_size_t, c.POINTER(c.c_size_t)]
    process = WindowsGame.__new__(WindowsGame)
    process.c, process.k, process.handle = c, kernel, kernel.GetCurrentProcess()
    process._observer = None
    buffers = [c.create_string_buffer(bytes([index % 256]) * 64) for index in range(count)]
    addresses = [c.addressof(buffer) for buffer in buffers]
    def acquire():
        sample = GuiSample(process)
        for address in addresses:
            sample.read(address, 64)
        for address in addresses[::2]:
            sample.shown(address + 63)
        return sample
    def full_sample():
        acquire().verify()
    sample = acquire()
    reads = compile_watch_reads(sample.witnesses)
    observer = Observer()
    process._observer = observer
    packet = process.prepare_watch(reads)
    def probe():
        if process._observer is None:
            return watch_changed(process, reads)
        return process.probe_watch(packet)
    runs = {'verify': sample.verify, 'watch': probe, 'acquire_and_verify': full_sample}
    samples = {name: {'python': [], 'rust': []} for name in runs}
    try:
        for _ in range(5):
            for mode in (None, observer):
                process._observer = mode
                for action in runs.values():
                    action()
        for batch in range(batches):
            for name, action in runs.items():
                for mode in (('python', 'rust') if batch % 2 == 0 else ('rust', 'python')):
                    process._observer = observer if mode == 'rust' else None
                    started = time.perf_counter()
                    for _ in range(iterations):
                        action()
                    samples[name][mode].append((time.perf_counter() - started) * 1000 / iterations)
        medians = {name: {mode: statistics.median(times) for mode, times in modes.items()}
                   for name, modes in samples.items()}
        for row in medians.values():
            row['reduction_percent'] = (1 - row['rust'] / row['python']) * 100
        return {'fixture_only': True, 'game_live_verified': False, 'visible_hud_latency_verified': False,
                'witnesses': len(sample.witnesses), 'visibility': len(sample.visibility),
                'watch_ranges': len(reads), 'iterations': iterations, 'batches': batches,
                'scope': 'owned process buffers; includes Python packing and FFI; no cached sample verification',
                'watch_preparation': 'once per qualified sample; unchanged two-pass comparison semantics',
                'median_ms': medians, 'samples_ms': samples}
    finally:
        process.handle = None  # Pseudo-handle; never close it.


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--iterations', type=int, default=30)
    args = parser.parse_args()
    if args.iterations < 1:
        parser.error('iterations must be positive')
    result = run(args.iterations)
    if args.output:
        args.output.write_text(json.dumps(result, indent=2), encoding='utf8')
    print(json.dumps(result['median_ms'], indent=2))
