"""Selected ideas and active advisor membership for the pinned native build.

Only observed identities are exported. English aliases, role names and traits
belong to the static catalog, not to guessed native enum values. Provenance and
coverage: docs/politics-live.md. Uses CountryReader's bounded witnessed reads.
"""
import struct

from .live import LiveReadError, MILITARY_HASH, UPDATED_HASH


# Current exact PE: country getter, owner assignment in CPoliticalStatus's
# constructor, and CHasIdeaTrigger's membership loop. See country-metrics-live.md.
CURRENT_IDEA_SIGNATURES = (
    (0x717370, bytes.fromhex('488b81900f0000c3')),
    (0xBFA327, bytes.fromhex('4889b3c8000000')),
    (0x3F4776, bytes.fromhex('e8f52b32004863485c85c90f8eb7fdffff488bd1488bcb488b40504839307413ffc348ffc14883c008483bca7cede995fdffff83fbff')),
)

# Current PE: CCountryCharacters getter/owner, active-advisor membership used by
# CIsHiredAsAdvisor's predicate, and CAdvisor/CCharacter identity accessors.
# See docs/politics-live.md and .local/politics-production-signatures.json.
CURRENT_ADVISOR_SIGNATURES = (
    (0x734AD0, bytes.fromhex('4881c1f00f0000e9a40afeff')),
    (0x2E9E50, bytes.fromhex('488d4158c3')),
    (0x14DD178, bytes.fromhex('488bcfe8507925ff488bc8e8c8cce0fe33d24863480c85c97e21488b004c8bc18bca660f1f4400004839187413ffc248ffc14883c008493bc87cedbaffffffff488b5c243083faff0f95c04883c4205f')),
    (0x11703EB, bytes.fromhex('488d05c6658901488901488951084883c110')),
    (0x132DA40, bytes.fromhex('488d0509536f0148890748895f104533f64c8977184c897720448977280f57c00f1147304c89774048c747480f00000044887730')),
    (0x12BB80, bytes.fromhex('488b4110c3')),
    (0xABC220, bytes.fromhex('488d4168c3')),
    (0x707BD0, bytes.fromhex('488d4130c3')),
    (0xD73120, bytes.fromhex('488d4148c3')),
)


def _ideas(r):
    current = getattr(r.process.profile, 'sha256', None) == MILITARY_HASH
    if current:
        for rva, signature in CURRENT_IDEA_SIGNATURES:
            if r.process.profile.bytes(rva, len(signature)) != signature:
                raise LiveReadError('Idea membership signature does not match the executable.')
    manager = r.typed(r.pointer(r.country + 0xF90), 'CPoliticalStatus')
    if current and r.pointer(manager + 0xC8) != r.country:
        raise LiveReadError('Idea membership belongs to another country.')
    rows, identities, law_groups = [], set(), set()
    for idea in r.array(manager + 0x50, 4096):
        r.typed(idea, 'CIdea')
        key = r.s.string(idea + 0x18)
        group = r.typed(r.pointer(idea + 0xAA0), 'CIdeaGroupType')
        category = r.typed(r.pointer(group + 0x60), 'CIdeaCategory')
        group_id = r.s.string(group + 0x38)
        category_id = r.s.string(category + 0x10)
        law_flag = r.s.number(group + 0x58, '<B')
        if law_flag not in (0, 1) or key in identities:
            raise LiveReadError('Identità o flag legge non valido')
        if law_flag and group_id in law_groups:
            raise LiveReadError('Più leggi selezionate nello stesso gruppo')
        identities.add(key)
        if law_flag:
            law_groups.add(group_id)
        rows.append({'id': key, 'group': group_id, 'category': category_id,
                     'is_law': bool(law_flag)})
    return {'status': 'observed', 'items': rows}


def _advisors(r):
    if getattr(r.process.profile, 'sha256', None) == MILITARY_HASH:
        for rva, signature in CURRENT_ADVISOR_SIGNATURES:
            if r.process.profile.bytes(rva, len(signature)) != signature:
                raise LiveReadError('Active advisor membership signature does not match the executable.')
    manager = r.typed(r.pointer(r.country + 0xFF0), 'CCountryCharacters')
    if r.pointer(manager + 8) != r.country:
        raise LiveReadError('Incarichi di un’altra nazione')
    rows, identities = [], set()
    # +40 lists available roles; only +58 is active appointment membership.
    for role in r.array(manager + 0x58, 4096):
        r.typed(role, 'CAdvisor')
        character = r.typed(r.pointer(role + 0x10), 'CCharacter')
        if r.integer(character + 0x68, 8191) != r.tag:
            raise LiveReadError('Personaggio di un’altra nazione')
        key, character_id = r.s.string(role + 0x30), r.s.string(character + 0x48)
        identity = (key, character_id)
        if identity in identities:
            raise LiveReadError('Identità incarico duplicata')
        identities.add(identity)
        rows.append({'id': key, 'character_id': character_id,
                     'slot': r.integer(role + 0x28)})
    return {'status': 'observed', 'items': rows}


def _power(r):
    manager=r.typed(r.pointer(r.country+0xF90),'CPoliticalStatus')
    amount=struct.unpack('<q',r.track(manager+0xE0,8))[0]
    if not -100_000_000_000<=amount<=100_000_000_000:
        raise LiveReadError('Potere politico fuori intervallo')
    return {'status':'observed','available':amount/100000}


def read_politics(reader):
    """Independently qualify laws/ideas and appointments; never publish a prefix."""
    result = {}
    for key, read in (('ideas', _ideas), ('advisors', _advisors), ('power',_power)):
        build=getattr(reader.process.profile,'sha256',None)
        if build == UPDATED_HASH and key != 'ideas' or build == MILITARY_HASH and key == 'power':
            result[key] = {'status':'unavailable','reason':'Sezione non ancora verificata per questa versione del gioco.'}
            continue
        try:
            if reader.integer(reader.country + 8, 8191) != reader.tag:
                raise LiveReadError('Identità della nazione cambiata')
            result[key] = read(reader)
        except (LiveReadError, OSError, ValueError, struct.error, KeyError) as exc:
            result[key] = {'status': 'unavailable', 'reason': str(exc)}
    result['status'] = ('observed' if all(x['status'] == 'observed' for x in result.values())
                        else 'partial' if any(x['status'] == 'observed' for x in result.values())
                        else 'unavailable')
    result['reason'] = 'Idee e leggi selezionate; incarichi attivi del paese. Nessuna disponibilità o costo inferito.'
    return result
