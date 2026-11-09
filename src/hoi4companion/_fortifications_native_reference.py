"""Read already-qualified provincial building/status layouts, never queue totals.

Enabled only through the existing per-build country-section capability gate.
New executable profiles do not inherit these offsets automatically.
"""
import re
import struct
from .live import LiveReadError


def read(reader):
    r=reader
    registry=r.typed(r.u(r.process.base+getattr(r.process.profile,'building_database_rva',0x332EE28)),'CBuildingDatabase')
    definitions={}
    for ordinal,definition in enumerate(r.array(registry+0x40,512)):
        if not definition:continue
        if r.process.profile.class_name(r.u(definition)-r.process.base)!='.?AVCBuildingTemplate@@':continue
        key=r.s.string(definition+0x270)
        if key in ('bunker','coastal_bunker'):
            if key in definitions:raise LiveReadError('Duplicate provincial fort definition')
            slot=r.integer(definition+0x2E0,511)
            if slot!=ordinal:raise LiveReadError('Fort definition registry mismatch')
            definitions[key]=(definition,slot)
    if not definitions:raise LiveReadError('Provincial fort definitions unavailable')
    data,capacity,count=struct.unpack('<Qii',r.track(r.world+0x2B0,16))
    if not 0<count<=capacity<=20000 or not data:raise LiveReadError('Invalid province registry')
    state_data,state_capacity,state_count=struct.unpack('<Qii',r.track(r.world+0x2C8,16))
    if not 0<state_count<=state_capacity<=4096 or not state_data:raise LiveReadError('Invalid state registry')
    tags=r.pointer(r.world+0x358)
    tag_count=r.integer(r.world+0x364,8192,1)
    names={0:None};states={};controllers={};items=[];controller_slots={}
    def name(index):
        if index not in names:
            if not 0<index<tag_count:raise LiveReadError('Invalid province controller')
            value=r.s.string(tags+index*32)
            if not re.fullmatch('[A-Z0-9]{3}',value):raise LiveReadError('Invalid controller country tag')
            names[index]=value
        return names[index]
    for offset in range(0,count,8192):
        raw=r.s.read(data+offset*8,min(8192,count-offset)*8)
        for relative,(province,) in enumerate(struct.iter_unpack('<Q',raw)):
            identifier=offset+relative
            if not province:continue
            r.typed(province,'CProvince')
            if r.u(province+0xA4,'<i')!=identifier:raise LiveReadError('Province registry identity mismatch')
            state=r.u(province+0xC0)
            if not state:continue  # Ocean provinces have no state/build goal.
            if state not in states:
                r.typed(state,'CState')
                state_id=r.u(state+0x58,'<i')
                if not 0<state_id<state_count or r.pointer(state_data+state_id*8)!=state:
                    raise LiveReadError('Province/state registry mismatch')
                states[state]=(state_id,r.integer(state+0xC8,8191),r.integer(state+0xCC,8191))
            state_id,owner,state_controller=states[state]
            controller=r.u(province+0x188,'<i')
            if not 0<=controller<=8191:raise LiveReadError('Invalid province controller index')
            controller_slots[identifier]=province+0x188
            controllers[str(identifier)]=name(controller)
            if not owner==state_controller==controller==r.tag:continue
            status=r.typed(province+0x190,'CBuildingStatus')
            flags,location,national=struct.unpack('<iii',r.track(status+0x68,12))
            if (not 0<=flags<=0x7fffffff or not flags&1 or location!=identifier or national!=0):
                raise LiveReadError('Provincial building container identity mismatch')
            # Observe only the two fort slots, not every building kind on every
            # tile. All consumed bytes still participate in full revalidation.
            mapping,map_capacity,map_count=struct.unpack('<Qii',r.track(status+0x20,16))
            members,member_capacity,member_count=struct.unpack('<Qii',r.s.read(status+0x38,16))
            if (not 0<=map_count<=map_capacity<=512 or (map_count and not mapping)
                    or not 0<=member_count<=member_capacity<=512 or (member_count and not members)):
                raise LiveReadError('Invalid provincial building container')
            row={'id':identifier,'state_id':state_id}
            indices=set()
            for key,(definition,ordinal) in definitions.items():
                if ordinal>=map_count:raise LiveReadError('Incomplete province building map')
                index=struct.unpack('<i',r.track(mapping+ordinal*4,4))[0]
                if index==-1:
                    row[key]=0
                    continue
                if not 0<=index<member_count or index in indices:raise LiveReadError('Invalid province building index')
                indices.add(index)
                building=r.typed(r.pointer(members+index*8),'CBuilding')
                if struct.unpack('<QQ',r.track(building+0x1D8,16))!=(status,definition):
                    raise LiveReadError('Provincial fort owner/template mismatch')
                level=struct.unpack('<h',r.track(building+0x40,2))[0]
                if not 0<=level<=1000:raise LiveReadError('Invalid provincial fort level')
                row[key]=level
            items.append(row)
    # Full audit witnesses every decoded controller/identity. Cheap probes need
    # only own tiles, adjacent controllers and state transfers, not distant wars.
    watched={row['id'] for row in items}
    topology=getattr(r.process.profile,'province_neighbors',None)
    if isinstance(topology,dict):
        for row in items:
            neighbors=topology.get(row['id'])
            if isinstance(neighbors,list):watched.update(neighbors)
    for identifier in sorted(watched):
        if identifier in controller_slots:r.track(controller_slots[identifier],4)
    return {'status':'observed','items':items,'controllers':controllers,
            'reason':'Built provincial levels; exact mapped absence is zero. Queue removal is not completion.'}
