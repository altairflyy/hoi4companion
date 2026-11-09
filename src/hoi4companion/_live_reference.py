"""Legacy live-decoding test oracle; never a runtime fallback.

No injected module, input automation, process write, console command or save read.
Profiles are pinned to exact executable hashes. Field locations were
cross-checked against those PEs; runtime coverage is documented by feature.
Reference locations and provenance are recorded in docs/live-state.md.
"""
from __future__ import annotations

from datetime import datetime, timezone
import hashlib
import os
from pathlib import Path
import re
import struct
import time

SUPPORTED_HASH = '7dc947be34970da1e1c787bcddbe7f62b610ff258aebf8f06aa8b348f3a031d5'
WORLD_RVA = 0x332F260
UPDATED_HASH = 'b6650c85002374218e60598bd04aaa26cbad996fedce77126fcf92fc6bd6d888'
MILITARY_HASH = 'cf781ed178cd801643b233b7e3fcd1c0efd19bd01aecd24bf07a35f69f10476f'
FOREIGN_INTERACTIONS_LAYOUT = {'version':1,'signatures':(
    (0xD809CA,bytes.fromhex('babc2f0000488bc8e8b9')),
    (0xD80A0A,bytes.fromhex('bad72c0000488bc8e879')),
    (0xD8F4C0,bytes.fromhex('4883c110e8378ce7ff')),
    (0xD8F4F8,bytes.fromhex('807b4800')),
    (0x1065007,bytes.fromhex('488b9df0000000486385fc000000488d34c3')),
    (0x1064F5E,bytes.fromhex('440fb685c0000000')),
    (0xC7EBF0,bytes.fromhex('488d8f88100000e8c49d3c00')),
    (0xC7EC4F,bytes.fromhex('48638780100000')),
    (0x104ABF3,bytes.fromhex('48634318488b7b1083f808775b48')),
    (0x1AC5640,bytes.fromhex('488bc24c8d4110488bc8ba6b000000e9fc3514ff')),
    (0x1AC45DD,bytes.fromhex('4c8d4110488bd9ba6b000000488bcfe85f4614ff')),
    (0x104AAB0,bytes.fromhex('48895108c3')),
    (0xC7ED04,bytes.fromhex('6eecc70075ecc7007cecc700e8ecc70083ecc7008aecc700')),
    (0x104ACD4,bytes.fromhex('3fac04011dac04014dac040131ac040154ac040113ac040127ac040138ac040146ac0401')),
)}
MILITARY_LAYOUT = {
    'experience_offset':0x1588,'diplomacy_offset':0xF88,
    'signatures':(
        (0x717170,bytes.fromhex('488b8188150000c3')),
        (0x133906C,bytes.fromhex('4c8d4610493900740dba92330000')),
        (0x1339097,bytes.fromhex('4c8d4628493900740dba93330000')),
        (0x13390C2,bytes.fromhex('4c8d4640493900740dba94330000')),
        (0x3F78D2,bytes.fromhex('33d2488b88880f0000e890cb9a00')),
        (0xDA447F,bytes.fromhex('83b9a4000000008bfa740485d27457')),
        (0xD96155,bytes.fromhex('4889aed0020000')),
    )}
EXECUTABLE_PROFILES = {
    SUPPORTED_HASH: {'world_rva': WORLD_RVA, 'technology_definition_vtable': 0x293C2E0},
    # The updated build exposes only separately qualified sections; unsupported
    # native GUI geometry and other country domains remain unavailable.
    UPDATED_HASH: {'world_rva': 0x33DD6B0, 'technology_definition_vtable': 0x29B9EE0,
                   'campaign_signature_rva': 0x209022,
                   'campaign_signature': bytes.fromhex('4d8d87280700004533c9ba803b0000488bcb'),
                   'campaign_token_rva': 0x34D0AD0,
                   'building_database_rva': 0x33DD268,
                   'production_owner_offset': 0x2A8,
                   'production_totals': (0x2D0, 0x330, 0x390),
                   'country_sections': ('divisions','manpower','states','production','politics')},
    MILITARY_HASH: {'world_rva':0x33DD6B0,'technology_definition_vtable':0x29B9EE0,
                   'joint_focus_vtable':0x27B7AE8,
                   'gui_layout':{'root_rva':0x35010D8,'registry':(0xE8,0x100),
                                 'view_container':0x5F8,'container_children':0x1270,
                                 'icon_sprite':0xF0,'icon_anchor':0xF8,'drag_anchor':0x118,'drag_sprite':0x120,'button_sprite':0xF8,'button_anchor':0x1B8,'container_drags':0x1258,'container_buttons':0x12A0,'container_icons':0x12D8,
                                 'container_clip':0x1794,'container_outer':0x17F0,
                                 'container_inner':0x17F8,'focus_tree_fields':(0x3A48,0x3A50),
                                 'focus_country_optional':0x3A58,'focus_country_fields':(0x3A60,0x3A68)},
                   'campaign_signature_rva':0x209022,
                   'campaign_signature':bytes.fromhex('4d8d87280700004533c9ba803b0000488bcb'),
                   'campaign_token_rva':0x34D0AD0,
                   'production_owner_offset':0x2A8,'production_totals':(0x2D0,0x330,0x390),
                   'military_layout':MILITARY_LAYOUT,
                   'country_metrics_layout':{'version':1,'path':'effective-war-support'},
                   'foreign_interactions_layout':FOREIGN_INTERACTIONS_LAYOUT,
                   'country_sections':('production','experience','diplomacy','metrics','politics','doctrines','intelligence')},
}


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


class ExecutableProfile:
    def __init__(self, path):
        self.path = Path(path).resolve()
        self.data = self.path.read_bytes()
        self.sha256 = hashlib.sha256(self.data).hexdigest()
        selected = EXECUTABLE_PROFILES.get(self.sha256)
        if selected is None:
            raise LiveReadError('Versione HOI4 non supportata dal reader live')
        self.world_rva = selected['world_rva']
        self.technology_definition_vtable = selected['technology_definition_vtable']
        for key in ('campaign_signature_rva','campaign_signature','campaign_token_rva','building_database_rva',
                    'production_owner_offset','production_totals','country_sections','military_layout','joint_focus_vtable','gui_layout','country_metrics_layout','foreign_interactions_layout'):
            if key in selected: setattr(self, key, selected[key])
        pe = struct.unpack_from('<I', self.data, 0x3C)[0]
        if self.data[pe:pe + 4] != b'PE\0\0' or struct.unpack_from('<H', self.data, pe + 4)[0] != 0x8664:
            raise LiveReadError('Eseguibile PE64 non valido')
        count = struct.unpack_from('<H', self.data, pe + 6)[0]
        opt_size = struct.unpack_from('<H', self.data, pe + 20)[0]
        self.image_base = struct.unpack_from('<Q', self.data, pe + 48)[0]
        self.image_size = struct.unpack_from('<I', self.data, pe + 80)[0]
        self.timestamp = struct.unpack_from('<I', self.data, pe + 8)[0]
        self.sections = []
        for index in range(count):
            off = pe + 24 + opt_size + 40 * index
            virtual_size, rva, size, offset = struct.unpack_from('<IIII', self.data, off + 8)
            self.sections.append((rva, size, offset))

    def bytes(self, rva, size):
        for start, length, offset in self.sections:
            if start <= rva and rva + size <= start + length:
                return self.data[offset + rva - start:offset + rva - start + size]
        raise LiveReadError('Riferimento PE non mappato')

    def class_name(self, vtable_rva):
        locator = struct.unpack('<Q', self.bytes(vtable_rva - 8, 8))[0] - self.image_base
        signature, displacement, _, descriptor, _, self_rva = struct.unpack('<6I', self.bytes(locator, 24))
        if signature != 1 or displacement != 0 or self_rva != locator:
            raise LiveReadError('RTTI non riconosciuto')
        return self.bytes(descriptor + 16, 128).split(b'\0', 1)[0].decode('ascii')


class WindowsGame:
    def __init__(self, profile):
        if os.name != 'nt':
            raise LiveReadError('Lettura live disponibile solo su Windows')
        import ctypes as c
        from ctypes import wintypes as w
        self.c, self.w, self.profile = c, w, profile
        k = self.k = c.WinDLL('kernel32', use_last_error=True)
        k.CreateToolhelp32Snapshot.argtypes = [w.DWORD, w.DWORD]
        k.CreateToolhelp32Snapshot.restype = w.HANDLE
        k.CloseHandle.argtypes = [w.HANDLE]
        class Entry(c.Structure):
            _fields_ = [('size', w.DWORD), ('usage', w.DWORD), ('pid', w.DWORD),
                        ('heap', c.c_size_t), ('module', w.DWORD), ('threads', w.DWORD),
                        ('parent', w.DWORD), ('priority', w.LONG), ('flags', w.DWORD), ('name', w.WCHAR * 260)]
        k.Process32FirstW.argtypes = k.Process32NextW.argtypes = [w.HANDLE, c.POINTER(Entry)]
        snapshot = k.CreateToolhelp32Snapshot(2, 0)
        if snapshot == c.c_void_p(-1).value:
            raise LiveReadError('Elenco processi non disponibile')
        found = []
        try:
            row = Entry(); row.size = c.sizeof(row)
            more = k.Process32FirstW(snapshot, c.byref(row))
            while more:
                if row.name.lower() == 'hoi4.exe':
                    found.append(row.pid)
                more = k.Process32NextW(snapshot, c.byref(row))
        finally:
            k.CloseHandle(snapshot)
        if len(found) != 1:
            raise LiveReadError('Gioco non avviato' if not found else 'Più processi HOI4: selezione ambigua')
        self.pid = found[0]
        k.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
        k.OpenProcess.restype = w.HANDLE
        # No VM_WRITE, VM_OPERATION, CREATE_THREAD, debugger, or privilege escalation.
        self.handle = k.OpenProcess(0x410, False, self.pid)
        if not self.handle:
            raise LiveReadError('Accesso in sola lettura al processo negato')
        k.ReadProcessMemory.argtypes = [w.HANDLE, c.c_void_p, c.c_void_p, c.c_size_t, c.POINTER(c.c_size_t)]
        k.QueryFullProcessImageNameW.argtypes = [w.HANDLE, w.DWORD, w.LPWSTR, c.POINTER(w.DWORD)]
        k.GetProcessTimes.argtypes = [w.HANDLE, c.POINTER(w.FILETIME), c.POINTER(w.FILETIME), c.POINTER(w.FILETIME), c.POINTER(w.FILETIME)]
        try:
            self.created = self.identity()
            class Module(c.Structure):
                _fields_ = [('size', w.DWORD), ('module_id', w.DWORD), ('pid', w.DWORD),
                            ('global_usage', w.DWORD), ('process_usage', w.DWORD), ('base', c.c_void_p),
                            ('image_size', w.DWORD), ('handle', w.HMODULE), ('name', w.WCHAR * 256), ('path', w.WCHAR * 260)]
            k.Module32FirstW.argtypes = k.Module32NextW.argtypes = [w.HANDLE, c.POINTER(Module)]
            snapshot = k.CreateToolhelp32Snapshot(0x18, self.pid)
            if snapshot == c.c_void_p(-1).value:
                raise LiveReadError('Modulo HOI4 non disponibile')
            self.base = None
            try:
                row = Module(); row.size = c.sizeof(row)
                more = k.Module32FirstW(snapshot, c.byref(row))
                while more:
                    if Path(row.path).resolve() == profile.path:
                        self.base = row.base
                        if row.image_size != profile.image_size:
                            raise LiveReadError('Dimensione modulo differente dal profilo')
                        break
                    more = k.Module32NextW(snapshot, c.byref(row))
            finally:
                k.CloseHandle(snapshot)
            if not self.base:
                raise LiveReadError('Percorso modulo diverso da quello configurato')
            header = self.read(self.base, 4096)
            pe = struct.unpack_from('<I', header, 0x3C)[0]
            if (header[pe:pe + 4] != b'PE\0\0' or
                    struct.unpack_from('<I', header, pe + 8)[0] != profile.timestamp):
                raise LiveReadError('Identità del modulo caricato non verificata')
            from .native_observer import optional_observer
            self._observer = optional_observer()
        except Exception:
            self.close()
            raise

    def identity(self):
        c, w, k = self.c, self.w, self.k
        path = c.create_unicode_buffer(32768); length = w.DWORD(len(path))
        if not k.QueryFullProcessImageNameW(self.handle, 0, path, c.byref(length)) or Path(path.value).resolve() != self.profile.path:
            raise LiveReadError('Identità processo non verificata')
        created, exited, kernel, user = (w.FILETIME() for _ in range(4))
        if not k.GetProcessTimes(self.handle, c.byref(created), c.byref(exited), c.byref(kernel), c.byref(user)) or exited.dwLowDateTime or exited.dwHighDateTime:
            raise LiveReadError('Processo terminato o non verificabile')
        return created.dwLowDateTime | created.dwHighDateTime << 32

    def read(self, address, size):
        c = self.c
        if not 0x10000 <= address < 0x7FFFFFFFFFFF or not 0 < size <= 65536:
            raise LiveReadError('Richiesta memoria fuori limite')
        buffer, count = c.create_string_buffer(size), c.c_size_t()
        if not self.k.ReadProcessMemory(self.handle, address, buffer, size, c.byref(count)) or count.value != size:
            raise LiveReadError('Memoria del gioco non leggibile')
        return buffer.raw

    def close(self):
        if getattr(self, 'handle', None):
            self.k.CloseHandle(self.handle)
            self.handle = None

    def verify_sample(self, witnesses, visibility=None):
        """Native double-check only; opening/identity and acquisition stay here."""
        observer = getattr(self, '_observer', None)
        if observer is None:
            return False
        from .native_observer import encode_sample
        try:
            if observer.changed(self.handle, encode_sample(witnesses, visibility)):
                raise LiveReadError('Campione incoerente: riprovare')
        except (ValueError, RuntimeError) as exc:
            raise LiveReadError(str(exc)) from exc
        return True

    def prepare_watch(self, reads):
        observer = getattr(self, '_observer', None)
        if observer is None:
            return None
        from .native_observer import encode_watch
        try:
            return encode_watch(reads)
        except (ValueError, RuntimeError) as exc:
            raise LiveReadError(str(exc)) from exc

    def probe_watch(self, packet):
        try:
            return self._observer.changed(self.handle, packet, passes=2)
        except (ValueError, RuntimeError) as exc:
            raise LiveReadError(str(exc)) from exc


def read_native_state(process, *, watch_plan=None):
    """Collect own-country candidate data; reject torn/unknown structures."""
    sample = NativeSample(process)
    u = sample.number
    i = lambda a: sample.number(a, '<i')
    base = process.base
    profile = getattr(process, 'profile', None)
    world_rva = getattr(profile, 'world_rva', WORLD_RVA)
    definition_vtable = getattr(profile, 'technology_definition_vtable', 0x293C2E0)
    world = u(base + world_rva)
    if not world:
        raise LiveReadError('Nessuna partita caricata')
    def obj(address, class_name):
        if process.profile.class_name(u(address) - base) != f'.?AV{class_name}@@':
            raise LiveReadError(f'Tipo nativo inatteso: {class_name}')
        return address
    obj(world, 'CCurrentGameState')
    tick = i(world + 0x468)
    native_tag = i(world + 0x520)
    # Do not silently use the alternate observer/selection field at +0x524.
    # The positive primary selector must be compared with the real player UI.
    if native_tag <= 0:
        raise LiveReadError('Paese giocato non identificato con certezza')
    tag_count = i(world + 0x364)
    if not native_tag < tag_count <= 8192:
        raise LiveReadError('Indice nazione fuori intervallo')
    tag = sample.string(u(world + 0x358) + native_tag * 32)
    if not re.fullmatch('[A-Z0-9]{3}', tag):
        raise LiveReadError('TAG nazione non valido')
    tags = sample.array(world + 0x340, 4)
    countries = sample.array(world + 0x310, 8)
    if not native_tag < len(tags) or not 0 <= tags[native_tag] < len(countries):
        raise LiveReadError('Mappa nazione non valida')
    country = obj(countries[tags[native_tag]], 'CCountry')
    if i(country + 8) != native_tag:
        raise LiveReadError('Identità nazione incoerente')
    tree = obj(u(country + 0x1370), 'CNationalFocusTree')
    tree_id = sample.string(tree + 8)
    focus_state = obj(u(country + 0x1380), 'CNationalFocusProgress')
    if u(focus_state + 8) != country:
        raise LiveReadError('Stato focus di un’altra nazione')
    focus_defs = sample.array(tree + 0x30, 8, 4096)
    if not focus_defs or len(focus_defs) != len(set(focus_defs)):
        raise LiveReadError('Albero dei focus vuoto o duplicato')
    focus_ids = {}
    for pointer in focus_defs:
        vtable=u(pointer)-base
        kind=process.profile.class_name(vtable)
        # Shared joint focuses are registered in each participating tree, not
        # owned by a single tree: their base constructor receives a null owner.
        # Admit only the separately qualified exact-build vtable and RTTI. The
        # selected country's tree membership and progress owner remain required.
        joint=(kind=='.?AVCJointNationalFocus@@' and
               vtable==getattr(profile,'joint_focus_vtable',None))
        if kind!='.?AVCNationalFocus@@' and not joint:
            raise LiveReadError('Tipo nativo inatteso: CNationalFocus')
        if u(pointer + 0x160) != (0 if joint else tree):
            raise LiveReadError('Focus non appartenente all’albero')
        focus_ids[pointer] = sample.string(pointer + 0x18)
    if len(set(focus_ids.values())) != len(focus_ids):
        raise LiveReadError('Identificatori focus duplicati')
    completed_ptrs = sample.array(focus_state + 0x40, 8, 4096)
    current_ptr = u(focus_state + 0x10)
    continuous_ptr = u(focus_state + 0x18)
    continuous_id = None
    if continuous_ptr:
        obj(continuous_ptr, 'CContinuousNationalFocus')
        continuous_id = sample.string(continuous_ptr + 0x18)
    if (not set(completed_ptrs) <= focus_ids.keys()
            or current_ptr and current_ptr not in focus_ids or current_ptr and current_ptr in completed_ptrs):
        raise LiveReadError('Stato focus incoerente')
    # Reload/complete effects can append an already completed, identical owned
    # pointer. Publish membership once; preserve the raw array for witnessing
    # and change detection below. Foreign pointers and active completions still
    # invalidate the entire sample before this interpretation.
    completed_unique = list(dict.fromkeys(completed_ptrs))
    # +0x18 is a continuous-focus pointer, NOT an elapsed duration.
    tech_state = obj(u(country + 0xF60), 'CTechnologyStatus')
    models = sample.array(tech_state + 0x88, 8, 4096)
    completed_models = sample.array(tech_state + 0x40, 8, 4096)
    slots = sample.array(tech_state + 0xA0, 8, 64)
    if not models or not slots or len(set(models)) != len(models) or len(set(slots)) != len(slots):
        raise LiveReadError('Stato ricerca vuoto o duplicato')
    if not set(completed_models) <= set(models) or len(set(completed_models)) != len(completed_models):
        raise LiveReadError('Elenco tecnologie completate incoerente')
    active = []
    for pointer in slots:
        obj(pointer, 'CResearchSlot')
        if u(pointer + 0x10) != tech_state:
            raise LiveReadError('Slot ricerca di un’altra nazione')
        target = u(pointer + 0x18)
        if target and target not in models:
            raise LiveReadError('Slot ricerca non riconosciuto')
        active.append(target)
    model_ids, researched = {}, []
    for index, pointer in enumerate(models, 1):
        # Runtime object's RTTI distinguishes CTechnology from CTechnologyStatus.
        obj(pointer, 'CTechnology')
        if u(pointer + 0x158) != tech_state:
            raise LiveReadError('Tecnologia di un’altra nazione')
        definition = u(pointer + 0x160)
        # Definition vtable is additionally bound to this immutable PE profile.
        if u(definition) != base + definition_vtable or i(definition + 0x38) != index:
            raise LiveReadError('Definizione tecnologia non riconosciuta')
        identifier = sample.string(definition + 0x10)
        model_ids[pointer] = identifier
        level, maximum = i(pointer + 0x174), i(definition + 0x3DC)
        if not 0 <= level <= 1000000 or not 0 <= maximum <= 1000000:
            raise LiveReadError('Livello tecnologia non valido')
        complete = level >= maximum if maximum > 0 else None
        if (pointer in completed_models) != (complete is True):
            raise LiveReadError('Livelli ed elenco completamenti non concordano')
        if pointer in active and complete:
            raise LiveReadError('Tecnologia contemporaneamente attiva e completata')
        if level > 0:
            researched.append(identifier)
    if len(set(model_ids.values())) != len(model_ids):
        raise LiveReadError('Identificatori ricerca duplicati')
    if watch_plan is not None:
        # Keep the mutable identity, ownership, membership, selection and level
        # bytes. Definitions/strings are revalidated by periodic full audits.
        # Tick alone is deliberately not a change: choices can change in pause.
        ranges = [(base + world_rva, 8), (world, 8), (world + 0x520, 4),
                  (world + 0x358, 8), (world + 0x364, 4),
                  (world + 0x340, 16), (world + 0x310, 16),
                  (country, 8), (country + 8, 4), (country + 0x1370, 8),
                  (country + 0x1380, 8), (country + 0xF60, 8),
                  (tree, 8), (focus_state, 16), (focus_state + 0x10, 16),
                  (tech_state, 8)]
        ranges += [(u(world + 0x340) + native_tag * 4, 4),
                   (u(world + 0x310) + tags[native_tag] * 8, 8)]
        for address, values, width in [(tree + 0x30, focus_defs, 8),
                                        (focus_state + 0x40, completed_ptrs, 8),
                                        (tech_state + 0x88, models, 8),
                                        (tech_state + 0x40, completed_models, 8),
                                        (tech_state + 0xA0, slots, 8)]:
            ranges.append((address, 16))
            if values:
                ranges.append((u(address), len(values) * width))
        for pointer in slots:
            ranges += [(pointer, 8), (pointer + 0x10, 16)]
        for pointer in models:
            ranges += [(pointer, 8), (pointer + 0x158, 32)]
        watched = {(address, size): sample.read(address, size) for address, size in ranges}
    overview = None
    campaign = None
    if process.profile.sha256 in (SUPPORTED_HASH, UPDATED_HASH, MILITARY_HASH):
        from .campaign import read_campaign
        previous_witnesses=set(sample.witnesses)
        campaign = read_campaign(sample,world)
        if watch_plan is not None:
            watched.update({key:sample.witnesses[key] for key in sample.witnesses.keys()-previous_witnesses})
        from .country import collect_country
        overview = collect_country(process, world, country, native_tag, tag, watch_plan)
    sample.verify()
    if process.identity() != process.created:
        raise LiveReadError('Sessione di processo cambiata')
    if watch_plan is not None:
        watch_plan.update(watched)
    return {'session_id': f'{process.pid}:{process.created}:{world:x}:{country:x}',
            'pid': process.pid, 'executable_sha256': process.profile.sha256,
            **({'campaign_identity':campaign} if campaign is not None else {}),
            **({'country_overview': overview} if overview is not None else {}),
            'country': tag, 'native_tick': tick,
            'country_identity_validation': 'primary_selector_and_country_map',
            'focus_complete': True, 'research_complete': True,
            'focus_ids': sorted(focus_ids.values()), 'completed_focuses': [focus_ids[x] for x in completed_unique],
            'native_focus_tree_id': tree_id, 'continuous_focus': continuous_id,
            'current_focus': focus_ids.get(current_ptr), 'focus_progress': None,
            'researched_technologies': researched,
            'technology_ids': sorted(model_ids.values()),
            'research_slots': [{'slot': index, 'technology_id': model_ids.get(pointer), 'progress': None}
                               for index, pointer in enumerate(active, 1)],
            'limitations': ['Reader sperimentale: valori da confrontare con l’interfaccia del gioco.',
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


class LiveMonitor:
    """Last good sample survives a disconnect, but never remains usable."""
    def __init__(self, collect):
        self.collect, self.last, self.last_time = collect, None, None

    def poll(self):
        try:
            value = self.collect()
            self.last, self.last_time = value, time.monotonic()
            return {'status': 'observed', 'source': 'process_memory', 'usable': True,
                    'validation': 'experimental', 'sample': value, 'age_seconds': 0,
                    'observed_at': datetime.now(timezone.utc).isoformat(), 'error': ''}
        except (LiveReadError, OSError, ValueError, struct.error) as exc:
            return {'status': 'stale' if self.last is not None else 'disconnected',
                    'source': 'process_memory', 'usable': False, 'validation': 'experimental',
                    'sample': self.last, 'age_seconds': round(time.monotonic() - self.last_time, 1) if self.last_time else None,
                    'error': str(exc)}
