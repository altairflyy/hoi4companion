"""Read-only event-popup HUD observations from rendered native grid rows.

Candidate addresses (including optional live-reader hints) are never evidence.
Every emitted window/choice is bound to the current GUI root, event instance,
rendered row, native option definition and current parent transform.
"""
import struct
import re
from .live import LiveReadError


def _hint_owner(value):
    text = value.get('instance_id') if isinstance(value, dict) else value
    if not isinstance(text, str) or not re.fullmatch('[0-9a-fA-F]{1,16}', text):
        raise LiveReadError('Candidato finestra evento non valido')
    try:
        address = int(text, 16)
    except ValueError as exc:
        raise LiveReadError('Candidato finestra evento non valido') from exc
    if not 0x10000 <= address < 0x7FFFFFFFFFFF:
        raise LiveReadError('Candidato finestra evento fuori intervallo')
    return address


def refresh_candidates(ui, root, bindings, hints=()):
    """Inspect only new registry entries when its occupancy changes.

    This is candidate discovery outside the final witnessed sample. It avoids
    the expensive full GUI-object scan on each frame. Absence is deliberately
    unknown when no event window has ever been qualified in this GUI context.
    """
    process = ui.process
    hints = tuple(hints)
    if len(hints) > 256:
        raise LiveReadError('Troppi candidati finestra evento')
    state = bindings.setdefault('event_registry', {
        'headers': bindings.get('registry_headers'),
        'seen': set(bindings.get('registry_seen', ())),
        'owners': set(bindings.get('events', ()))})
    headers = tuple(process.read(root+offset, 16) for offset in (0x108, 0x120))
    previous = state['headers']
    pages = {}
    def pointer(address):
        page = address & ~4095
        if address+8 > page+4096:
            return struct.unpack('<Q', process.read(address, 8))[0]
        if page not in pages:
            pages[page] = process.read(page, 4096)
        return struct.unpack_from('<Q', pages[page], address-page)[0]
    for index, raw in enumerate(headers):
        data, capacity, count = struct.unpack('<Qii', raw)
        if not 0 <= count <= capacity <= 500000:
            raise LiveReadError('Registro GUI eventi fuori intervallo')
        if previous and raw == previous[index]:
            continue
        old = struct.unpack('<Qii', previous[index]) if previous else None
        # Static registry appends; dynamic registry reuses holes. Read only
        # static tail when its allocation is unchanged, dynamic pointer array
        # otherwise. RTTI is inspected only for newly encountered pointers.
        start = old[2] if index == 0 and old and old[:2] == (data, capacity) and count >= old[2] else 0
        extent = count if index == 0 else capacity
        for offset in range(start, extent, 8192):
            raw_entries = process.read(data+offset*8, min(8192, extent-offset)*8)
            for (gui,) in struct.iter_unpack('<Q', raw_entries):
                if not gui or gui in state['seen']:
                    continue
                state['seen'].add(gui)
                if len(state['seen']) > 1000000:
                    raise LiveReadError('Inventario candidati GUI eventi oltre limite')
                try:
                    observer = pointer(gui+80)
                    name, displacement = ui._rtti(pointer(observer))
                    if name == '.?AVCEventWindow@@':
                        state['owners'].add(observer-displacement)
                except (LiveReadError, UnicodeError, struct.error, OSError):
                    continue
    state['headers'] = headers
    owners = state['owners'] | {_hint_owner(value) for value in hints}
    if len(owners) > 256:
        raise LiveReadError('Troppe finestre eventi candidate')
    return sorted(owners)


def read_event_ui(ui, s, root, owners, requested_ids, resolution):
    """Return popup metadata for occlusion, and only requested rendered choices."""
    from .focus_ui_native import _intersect, geometry
    screen = [0, 0, *resolution]
    windows, nodes = [], []
    for owner in owners:
        ui._typed(s, owner, 'CEventWindow')
        container = s.number(owner+0x50)
        shown = ui._container(s, container, root, owner+56)
        event = s.number(owner+0x68)
        if not shown:
            continue
        if not event:
            raise LiveReadError('Evento visibile senza identità nativa')
        ui._typed(s, event, 'CEvent')
        event_id = s.string(event+0x20)
        position = ui._typed(s, s.number(container+0x15F0), 'C2dPositionObject')
        current_resolution = list(struct.unpack('<ii', s.read(position+0x130, 8)))
        if current_resolution != resolution:
            raise LiveReadError('Risoluzione evento differente dal contesto GUI')
        transform = bytearray(0xD0); transform[0x90:0xD0] = s.read(position+0x90, 64)
        layout = bytearray(0x15B4); layout[0x15AC:0x15B4] = s.read(container+0x15AC, 8)
        popup = geometry(transform, layout, screen)
        native_clip = struct.unpack('<4i', s.read(container+0x1594, 16))
        clip = _intersect(popup['clip'], [native_clip[0], native_clip[1],
                                        native_clip[2]-native_clip[0], native_clip[3]-native_clip[1]])
        instance = format(owner, 'x')
        windows.append({'event_id': event_id, 'instance_id': instance, 'rect': popup['rect'],
                        'clip': clip, 'visible': True})
        if event_id not in requested_ids:
            continue
        definitions = s.array(event+0x368, maximum=256)
        definition_set = set(definitions)
        grid = ui._typed(s, s.number(owner+0x1280), 'CStandardGridBox')
        if s.number(grid+0x68) != root or s.number(grid+0x50) != owner+56:
            raise LiveReadError('Griglia scelte di un altro evento')
        seen_indices, seen_options = set(), set()
        for row in s.array(grid+0x1A0, maximum=256):
            ui._typed(s, row, 'CStandardGridBoxItem')
            item = ui._typed(s, s.number(row+0x10), 'CContainerWindow')
            # The event constructor binds each row's GUI observer to the same
            # event-window observer. Buttons retain their exact option pointer.
            row_shown = ui._container(s, item, root, owner+56)
            parent = ui._typed(s, s.number(item+0x15F0), 'C2dPositionObject')
            matrix = struct.unpack('<16f', s.read(parent+0x90, 64))
            buttons = s.array(item+0x10D8, maximum=64)
            if not buttons:
                raise LiveReadError('Scelta evento priva di pulsante nativo')
            row_options, row_nodes = set(), []
            for button in buttons:
                ui._typed(s, button, 'CButtonDrag')
                if s.number(button+0x68) != root or s.number(button+0x50) != owner+56:
                    raise LiveReadError('Pulsante scelta di un altro evento')
                option = ui._typed(s, s.number(button+8), 'CEventOption')
                if option not in definition_set:
                    raise LiveReadError('Scelta estranea all’evento visualizzato')
                row_options.add(option)
                if s.number(button+0x110) != parent:
                    raise LiveReadError('Ancoraggio pulsante evento non qualificato')
                bounds = ui._sprite_bounds(s, s.number(button+0x118), parent, matrix, clip)
                bounds['visible'] = bounds['visible'] and row_shown and s.shown(button+0x75)
                row_nodes.append(bounds)
            if len(row_options) != 1:
                raise LiveReadError('Riga evento associata a opzioni diverse')
            option = row_options.pop()
            index = s.number(option+0x1F0, '<i')
            identifier = s.string(option+0x178)
            if not 0 <= index <= 255 or index in seen_indices or identifier in seen_options:
                raise LiveReadError('Indice o identità scelta evento duplicati')
            seen_indices.add(index); seen_options.add(identifier)
            if len(row_nodes) != 1:
                raise LiveReadError('Geometria scelta evento ambigua')
            node = row_nodes[0]
            node.update(panel='event', identity=event_id+':'+identifier, event_id=event_id,
                        option_id=identifier, option_index=index, instance_id=instance)
            nodes.append(node)
    # Closed known instances are observed as hidden, but candidate discovery
    # does not prove global absence of another, newly allocated popup.
    return {'status': 'observed' if windows else 'unknown', 'event_windows': windows, 'nodes': nodes,
            'visible_ids': sorted({window['event_id'] for window in windows})}
