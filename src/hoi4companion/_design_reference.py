"""Exact-build, external read-only equipment design observations.

The complete native module/upgrade containers are required. A failed row never
escapes as a prefix or an empty design. See docs/design-live.md for provenance.
"""
import struct

from .live import LiveReadError, MILITARY_HASH


SIGNATURES = (
    (0xC39763, bytes.fromhex('498d4e68e8043a100084c075114d8d4668ba69300000')),
    (0xC39781, bytes.fromhex('4183bec400000000741fba6b3b0000488bcde808d49001498d96b8000000')),
    (0x19BC893, bytes.fromhex('488b1f48637f0c48c1e7044803fb')),
    (0x19BC8E1, bytes.fromhex('4c8b43084d85c074154180781000740e458b4008488bce8b13')),
    (0x1013E47, bytes.fromhex('488b770848636f1448c1e5044803ee')),
    (0x1013E70, bytes.fromhex('488b0e440fb676088b4908e810ad5201')),
    (0x1013EDC, bytes.fromhex('4883c610483bf5758b')),
    (0x253EB99, bytes.fromhex('803dd009150100')),
    (0x253EBA7, bytes.fromhex('3b1dc7091501')),
    (0x253EC1F, bytes.fromhex('48c1e30548031d76091501')),
)
TOKEN_INITIALIZED = 0x368F570
TOKEN_MAXIMUM = 0x368F574
TOKEN_TABLE = 0x368F5A0
MAX_ENTRIES = 256  # Reader admission bound, not a claimed engine limit.


def _token(reader, token):
    base = reader.process.base
    if reader.track(base + TOKEN_INITIALIZED, 1) != b'\1':
        raise LiveReadError('Equipment design token table is not initialized.')
    maximum = reader.integer(base + TOKEN_MAXIMUM, 1_000_000)
    if not 0 <= token <= maximum:
        raise LiveReadError('Equipment design identifier is outside the token table.')
    address = reader.pointer(base + TOKEN_TABLE) + token * 32
    value = reader.s.string(address)
    # Token labels and their storage are also part of the change probe.
    header = reader.track(address, 32)
    length, capacity = struct.unpack_from('<QQ', header, 16)
    data = address if capacity == 15 else struct.unpack_from('<Q', header)[0]
    reader.track(data, length + 1)
    return value


def _entries(reader, address):
    data, capacity, count = struct.unpack('<Qii', reader.track(address, 16))
    if not 0 <= count <= capacity <= MAX_ENTRIES or count and not data:
        raise LiveReadError('Equipment design container is invalid or exceeds the admission bound.')
    return reader.track(data, count * 16) if count else b''


def _read(reader, variant):
    profile = reader.process.profile
    if profile.sha256 != MILITARY_HASH or not callable(getattr(profile, 'bytes', None)):
        raise LiveReadError('Equipment design layout is not qualified for this executable.')
    for rva, signature in SIGNATURES:
        if profile.bytes(rva, len(signature)) != signature:
            raise LiveReadError('Equipment design executable signature does not match.')
    reader.typed(variant, 'CEquipmentVariant')
    reader.typed(reader.pointer(variant + 0x3F0), 'CEquipmentType')
    reader.typed(variant + 0x68, 'CEquipmentUpgradesInstance')
    # Verify the two serializer field identities in this running token cache.
    if _token(reader, 0x3069) != 'upgrades' or _token(reader, 0x3B6B) != 'modules':
        raise LiveReadError('Equipment design field identities do not match.')
    modules, slots = {}, set()
    for offset in range(0, len(raw := _entries(reader, variant + 0xB8)), 16):
        slot_token, module = struct.unpack_from('<I4xQ', raw, offset)
        slot = _token(reader, slot_token)
        if slot in slots:
            raise LiveReadError('Duplicate equipment design module slot.')
        slots.add(slot)
        if not module:
            continue  # Serializer explicitly omits null module selections.
        reader.typed(module, 'CEquipmentModule')
        if reader.track(module + 0x10, 1) != b'\1':
            raise LiveReadError('Equipment design module definition is not initialized.')
        modules[slot] = _token(reader, reader.integer(module + 8, 1_000_000))
    upgrades, seen = {}, set()
    for offset in range(0, len(raw := _entries(reader, variant + 0x70)), 16):
        upgrade, level = struct.unpack_from('<QB', raw, offset)
        reader.typed(upgrade, 'CEquipmentUpgrade')
        key = _token(reader, reader.integer(upgrade + 8, 1_000_000))
        if key in seen:
            raise LiveReadError('Duplicate equipment design upgrade.')
        seen.add(key)
        upgrades[key] = level  # Native serializer uses movzx from uint8.
    return {'status': 'observed', 'modules': modules, 'upgrades': upgrades}


def read_design(reader, variant):
    """Read a variant reached through an owner-validated production line.

    The caller must finish NativeSample.verify before publishing its enclosing
    production sample. Foreign licensed designs retain their original creator;
    ownership is the production line's parent country, checked by that caller.
    """
    try:
        return _read(reader, variant)
    except (LiveReadError, OSError, ValueError, struct.error, KeyError) as exc:
        return {'status': 'unavailable', 'reason': str(exc)}
