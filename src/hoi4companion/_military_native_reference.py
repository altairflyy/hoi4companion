"""Build-qualified experience and has_war observations; external reads only.

Static evidence and remaining runtime limits: docs/military-live.md.
"""
import struct

from .live import LiveReadError


def _qualified(reader):
    layout=getattr(reader.process.profile,'military_layout',None)
    if not isinstance(layout,dict):
        raise LiveReadError('Military state layout is not qualified for this executable.')
    for rva,signature in layout['signatures']:
        if reader.process.profile.bytes(rva,len(signature))!=signature:
            raise LiveReadError('Military state signature does not match the executable.')
    reader.typed(reader.country,'CCountry')
    if reader.integer(reader.country+8,8191)!=reader.tag:
        raise LiveReadError('Country identity changed while reading military state.')
    return layout


def read_experience(reader):
    layout=_qualified(reader)
    status=reader.typed(reader.pointer(reader.country+layout['experience_offset']),'CCountryExperienceStatus')
    if reader.pointer(status+8)!=reader.country:
        raise LiveReadError('Experience belongs to another country.')
    amounts={}
    for name,offset in (('army',0x10),('navy',0x28),('air',0x40)):
        raw=struct.unpack('<q',reader.track(status+offset,8))[0]
        if not 0<=raw<=10_000_000*32768:
            raise LiveReadError('Military experience is outside its qualified range.')
        amounts[name]=raw/32768
    return {'status':'observed',**amounts}


def read_diplomacy(reader):
    layout=_qualified(reader)
    status=reader.typed(reader.pointer(reader.country+layout['diplomacy_offset']),'CDiplomacyStatus')
    if reader.pointer(status+0x2D0)!=reader.country:
        raise LiveReadError('Diplomacy belongs to another country.')
    # Reproduce the read-only has_war(allow_non_major_war=false) getter. A
    # populated war list OR a pending CWarRelation is significant. Reading only
    # the cached count would incorrectly report peace during that transition.
    wars=reader.array(status+0x98,4096)
    at_war=bool(wars)
    if not at_war:
        for relation in reader.array(status+0x20,4096):
            token=reader.integer(relation+8,65535)
            if token==0x380A:
                reader.typed(relation,'CWarRelation')
                at_war=True
    result={'status':'observed','at_war':at_war}
    if getattr(reader.process.profile,'foreign_interactions_layout',None):
        from ._foreign_interactions_native_reference import read_relations
        try:result['interactions']=read_relations(reader,status)
        except (LiveReadError,OSError,ValueError,KeyError,struct.error) as exc:
            result['interactions']={'status':'unavailable','reason':str(exc)}
    return result
