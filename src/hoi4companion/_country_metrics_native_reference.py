"""External read-only effective war support for one exact executable.

No native calls. Context-dependent modifier paths fail closed.
Static provenance and runtime qualification limits: docs/country-metrics-live.md.
The layout requires explicit profile registration; importing it enables nothing.
"""
import hashlib
import struct

from .live import LiveReadError, MILITARY_HASH


COUNTRY_METRICS_LAYOUT = {'version': 1, 'path': 'effective-war-support'}
_FUNCTIONS = (
    (0x40BC70, 0x4F, '7c9967e9d99bf4cbe524ab9a4adb184b03910047726718d8f258e7389ab53727'),
    (0x741BE0, 0x15F8, '2ac5844aecbdb116f9f7c6990715b59bc1a51b1703d2846a2ff5840f394380fe'),
    (0x1F1FB0, 0x25, 'b0c01a4d42eefba0e06f60207c4f35c3d8521a769ff87bd67959f323268033e8'),
    (0x1DD900, 0x78, '2cb254c625879dd131bd760cea304f96f212b521285383928e47828667eb5c44'),
    (0x15B31D0, 0xBE, '85d9163d4c0835930cb8c5b0551eeeb23555c0bf769d14799ef003d49444c5b2'),
    (0x15B1A30, 0x31, 'f848324bd5bc0e134fa4d92ac9adfb836cbd74bf0d3e41e5761d051e0bab28ea'),
    (0x15B1A70, 0xC4, 'eb82c8a8242930ee3a18a12465cc2829a53877a44369f5a2573fd5c2c276fb36'),
    (0x15B2E50, 0x290, '236d80ad491aa5dd7ddd51dd624fa33170614c57f5e94d239a4cd4b3508e04c8'),
    (0x2572AE0, 0x57, 'cd50a39087b2738618b3ace767cd15a0b17adc2dbcaa3954d52a0fa626c8b1de'),
    (0x2572C70, 0x138, '60a7379faa0c45a2a46f40c8a69d0796e277e95803c9f27bc5cc8cf845ec41ca'),
    (0x5A51A0, 0x10B, '4de3048ac47a29e219da059ab711a07edf19e5cd81cdcc1eb93c6c53effdc37c'),
    (0xF81FC0, 0xEE, '5f668cf1d31e1daed2db7f973c89c86562730a7182567d589fdb6c50559fca62'),
    (0xDA4470, 0x87, '8a15912f99e6d9df42aa5b58a06000284f41b9d1e8a2a1758a71ae26d29a15cb'),
    (0xDA3190, 0x88, '0582baf2d9aae7cd7105d8aae2608c4476f6748d69c682acd202387b2acb98a1'),
    (0xDA2F50, 0x88, '0ab91fad6f1b8670df4e00e5c0bd5c969d2f61911da2c6d2c4e6c7b07fe78b75'),
    (0xD84980, 0x14, '60bc10f2876de35c5cf51b3d446e8a1540cdd88e633b0a5796416ae04855bfcb'),
    (0xD839D0, 0x14, '00e906037dc4abaddc92f4158e9e14b84f1f6eb182a4de339266100aa22689ab'),
    (0xC08580, 0x141, '24f9af80318ebe381cf2dc1a0c16b228c1ea2b1bb84204451ff9673ca0778903'),
)


def _trunc_div(value, divisor):
    return (1 if value >= 0 else -1) * (abs(value) // divisor)


def _to_q15(value):
    # 2572AE0: integer and fractional quotients both truncate toward zero.
    return _trunc_div(value, 100000) * 32768 + _trunc_div(
        (value - _trunc_div(value, 100000) * 100000) * 32768, 100000)


def _from_q15(value):
    # 2572C70: arithmetic shift plus rounded nonnegative remainder. This is
    # deliberately not float division or round(), especially for negatives.
    integer = value >> 15
    return integer * 100000 + ((value - integer * 32768) * 100000 + 16384) // 32768


def _integer64(reader, address, limit=10**10):
    value = struct.unpack('<q', reader.track(address, 8))[0]
    if abs(value) > limit:
        raise LiveReadError('Country metric exceeds its qualified arithmetic range.')
    return value


def _qualified(reader):
    profile = reader.process.profile
    if (profile.sha256 != MILITARY_HASH or
            getattr(profile, 'country_metrics_layout', None) != COUNTRY_METRICS_LAYOUT):
        raise LiveReadError('Effective country metrics are not qualified for this executable.')
    for rva, size, digest in _FUNCTIONS:
        if hashlib.sha256(profile.bytes(rva, size)).hexdigest() != digest:
            raise LiveReadError('Country metric signature does not match the executable.')
    reader.typed(reader.world, 'CCurrentGameState')
    reader.typed(reader.country, 'CCountry')
    if (reader.pointer(reader.process.base + profile.world_rva) != reader.world or
            reader.integer(reader.world + 0x520, 8191) != reader.tag or
            reader.integer(reader.country + 8, 8191) != reader.tag):
        raise LiveReadError('Country metric session identity changed.')
    # The primary selected country mapping must still point at this object.
    mapping = reader.s.array(reader.world + 0x340, 4, 8192)
    reader.track(reader.world + 0x340, 16)
    if mapping:
        reader.track(reader.pointer(reader.world + 0x340), 4 * len(mapping))
    countries = reader.array(reader.world + 0x310, 8192)
    if (not 0 <= reader.tag < len(mapping) or
            not 0 <= mapping[reader.tag] < len(countries) or
            countries[mapping[reader.tag]] != reader.country):
        raise LiveReadError('Country metric belongs to another country.')
    return mapping, countries


def _history_sum(reader):
    history = reader.typed(reader.pointer(reader.country + 0xFC8), 'CLoopHistory')
    ring = reader.typed(reader.pointer(history + 0x10), 'CLoopHistoryContainer')
    if reader.pointer(ring + 0x38) != history:
        raise LiveReadError('Country metric history has another owner.')
    rows = reader.array(ring + 8, 8192)
    capacity, head = struct.unpack('<ii', reader.track(ring + 0x20, 8))
    wrapped = reader.track(ring + 0x28, 1)[0]
    if (capacity != len(rows) or wrapped not in (0, 1) or
            not (0 <= head < capacity if capacity else head == 0 and not wrapped)):
        raise LiveReadError('Country metric history ring is invalid.')
    total = 0
    for index in range(capacity if wrapped else head):
        row = rows[(head + index) % capacity] if wrapped else rows[index]
        if reader.pointer(row + 0x18) != history:
            raise LiveReadError('Country metric history row has another owner.')
        data, row_capacity, count = struct.unpack('<Qii', reader.track(row, 16))
        if not data or not 2 <= count <= row_capacity <= 512:
            raise LiveReadError('Country metric history row is incomplete.')
        total += _integer64(reader, data + 8)
        # 15B31D0 saturates after every row. Conservative input range guards
        # avoid machine overflow before reproducing the native saturation.
        total = max(-0x53E2D620FB40, min(0x53E2D620FB40, total))
        if abs(total) > 10**10:
            raise LiveReadError('Country metric history exceeds its qualified range.')
    return total


def _modifier(reader, index):
    data, capacity, count = struct.unpack('<Qii', reader.track(reader.country + 0x5B8, 16))
    if not 0 <= count <= capacity <= 4096 or (count and not data):
        raise LiveReadError('Country metric modifier container is invalid.')
    rows = reader.track(data, count * 16) if count else b''
    last, found = -1, None
    for offset in range(0, len(rows), 16):
        identifier, _, value = struct.unpack_from('<IIq', rows, offset)
        if identifier <= last:
            raise LiveReadError('Country metric modifiers are not unique and ordered.')
        last = identifier
        if identifier == index:
            found = value
    if found is None:
        return 0  # Exact early return of 5A51A0; no definition consulted.
    definitions = reader.pointer(reader.process.base + 0x33DD1D0)
    flags = struct.unpack('<I', reader.track(definitions + index * 0x78 + 0x60, 4))[0]
    context = struct.unpack('<I', reader.track(definitions + index * 0x78 + 0x68, 4))[0]
    if context:
        raise LiveReadError('Country metric modifier requires an unqualified native context.')
    if abs(found) > 10**10:
        raise LiveReadError('Country metric modifier is outside its qualified range.')
    return max(0, min(100000, found)) if flags & 0x10 else found


def _war_addition(reader, mapping, countries):
    diplomacy = reader.typed(reader.pointer(reader.country + 0xF88), 'CDiplomacyStatus')
    if reader.pointer(diplomacy + 0x2D0) != reader.country:
        raise LiveReadError('Country metric diplomacy has another owner.')
    at_war = defensive = offensive = False
    for relation in reader.array(diplomacy + 0x20, 4096):
        if reader.integer(relation + 8, 65535) != 0x380A:
            continue
        reader.typed(relation, 'CWarRelation')
        own = reader.integer(relation + 0x10, 8191, 1)
        other = reader.integer(relation + 0x14, 8191, 1)
        swapped = reader.track(relation + 0x158, 1)[0]
        if (swapped not in (0, 1) or max(own, other) >= len(mapping) or
                not 0 <= mapping[own] < len(countries) or
                not 0 <= mapping[other] < len(countries) or
                mapping[reader.tag] not in (mapping[own], mapping[other]) or
                mapping[own] == mapping[other]):
            raise LiveReadError('Country metric war relation identity is invalid.')
        # C08580 equates aliases by the country's world mapping index.
        defender = own if swapped else other
        attacker = other if swapped else own
        defensive |= mapping[defender] == mapping[reader.tag]
        offensive |= mapping[attacker] == mapping[reader.tag]
        # Relations are shared by both participating countries. DA4470 still
        # resolves their fixed +14 end, which can be the selected country.
        right_country = reader.typed(countries[mapping[other]], 'CCountry')
        native_tag = reader.integer(right_country + 8, 8191, 1)
        if native_tag >= len(mapping) or mapping[native_tag] != mapping[other]:
            raise LiveReadError('Country metric war opponent identity is invalid.')
        opponent_diplomacy = reader.typed(reader.pointer(right_country + 0xF88), 'CDiplomacyStatus')
        if reader.pointer(opponent_diplomacy + 0x2D0) != right_country:
            raise LiveReadError('Country metric opponent diplomacy has another owner.')
        # DA4470(allow_non_major_war=true) consults relation+14's country,
        # testing diplomacy+1F0 <= 0. Its cached war count is not sufficient.
        at_war |= reader.integer(opponent_diplomacy + 0x1F0, minimum=-2_000_000_000) <= 0
    total = 0
    if at_war and defensive:
        total += _integer64(reader, reader.process.base + 0x33E2C48, 10**7)
    if at_war and offensive:
        total += _integer64(reader, reader.process.base + 0x33E2CE0, 10**7)
    return total


def read_metrics(reader):
    """Return support from the statically reconstructed effective getter.

    The caller must verify NativeSample before publication, as collect_country
    does for all sections. Every mutable dependency joins reader.watch.
    Stability and context-dependent modifiers remain unsupported.
    """
    mapping, countries = _qualified(reader)
    value = _integer64(reader, reader.country + 0x10D8)
    value += _war_addition(reader, mapping, countries)
    threat = reader.typed(reader.pointer(reader.world + 0x6B0), 'CWorldThreat')
    tension = _integer64(reader, threat + 8, 100000)
    if tension < 0:
        raise LiveReadError('World tension is outside its qualified range.')
    base = reader.process.base
    value += _trunc_div(tension * _integer64(reader, base + 0x33E2D70, 10**7), 100000)
    value += _from_q15(_integer64(reader, reader.country + 0x14C8))
    history_q15 = _to_q15(_history_sum(reader))
    history_q15 = (history_q15 * _integer64(reader, base + 0x33DF5F0, 32768 * 1000)) >> 15
    history_value = _from_q15(history_q15)
    limit = _integer64(reader, base + 0x33DF678, 10**7)
    if limit < 0:
        raise LiveReadError('War support history limit is invalid.')
    value += max(-limit, min(limit, history_value))
    for offset in (0x14D0, 0x14D8, 0x14E0):
        value += _from_q15(_integer64(reader, reader.country + offset))
    value += _modifier(reader, 149)
    upper = _integer64(reader, base + 0x33E4528)
    lower = _integer64(reader, base + 0x33E45D8)
    if (lower, upper) != (0, 100000):
        raise LiveReadError('War support bounds are outside the qualified 0..1 scale.')
    return {'status': 'observed', 'war_support': max(lower, min(upper, value)) / 100000}
