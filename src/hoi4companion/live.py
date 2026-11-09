"""Experimental external HOI4 observer. VM_READ + QUERY_INFORMATION only.

No injected module, input automation, process write, console command or save read.
Profiles are pinned to exact executable hashes. Field locations were
cross-checked against those PEs; runtime coverage is documented by feature.
Reference locations and provenance are recorded in docs/live-state.md.
"""
from __future__ import annotations

from datetime import datetime, timezone
import os
from pathlib import Path
import re
import struct
import time

SUPPORTED_HASH = '7dc947be34970da1e1c787bcddbe7f62b610ff258aebf8f06aa8b348f3a031d5'
WORLD_RVA = 0x332F260
UPDATED_HASH = 'b6650c85002374218e60598bd04aaa26cbad996fedce77126fcf92fc6bd6d888'
MILITARY_HASH = 'cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f'
from .live_profiles import profiles,NativeProfile
EXECUTABLE_PROFILES=profiles()
MILITARY_LAYOUT=EXECUTABLE_PROFILES[MILITARY_HASH]['military_layout']
FOREIGN_INTERACTIONS_LAYOUT=EXECUTABLE_PROFILES[MILITARY_HASH]['foreign_interactions_layout']


class LiveReadError(RuntimeError):
    pass


class NativeSample:
    """Every byte used is witnessed again before a sample can escape."""
    def __init__(self, process, budget=4 * 1024 * 1024):
        self.process, self.budget = process, budget
        self.witnesses = {}
        self.used = 0
        self.verification_ms = 0.
        self.verification_engine = 'python'

    def read(self, address, size):
        if not isinstance(address, int) or not 0x10000 <= address < 0x7FFFFFFFFFFF or not 0 <= size <= 65536:
            raise LiveReadError('Indirizzo o dimensione non validi')
        self.used += size
        if self.used > self.budget:
            raise LiveReadError('Limite di lettura raggiunto')
        if size == 0:
            return b''
        data = self.process.read(address, size)
        if len(data) != size:
            raise LiveReadError('Lettura parziale')
        old = self.witnesses.setdefault((address, size), data)
        if old != data:
            raise LiveReadError('Stato cambiato durante la lettura')
        return data

    def number(self, address, fmt='<Q'):
        return struct.unpack(fmt, self.read(address, struct.calcsize(fmt)))[0]

    def array(self, address, item_size=8, maximum=8192):
        data, capacity, count = struct.unpack('<Qii', self.read(address, 16))
        if not 0 <= count <= capacity <= maximum or (count and not data):
            raise LiveReadError('Contenitore nativo non valido')
        if not count:
            return []
        raw = self.read(data, count * item_size)
        return list(struct.unpack('<' + ('Q' if item_size == 8 else 'i') * count, raw))

    def string(self, address):
        header = self.read(address, 32)
        length, capacity = struct.unpack_from('<QQ', header, 16)
        if not 0 < length <= 160 or not length <= capacity <= 65535 or capacity < 15:
            raise LiveReadError('Identificatore nativo non valido')
        pointer = address if capacity == 15 else struct.unpack_from('<Q', header)[0]
        raw = self.read(pointer, length + 1)
        if raw[-1] != 0:
            raise LiveReadError('Identificatore privo di terminatore')
        try:
            value = raw[:-1].decode('ascii')
        except UnicodeError as exc:
            raise LiveReadError('Identificatore non ASCII') from exc
        if not re.fullmatch(r'[A-Za-z0-9_.-]+', value):
            raise LiveReadError('Caratteri inattesi in un identificatore')
        return value

    def verify(self):
        self._verify()

    def _verify(self, visibility=None):
        started = time.perf_counter()
        try:
            self._verify_bytes(visibility)
        finally:
            self.verification_ms = (time.perf_counter() - started) * 1000

    def _verify_bytes(self, visibility=None):
        verifier = getattr(self.process, 'verify_sample', None)
        if callable(verifier) and verifier(self.witnesses, visibility):
            self.verification_engine = 'rust'
            return
        self.verification_engine = 'python'
        for (address, size), data in self.witnesses.items():
            if self.process.read(address, size) != data:
                raise LiveReadError('Campione incoerente: riprovare')
        for address, expected in (visibility or {}).items():
            value = self.process.read(address, 1)
            if len(value) != 1 or value[0] & 8 != expected:
                raise LiveReadError('Campione incoerente: riprovare')


class ExecutableProfile(NativeProfile):
    """Compatibility name for native immutable executable qualification."""


from .game_process import NativeGame


class WindowsGame(NativeGame):
    """Compatibility name for the Rust-qualified, owned read-only handle."""


def read_native_state(process, *, watch_plan=None):
    """Decode qualified core state in Rust and retain all existing witnesses."""
    from .live_core import read_core
    sample=NativeSample(process)
    packet=read_core(sample,watch=watch_plan is not None)
    world,country,native_tag=packet['world'],packet['country_pointer'],packet['native_tag']
    state=packet['state'];tag=state['country']
    watched={(address,size):sample.witnesses[address,size] for address,size in packet['ranges']}
    overview=campaign=None
    if process.profile.sha256 in (SUPPORTED_HASH,UPDATED_HASH,MILITARY_HASH):
        from .campaign import read_campaign
        previous_witnesses=set(sample.witnesses)
        campaign=read_campaign(sample,world)
        if watch_plan is not None:
            watched.update({key:sample.witnesses[key] for key in sample.witnesses.keys()-previous_witnesses})
        from .country import collect_country
        overview=collect_country(process,world,country,native_tag,tag,watch_plan)
    sample.verify()
    if process.identity()!=process.created:raise LiveReadError('Sessione di processo cambiata')
    if watch_plan is not None:watch_plan.update(watched)
    return {'session_id':f'{process.pid}:{process.created}:{world:x}:{country:x}',
        'pid':process.pid,'executable_sha256':process.profile.sha256,
        **({'campaign_identity':campaign} if campaign is not None else {}),
        **({'country_overview':overview} if overview is not None else {}),**state,
        'limitations':['Reader sperimentale: valori da confrontare con l’interfaccia del gioco.',
            'Progresso percentuale e disponibilità dinamica non ancora qualificati.',
            'Campione coerente per rilettura, non una transazione atomica del motore.']}


def compile_watch_reads(witnesses):
    """Batch syscall I/O within 4 KiB pages, compare only selected bytes.

    Gaps can contain unrelated moving counters; comparing those gaps would make
    a paused/research observer refresh for unrelated simulation activity.
    """
    pages = {}
    for (address, size), expected in witnesses.items():
        offset = 0
        while offset < size:
            current = address + offset
            page = current & ~4095
            count = min(size - offset, 4096 - (current - page))
            pages.setdefault(page, []).append((current, expected[offset:offset + count]))
            offset += count
    result = []
    for page, parts in sorted(pages.items()):
        start = min(address for address, _ in parts)
        end = max(address + len(expected) for address, expected in parts)
        result.append((start, end - start, [(address - start, expected) for address, expected in parts]))
    if sum(size for _, size, _ in result) * 2 > 4 * 1024 * 1024:
        raise LiveReadError('Limite del controllo leggero raggiunto')
    return result


def watch_changed(process, reads):
    # Two passes over only selected mutable bytes, like full-sample witnessing.
    for _ in range(2):
        for address, size, parts in reads:
            data = process.read(address, size)
            if len(data) != size:
                raise LiveReadError('Lettura parziale durante controllo cambiamenti')
            if any(data[offset:offset + len(expected)] != expected for offset, expected in parts):
                return True
    return False


class NativeProvider:
    """Native-owned Windows process, decoding, watch cache and integrity audits."""
    def __new__(cls,executable,*,audit_seconds=5,clock=time.monotonic,
                process_factory=WindowsGame,profile_factory=ExecutableProfile,province_neighbors=None):
        if process_factory is not WindowsGame or profile_factory is not ExecutableProfile:
            from .live_callback_transport import CallbackProvider
            return CallbackProvider(executable,audit_seconds=audit_seconds,clock=clock,
                process_factory=process_factory,profile_factory=profile_factory,province_neighbors=province_neighbors)
        return super().__new__(cls)

    def __init__(self,executable,*,audit_seconds=5,clock=time.monotonic,
                 process_factory=WindowsGame,profile_factory=ExecutableProfile,province_neighbors=None):
        import hashlib,weakref
        from .backend_core import _request
        from .live_profiles import MANIFEST
        self.clock=clock
        self.province_neighbors=province_neighbors
        self._handle=_request(0,'provider_open',path=str(Path(executable).resolve()),
            manifest_sha256=hashlib.sha256(MANIFEST.read_bytes()).hexdigest(),
            audit_seconds=audit_seconds,province_neighbors=province_neighbors)
        self._finalizer=weakref.finalize(self,_request,0,'provider_release',handle=self._handle)

    @property
    def metrics(self):
        from .backend_core import _request
        return _request(0,'provider_inspect',handle=self._handle)['metrics']

    def collect(self):
        from .backend_core import _request
        try:return _request(0,'provider_collect',handle=self._handle,now=self.clock())
        except ValueError as exc:raise LiveReadError(str(exc)) from exc

    def close(self):
        from .backend_core import _request
        _request(0,'provider_close',handle=self._handle)


class LiveMonitor:
    """Acquisition callback adapter; Rust owns last-good samples and freshness."""
    def __init__(self,collect):
        from .backend_core import NativeMonitor
        self.collect=collect;self._native=NativeMonitor()

    def poll(self):
        try:
            value=self.collect()
        except (LiveReadError,OSError,ValueError,struct.error) as exc:
            return self._native.failed(str(exc),time.monotonic())
        return self._native.accept(value,time.monotonic(),datetime.now(timezone.utc).isoformat())
