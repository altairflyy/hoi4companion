"""Prepare authored event/answer identities; no forecasting or strategy inference."""
import copy
import re
import unicodedata

from .political_import import bind_political


def _words(value):
    return re.findall(r'\w+',unicodedata.normalize('NFKD',value).casefold())


def compile_event_plan(guide,catalog):
    """Resolve the entire guide once, then leave applicability to its runtime."""
    events=[e for e in catalog if e['kind']=='event']
    options=[e for e in catalog if e['kind']=='event_option']
    bindings={};unresolved=[]
    for rows in guide.get('tracks',{}).values():
        for source in rows:
            original=source.get('action',{}).get('params',{})
            if not any(k in original for k in ('event_id','event_name','event_option_id','event_option')):continue
            step=copy.deepcopy(source);params=step['action']['params']
            reason=bind_political(step,events+options,None)
            resolution='catalog_identity'
            if reason!='event_visible_option' and params.get('event_option_id') is None:
                # A guide may quote a substantial contiguous part of a button.
                # Accept only a unique whole-word phrase, never prose inference,
                # fuzzy similarity, or a fallback from an explicit invalid ID.
                quote=original.get('event_option');words=_words(quote) if isinstance(quote,str) else []
                scope=params.get('event_id')
                if original.get('event_name') is not None and not scope:words=[]
                candidates={}
                if len(words)>=4:
                    needle=' '+' '.join(words)+' '
                    for option in options:
                        if scope is not None and option.get('event_id')!=scope:continue
                        names=[option.get('name',''),option.get('localized_name',''),*option.get('aliases',[])]
                        if any(needle in ' '+' '.join(_words(n))+' ' for n in names if isinstance(n,str)):
                            candidates[(option.get('event_id'),option['id'])]=option
                if len(candidates)==1:
                    option=next(iter(candidates.values()))
                    # Even a quoted label must belong to a catalogued event.
                    if any(e['id']==option.get('event_id') for e in events):
                        params.update(event_id=option['event_id'],event_option_id=option['id'])
                        reason='event_visible_option';resolution='unique_label_fragment'
            event_id=params.get('event_id');option_id=params.get('event_option_id')
            choices=[e for e in options if e['id']==option_id and e.get('event_id')==event_id]
            definitions=[e for e in events if e['id']==event_id]
            if reason!='event_visible_option' or len(choices)!=1 or len(definitions)!=1:
                unresolved.append({'key':source['key'],'title':source.get('title') or source['key'],
                                   'reason':reason or 'event_reference_missing'})
                continue
            option=choices[0];event=definitions[0]
            labels=list(dict.fromkeys(n for n in (option.get('localized_name'),option.get('name'),
                                                 *option.get('aliases',[])) if isinstance(n,str) and n))
            titles=list(dict.fromkeys(n for n in (event.get('localized_name'),event.get('name')) if isinstance(n,str) and n))
            bindings[source['key']]={'key':source['key'],'panel':'event','identity':event_id+':'+option_id,
                'event_id':event_id,'option_id':option_id,'labels':labels,'event_titles':titles,
                'event_title':titles[0] if titles else event_id,'resolution':resolution,
                'title':labels[0] if labels else option_id}
    return {'schema_version':1,'guide_id':guide.get('id'),'bindings':bindings,'unresolved':unresolved}
