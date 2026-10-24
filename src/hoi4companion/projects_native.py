"""Build-pinned, read-only special projects. See docs/special-projects-live.md."""
import struct

from .live import LiveReadError


def read_projects(reader):
    r=reader
    status=r.typed(r.pointer(r.country+0xFA8),'CProgramStatus@NProject')
    if r.integer(status+0x10,8191)!=r.tag:
        raise LiveReadError('Programmi di ricerca di una nazione diversa')
    pool=r.typed(r.pointer(status+0x18),'CProjectPool@NProject')
    if r.integer(pool+8,8191)!=r.tag:
        raise LiveReadError('Progetti di una nazione diversa')
    programs={}
    for pointer in r.array(status+0x20,256):
        r.typed(pointer,'CProgram@NProject')
        if r.integer(pointer+0x18,8191)!=r.tag:
            raise LiveReadError('Impianto di un’altra nazione')
        handle=r.track(pointer+8,8)
        if 0 in struct.unpack('<II',handle) or handle in programs:
            raise LiveReadError('Identità impianto non valida')
        programs[handle]={'facility':r.s.string(pointer+0x20),'template_key':r.track(pointer+0x50,8)}
    data,capacity,count=struct.unpack('<Qii',r.track(pool+0x10,16))
    if not 0<=count<=capacity<=1024 or count and not data:
        raise LiveReadError('Catalogo progetti nativo non valido')
    rows=[];seen=set()
    for index in range(count):
        project=r.typed(data+index*0x1C0,'CProject@NProject')
        template=r.typed(r.pointer(project+0x20)-0x18,'CProjectTemplate@NProject')
        key=r.s.string(template+0x238)
        if key in seen:raise LiveReadError('ID progetto duplicato')
        seen.add(key)
        machine=r.typed(r.pointer(project+0x28),'CProjectStateMachine@NProject')
        current=r.pointer(machine+8)
        pending=r.track(machine+0x10,1)[0]
        if pending not in (0,1):raise LiveReadError('Transizione progetto non valida')
        completed=not pending and current==machine+0xE8
        if completed:
            r.typed(current,'CSimpleProjectState@NProject')
            if r.s.string(current+0x58)!='RESEARCH_COMPLETED':
                raise LiveReadError('Stato finale progetto non riconosciuto')
        handle=r.track(project+0x60,8)
        linked=handle!=b'\0'*8
        if linked and not completed and handle not in programs:
            raise LiveReadError('Impianto associato al progetto non verificabile')
        if linked and not completed and r.track(project+0x18,8)!=programs[handle]['template_key']:
            raise LiveReadError('Progetto e impianto non corrispondono')
        active=linked and not completed
        rows.append({'id':key,'active':active,'completed':completed,
                     **({'facility':programs[handle]['facility']} if active else {})})
    return {'status':'observed','items':rows,
            'reason':'Progetti della nazione corrente; impianto associato distinto dallo stato finale di ricerca.'}
