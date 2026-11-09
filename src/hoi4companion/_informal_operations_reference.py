"""Promote unambiguous source instructions using existing operation binders.

Only complete commands are admitted. Residual methods and informal timing are
retained, never converted into guessed live observations or confirmations.
"""
from copy import deepcopy
import hashlib
import re

from ._informal_guides_reference import explicit_action
from .political_import import bind_political
from ._informal_targets_reference import named_command, production_command
from .campaign_conditions import timing_condition


_AGENCY = re.compile(r'(?:create|form)\s+(?:an?\s+)?(?:intelligence\s+)?agency(?:\s*(&\s*setup\s+done)|\s*\(([^()]+)\))?', re.I)
_TIMING = re.compile(r'((?:on|after|before|shortly before|when|once|at)\b[^:]+):\s*(.*)', re.I)
_TIMING_HEADING = re.compile(r'(?:on|after|before|shortly before|when|once)\s+[^:\n]+(?:…|\.{3})',re.I)


def _generated_timings(guide_id,key,text):
    # Older multiline imports hashed the original source key before assigning
    # stable substep keys. Accept only those exact generated placeholders.
    keys=dict.fromkeys((key,re.sub(r'(?:--operation-\d+)+$','',key)))
    return [{'op':'fact','key':'guide.conditions.'+hashlib.sha256(
        (guide_id+'\0'+source+'\0'+text).encode('utf8')).hexdigest()[:24],
        'test':'eq','value':True} for source in keys]


def promote_operations(guide, catalog):
    """Return evidence of conversions; use only unique country-scoped identities."""
    if guide.get('dependency_policy','legacy')!='explicit':return []
    from ._action_blocks_reference import annotate as annotate_blocks
    conversions=annotate_blocks(guide)
    from .reference_designs import bindings
    known_variants=bindings(guide,catalog)
    def condition(text,params):
        known=timing_condition(text,catalog,guide['applies_to'].get('country'),
            scope=guide['applies_to'].get('tree_id'),currency=params.get('cost_currency'))
        if known:return known
        # Editorial ellipses refer only to a unique focus actually in this
        # guide. Do not expand an ambiguous prefix across the whole catalog.
        hit=re.fullmatch(r'(on|after|once)\s+(.+?)(?:…|\.{3})',text or '',re.I)
        if not hit:return None
        from .research_upgrades import normal_name
        ids={r['action']['entity']['id'] for rows in guide['tracks'].values() for r in rows if r['action']['kind']=='select_focus'}
        needle=normal_name(hit[2]);found={e['id'] for e in catalog.rows(('focus',)) if e['id'] in ids
            and any(normal_name(n).startswith(needle) for n in [e.get('name',''),*e.get('aliases',[])] if isinstance(n,str))}
        if len(found)!=1:return None
        target=found.pop();done={'op':'fact','key':'focus.completed','test':'contains','value':target}
        return {'op':'any','args':[{'op':'fact','key':'focus.active','test':'contains','value':target},done]} if hit[1].lower()=='on' else done
    all_keys={row['key'] for rows in guide['tracks'].values() for row in rows}
    # Old compilers retained their unresolved production tokens as synthetic
    # reminders. Reparse the original full line, never a shortened remainder:
    # otherwise a removed quantity could be reassigned to the wrong group.
    sources={s['id']:s for s in guide.get('sources',[]) if s.get('type')=='file' and '[Table row]' in s.get('text','')}
    for rows in guide['tracks'].values():
        parents=[]
        for row in rows:
            p=row['action'].get('params',{});instructions=p.get('instructions',[])
            if p.get('category')!='production' or not isinstance(instructions,list) or len(instructions)!=1 or not isinstance(instructions[0],str):continue
            quotes={(e.get('source_id'),e.get('quote')) for e in row.get('evidence',[]) if e.get('source_id') in sources and isinstance(e.get('quote'),str) and instructions[0] in e['quote']}
            if not quotes:continue
            parsed=production_command(instructions[0],catalog,guide['applies_to'].get('country'),known_variants)
            if parsed:parents.append((row,quotes,parsed))
        retained=[]
        for row in rows:
            p=row['action'].get('params',{});instructions=p.get('instructions',[])
            if row.get('tracking')!='reminder' or not re.search(r'--operation-\d+$',row['key']) or not isinstance(instructions,list) or len(instructions)!=1 or not isinstance(instructions[0],str) or not re.match(r'^(MIL|NAV|Production)\s*:',instructions[0],re.I):
                retained.append(row);continue
            quote_set={(e.get('source_id'),e.get('quote')) for e in row.get('evidence',[])}
            candidates=[(parent,parsed) for parent,quotes,parsed in parents if parent['key']!=row['key']
                        and re.sub(r'(?:--operation-\d+)+$','',parent['key'])==re.sub(r'(?:--operation-\d+)+$','',row['key'])
                        and quote_set and quote_set<=quotes and all(instructions[0] not in (quote or '') for _,quote in quote_set)]
            if len(candidates)!=1:retained.append(row);continue
            parent,parsed=candidates[0];remainder=parsed['remainder']
            if not remainder:
                conversions.append({'key':row['key'],'actions':0,'step_keys':[parent['key']],'unverifiable_timing':False});continue
            if instructions!=[remainder] or p.get('production_residual_for')!=parent['key']:
                p['instructions']=[remainder];p['production_residual_for']=parent['key'];row['title']=remainder;row['name']=remainder
                conversions.append({'key':row['key'],'actions':0,'step_keys':[row['key']],'unverifiable_timing':False})
            retained.append(row)
        rows[:]=retained
    for rows in guide['tracks'].values():
        expanded=[]
        for row in rows:
            params=row['action'].get('params',{})
            if params.get('production_residual_for'):
                expanded.append(row);continue
            if params.get('category')=='production' and row.get('tracking')!='reminder':
                # Rebind old imports in memory using the same generic resolver.
                from .operation_import import _bind_production
                probe=deepcopy(row)
                if _bind_production(probe,catalog,guide['applies_to'].get('country'),known_variants) in ('production_layout','production_live_assignment','production_remaining_assignment','production_relative_assignment'):
                    if probe!=row:
                        row.clear();row.update(probe);params=row['action']['params']
                        conversions.append({'key':row['key'],'actions':0,'step_keys':[row['key']],'unverifiable_timing':False})
                instructions=params.get('instructions',[])
                if isinstance(instructions,list) and len(instructions)==1 and params.get('production_targets'):
                    parsed=production_command(instructions[0],catalog,guide['applies_to'].get('country'),known_variants)
                    if parsed:
                        def signature(p):
                            import json
                            return json.dumps({k:p.get(k) for k in ('production_selector','requested_factories','assigned_factories','quantity_mode','production_priority','assignment_test')},sort_keys=True)
                        old=params['production_targets'];new=parsed['params']['production_targets']
                        signatures={signature(p) for p in new}
                        # Extend only; never reverse an editor's changed target.
                        if len(new)>len(old) and all(signature(p) in signatures for p in old):
                            params['production_targets']=new
                            conversions.append({'key':row['key'],'actions':0,'step_keys':[row['key']],'unverifiable_timing':False})
            # Structured guides can already carry this timing without requiring
            # an informal reminder to be expanded first.
            known_timing=condition(params.get('timing'),params)
            if known_timing:
                previous=_generated_timings(guide['id'],row['key'],params['timing'])
                if ('when' not in row or row['when'] in previous) and row.get('when')!=known_timing:
                    row['when']=known_timing
                    conversions.append({'key':row['key'],'actions':0,'step_keys':[row['key']],'unverifiable_timing':False})
            instructions=params.get('instructions')
            if (row.get('tracking')!='reminder' or row['action']['kind']!='manual'
                    or not isinstance(instructions,list) or not instructions
                    or any(not isinstance(s,str) for s in instructions)):
                expanded.append(row);continue
            pieces=[];timing=None;promoted=0
            def piece(text, action=None, cost=None):
                item=deepcopy(row)
                item['title']=text
                item['action']={'kind':'manual','params':{**deepcopy(params),'instructions':[text]}}
                if action:
                    item.pop('tracking',None)
                    item['action']['params'].update(action)
                    if cost is not None:item.setdefault('political_power_cost',cost)
                if timing:
                    item['action']['params'].setdefault('timing',timing)
                    generated=_generated_timings(guide['id'],row['key'],timing)
                    unresolved=generated[0]
                    # Retain authored predicates. Only our exact historical
                    # placeholder can be upgraded to a newly qualified rule.
                    if 'when' not in item or item['when'] in generated:
                        item['when']=condition(timing,item['action']['params']) or unresolved
                    item['note']='\n'.join(dict.fromkeys(s for s in [row.get('note'),timing] if s))
                pieces.append(item)
                return item
            for instruction in instructions:
                for raw in instruction.splitlines():
                    text=re.sub(r'^\s*[-•]\s*','',raw).strip()
                    if not text:continue
                    original_text=text
                    from ._event_instructions_reference import event_command
                    event=event_command(text,catalog,guide['applies_to'].get('country'))
                    if event:
                        item=piece(text,event['params'])
                        for field,policy in event['policies'].items():item.setdefault(field,policy)
                        promoted+=1;continue
                    context=_TIMING.fullmatch(text)
                    if not context and _TIMING_HEADING.fullmatch(text):
                        timing=text
                        continue
                    if context:
                        timing=context[1].strip()
                        text=context[2].strip()
                        if not text:continue
                        event=event_command(text,catalog,guide['applies_to'].get('country'))
                        if event:
                            item=piece(text,event['params'])
                            for field,policy in event['policies'].items():item.setdefault(field,policy)
                            promoted+=1;continue
                    from ._fortifications_reference import command as fortification_command
                    from ._foreign_commands_reference import commands as foreign_commands, prefix_commands
                    foreign,remainder=prefix_commands(text,catalog)
                    if foreign:
                        base=len(pieces)
                        for index,command in enumerate(foreign):
                            item=piece(command['text'],command['params'])
                            if index:item['_auxiliary_vocabulary_note']=True
                            item['_foreign_after']=[base+n for n in command['after_keys']]
                        if remainder:piece(remainder)['_auxiliary_vocabulary_note']=True
                        promoted+=len(foreign);continue
                    fortified=fortification_command(text,catalog)
                    if fortified:
                        piece(text.split(',',1)[0],fortified['params'])
                        if fortified['remainder']:
                            piece(fortified['remainder'])['_auxiliary_vocabulary_note']=True
                        promoted+=1;continue
                    from ._action_commands_reference import sequence as command_sequence
                    commands=command_sequence(text,catalog,guide['applies_to'].get('country'))
                    if commands:
                        previous=None
                        for command in commands:
                            item=piece(command['text'],command['params'])
                            for field,policy in command['policies'].items():item.setdefault(field,policy)
                            if command['after_previous']:
                                item['_sequence_after']=(len(pieces)-2,previous['milestone'])
                            previous=command
                            promoted+=1
                        continue
                    law=explicit_action(text)
                    if law and law['type']=='law':
                        probe={'action':{'kind':'manual','params':{'category':'politics','law_name':law['target']}}}
                        if bind_political(probe,catalog,guide['applies_to'].get('country'))=='politics_live_law':
                            piece(text,probe['action']['params'],law.get('political_power_cost'))
                            promoted+=1
                        else:piece(original_text)
                        continue
                    agency=_AGENCY.fullmatch(text)
                    if agency:
                        piece('Create intelligence agency',{'category':'projects','operation_id':'create_intelligence_agency'})
                        promoted+=1
                        if agency[1]:piece(agency[1].lstrip('& ').strip())
                        if agency[2]:
                            for clause in agency[2].split(','):
                                foreign=foreign_commands(clause.strip(),catalog)
                                if foreign:
                                    for command in foreign:
                                        piece(command['text'],command['params'])['_auxiliary_vocabulary_note']=True
                                        promoted+=1
                                else:piece(clause.strip())['_auxiliary_vocabulary_note']=True
                        continue
                    production=production_command(text,catalog,guide['applies_to'].get('country'),known_variants)
                    if production:
                        piece(text,production['params']);promoted+=1
                        if production['remainder']:piece(production['remainder'])
                        continue
                    from ._doctrine_commands_reference import doctrine_command
                    doctrine=doctrine_command(text,catalog,guide['applies_to'].get('country'))
                    if doctrine:
                        if doctrine.get('prefix'):
                            prefix=doctrine['prefix'];prior=piece(prefix['text'],prefix['params'])
                            for field,policy in prefix['policies'].items():prior.setdefault(field,policy)
                        item=piece(doctrine['text'],doctrine['params'])
                        for field,policy in doctrine['policies'].items():item.setdefault(field,policy)
                        if doctrine['remainder']:piece(doctrine['remainder'])['_auxiliary_vocabulary_note']=True
                        promoted+=1;continue
                    named=named_command(text,catalog,guide['applies_to'].get('country'))
                    if named:
                        previous_timing=timing
                        if named['timing']:timing='; '.join(s for s in (timing,named['timing']) if s)
                        item=piece(text,named['params'])
                        for field,policy in named['policies'].items():item.setdefault(field,policy)
                        timing=previous_timing
                        promoted+=1
                        continue
                    piece(original_text)
            if not promoted:
                expanded.append(row);continue
            # The first piece preserves the source key; substeps have stable
            # keys and copy source quotes/policies. Never overwrite other keys.
            # Explanatory notes added by a newer vocabulary must not shift the
            # historical keys of subsequent actions/checks in the source block.
            keys=[];core=0;notes=0
            for item in pieces:
                if item.pop('_auxiliary_vocabulary_note',False):
                    notes+=1
                    while row['key']+'--note-'+str(notes) in all_keys:notes+=1
                    keys.append(row['key']+'--note-'+str(notes))
                else:
                    core+=1;keys.append(row['key'] if core==1 else row['key']+'--operation-'+str(core))
            if len(pieces)>512 or any(key in all_keys for key in keys[1:]):
                expanded.append(row);continue
            for item,key in zip(pieces,keys):item['key']=key
            for item in pieces:
                foreign_after=item.pop('_foreign_after',[])
                if foreign_after:
                    item['depends_on']=list(dict.fromkeys([*item.get('depends_on',[]),*(keys[index] for index in foreign_after)]))
                dependency=item.pop('_sequence_after',None)
                if dependency is not None:
                    index,milestone=dependency
                    field='depends_on_started' if milestone=='start' else 'depends_on'
                    item[field]=list(dict.fromkeys([*item.get(field,[]),keys[index]]))
            expanded.extend(pieces);all_keys.update(keys)
            conversions.append({'key':row['key'],'actions':promoted,'step_keys':keys,
                                'unverifiable_timing':bool(timing and condition(timing,params) is None and 'when' not in row)})
        rows[:]=expanded
    from .reference_designs import attach_images
    from ._production_sequence_reference import annotate_word_revisions
    from ._production_milestones_reference import annotate as annotate_milestones
    return conversions+attach_images(guide,catalog)+annotate_word_revisions(guide)+annotate_milestones(guide)
