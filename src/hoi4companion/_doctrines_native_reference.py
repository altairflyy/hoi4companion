"""Exact-build, external read-only doctrine selection membership.

Mirrors CHasDoctrineTrigger's selected grand/subdoctrine pointers, not unlocked
or available definitions. No engine calls; see docs/doctrines-live.md.
"""
import hashlib
import struct

from .live import LiveReadError, MILITARY_HASH


_FUNCTIONS = (
    (0x572A30, 290, 'b3492d37117969070e59e594bf1ab16ee74661ff9d5eb9853d108d8af49b8066'),
    (0x1460C71, 27, '04b5b9b6d46a89a23923f7fe5650a5fec917b6a31b2e43d1729122c9e07fc632'),
    (0xDE4EB0, 55, 'f38fdc1da64da292cbc1bc4f8568c2e6464ab7cc19a53c7c9f9cf4bf170c2ee8'),
    (0xDE2099, 41, '95d18c3ed69e70881c97e20f519b3b0b149f946d6c715d22d6ddcd6b4bb4d8da'),
    (0x253EB99, 23, '318cb0f871a1f97ad8c157034a0e3c07dc650a9f9a1d5469080b5a6377be3c77'),
    (0x253EC1F, 11, '14b60f1d321877cd236aaf39e72fc8c4fe7365aa4a05d1e69596df088e279547'),
)


def _inline(reader, address, stride, maximum):
    data, capacity, count = struct.unpack('<Qii', reader.track(address, 16))
    if not 0 <= count <= capacity <= maximum or (count and not data):
        raise LiveReadError('Invalid doctrine membership container.')
    # Only headers and consulted fields are watched; unused progress fields
    # need not invalidate an unchanged selection.
    return [data + i * stride for i in range(count)]


def _selected(reader):
    profile, base = reader.process.profile, reader.process.base
    if getattr(profile, 'sha256', None) != MILITARY_HASH:
        raise LiveReadError('Doctrine selections are not qualified for this executable.')
    for rva, size, digest in _FUNCTIONS:
        if hashlib.sha256(profile.bytes(rva, size)).hexdigest() != digest:
            raise LiveReadError('Doctrine signature does not match the executable.')
    reader.typed(reader.world, 'CCurrentGameState')
    reader.typed(reader.country, 'CCountry')
    if (reader.pointer(base + profile.world_rva) != reader.world or
            reader.integer(reader.world + 0x520, 8191) != reader.tag or
            reader.integer(reader.country + 8, 8191) != reader.tag):
        raise LiveReadError('Doctrine session identity changed.')
    # Aliased nation tags may legitimately share a country index.
    mapping = reader.s.array(reader.world + 0x340, 4, 8192)
    reader.track(reader.world + 0x340, 16)
    if mapping:
        reader.track(reader.pointer(reader.world + 0x340), len(mapping) * 4)
    countries = reader.array(reader.world + 0x310, 8192)
    if (not 0 < reader.tag < len(mapping) or
            not 0 <= mapping[reader.tag] < len(countries) or
            countries[mapping[reader.tag]] != reader.country):
        raise LiveReadError('Doctrine country mapping is invalid.')
    system = reader.typed(reader.pointer(reader.world + 0x400), 'CDoctrineSystem@NDoctrines')
    statuses = _inline(reader, system + 8, 0xA0, 8192)
    index = mapping[reader.tag]
    if index >= len(statuses):
        # The native getter falls back to the first record. An external observer
        # must instead refuse another country's record.
        raise LiveReadError('Doctrine country record is missing.')
    status = reader.typed(statuses[index], 'CCountryDoctrineStatus@NDoctrines')
    if reader.integer(status + 8, 8191) != reader.tag:
        raise LiveReadError('Doctrine record belongs to another country.')
    rows, identities, templates = [], set(), set()

    def selected(definition, kind):
        reader.typed(definition, kind + '@NDoctrines')
        if reader.track(base + 0x368F570, 1) != b'\x01':
            raise LiveReadError('Doctrine atom table is not initialized.')
        maximum = reader.integer(base + 0x368F574, 1_000_000)
        token = reader.integer(definition + 8, maximum, 1)
        atoms = reader.pointer(base + 0x368F5A0)
        key = reader.s.string(atoms + token * 0x20)
        if key in identities:
            raise LiveReadError('Duplicate selected doctrine identity.')
        identities.add(key)
        rows.append({'id': key})

    for folder in _inline(reader, status + 0x10, 0x50, 64):
        reader.typed(folder, 'CFolderStatus@NDoctrines')
        template = reader.typed(reader.pointer(folder + 8), 'CFolderTemplate@NDoctrines')
        if template in templates:
            raise LiveReadError('Duplicate doctrine folder.')
        templates.add(template)
        grand = reader.pointer(folder + 0x10)
        if grand:
            selected(grand, 'CGrandDoctrineTemplate')
        for track in _inline(reader, folder + 0x18, 0x60, 256):
            reader.typed(track, 'CTrackStatus@NDoctrines')
            sub = reader.pointer(track + 8)
            if sub:
                if not grand:
                    raise LiveReadError('Selected subdoctrine has no selected grand doctrine.')
                selected(sub, 'CSubDoctrineTemplate')
    return {'status': 'observed', 'items': rows}


def read_doctrines(reader):
    """Return the complete qualified selection or no selection data at all."""
    try:
        return _selected(reader)
    except (LiveReadError, OSError, ValueError, struct.error, KeyError) as exc:
        return {'status': 'unavailable', 'reason': str(exc)}
