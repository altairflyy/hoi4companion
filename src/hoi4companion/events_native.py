"""Passive, build-pinned event popup and decision observations.

Event choices come from rendered grid rows, not every conditional option in the
definition. Decision manager membership is deliberately not called eligibility,
activation or completion. No native function is invoked. See docs/events-live.md.
"""
import struct
from .live import LiveReadError

GUI_RVA=0x34531E0


def _discover_windows(r,root):
    """Bounded discovery only; every selected binding is validated afterwards.

    The registry is not a coherent gameplay snapshot: animation modifies unrelated
    widgets. Candidate reads never escape as observations. Cache only process-local
    window addresses; verify type, root and binding on every subsequent sample.
    """
    cache=getattr(r.process,'_hoi4_event_cache',None)
    headers=tuple(r.track(root+offset,16) for offset in (264,288))
    if cache and len(cache)==3 and cache[0]==root and cache[1] and cache[2]==headers:return cache[1]
    owners=set(); seen=set()
    for offset,dynamic in ((264,False),(288,True)):
        header=r.track(root+offset,16)
        pointer,capacity,count=struct.unpack('<Qii',header)
        extent=capacity if dynamic else count
        if not 0<=count<=capacity<=500000: raise LiveReadError('Registro GUI fuori intervallo')
        for start in range(0,extent,8192):
            data=r.process.read(pointer+start*8,min(8192,extent-start)*8)
            for (gui,) in struct.iter_unpack('<Q',data):
                if not gui or gui in seen:continue
                seen.add(gui)
                try:
                    parent=struct.unpack('<Q',r.process.read(gui+80,8))[0]
                    if parent and struct.unpack('<Q',r.process.read(parent,8))[0]==r.process.base+0x29A4878:
                        owners.add(parent-56)
                except (OSError,LiveReadError,struct.error):
                    continue
    if not owners: raise LiveReadError('Finestra eventi non individuata; assenza non qualificata')
    r.process._hoi4_event_cache=(root,sorted(owners),headers)
    return sorted(owners)


def read_events(r):
    root=r.typed(r.pointer(r.process.base+GUI_RVA),'CGameGui')
    owners=_discover_windows(r,root)
    visible=[]
    for owner in owners:
        r.typed(owner,'CEventWindow')
        container=r.typed(r.pointer(owner+0x50),'CContainerWindow')
        gui=container+48
        if r.pointer(gui+104)!=root: raise LiveReadError('Finestra eventi di un altro contesto GUI')
        if r.pointer(gui+80)!=owner+56:raise LiveReadError('Finestra eventi non associata al proprietario')
        shown=struct.unpack('<B',r.track(gui+117,1))[0]&8
        event=r.pointer(owner+0x68)
        if not shown or not event: continue
        r.typed(event,'CEvent'); identifier=r.s.string(event+0x20)
        definition=set(r.array(event+0x368,256))
        grid=r.typed(r.pointer(owner+0x1280),'CStandardGridBox')
        choices=[]
        for row in r.array(grid+0x1A0,256):
            r.typed(row,'CStandardGridBoxItem')
            item=r.typed(r.pointer(row+16),'CContainerWindow')
            buttons=r.array(item+0x10D8,64)
            if not buttons: raise LiveReadError('Scelta evento priva di pulsante qualificato')
            row_options=set()
            for button in buttons:
                r.typed(button,'CButtonDrag')
                option=r.typed(r.pointer(button+8),'CEventOption')
                if option not in definition: raise LiveReadError('Scelta estranea all’evento visualizzato')
                row_options.add(option)
            if len(row_options)!=1: raise LiveReadError('Riga evento associata a opzioni diverse')
            option=row_options.pop()
            choices.append((r.integer(option+0x1F0,255),r.s.string(option+0x178)))
        if len({index for index,key in choices})!=len(choices): raise LiveReadError('Indice scelta evento duplicato')
        visible.append({'instance_id':format(owner,'x'),'event_id':identifier,
                        'option_ids':sorted({key for index,key in choices})})
    return {'status':'observed','visible':visible,
            'reason':'Finestra eventi marcata visibile dal motore; scelte dalla griglia corrente. Nessuna risposta scelta inferita.'}


def read_decisions(r):
    manager=r.typed(r.pointer(r.country+0xFA0),'CDecisionStatus')
    if r.pointer(manager+8)!=r.country: raise LiveReadError('Decisioni di un’altra nazione')
    listed=[]
    def identifier(pointer):
        r.typed(pointer,'CDecision')
        return r.s.string(pointer+0x120)
    for pointer in r.array(manager+0x40,4096): listed.append(identifier(pointer))
    if len(set(listed))!=len(listed):raise LiveReadError('Identità decisione duplicata')
    # This is a native execution/random counter, not a completion history.
    pointer,capacity,count=struct.unpack('<Qii',r.track(manager+0x148,16))
    if not 0<=count<=capacity<=2048:raise LiveReadError('Contatori decisioni fuori intervallo')
    raw=r.track(pointer,count*24) if count else b''
    counters=[];seen=set()
    for vtable,decision,amount,target in struct.iter_unpack('<QQiI',raw):
        if vtable!=r.process.base+0x27EB570 or amount<0:raise LiveReadError('Contatore decisione non qualificato')
        key=identifier(decision)
        if (key,target) in seen:raise LiveReadError('Contatore decisione duplicato')
        seen.add((key,target));counters.append({'id':key,'native_target_tag':target,'native_execution_counter':amount})
    return {'status':'observed','listed_ids':sorted(listed),'execution_counters':counters,
            'reason':'Appartenenza al manager e contatori nativi. Disponibilità, avvio, scelta ed effetti non ancora qualificati.'}
