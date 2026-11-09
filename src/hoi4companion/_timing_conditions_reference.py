"""Compile complete, bounded timing phrases to existing declarative predicates.

No observations, strategy, native reads or guessed campaign outcomes live here.
"""
import re

from .contracts import EXPERIENCE_CURRENCIES
from .resolution import ResolutionContext,catalog_index


def _fact(key,test,value):return {'op':'fact','key':key,'test':test,'value':value}


def _selected(target):
    return {'op':'any','args':[_fact('focus.active','contains',target),_fact('focus.completed','contains',target)]}


def _atom(text,catalog,country,scope,currency):
    from .campaign_conditions import AFTER_WAR,_AFTER_WAR
    deadline=re.fullmatch(r'(?:(?:on|after)\s+([A-Z0-9]{3})\s+down\s*/\s*)?no later than\s+([A-Za-z]+)\s+(\d{4}|\d{2})',text,re.I)
    if deadline:
        from .campaign_calendar import MONTH_DAYS
        months=('january','february','march','april','may','june','july','august','september','october','november','december')
        matches=[i+1 for i,m in enumerate(months) if deadline[2].lower() in (m,m[:3])]
        if not matches:return None
        year=int(deadline[3])+(1900 if len(deadline[3])==2 else 0)
        if not 1<=year<=9999:return None
        month=matches[0]
        # A month-only latest deadline is certainly reached at the end of it.
        by_date=_fact('campaign.date','gte',year*10000+month*100+MONTH_DAYS[month-1])
        if not deadline[1]:return by_date
        # No qualified defeat observer currently supplies this fact. It stays
        # unknown; a reached deadline can still satisfy the explicit alternative.
        return {'op':'any','args':[_fact('campaign.defeated.'+deadline[1].upper(),'eq',True),by_date]}
    if text.casefold().strip(' .:') in _AFTER_WAR:
        return _fact(AFTER_WAR,'eq',True)
    threshold=re.fullmatch(r'(?:at|when|once)\s+(\d{1,7}(?:\.\d{1,4})?)\s*'
        r'(PP|political\s+power|AE|army\s+experience|NE|naval\s+experience|AFE|air\s+experience)',text,re.I)
    if threshold:
        unit=' '.join(threshold[2].casefold().split())
        keys={'pp':'country.politics.political_power','political power':'country.politics.political_power',
              'ae':EXPERIENCE_CURRENCIES.get(currency,EXPERIENCE_CURRENCIES['army_experience']),
              'army experience':EXPERIENCE_CURRENCIES['army_experience'],'ne':EXPERIENCE_CURRENCIES['navy_experience'],
              'naval experience':EXPERIENCE_CURRENCIES['navy_experience'],'afe':EXPERIENCE_CURRENCIES['air_experience'],
              'air experience':EXPERIENCE_CURRENCIES['air_experience']}
        amount=float(threshold[1]);amount=int(amount) if amount.is_integer() else amount
        return _fact(keys[unit],'gte',amount)
    if catalog is None:return None
    hit=re.fullmatch(r'(on|after|when|once|before)\s+(.+)',text,re.I)
    if not hit:return None
    prefix,query=hit[1].casefold(),hit[2].strip()
    explicit=None;mode='completed' if prefix in ('after','once') else 'selected'
    picking=re.match(r'(?i)^(?:picking|selecting|starting)\s+',query)
    if picking:
        query=query[picking.end():];explicit='focus';mode='before' if prefix=='before' else 'selected'
    elif prefix=='before':return None
    marker=re.search(r'(?i)\s+(?:is\s+)?(done|completed|finished)$',query)
    if marker:query=query[:marker.start()].strip();mode='completed'
    marker=re.search(r'(?i)\s+(focus|research|tech|technology)$',query)
    if marker:explicit='focus' if marker[1].lower()=='focus' else 'technology';query=query[:marker.start()].strip()
    suffix=re.search(r'(?i)\s+(pick|selected|started)$',query)
    if suffix:explicit='focus';query=query[:suffix.start()].strip();mode='selected'
    query=query.strip(' "“”\'')
    if not query or re.search(r'[,;():]|\b(?:if|until|before|after|around|usually)\b',query,re.I):return None
    kinds=(explicit,) if explicit else ('focus','technology')
    index=catalog_index(catalog);context=ResolutionContext(country=country,scope=scope)
    candidates=[row for kind in kinds for row in index.matches(query,(kind,),context)]
    if not candidates and 'technology' in kinds:
        from .research_upgrades import expanded_research_name
        expanded=expanded_research_name(query)
        if expanded!=query:candidates=index.matches(expanded,('technology',),context)
    if len(candidates)!=1:return None
    entry=candidates[0]
    if entry['kind']=='technology':return _fact('research.completed','contains',entry['id'])
    if mode=='completed':return _fact('focus.completed','contains',entry['id'])
    condition=_selected(entry['id'])
    return {'op':'not','arg':condition} if mode=='before' else condition


def compile_timing(text,catalog=None,country=None,*,scope=None,currency=None,depth=0):
    if not isinstance(text,str) or len(text)>1000 or depth>6:return None
    text=' '.join(text.strip().rstrip(':').split())
    if not text:return None
    # Protect native entity names containing AND/OR by resolving the full atom
    # first. Every component of a compound expression must then be understood.
    atom=_atom(text,catalog,country,scope,currency)
    if atom:return atom
    quote=None;tokens=[];start=0
    for match in re.finditer(r'["“”]|\s+(and|or)\s+|;\s*',text,re.I):
        token=match[0]
        if token in ('"','“','”'):
            quote=None if quote else token;continue
        if quote:continue
        tokens.append((text[start:match.start()].strip(),'any' if (match[1] or '').lower()=='or' else 'all'))
        start=match.end()
    if not tokens:return None
    if len({op for _,op in tokens})!=1 or len(tokens)>15:return None
    parts=[part for part,_ in tokens]+[text[start:].strip()]
    conditions=[compile_timing(part,catalog,country,scope=scope,currency=currency,depth=depth+1) for part in parts]
    if any(c is None for c in conditions):return None
    return {'op':tokens[0][1],'args':conditions}
