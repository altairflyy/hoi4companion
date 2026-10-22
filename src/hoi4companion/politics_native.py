"""Selected ideas and active advisor membership for the pinned native build.

Only observed identities are exported. English aliases, role names and traits
belong to the static catalog, not to guessed native enum values. Provenance and
coverage: docs/politics-live.md. Uses CountryReader's bounded witnessed reads.
"""
import struct

from .live import LiveReadError


def _ideas(r):
    manager = r.typed(r.pointer(r.country + 0xF90), 'CPoliticalStatus')
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


def read_politics(reader):
    """Independently qualify laws/ideas and appointments; never publish a prefix."""
    result = {}
    for key, read in (('ideas', _ideas), ('advisors', _advisors)):
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
