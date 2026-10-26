"""Read-only screen annotations. OCR is visual evidence, never run completion."""
import math
import unicodedata


def text_key(value):
    if not isinstance(value,str):return ''
    return ''.join(c for c in unicodedata.normalize('NFKD',value).casefold() if c.isalnum())


PANELS={
    'focus':('focus nazionali','national focus'),
    'research':('ricerca','research','armi ed equipaggiamento','weapons and equipment','ingegneria elettronica'),
    'politics':('leggi e governo','laws and government','political advisors','consiglieri politici','personale militare'),
    'production':('produzione','production'),
}


def targets(guide,runtime,catalog,state,*,prepare_events=False,event_plan=None):
    """Project only the engine's admitted steps; never reinterpret prose or timing."""
    sample=state.get('sample')
    if not state.get('usable') or not isinstance(sample,dict):return []
    country=sample.get('country')
    required_country=guide.get('applies_to',{}).get('country')
    if (runtime.get('guide_id')!=guide.get('id') or runtime.get('mode')!='live'
            or not state.get('usable') or not country or runtime.get('country')!=country
            or required_country and country!=required_country):return []
    political=runtime.get('action_sequence',{}).get('political')
    context=runtime.get('action_context')
    if isinstance(context,dict):
        from .contracts import normalise_state
        normalized=normalise_state(state)
        session=context.get('session_id');campaign=context.get('campaign_id')
        observed=normalized['facts'].get('campaign.id',{})
        if session is not None and normalized.get('session_id')!=session:return []
        if campaign is not None and (observed.get('status')!='observed' or not observed.get('source') or observed.get('value')!=campaign):return []
    rows={r['key']:r for lane in guide['tracks'].values() for r in lane}
    native=set(k for lane in runtime.get('parallel',{}).values() for k in lane.get('steps',[]))
    overview=state.get('sample',{}).get('country_overview',{})
    events=overview.get('events',{}) if overview.get('country')==country else {}
    # Both the native watch list and the visual path use the same scoped
    # event/answer binding. A plan cached for another guide must never supply
    # its answer to a step whose key happens to be reused by the new guide.
    if event_plan is None or event_plan.get('guide_id')!=guide.get('id'):
        from .guide_events import compile_event_plan
        event_plan=compile_event_plan(guide,catalog)
    bindings=event_plan.get('bindings',{})
    result=[];seen=set()
    # Preserve the runtime's priority order, including across research folders.
    order={key:index for index,key in enumerate(runtime.get('parallel',{}).get('research',{}).get('steps',[]))}
    for item in sorted(runtime.get('steps',[]),key=lambda item:order.get(item['key'],len(order))):
        row=rows.get(item['key']);status=item.get('status');reason=item.get('reason')
        if not row or (row.get('tracking')=='reminder' and not (prepare_events and item['key'] in bindings)) or status not in ('ready','active','unverifiable'):continue
        if status=='unverifiable' and reason!='manual_confirmation_required':continue
        action=row['action'];params=action.get('params',{});ref=action.get('entity',{});panel=None;entries=[];labels=[];identity=None
        if any(k in params for k in ('event_id','event_name','event_option_id','event_option')):
            binding=bindings.get(item['key'])
            if not binding:continue
            identity=binding['identity']
            if ('event',identity) not in seen:
                rows_visible=[e for e in events.get('visible',[]) if e.get('event_id')==binding['event_id']] if events.get('status')=='observed' else []
                live_visible=any(binding['option_id'] in e.get('option_ids',[]) for e in rows_visible)
                if not prepare_events and not live_visible:continue
                result.append({**binding,'status':status,'country':country,'watch':prepare_events,
                    'instances':[e['instance_id'] for e in rows_visible if isinstance(e.get('instance_id'),str)],
                    'live_visible':live_visible})
                seen.add(('event',identity))
            continue
        kind=action['kind']
        if kind in ('select_focus','research'):
            # Active native steps are still highlighted by the tree views.
            # Only NEW choices consume the runtime's free parallel capacity.
            if item['key'] not in native and status!='active':continue
            panel='focus' if kind=='select_focus' else 'research';identity=ref.get('id')
            entries=[e for e in catalog if e['kind']==ref.get('kind') and e['id']==identity and (not ref.get('scope') or e.get('scope')==ref['scope'])]
        elif kind=='special_project':
            if status=='active':continue
            panel='projects';identity=ref.get('id')
            entries=[e for e in catalog if e['kind']=='project' and e['id']==identity]
        elif params.get('category')=='production':
            panel='production';identity=params.get('variant_name') or params.get('equipment_id') or params.get('equipment_name')
            selector=params.get('production_selector')
            if not selector:continue
            # The runtime has already matched and qualified these native rows.
            # Reuse that evidence instead of resolving a family name a second time.
            production=item.get('production') or {}
            labels=[line['name'] for line in production.get('lines',[]) if isinstance(line.get('name'),str)]
            if not labels:labels=[params['variant_name']] if params.get('variant_name') else []
            if not labels:
                from .research_upgrades import exact_matches
                entries=exact_matches(params.get('equipment_id') or params.get('equipment_name',''),[e for e in catalog if e['kind']=='equipment'])
        elif params.get('category') in ('politics','characters'):
            if item['key']!=political:continue
            panel='politics';identity=params.get('advisor_id') or params.get('law_id') or params.get('idea_id')
            identifiers={params.get('law_id'),params.get('idea_id'),params.get('advisor_id')}-{None}
            entries=[e for e in catalog if e['kind'] in ('law','idea','advisor') and e['id'] in identifiers
                     and (not params.get('role') or e.get('slot',e.get('group'))==params['role'])
                     and (not params.get('law_group') or e.get('group')==params['law_group'])]
            labels=[params.get('person_name')] if params.get('person_name') else []
        if not panel or not identity or (panel,identity) in seen:continue
        if panel in ('politics','projects'):
            from .action_readiness import hud_resource_ready
            if not hud_resource_ready(item.get('action_readiness')):continue
        if len(entries)>1:continue
        if panel in ('politics','projects') and len(entries)!=1:continue
        for entry in entries:labels.extend([entry.get('localized_name'),entry.get('name'),*(entry.get('aliases',[]) if panel not in ('politics','projects') else [])])
        labels=list(dict.fromkeys(x for x in labels if isinstance(x,str) and len(text_key(x))>=4))
        if not labels:continue
        seen.add((panel,identity));result.append({'key':item['key'],'panel':panel,'identity':identity,'labels':labels,
            'title':row.get('title') or row.get('name') or item['key'],'status':status,
            'folder':row.get('folder'),'folder_name':row.get('folder_name'),
            'event_title':params.get('event_name'),'event_titles':[label for e in catalog if panel=='event' and e['kind']=='event' and e['id']==params.get('event_id') for label in (e.get('name'),e.get('localized_name')) if label],
            'country':country,'scope':ref.get('scope') or (guide.get('applies_to',{}).get('tree_id') if panel=='focus' else None),
            **({'action_readiness':item.get('action_readiness'),'entity_kind':entries[0]['kind'],'category_id':params.get('law_group') or params.get('role') or entries[0].get('group'), 'specialization':entries[0].get('specialization')} if panel in ('politics','projects') else {}),
            'quantity':(item.get('production') or {}).get('target',params.get('requested_factories',params.get('assigned_factories')))})
    # Conflicting authored answers are not settled by order, priority or text.
    choices={}
    for p in result:
        if p['panel']=='event':choices.setdefault(p.get('event_id') or p['identity'].split(':',1)[0],set()).add(p['identity'])
    return [p for p in result if p['panel']!='event' or len(choices[p.get('event_id') or p['identity'].split(':',1)[0]])==1]


def rectangles(ocr,proposals):
    """Exact, unique visible labels only. Coordinates are screenshot pixels."""
    width,height=ocr.get('width',0),ocr.get('height',0)
    if not 0<width<=8192 or not 0<height<=8192:return []
    lines=[]
    for line in ocr.get('lines',[]):
        words=line.get('words',[])
        if not words:continue
        if any(not all(type(w.get(k)) in (int,float) and math.isfinite(w[k]) for k in ('x','y','width','height')) or min(w['x'],w['y'])<0 or min(w['width'],w['height'])<=0 or w['x']+w['width']>width or w['y']+w['height']>height for w in words):continue
        x=min(w['x'] for w in words);y=min(w['y'] for w in words)
        lines.append({'text':line.get('text',''),'x':x,'y':y,'width':max(w['x']+w['width'] for w in words)-x,'height':max(w['y']+w['height'] for w in words)-y})
    keys=[text_key(line['text']) for line in lines]
    found=[]
    for proposal in proposals:
        panel=proposal['panel']
        if panel in ('politics','projects'):continue
        if panel=='event':
            if proposal.get('watch') and not proposal.get('live_visible'):continue
            titles={text_key(t) for t in [proposal.get('event_title'),*proposal.get('event_titles',[])]}-{''}
            if not any(key in titles for key in keys):continue
        elif panel=='focus':
            header=[line for line in lines if line['y']<max(150,height*.12)]
            italian=any(text_key(a['text'])=='focus' and 'nazionali' in text_key(b['text']) and abs(a['y']-b['y'])<30 for a in header for b in header)
            if not italian and not any('nationalfocus' in text_key(line['text']) or 'focusnazionali' in text_key(line['text']) for line in header):continue
        elif not any(text_key(anchor) in ''.join(keys) for anchor in PANELS.get(panel,())):continue
        wanted={text_key(label) for label in proposal['labels']};matches=[]
        for line in lines:
            combined=[line];current=line
            for _ in range(3):
                text=' '.join(part['text'] for part in combined)
                if text_key(text) in wanted:
                    left=min(part['x'] for part in combined);top=min(part['y'] for part in combined)
                    right=max(part['x']+part['width'] for part in combined);bottom=max(part['y']+part['height'] for part in combined)
                    matches.append((left,top,right,bottom));break
                following=[other for other in lines if 0<=other['y']-(current['y']+current['height'])<=max(10,current['height']*.8)
                    and abs((other['x']+other['width']/2)-(current['x']+current['width']/2))<max(current['width'],other['width'])/2]
                if len(following)!=1:break
                current=following[0];combined.append(current)
        matches=list(dict.fromkeys(matches))
        if len(matches)==1:
            x,y,right,bottom=matches[0]
            found.append({**proposal,'rect':[max(0,x-4),max(0,y-3),min(width,right+4),min(height,bottom+3)],'basis':'exact_unique_ocr'})
    # A label shared by different entities is not enough to choose one of them.
    return [item for item in found if len({other['identity'] for other in found if other['rect']==item['rect']})==1]
