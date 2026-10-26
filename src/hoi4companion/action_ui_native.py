"""Optional action controls using the common witnessed GUI/button layout.

Unrecognised payloads remain anonymous controls. In particular, a program
caption never proves facility specialization or project availability.
"""
import struct
from .live import NativeSample, LiveReadError

BUTTONS={'topbar':{'player_flag','technology_button'},'countrytechnologyview':{'facilities_tab'},
         'countrypoliticsview':{'add_idea_button','open_roster_button','idea_entry','button'},
         'political_ideas_window':{'idea_entry_bg','button'},
         'facilities_view':{'open_facility_button','facility_button'},
         'program_window':{'start_research_button','project_button','select_project'}}

ROOTS={'topbar','countrypoliticsview','political_ideas_window','countrytechnologyview',
       'facilities_view','program_window'}


def discover_roots(ui, root, bindings):
    # Reuse the common GUI registry, no process-wide memory scan. Discovery is
    # outside the witnessed sample; every retained binding is requalified below.
    cache=bindings.setdefault('action_roots',{'seen':set(),'roots':{}})
    sample=NativeSample(ui.process)
    registry=bindings.get('event_registry',{}).get('seen',bindings.get('registry_seen',()))
    for candidate in sorted(set(registry)-cache['seen']):
        cache['seen'].add(candidate)
        try:
            ui._typed(sample,candidate,'CContainerWindow')
            definition=ui._typed(sample,sample.number(candidate+0x90),'CContainerWindowType')
            name=sample.string(definition+0x28)
            if name in ROOTS and sample.number(candidate+0x98)==root:
                cache['roots'][candidate]=name
        except (LiveReadError,OSError,ValueError,struct.error,UnicodeError):continue
    return dict(cache['roots'])


def payload_identity(ui,s,button,country_pointer):
    """Known native data objects only; role presence is not a click outcome."""
    try:
        payload=s.number(button+8)
        name,offset=ui._rtti(s.number(payload))
        if offset!=0:return {}
        if name=='.?AVCIdeaGroupType@@':
            ui._typed(s,payload,'CIdeaGroupType')
            return {'category_id':s.string(payload+0x38)}
        if name=='.?AVCIdea@@':
            ui._typed(s,payload,'CIdea')
            group=ui._typed(s,s.number(payload+0xAA0),'CIdeaGroupType')
            flag=s.number(group+0x58,'<B')
            if flag not in (0,1):return {}
            return {'entity_kind':'law' if flag else 'idea','entity_id':s.string(payload+0x18),
                    'category_id':s.string(group+0x38)}
        if name=='.?AVCAdvisor@@':
            ui._typed(s,payload,'CAdvisor')
            person=ui._typed(s,s.number(payload+0x10),'CCharacter')
            if s.number(person+0x68,'<i')!=s.number(country_pointer+8,'<i'):return {}
            # Numeric slot enums are not mapped speculatively. The token is
            # resolved to its unique role by the installed catalog at admission.
            return {'entity_kind':'advisor','entity_id':s.string(payload+0x30),
                    'character_id':s.string(person+0x48)}
    except (LiveReadError,OSError,ValueError,struct.error,UnicodeError):pass
    return {}


def read_controls(ui,s,root,roots,resolution,country_pointer):
    from .focus_ui_native import _intersect
    panels={};controls=[]
    def visit(container,root_name,observer,clip,depth,seen):
        if depth>12 or container in seen or len(seen)>=2048:raise LiveReadError('Gerarchia GUI azioni oltre limite')
        seen.add(container)
        if not ui._container(s,container,root,observer):return
        parent=None;matrix=None
        raw=struct.unpack('<4i',s.read(container+0x1594,16))
        clip=_intersect(clip,[raw[0],raw[1],raw[2]-raw[0],raw[3]-raw[1]])
        for button in s.array(container+0x1120,maximum=256):
            if ui._rtti(s.number(button))!=('.?AVCButtonStandard@@',0):continue
            ui._typed(s,button,'CButtonStandard')
            if len(controls)>=4096:raise LiveReadError('Troppi controlli GUI azioni')
            if s.number(button+0x68)!=root or s.number(button+0x50)!=observer:
                raise LiveReadError('Pulsante azione di un altro proprietario')
            definition=ui._typed(s,s.number(button+0x60),'CButtonType')
            control_name=s.string(definition+0x28)
            if control_name not in BUTTONS.get(root_name,set()):continue
            if parent is None:
                parent=ui._typed(s,s.number(container+0x15F8),'C2dPositionObject')
                matrix=struct.unpack('<16f',s.read(parent+0x90,64))
            if s.number(button+0x1B0)!=parent:raise LiveReadError('Ancoraggio azione incoerente')
            bounds=ui._sprite_bounds(s,s.number(button+0xF0),parent,matrix,clip)
            controls.append({**bounds,'root':root_name,'name':control_name,
                             'visible':bounds['visible'] and s.shown(button+0x75),
                             **payload_identity(ui,s,button,country_pointer)})
        for pointer in s.array(container+0x10F0,maximum=2048):
            # The container's IWindow entries use the already witnessed MSVC
            # displacement of 48 bytes (same path as technology folders).
            name,displacement=ui._rtti(s.number(pointer))
            if name!='.?AVCContainerWindow@@' or displacement!=48:continue
            if root_name=='topbar':
                definition=ui._typed(s,s.number(pointer-48+0x90),'CContainerWindowType')
                if s.string(definition+0x28) not in ('country_menu_window','after_faction_button','after_intel_agency_button'):continue
            visit(pointer-48,root_name,observer,clip,depth+1,seen)
    try:
        for container,name in roots.items():
            ui._typed(s,container,'CContainerWindow')
            definition=ui._typed(s,s.number(container+0x90),'CContainerWindowType')
            if s.string(definition+0x28)!=name:raise LiveReadError('Contesto GUI azione cambiato')
            observer=s.number(container+0x80)
            shown=ui._container(s,container,root,observer)
            if name in panels:raise LiveReadError('Viste azione duplicate')
            panels[name]=shown
            if shown:visit(container,name,observer,[0,0,*resolution],0,set())
        # A partial set cannot prove that an entity roster is closed.
        coverage={'politics':{'topbar','countrypoliticsview','political_ideas_window'}<=panels.keys(),
                  'projects':{'topbar','countrytechnologyview','facilities_view','program_window'}<=panels.keys()}
        status='observed' if any(coverage.values()) else 'partial'
        return {'status':status,'panels':panels,'controls':controls,'coverage':coverage,
                'reason':'Controlli nativi qualificati; modelli o specializzazioni non riconosciuti restano sconosciuti.'}
    except (LiveReadError,OSError,ValueError,struct.error,UnicodeError) as exc:
        return {'status':'unavailable','panels':{},'controls':[],'reason':str(exc)}
