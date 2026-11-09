"""Explicit injected-memory transport used by structural tests/custom fixtures.
The real Windows provider is entirely native; this is never an error fallback.
"""
from pathlib import Path
import struct
import time
from .live import WindowsGame, ExecutableProfile, LiveReadError, read_native_state, compile_watch_reads, watch_changed

class CallbackProvider:
    """One read-only handle; cheap change checks, bounded full integrity audits.

    This is still memory polling, not an engine event subscription. A changed
    byte invalidates the cached sample immediately and triggers full validation.
    Unchanged probes don't decode strings/definitions or rescan processes.
    """
    def __init__(self, executable, *, audit_seconds=5, clock=time.monotonic,
                 process_factory=WindowsGame, profile_factory=ExecutableProfile, province_neighbors=None):
        self.executable, self.profile = Path(executable), None
        self.stat_key = None
        self.process, self.watch_plan, self.watch_reads, self.cached = None, {}, [], None
        self.watch_packet = None
        self.last_audit = None
        self.audit_seconds, self.clock = audit_seconds, clock
        self.process_factory, self.profile_factory = process_factory, profile_factory
        self.province_neighbors = province_neighbors
        self.metrics = {'probes': 0, 'full_reads': 0, 'changes': 0,
                        'last_read_ms': None, 'watch_ranges': 0, 'watch_bytes': 0,
                        'probe_read_calls': 0, 'probe_read_bytes': 0}

    def collect(self):
        started = self.clock()
        try:
            stat = self.executable.stat()
            key = stat.st_size, stat.st_mtime_ns
            if self.profile is None or key != self.stat_key:
                self.close()
                self.profile = self.profile_factory(self.executable)
                if self.province_neighbors is not None:self.profile.province_neighbors=self.province_neighbors
                self.stat_key = key
            if self.process is None:
                self.process = self.process_factory(self.profile)
            process = self.process
            if process.identity() != process.created:
                raise LiveReadError('Sessione di processo cambiata')
            changed = False
            full = self.cached is None or self.last_audit is None or started - self.last_audit >= self.audit_seconds
            if not full:
                self.metrics['probes'] += 1
                changed = (process.probe_watch(self.watch_packet) if self.watch_packet is not None
                           else watch_changed(process, self.watch_reads))
                full = changed
            if full:
                plan = {}
                value = read_native_state(process, watch_plan=plan)
                self.cached, self.watch_plan = value, plan
                self.watch_reads = compile_watch_reads(plan)
                prepare = getattr(process, 'prepare_watch', None)
                self.watch_packet = prepare(self.watch_reads) if callable(prepare) else None
                self.last_audit = self.clock()
                self.metrics['full_reads'] += 1
                self.metrics['changes'] += int(changed)
                self.metrics['watch_ranges'] = len(plan)
                self.metrics['watch_bytes'] = sum(size for _, size in plan)
                self.metrics['probe_read_calls'] = 2 * len(self.watch_reads)
                self.metrics['probe_read_bytes'] = 2 * sum(size for _, size, _ in self.watch_reads)
            # Refresh diagnostic tick even if focus/research did not change.
            # It is excluded from the SSE semantic-change signature.
            world = int(self.cached['session_id'].split(':')[2], 16)
            tick = struct.unpack('<i', process.read(world + 0x468, 4))[0]
            if process.identity() != process.created:
                raise LiveReadError('Sessione di processo cambiata')
            self.metrics['last_read_ms'] = round((self.clock() - started) * 1000, 3)
            return {**self.cached, 'native_tick': tick}
        except Exception:
            self.close()
            raise

    def close(self):
        if self.process is not None:
            self.process.close()
        self.process, self.watch_plan, self.watch_reads, self.cached = None, {}, [], None
        self.watch_packet = None
        self.last_audit = None


