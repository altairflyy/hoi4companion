"""Build-pinned, passive native GUI geometry for the external overlay.

This observes frontend objects and their world transforms; it never invokes a
game function, writes memory, or infers an ID from localized text. Coordinates
are in the game's GUI pixel space, with its observed resolution attached.
"""
from __future__ import annotations

from functools import lru_cache
import math
import re
import struct
import time

from .live import ExecutableProfile, WindowsGame, NativeSample, LiveReadError, WORLD_RVA
from .events_native import GUI_RVA


class GuiSample(NativeSample):
    """Witness the visibility bit separately from unrelated widget flags."""
    def __init__(self,process):
        super().__init__(process)
        self.visibility={}

    def shown(self,address):
        key=(address,1)
        full_witness=key in self.witnesses
        value=self.read(address,1)[0] & 8
        # Reuse the bounded/complete read checks; keep a full-byte witness if
        # any other consumer explicitly required one. Here only bit 3 is used.
        if not full_witness:del self.witnesses[key]
        if self.visibility.setdefault(address,value)!=value:
            raise LiveReadError('Stato cambiato durante la lettura')
        return bool(value)

    def verify(self):
        super().verify()
        for address,expected in self.visibility.items():
            value=self.process.read(address,1)
            if len(value)!=1 or value[0] & 8 != expected:
                raise LiveReadError('Campione incoerente: riprovare')


def _intersect(a, b):
    x, y = max(a[0], b[0]), max(a[1], b[1])
    right, bottom = min(a[0]+a[2], b[0]+b[2]), min(a[1]+a[3], b[1]+b[3])
    return [x, y, max(0., right-x), max(0., bottom-y)]


def geometry(position, container, clip):
    """Pure fixture-testable decoder of the observed C2dPositionObject matrix."""
    matrix = struct.unpack_from('<16f', position, 0x90)
    sx, sy, x, negative_y = matrix[0], matrix[5], matrix[12], matrix[13]
    width, height = struct.unpack_from('<ii', container, 0x15AC)
    if (not all(math.isfinite(v) for v in matrix) or not 0.01 <= sx <= 10
            or not 0.01 <= sy <= 10 or abs(sx-sy) > 0.0001
            or any(abs(matrix[i]) > 0.0001 for i in (1, 2, 3, 4, 6, 7, 8, 9, 11))
            or not 0 < width <= 65536 or not 0 < height <= 65536):
        raise LiveReadError('Trasformazione GUI non qualificata')
    rect = [x, -negative_y, width*sx, height*sy]
    clipped = _intersect(rect, clip)
    return {'rect': rect, 'clip': clipped, 'visible': clipped[2] > 0 and clipped[3] > 0, 'scale': sx}


def sprite_geometry(matrix, size, resolution, clip):
    """CSprite world matrix uses a centered, Y-up bottom-left origin."""
    sx, sy, x, y = matrix[0], matrix[5], matrix[12], matrix[13]
    if (not all(math.isfinite(v) for v in matrix) or not 0.01 <= sx <= 10
            or abs(sx-sy) > 0.0001 or not all(0 < v <= 65536 for v in size)
            or any(abs(matrix[i]) > 0.0001 for i in (1, 2, 3, 4, 6, 7, 8, 9, 11))):
        raise LiveReadError('Trasformazione sprite non qualificata')
    width, height = size[0]*sx, size[1]*sy
    rect = [resolution[0]/2+x, resolution[1]/2-y-height, width, height]
    clipped = _intersect(rect, clip)
    return {'rect': rect, 'clip': clipped, 'visible': clipped[2] > 0 and clipped[3] > 0, 'scale': sx}


def anchored_sprite_geometry(parent, position, size, child_scale, clip):
    """Compose current GUI parent with the engine's resolved child position.

    C2dPositionObject +128/+12C stores the pixel position after the game's
    orientation/center-position handling. Its parent matrix is in top-left GUI
    coordinates, so this does not depend on a cached rendering matrix.
    """
    sx, sy = parent[0], parent[5]
    if (not all(math.isfinite(v) for v in (*parent, *position, child_scale))
            or not 0.01 <= sx <= 10 or abs(sx-sy) > 0.0001
            or not 0.01 <= child_scale <= 10 or not all(0 < v <= 65536 for v in size)
            or any(abs(parent[i]) > 0.0001 for i in (1, 2, 3, 4, 6, 7, 8, 9, 11))):
        raise LiveReadError('Ancoraggio sprite non qualificato')
    rect = [parent[12]+position[0]*sx, -parent[13]+position[1]*sy,
            size[0]*sx*child_scale, size[1]*sy*child_scale]
    clipped = _intersect(rect, clip)
    return {'rect': rect, 'clip': clipped, 'visible': clipped[2] > 0 and clipped[3] > 0,
            'scale': sx*child_scale}


class UiReader:
    """Own one process handle and cache discovery, never mutable geometry.

    Snapshot raises LiveReadError on a torn/foreign/unqualified observation.
    A successful hidden panel is authoritative and contains no matching nodes.
    The optional process is a test seam and remains owned by its caller.
    """
    def __init__(self, executable_path=None, *, process=None):
        self._owns = process is None
        self.process = process or WindowsGame(ExecutableProfile(executable_path))
        self.profile = self.process.profile
        self._cache = None
        self._missing_refreshes = {}
        self.discovery_ms = 0.

    def close(self):
        if self._owns:
            self.process.close()

    @lru_cache(maxsize=16384)
    def _rtti(self, vtable):
        p = self.profile
        locator = struct.unpack('<Q', p.bytes(vtable-self.process.base-8, 8))[0]-p.image_base
        signature, displacement, _, descriptor, _, self_rva = struct.unpack('<6I', p.bytes(locator, 24))
        if signature != 1 or self_rva != locator:
            raise LiveReadError('RTTI GUI non riconosciuto')
        return p.bytes(descriptor+16, 128).split(b'\0', 1)[0].decode('ascii'), displacement

    def _typed(self, s, address, name):
        if self._rtti(s.number(address)) != ('.?AV'+name+'@@', 0):
            raise LiveReadError('Tipo GUI inatteso: '+name)
        return address

    def _discover(self, s, root):
        headers = tuple(s.read(root+off, 16) for off in (0x108, 0x120))
        # Registry occupancy changes with unrelated transient UI widgets. The
        # allocation identity is stable; selected bindings are fully validated
        # on every snapshot and invalidate this cache on any mismatch.
        allocation = tuple(struct.unpack('<Qii', raw)[:2] for raw in headers)
        if any(not 0 <= count <= capacity <= 500000
               for _, capacity, count in (struct.unpack('<Qii', raw) for raw in headers)):
            raise LiveReadError('Registro GUI fuori intervallo')
        if self._cache and self._cache[:2] == (root, allocation):
            return self._cache[2]
        start = time.perf_counter()
        process, pages, seen, owners = self.process, {}, set(), {}

        def pointer(address):
            page = address & ~4095
            if address+8 > page+4096:
                return struct.unpack('<Q', process.read(address, 8))[0]
            if page not in pages:
                pages[page] = process.read(page, 4096)
            return struct.unpack_from('<Q', pages[page], address-page)[0]

        allowed = {'CNationalFocusView', 'CCountryTechnologyView', 'CCountryTechTreeView', 'CEventWindow',
                   'CNationalFocusTreeItem', 'CTechnologyTreeTechItem'}
        for raw, dynamic in zip(headers, (False, True)):
            data, capacity, count = struct.unpack('<Qii', raw)
            if not 0 <= count <= capacity <= 500000:
                raise LiveReadError('Registro GUI fuori intervallo')
            extent = capacity if dynamic else count
            for offset in range(0, extent, 8192):
                block = process.read(data+offset*8, min(8192, extent-offset)*8)
                for (gui,) in struct.iter_unpack('<Q', block):
                    if not gui or gui in seen:
                        continue
                    seen.add(gui)
                    try:
                        observer = pointer(gui+80)
                        name, displacement = self._rtti(pointer(observer))
                        # MSVC spelling is .?AV + class + @@ (four-byte prefix).
                        short = name[4:-2] if name.startswith('.?AV') else ''
                        if short in allowed:
                            owners[observer-displacement] = short
                    except (LiveReadError, UnicodeError, struct.error, OSError):
                        continue
        views = {name: [a for a, n in owners.items() if n == name]
                 for name in ('CNationalFocusView', 'CCountryTechnologyView')}
        if any(len(values) != 1 for values in views.values()):
            raise LiveReadError('Viste GUI mancanti o ambigue')
        # Candidate discovery is not a sample. Only these bindings are exposed,
        # and identity, ownership and geometry are reread/witnessed below.
        bindings = {'views': {name: values[0] for name, values in views.items()},
                    'focus': {}, 'research': {},
                    'events': sorted(a for a, name in owners.items() if name == 'CEventWindow'),
                    'registry_seen': seen, 'registry_headers': headers}
        trees = []
        for address, name in owners.items():
            if name != 'CCountryTechTreeView':
                continue
            try:
                container = pointer(address+0x578)
                definition = pointer(container+0x90)
                if NativeSample(process).string(definition+0x28) == 'countrytechtreeview':
                    trees.append(address)
            except (LiveReadError, OSError, struct.error):
                continue
        if len(trees) != 1:
            raise LiveReadError('Albero ricerca mancante o ambiguo')
        bindings['views']['CCountryTechTreeView'] = trees[0]
        tree_container = pointer(trees[0]+0x578)
        bindings['folders'] = {}
        bindings['tabs_container'] = None
        for gui in NativeSample(process).array(tree_container+0x10F0, maximum=2048):
            try:
                class_name, displacement = self._rtti(pointer(gui))
                if class_name != '.?AVCContainerWindow@@' or displacement != 48:
                    continue
                name = NativeSample(process).string(pointer(gui+0x60)+0x28)
                if name == 'folder_tabs':
                    bindings['tabs_container'] = gui-48
                if name.endswith('_folder') or name in ('mtgnavalfolder', 'mtgnavalsupportfolder'):
                    bindings['folders'][name] = gui-48
            except (LiveReadError, OSError, struct.error):
                continue
        if not bindings['folders']:
            raise LiveReadError('Cartelle albero ricerca non individuate')
        for address, name in owners.items():
            try:
                if name == 'CNationalFocusTreeItem':
                    definition = pointer(address+0x20)
                    key = NativeSample(process).string(definition+0x18)
                    bindings['focus'].setdefault(key, []).append(address)
                elif name == 'CTechnologyTreeTechItem':
                    model = pointer(address+0xC8)
                    definition = pointer(model+0x160)
                    key = NativeSample(process).string(definition+0x10)
                    bindings['research'].setdefault(key, []).append(address)
            except (LiveReadError, OSError, struct.error):
                continue
        self.discovery_ms = (time.perf_counter()-start)*1000
        self._cache = (root, allocation, bindings)
        return bindings

    def _country(self, s):
        u = s.number
        world = self._typed(s, u(self.process.base+WORLD_RVA), 'CCurrentGameState')
        native = u(world+0x520, '<i')
        count = u(world+0x364, '<i')
        if not 0 < native < count <= 8192:
            raise LiveReadError('Identità nazione non qualificata')
        country_tag = s.string(u(world+0x358)+native*32)
        if not re.fullmatch('[A-Z0-9]{3}', country_tag):
            raise LiveReadError('TAG nazione non valido')
        def element(address, index, size, fmt):
            data, capacity, amount = struct.unpack('<Qii', s.read(address, 16))
            if not 0 <= index < amount <= capacity <= 8192:
                raise LiveReadError('Mappa nazione non valida')
            return u(data+index*size, fmt)
        index = element(world+0x340, native, 4, '<i')
        country = self._typed(s, element(world+0x310, index, 8, '<Q'), 'CCountry')
        if u(country+8, '<i') != native:
            raise LiveReadError('Identità nazione incoerente')
        tree = self._typed(s, u(country+0x1370), 'CNationalFocusTree')
        technology = self._typed(s, u(country+0xF60), 'CTechnologyStatus')
        return country_tag, tree, s.string(tree+8), technology, country

    def _container(self, s, pointer, root, observer):
        self._typed(s, pointer, 'CContainerWindow')
        if s.number(pointer+0x98) != root or s.number(pointer+0x80) != observer:
            raise LiveReadError('Proprietario GUI incoerente')
        return s.read(pointer+0xA4, 2)[1] & 8 != 0

    def _sprite_bounds(self, s, sprite, parent, transform, clip):
        self._typed(s, sprite, 'CSprite')
        if s.number(sprite+0x118) != parent:
            raise LiveReadError('Ancoraggio sprite estraneo al widget')
        sprite_type = self._typed(s, s.number(sprite+0x180), 'CSpriteType')
        columns, rows = struct.unpack('<ii', s.read(sprite_type+0x90, 8))
        width, height = struct.unpack('<ii', s.read(sprite_type+0xA4, 8))
        if s.number(sprite_type+0x10C, '<i') == 1:
            columns, rows = rows, columns
        if not 0 < columns <= 1024 or not 0 < rows <= 1024:
            raise LiveReadError('Fotogrammi sprite non validi')
        return anchored_sprite_geometry(transform, struct.unpack('<2f', s.read(sprite+0x128, 8)),
            [int(width/columns), int(height/rows)], s.number(sprite+0x124, '<f'), clip)

    def _focus_geometry(self, s, container, root, observer, resolution, clip):
        parts = {}
        parent = self._typed(s, s.number(container+0x15F8), 'C2dPositionObject')
        transform = struct.unpack('<16f', s.read(parent+0x90, 64))
        for button in s.array(container+0x1120, maximum=64):
            self._typed(s, button, 'CButtonStandard')
            definition = self._typed(s, s.number(button+0x60), 'CButtonType')
            name = s.string(definition+0x28)
            if name not in ('bg', 'symbol'):
                continue
            if s.number(button+0x68) != root or s.number(button+0x50) != observer:
                raise LiveReadError('Pulsante focus di un altro proprietario')
            sprite = self._typed(s, s.number(button+0xF0), 'CSprite')
            if s.number(button+0x1B0) != parent or s.number(sprite+0x118) != parent:
                raise LiveReadError('Ancoraggio pulsante estraneo al focus')
            parts[name] = self._sprite_bounds(s, sprite, parent, transform, clip)
        if set(parts) != {'bg', 'symbol'}:
            raise LiveReadError('Contorno focus senza simbolo e sfondo qualificati')
        a, b = parts['bg'], parts['symbol']
        if abs(a['scale']-b['scale']) > 0.0001:
            raise LiveReadError('Scale focus incoerenti')
        left, top = min(a['rect'][0], b['rect'][0]), min(a['rect'][1], b['rect'][1])
        right = max(v['rect'][0]+v['rect'][2] for v in parts.values())
        bottom = max(v['rect'][1]+v['rect'][3] for v in parts.values())
        rect = [left, top, right-left, bottom-top]
        clipped = _intersect(rect, clip)
        return {'rect': rect, 'clip': clipped, 'visible': clipped[2] > 0 and clipped[3] > 0,
                'scale': a['scale'], 'parts': {name: part['rect'] for name, part in parts.items()}}

    def _research_geometry(self, s, container, root, observer, clip):
        """Read the real background button; keep its icon as supporting data."""
        outer = self._typed(s, s.number(container+0x15F0), 'C2dPositionObject')
        outer_transform = struct.unpack('<16f', s.read(outer+0x90, 64))
        backgrounds = []
        for button in s.array(container+0x10D8, maximum=64):
            self._typed(s, button, 'CButtonDrag')
            definition = self._typed(s, s.number(button+0x60), 'CButtonType')
            if s.string(definition+0x28) != 'Background':
                continue
            if s.number(button+0x68) != root or s.number(button+0x50) != observer:
                raise LiveReadError('Sfondo ricerca di un altro proprietario')
            if s.number(button+0x110) != outer:
                raise LiveReadError('Ancoraggio sfondo ricerca non qualificato')
            backgrounds.append(self._sprite_bounds(s, s.number(button+0x118), outer, outer_transform, clip))
        if len(backgrounds) != 1:
            raise LiveReadError('Sfondo tecnologia mancante o ambiguo')
        parent = self._typed(s, s.number(container+0x15F8), 'C2dPositionObject')
        transform = struct.unpack('<16f', s.read(parent+0x90, 64))
        matches = []
        for icon in s.array(container+0x1150, maximum=64):
            self._typed(s, icon, 'CIcon')
            definition = self._typed(s, s.number(icon+0x60), 'CIconType')
            if s.string(definition+0x28) != 'Icon':
                continue
            if s.number(icon+0x68) != root or s.number(icon+0x50) != observer:
                raise LiveReadError('Icona ricerca di un altro proprietario')
            sprite = self._typed(s, s.number(icon+0xE8), 'CSprite')
            if s.number(icon+0xF0) != parent or s.number(sprite+0x118) != parent:
                raise LiveReadError('Ancoraggio icona estraneo alla ricerca')
            result = self._sprite_bounds(s, sprite, parent, transform, clip)
            matches.append(result)
        if len(matches) != 1:
            raise LiveReadError('Icona tecnologia mancante o ambigua')
        result = backgrounds[0]
        result['parts'] = {'background': result['rect'][:], 'icon': matches[0]['rect'][:]}
        return result

    def _research_tabs(self, s, container, root, observer, clip, folders):
        self._container(s, container, root, observer)
        parent = self._typed(s, s.number(container+0x15F8), 'C2dPositionObject')
        transform = struct.unpack('<16f', s.read(parent+0x90, 64))
        result = []
        for button in s.array(container+0x1120, maximum=64):
            self._typed(s, button, 'CButtonStandard')
            definition = self._typed(s, s.number(button+0x60), 'CButtonType')
            name = s.string(definition+0x28)
            if not name.endswith('_tab') or name[:-4] not in folders:
                continue
            if s.number(button+0x68) != root or s.number(button+0x50) != observer:
                raise LiveReadError('Scheda ricerca di un altro proprietario')
            sprite = self._typed(s, s.number(button+0xF0), 'CSprite')
            if s.number(button+0x1B0) != parent or s.number(sprite+0x118) != parent:
                raise LiveReadError('Ancoraggio scheda ricerca non qualificato')
            data = self._sprite_bounds(s, sprite, parent, transform, clip)
            data.update(panel='research_tab', identity=name[:-4],
                        visible=s.shown(button+0x75) and data['visible'])
            result.append(data)
        return result

    def _research_folder(self, s, container, folders):
        anchors = {s.number(c+0x15F8): name for name, c in folders.items()}
        current = s.number(container+0x15F0)
        visited = set()
        for _ in range(16):
            if not current or current in visited:
                break
            visited.add(current)
            self._typed(s, current, 'C2dPositionObject')
            if current in anchors:
                return anchors[current]
            current = s.number(current+0x118)
        raise LiveReadError('Cartella della tecnologia non qualificata')

    def snapshot(self, *, focus_ids=(), technology_ids=(), event_ids=(), event_instances=()):
        started = time.perf_counter()
        for attempt in range(2):
            try:
                extra = {'event_ids': event_ids, 'event_instances': event_instances} if event_ids or event_instances else {}
                result = self._snapshot(focus_ids=focus_ids, technology_ids=technology_ids, **extra)
                result['diagnostics'].update(snapshot_ms=(time.perf_counter()-started)*1000,
                                             retry_count=attempt)
                return result
            except (LiveReadError, OSError, struct.error) as exc:
                if isinstance(exc,LiveReadError) and str(exc) in (
                        'Campione incoerente: riprovare','Stato cambiato durante la lettura'):
                    # Movement can tear a sample without invalidating registry
                    # candidates. Retry once with NEW witnesses and all owner/
                    # country/ID checks. No coordinates survive either attempt.
                    if attempt==0:continue
                    raise
                # Allocators can reuse slots without resizing the arrays. An
                # actual binding/type/identity failure invalidates discovery.
                self._cache = None
                raise

    def _snapshot(self, *, focus_ids=(), technology_ids=(), event_ids=(), event_instances=()):
        started = time.perf_counter()
        p = self.process
        if p.identity() != p.created:
            raise LiveReadError('Processo GUI cambiato')
        # Candidate discovery can take hundreds of milliseconds and its GUI
        # registries contain unrelated transient widgets. Those bytes are not
        # observations: do not retain their witnesses in the final sample.
        candidate = NativeSample(p)
        candidate_root = self._typed(candidate, candidate.number(p.base+GUI_RVA), 'CGameGui')
        bindings = self._discover(candidate, candidate_root)
        from .event_ui_native import refresh_candidates, read_event_ui
        event_owners = refresh_candidates(self, candidate_root, bindings, event_instances)
        s = GuiSample(p)
        root = self._typed(s, s.number(p.base+GUI_RVA), 'CGameGui')
        if root != candidate_root:
            raise LiveReadError('Contesto GUI cambiato durante la ricerca')
        country, tree, scope, technology, country_pointer = self._country(s)
        panels, nodes, tabs, resolution = {}, [], [], None
        for panel, name, targets in (('focus', 'CNationalFocusView', set(focus_ids)),
                                     ('research', 'CCountryTechTreeView', set(technology_ids))):
            owner = self._typed(s, bindings['views'][name], name)
            container = s.number(owner+0x578)
            visible = self._container(s, container, root, owner+40)
            position = self._typed(s, s.number(container+0x15F0), 'C2dPositionObject')
            data = s.read(position+0x130, 8)
            current_resolution = list(struct.unpack('<ii', data))
            if not all(64 <= v <= 32768 for v in current_resolution):
                raise LiveReadError('Risoluzione GUI non valida')
            if resolution is not None and resolution != current_resolution:
                raise LiveReadError('Viste con risoluzioni incoerenti')
            resolution = current_resolution
            raw_clip = struct.unpack('<4i', s.read(container+0x1594, 16))
            clip = _intersect([raw_clip[0], raw_clip[1], raw_clip[2]-raw_clip[0], raw_clip[3]-raw_clip[1]],
                              [0, 0, *resolution])
            panels[panel] = {'visible': visible, 'clip': clip}
            if panel == 'research':
                definition = self._typed(s, s.number(container+0x90), 'CContainerWindowType')
                if s.string(definition+0x28) != 'countrytechtreeview':
                    raise LiveReadError('Contesto albero ricerca cambiato')
                if visible and not bindings.get('tabs_container'):
                    raise LiveReadError('Schede native di ricerca non qualificate')
                if visible and bindings.get('tabs_container'):
                    tabs = self._research_tabs(s, bindings['tabs_container'], root, owner+40, clip, bindings['folders'])
                selected = []
                for folder_name, folder_container in bindings['folders'].items():
                    if self._container(s, folder_container, root, owner+40):
                        definition = self._typed(s, s.number(folder_container+0x90), 'CContainerWindowType')
                        if s.string(definition+0x28) != folder_name:
                            raise LiveReadError('Identità cartella ricerca cambiata')
                        selected.append((folder_name, folder_container))
                if visible and len(selected) != 1:
                    raise LiveReadError('Cartella ricerca visibile mancante o ambigua')
                panels[panel]['folder'] = selected[0][0] if len(selected)==1 else None
                if visible:
                    folder_raw = struct.unpack('<4i', s.read(selected[0][1]+0x1594, 16))
                    clip = _intersect(clip, [folder_raw[0], folder_raw[1],
                                            folder_raw[2]-folder_raw[0], folder_raw[3]-folder_raw[1]])
                    panels[panel]['clip'] = clip
            if not visible:
                continue
            if targets - bindings[panel].keys():
                # New widgets can occupy existing slots without changing the
                # allocation identity. Retry discovery once for this request
                # context, never every frame for an absent/offscreen target.
                context = (root, country, scope, panel, panels[panel].get('folder'), tuple(sorted(targets)))
                if context not in self._missing_refreshes:
                    self._missing_refreshes[context] = None
                    if len(self._missing_refreshes) > 128:
                        self._missing_refreshes.pop(next(iter(self._missing_refreshes)))
                    self._cache = None
                    # Discard pre-discovery witnesses; the recursive sample
                    # starts only after candidate discovery has completed.
                    return self._snapshot(focus_ids=focus_ids, technology_ids=technology_ids,
                                          event_ids=event_ids, event_instances=event_instances)
            if panel == 'focus':
                # The frontend has separate viewed/selected/context bindings.
                # Only the qualified own-country mode is admitted; comparison
                # or foreign-country views are deliberately not extrapolated.
                if any(s.number(owner+offset) != tree for offset in (0x20E8, 0x20F0)):
                    raise LiveReadError('Vista focus di un altro albero')
                # +20F8 is nullable (observed null in an own-country view).
                # The two persistent bindings must still identify the player.
                optional_country = s.number(owner+0x20F8)
                if (optional_country not in (0, country_pointer)
                        or any(s.number(owner+offset) != country_pointer for offset in (0x2100, 0x2108))):
                    raise LiveReadError('Vista focus di un’altra nazione')
            for key in sorted(targets):
                for item in bindings[panel].get(key, ()):
                    self._typed(s, item, 'CNationalFocusTreeItem' if panel == 'focus' else 'CTechnologyTreeTechItem')
                    item_container = s.number(item+0x10)
                    shown = self._container(s, item_container, root, item+24)
                    if panel == 'focus':
                        definition = self._typed(s, s.number(item+0x20), 'CNationalFocus')
                        if s.number(definition+0x160) != tree or s.string(definition+0x18) != key:
                            raise LiveReadError('Focus GUI estraneo alla nazione o ID cambiato')
                    else:
                        model = self._typed(s, s.number(item+0xC8), 'CTechnology')
                        if s.number(model+0x158) != technology:
                            raise LiveReadError('Ricerca GUI di un’altra nazione')
                        definition = self._typed(s, s.number(model+0x160), 'CTechnologyTemplate')
                        if s.string(definition+0x10) != key:
                            raise LiveReadError('ID ricerca GUI cambiato')
                        tree_view = self._typed(s, s.number(item+0x30), 'CCountryTechTreeView')
                        if tree_view != owner:
                            raise LiveReadError('Tecnologia di un altro albero visualizzato')
                        tree_container = s.number(tree_view+0x578)
                        tree_shown = self._container(s, tree_container, root, tree_view+40)
                        raw = struct.unpack('<4i', s.read(tree_container+0x1594, 16))
                        tree_clip = _intersect(clip, [raw[0], raw[1], raw[2]-raw[0], raw[3]-raw[1]])
                        shown = shown and tree_shown
                        item_folder = self._research_folder(s, item_container, bindings['folders'])
                        shown = shown and item_folder == panels['research']['folder']
                    if panel == 'focus':
                        node = self._focus_geometry(s, item_container, root, item+24, resolution, clip)
                    else:
                        node = self._research_geometry(s, item_container, root, item+24, tree_clip)
                    node.update(panel=panel, identity=key, scope=scope if panel=='focus' else None,
                                visible=shown and node['visible'],
                                folder=item_folder if panel=='research' else None)
                    nodes.append(node)
        active = [key for key, value in panels.items() if value['visible']]
        if len(active) > 1:
            raise LiveReadError('Viste sovrapposte: geometria non esclusiva')
        events = read_event_ui(self, s, root, event_owners, set(event_ids), resolution)
        event_visible = bool(events['event_windows'])
        panels['event'] = {'visible': event_visible, 'clip': [0, 0, *resolution], 'status': events['status']}
        if event_visible:
            active = ['event']
        nodes.extend(events['nodes'])
        s.verify()
        return {'available': True, 'source': 'native_gui', 'pid': p.pid, 'process_created': p.created,
                'country': country, 'scope': scope, 'resolution': resolution,
                'active_panel': active[0] if active else None, 'panels': panels, 'nodes': nodes, 'tabs': tabs,
                'events': {'status': events['status'], 'visible_ids': events['visible_ids']},
                'event_windows': events['event_windows'],
                'diagnostics': {'snapshot_ms': (time.perf_counter()-started)*1000,
                                'discovery_ms': self.discovery_ms, 'bytes': s.used}}
