"""Build-pinned, read-only relation and operative mission observations.

No commands or timers: changed bytes join the existing native watch plan.
"""
import struct
from .live import LiveReadError
from .foreign_interactions import MISSIONS, STATES

MISSION_CLASSES=('CNoMission','CBuildIntelNetwork','CQuietIntelNetwork','CCounterIntelligence',
                 'CRootOutResistance','CBoostIdeology','CControlTrade','CDiplomaticPressure','CPropaganda')


def _qualified(r):
    layout=getattr(r.process.profile,'foreign_interactions_layout',None)
    if not isinstance(layout,dict) or layout.get('version')!=1:
        raise LiveReadError('Foreign interactions are not qualified for this executable.')
    for address,expected in layout['signatures']:
        if r.process.profile.bytes(address,len(expected))!=expected:
            raise LiveReadError('Foreign interaction executable signature changed.')
    r.typed(r.country,'CCountry')
    if r.integer(r.country+8,8191,1)!=r.tag:raise LiveReadError('Foreign interaction country identity mismatch.')


def _countries(r):
    countries=r.array(r.world+0x310,8192)
    mapping=r.s.array(r.world+0x340,4,8192)
    r.track(r.world+0x340,16)
    if mapping:r.track(r.u(r.world+0x340),len(mapping)*4)
    tags=r.pointer(r.world+0x358);count=r.integer(r.world+0x364,8192,1)
    if count!=len(mapping):raise LiveReadError('Foreign country registry count mismatch.')
    if not 0<r.tag<count or not 0<=mapping[r.tag]<len(countries) or countries[mapping[r.tag]]!=r.country:
        raise LiveReadError('Foreign country registry owner mismatch.')
    import re
    names={};aliases={};checked=set()
    for index in range(1,count):
        slot=mapping[index]
        if slot==-1:continue
        if not 0<=slot<len(countries):raise LiveReadError('Invalid foreign country mapping.')
        country=countries[slot]
        if country not in checked:
            r.typed(country,'CCountry');native=r.integer(country+8,8191,1)
            if native>=count or mapping[native]!=slot:raise LiveReadError('Foreign country reciprocal identity mismatch.')
            checked.add(country)
        name=r.s.string(tags+index*32)
        if not re.fullmatch('[A-Z0-9]{3}',name):raise LiveReadError('Invalid foreign country tag.')
        names[index]=name;aliases.setdefault(slot,[]).append(name)
    # Civil-war tags may be reused in the registry. Equal text is a usable
    # alias only when every occurrence resolves to the same country object.
    slots_by_name={}
    for index,name in names.items():slots_by_name.setdefault(name,set()).add(mapping[index])
    ambiguous={name for name,slots in slots_by_name.items() if len(slots)>1}
    names={index:name for index,name in names.items() if name not in ambiguous}
    if r.tag not in names:raise LiveReadError('Played country tag is ambiguous.')
    aliases={slot:sorted({name for name in values if name not in ambiguous}) for slot,values in aliases.items()}
    return mapping,names,aliases


def read_relations(r,status):
    _qualified(r)
    r.typed(status,'CDiplomacyStatus')
    if r.pointer(status+0x2D0)!=r.country:raise LiveReadError('Foreign diplomacy owner mismatch.')
    mapping,names,aliases=_countries(r);own=mapping[r.tag]
    improved=set();pacts=set()
    classes={'.?AVCImproveRelationsRelation@@':(0x2CD7,improved),'.?AVCNonAggressionPactRelation@@':(0x2FBC,pacts)}
    for relation in r.array(status+0x20,4096):
        kind=r.process.profile.class_name(r.u(relation)-r.process.base)
        native_token=r.integer(relation+8,65535)
        if kind not in classes:
            if native_token in (0x2CD7,0x2FBC):raise LiveReadError('Diplomatic relation token/class mismatch.')
            continue
        expected,values=classes[kind]
        if native_token!=expected:raise LiveReadError('Diplomatic relation type/token mismatch.')
        left,right=struct.unpack('<ii',r.track(relation+0x10,8))
        cancelled=r.track(relation+0x48,1)[0]
        if cancelled not in (0,1) or left not in names or right not in names or mapping[left]==mapping[right] or own not in (mapping[left],mapping[right]):
            raise LiveReadError('Diplomatic relation endpoint identity mismatch.')
        if cancelled:continue
        # Improving our opinion of another country is directional; a pact is bilateral.
        if expected==0x2CD7 and mapping[left]!=own:continue
        other=right if mapping[left]==own else left
        values.update(aliases[mapping[other]])
    return {'status':'observed','countries':sorted(set(names.values())),'improving_relations':sorted(improved),'non_aggression_pacts':sorted(pacts)}


def read_intelligence(r):
    _qualified(r)
    agency=r.typed(r.pointer(r.country+0xFC0),'CIntelligenceAgency')
    if r.pointer(agency+0x78)!=r.country:raise LiveReadError('Intelligence agency owner mismatch.')
    created,creating=r.track(agency+0xC0,2)
    if created not in (0,1) or creating not in (0,1):raise LiveReadError('Invalid intelligence creation flags.')
    result={'status':'observed','is_created':bool(created),'in_creation':bool(creating)}
    try:
        mapping,names,aliases=_countries(r);items=[];identities=set()
        for agent in r.array(agency+0xF0,4096):
            r.typed(agent,'COperativeLeader')
            handle,sequence=struct.unpack('<II',r.track(agent+8,8))
            identity=f'{handle:x}:{sequence:x}'
            if handle!=0x1269 or sequence==0 or identity in identities:raise LiveReadError('Invalid operative identity.')
            identities.add(identity)
            state=r.integer(agent+0x1080,5)
            mission=r.typed(agent+0x1088,'COperativeMission')
            if r.pointer(mission+8)!=agent:raise LiveReadError('Operative mission belongs to another agent.')
            kind=r.integer(mission+0x18,8)
            data=r.pointer(mission+0x10)
            if r.process.profile.class_name(r.u(data)-r.process.base)!=f'.?AV{MISSION_CLASSES[kind]}@NOperativeMissions@@':
                raise LiveReadError('Operative mission enum/class mismatch.')
            target=None
            if kind:
                if r.pointer(data+8)!=mission:raise LiveReadError('Mission data belongs to another mission container.')
                tag=r.integer(data+0x10,8191,1)
                if tag not in names:raise LiveReadError('Operative mission target is unavailable.')
                target=names[tag]
            items.append({'id':identity,'mission_id':MISSIONS[kind],'target_country':target,'state':STATES[state]})
        result['missions']={'status':'observed','countries':sorted(set(names.values())),'items':items}
    except (LiveReadError,OSError,ValueError,KeyError,struct.error) as exc:
        result['missions']={'status':'unavailable','reason':str(exc)}
    return result
