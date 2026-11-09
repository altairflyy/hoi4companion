"""Read-only add-product controls and native equipment picker identities.

Exact profile only, admitted by production_ui_native. Constructor/setter
evidence: CProductionEquipmentWindow +10F8=view (callers 17D17AF/17D188D);
native item lists +10C0/+10D8 (1E3C2F2/1E3CFF8); item +11D8=picker
(1DD219D), +1158=variant (1DE9936). Never invokes a game command.
"""
import struct
from .live import LiveReadError


def button(ui,s,root,address,owner,kind,name,clip):
    ui._typed(s,address,'CButtonStandard')
    observer=s.number(address+0x50);typ,offset=ui._rtti(s.number(observer))
    if typ!='.?AV'+kind+'@@' or observer-offset!=owner:raise LiveReadError('Foreign production button observer')
    if s.number(address+0x68)!=root:raise LiveReadError('Foreign production GUI root')
    definition=ui._typed(s,s.number(address+0x60),'CButtonType')
    if s.string(definition+0x28)!=name:raise LiveReadError('Production button identity changed')
    parent=ui._typed(s,s.number(address+ui.layout('button_anchor',0x1B0)),'C2dPositionObject')
    matrix=struct.unpack('<16f',s.read(parent+0x90,64))
    bounds=ui._sprite_bounds(s,s.number(address+ui.layout('button_sprite',0xF0)),parent,matrix,clip)
    return {**bounds,'visible':bounds['visible'] and s.shown(address+0x75),'id':name}


def read(ui,s,root,bindings,resolution,view,names):
    add=[];pickers=[];entries=[];context=[]
    for record in bindings.get('production_add',[]):
        if record['owner']!=view:continue
        add.append(button(ui,s,root,record['address'],view,'CCountryProductionLineView',record['name'],[0,0,*resolution]))
    windows=[]
    for offset in (0x2730,0x2738):
        address=s.number(view+offset)
        if not address:continue
        ui._typed(s,address,'CProductionEquipmentWindow')
        if s.number(address+0x10F8)!=view:raise LiveReadError('Foreign production equipment picker')
        members=s.array(address+0x10C0,maximum=4096)+s.array(address+0x10D8,maximum=4096)
        if len(set(members))!=len(members):raise LiveReadError('Duplicate production picker entries')
        windows.append((address,members));context.append((address,tuple(members)))
    for record in bindings.get('production_pickers',[]):
        owner=record['owner']
        candidates=[members for address,members in windows if address==owner]
        if len(candidates)!=1:continue
        container=record['container']
        if not ui._container(s,container,root,owner+record['observer_offset']):continue
        definition=ui._typed(s,s.number(container+0x90),'CContainerWindowType')
        if s.string(definition+0x28)!=record['name']:raise LiveReadError('Production picker identity changed')
        viewport=[]
        for child in s.array(container+ui.layout('container_children',0x10F0),maximum=2048):
            if ui._rtti(s.number(child))!=('.?AVCContainerWindow@@',48):continue
            child-=48
            definition=ui._typed(s,s.number(child+0x90),'CContainerWindowType')
            if s.string(definition+0x28)=='equipments':viewport.append(child)
        if len(viewport)!=1:raise LiveReadError('Production picker viewport missing or ambiguous')
        if not ui._container(s,viewport[0],root,owner+record['observer_offset']):continue
        raw=struct.unpack('<4i',s.read(viewport[0]+ui.layout('container_clip',0x1594),16))
        clip=[raw[0],raw[1],raw[2]-raw[0],raw[3]-raw[1]]
        pickers.append({'visible':True,'name':record['name']})
        for item in candidates[0]:
            ui._typed(s,item,'CProductionInfoEquipmentItem')
            if s.number(item+0x11D8)!=owner:raise LiveReadError('Foreign production picker entry')
            variant=ui._typed(s,s.number(item+0x1158),'CEquipmentVariant')
            equipment=ui._typed(s,s.number(variant+0x3F0),'CEquipmentType')
            row=s.number(item+0x10)
            if not ui._container(s,row,root,item+24):continue
            buttons=[]
            for address in s.array(row+ui.layout('container_buttons',0x1120),maximum=256):
                if ui._rtti(s.number(address))!=('.?AVCButtonStandard@@',0):continue
                definition=ui._typed(s,s.number(address+0x60),'CButtonType')
                if s.string(definition+0x28)=='equipment_button':buttons.append(address)
            if len(buttons)!=1:continue
            bounds=button(ui,s,root,buttons[0],item,'CProductionInfoEquipmentItem','equipment_button',clip)
            from .design_native import read_design
            entries.append({**bounds,'equipment':s.string(equipment+0x18),'variant_name':names.name(variant+0x28),
                            'design':read_design(names,variant)})
    return {'add':add,'picker_open':bool(pickers),'picker_count':len(pickers),'equipment':entries,'context':tuple(context)}
