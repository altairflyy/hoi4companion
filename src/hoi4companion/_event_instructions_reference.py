"""Resolve explicit Word event/button instructions, without choosing a strategy."""
import re

from ._informal_guides_reference import explicit_action
from .resolution import ResolutionContext
from .political_import import bind_political


def _unquote(text):
    return text.strip().strip('"“”‘’').strip()


def event_command(text, index, country):
    """Only a unique catalogued answer and its parent can become an action.

    No synonyms for outcomes, approximate spelling, option-number guesses or
    inference from a historical calendar. Original source evidence is retained
    by the operation compiler; this function neither reads nor changes the game.
    """
    if not isinstance(text,str) or not 0<len(text)<=1024 or '\n' in text:return None
    value=text.strip();policies={}
    cost=re.search(r'\s*\((\d{1,6})\s*PP\)\s*$',value,re.I)
    if cost:
        policies['political_power_cost']=int(cost[1]);value=value[:cost.start()].strip()
    event_name=None;explicit=False
    canonical=explicit_action(value)
    if canonical and canonical['type']=='event':
        event_name=canonical['event'];query=canonical['option'];explicit=True
    else:
        match=re.fullmatch(r'(?:on\s+)?event\s*:\s*(.+)',value,re.I)
        titled=re.fullmatch(r'(?:on\s+)?(?:the\s+)?(.+?)\s+event\s*:\s*(?:(?:choose|select|pick)\s*:?\s*)?(.+)',value,re.I)
        suffix=re.fullmatch(r'(.+?)\s*\(\s*event(?:\s*x\s*\d+)?\s*\)',value,re.I)
        if match:query=match[1];explicit=True
        elif titled:event_name,query=titled.groups();explicit=True
        elif suffix:query=suffix[1];explicit=True
        else:query=value
    query=_unquote(query)
    abbreviated=bool(re.search(r'(?:…|\.{3})\s*$',query))
    query=re.sub(r'(?:…|\.{3})\s*$','',query).strip()
    if not explicit and not abbreviated:return None
    if not query or '/' in query or ';' in query:return None
    # Two whole words are sufficient for an explicit abbreviated button only
    # when the local catalog makes the identity unique (e.g. "Go forth…").
    words=re.findall(r'\w+',query)
    if abbreviated and (len(words)<2 or sum(map(len,words))<7):return None
    # A short bare phrase such as "Welcome back…" can uniquely match an
    # unrelated country's event while the intended label is absent locally.
    # Reuse the event-plan fragment threshold unless the source says "event".
    context=ResolutionContext(country=country)
    filters={}
    if event_name:
        parents=index.matches(_unquote(event_name),('event',),context)
        if len(parents)!=1:return None
        filters['event_id']=parents[0]['id']
    options=index.matches(query,('event_option',),context,**filters)
    if len(options)!=1:return None
    option=options[0]
    parents=index.matches(option.get('event_id'),('event',),context,explicit=True)
    if len(parents)!=1:return None
    if not explicit and len(words)<4 and (not country or country not in parents[0].get('identity_countries',[])):return None
    # Plain ellipses must not silently reinterpret another native action.
    if not explicit and any(index.matches(query,(kind,),context) for kind in ('focus','technology','advisor','decision','idea')):return None
    params={'category':'politics','event_id':parents[0]['id'],'event_option_id':option['id']}
    probe={'action':{'kind':'manual','params':params}}
    if bind_political(probe,index,country)!='event_visible_option':return None
    return {'params':params,'policies':policies}
