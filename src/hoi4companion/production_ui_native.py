"""Passive production controls; missing/foreign bindings never produce rectangles."""
import struct
import math
from .live import LiveReadError

def read(ui,s,root,bindings,resolution,country):
    from .focus_ui_native import _intersect
    from .live import MILITARY_HASH
    if getattr(ui.profile,'sha256',None)!=MILITARY_HASH:
        return {'status':'unavailable','reason':'Production GUI profile unavailable'}
    owners=bindings.get('production_owners',{})
    views=[a for a,n in owners.items() if n=='CCountryProductionLineView']
    if len(views)!=1:return {'status':'unavailable','reason':'Production view missing or ambiguous'}
    view=ui._typed(s,views[0],'CCountryProductionLineView')
    container=s.number(view+ui.layout('view_container',0x578))
    visible=ui._container(s,container,root,view+40)
    definition=ui._typed(s,s.number(container+0x90),'CContainerWindowType')
    if s.string(definition+0x28)!='countryproductionlineview':raise LiveReadError('Production view identity changed')
    access=[];lines=[]
    from .country import CountryReader
    names=CountryReader(ui.process,0,country,0);names.s=s;names.u=s.number
    from .production_controls_native import read as read_controls
    controls=read_controls(ui,s,root,bindings,resolution,view,names)
    # Candidate registry addresses are cached; every selected control is read
    # and witnessed again, including the observer RTTI and current anchors.
    for address in bindings.get('production_access',[]):
        ui._typed(s,address,'CButtonStandard')
        observer=s.number(address+0x50);name,offset=ui._rtti(s.number(observer))
        if name!='.?AVCTopBar@@' or owners.get(observer-offset)!='CTopBar':raise LiveReadError('Foreign production access control')
        if s.number(address+0x68)!=root:raise LiveReadError('Foreign production GUI root')
        definition=ui._typed(s,s.number(address+0x60),'CButtonType')
        if s.string(definition+0x28)!='production_button':raise LiveReadError('Production access identity changed')
        parent=ui._typed(s,s.number(address+ui.layout('button_anchor',0x1B0)),'C2dPositionObject')
        matrix=struct.unpack('<16f',s.read(parent+0x90,64))
        bounds=ui._sprite_bounds(s,s.number(address+ui.layout('button_sprite',0xF0)),parent,matrix,[0,0,*resolution])
        access.append({**bounds,'visible':bounds['visible'] and s.shown(address+0x75)})
    if visible:
        # Use the engine's scrolling viewport, including its margins; the row
        # container itself can carry a full-screen clip behind the header.
        viewport=container
        for name in ('military_tab','production_lines'):
            candidates=[]
            for child in s.array(viewport+ui.layout('container_children',0x10F0),maximum=2048):
                if ui._rtti(s.number(child))!=('.?AVCContainerWindow@@',48):continue
                child-=48
                definition=ui._typed(s,s.number(child+0x90),'CContainerWindowType')
                if s.string(definition+0x28)==name:candidates.append(child)
            if len(candidates)!=1:raise LiveReadError('Production viewport missing or ambiguous')
            viewport=candidates[0]
            if not ui._container(s,viewport,root,view+40):raise LiveReadError('Production viewport hidden')
        raw=struct.unpack('<4i',s.read(viewport+ui.layout('container_clip',0x1594),16))
        viewport_clip=[raw[0],raw[1],raw[2]-raw[0],raw[3]-raw[1]]
        # Row allocation is lazy. This build's base equipment-item constructor
        # stores the line at +3878 (military-item constructor); native pool membership is mandatory.
        status=ui._typed(s,s.number(country+0xF68),'CProductionStatus')
        if s.number(status+getattr(ui.profile,'production_owner_offset',0x290))!=country:raise LiveReadError('Foreign production owner')
        pool=s.array(status+0x58,maximum=4096)
        for item,kind in owners.items():
            if kind!='CProductionLineMilitaryItem':continue
            ui._typed(s,item,kind);line=s.number(item+0x3878)
            if line not in pool:continue
            if s.number(item+0xB90)!=view:raise LiveReadError('Foreign production view')
            ui._typed(s,line,'CMilitaryProductionLine')
            if s.number(line+0x20)!=status:raise LiveReadError('Foreign production line')
            variant=ui._typed(s,s.number(line+0x88),'CEquipmentVariant')
            equipment=ui._typed(s,s.number(variant+0x3F0),'CEquipmentType')
            row_container=s.number(item+0x10)
            shown=ui._container(s,row_container,root,item+0xB48)
            if not shown:continue
            position=ui._typed(s,s.number(row_container+ui.layout('container_outer',0x15F0)),'C2dPositionObject')
            matrix=struct.unpack('<16f',s.read(position+0x90,64))
            width,height=struct.unpack('<ii',s.read(row_container+0x17AC,8))
            if not 8<=width<=resolution[0] or not 8<=height<=resolution[1]:raise LiveReadError('Production row size unqualified')
            rect=[matrix[12],-matrix[13],width*matrix[0],height*matrix[5]]
            if (not all(math.isfinite(v) for v in matrix) or not 0.01<=matrix[0]<=10
                    or abs(matrix[0]-matrix[5])>.0001
                    or any(abs(matrix[i])>.0001 for i in (1,2,3,4,6,7,8,9,11))):raise LiveReadError('Production row transform unqualified')
            raw=struct.unpack('<4i',s.read(container+ui.layout('container_clip',0x1594),16))
            clip=_intersect(rect,[raw[0],raw[1],raw[2]-raw[0],raw[3]-raw[1]])
            clip=_intersect(clip,viewport_clip)
            raw=struct.unpack('<4i',s.read(row_container+ui.layout('container_clip',0x1594),16))
            clip=_intersect(clip,[raw[0],raw[1],raw[2]-raw[0],raw[3]-raw[1]])
            clip=_intersect(clip,[0,0,*resolution])
            lines.append({'line_id':str(pool.index(line)+1),'equipment':s.string(equipment+0x18),
                          'variant_name':names.name(variant+0x28),
                          'rect':rect,'clip':clip,'visible':clip[2]>0 and clip[3]>0})
    return {'status':'observed','visible':visible,'access':access,'lines':lines,
            **{k:controls[k] for k in ('add','picker_open','picker_count','equipment')},
            'discovery_context':(tuple(pool) if visible else (),controls['context'])}
