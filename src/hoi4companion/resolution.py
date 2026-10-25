"""Catalog-generation indexes and conservative identity context.

No live availability, progress or strategy enters this module. False national
constraints prune identities; unknown constraints retain all candidates.
"""
from collections import defaultdict, OrderedDict
from copy import deepcopy
from dataclasses import dataclass
import re
from threading import RLock

from .research_upgrades import normal_name, typo_matches


@dataclass(frozen=True)
class ResolutionContext:
    country: str | None = None
    scope: str | None = None


def _country_test(node,country,depth=0):
    if not country or not isinstance(node,dict) or depth>32:return None
    op=node.get('op')
    if op=='tag':return node.get('value')==country if isinstance(node.get('value'),str) else None
    if op in ('true','false'):return op=='true'
    # An original tag is not necessarily the current tag (civil wars, releases).
    if op=='not':
        value=_country_test(node.get('arg'),country,depth+1)
        return None if value is None else not value
    if op in ('all','any') and isinstance(node.get('args'),list):
        values=[_country_test(arg,country,depth+1) for arg in node['args']]
        if op=='all':return False if False in values else None if None in values else True
        return True if True in values else None if None in values else False
    return None


class CatalogIndex:
    def __init__(self,entries):
        self._entries=tuple(deepcopy(list(entries)))
        self._kinds=defaultdict(set)
        self._ids=defaultdict(set)
        self._names=defaultdict(set)
        self._aliases=defaultdict(set)
        self._country_aliases=defaultdict(set)
        self._country_prefixes=defaultdict(set)
        self._prefixes=defaultdict(set)
        self._event_options=defaultdict(set)
        self._events=defaultdict(list)
        self._contexts=OrderedDict()
        self._lock=RLock()
        for i,row in enumerate(self._entries):
            kind=row['kind'];self._kinds[kind].add(i)
            self._ids[kind,row['id']].add(i)
            self._names[kind,normal_name(row['name'])].add(i)
            aliases=[row['id'],row.get('localized_name',''),*row.get('aliases',[])]
            for alias in aliases:
                if alias:self._aliases[kind,normal_name(alias)].add(i)
            for country,values in row.get('country_aliases',{}).items():
                for alias in values:
                    self._country_aliases[kind,country,normal_name(alias)].add(i)
                    words=tuple(re.findall(r'\w+',alias.casefold()))
                    for end in range(2,len(words)+1):self._country_prefixes[kind,country,words[:end]].add(i)
            for name in [row['name'],*aliases]:
                words=tuple(re.findall(r'\w+',name.casefold()))
                for end in range(2,len(words)+1):self._prefixes[kind,words[:end]].add(i)
            if kind=='event':self._events[row['id']].append(row)
            if kind=='event_option':self._event_options[row.get('event_id')].add(i)
        self._geography=None

    def __iter__(self):
        return iter(self._entries)

    def __len__(self):
        return len(self._entries)

    def export(self):
        return deepcopy(list(self._entries))

    @property
    def geography(self):
        with self._lock:
            if self._geography is None:
                from .geography import GeographyIndex
                self._geography=GeographyIndex(self._entries)
            return self._geography

    def _allowed(self,row,context):
        if context.scope and row['kind']=='focus' and row.get('scope')!=context.scope:return False
        if context.country:
            # The political `countries` field can be only an ID-prefix hint.
            # Where scripts provide a constraint it is the authoritative source.
            if 'country_condition' in row:
                national=_country_test(row['country_condition'],context.country)
                if national is False:return False
                if national is None and row.get('countries') and context.country not in row['countries']:return False
            elif row.get('countries') and context.country not in row['countries']:return False
            if row['kind']=='event_option':
                parents=self._events.get(row.get('event_id'),[])
                if parents and all(_country_test(p.get('country_condition'),context.country) is False for p in parents):return False
        return True

    def _domain(self,kinds,context,filters):
        kinds=tuple(sorted(kinds));key=(kinds,context)
        with self._lock:
            if key not in self._contexts:
                indices=set().union(*(self._kinds.get(kind,set()) for kind in kinds))
                self._contexts[key]=frozenset(i for i in indices if self._allowed(self._entries[i],context))
                if len(self._contexts)>64:self._contexts.popitem(last=False)
            self._contexts.move_to_end(key)
            indices=self._contexts[key]
        if 'event_id' in filters:indices=indices & self._event_options.get(filters['event_id'],set())
        if filters:indices=frozenset(i for i in indices if all(self._entries[i].get(k)==v for k,v in filters.items()))
        return indices

    def rows(self,kinds,context=None,**filters):
        return [self._entries[i] for i in sorted(self._domain(kinds,context or ResolutionContext(),filters))]

    def matches(self,query,kinds,context=None,*,explicit=False,typos=False,**filters):
        if not isinstance(query,str) or not query:return []
        context=context or ResolutionContext();domain=self._domain(kinds,context,filters)
        ids=set().union(*(self._ids.get((kind,query),set()) for kind in kinds))
        if ids or explicit:return [self._entries[i] for i in sorted(ids & domain)]
        needle=normal_name(query)
        names=set().union(*(self._names.get((kind,needle),set()) for kind in kinds)) & domain
        aliases=set().union(*(self._aliases.get((kind,needle),set()) | self._country_aliases.get((kind,context.country,needle),set()) for kind in kinds)) & domain
        words=tuple(re.findall(r'\w+',query.casefold()))
        prefixes=set().union(*(self._prefixes.get((kind,words),set()) | self._country_prefixes.get((kind,context.country,words),set()) for kind in kinds)) & domain if len(words)>=2 else set()
        selected=names or aliases or prefixes
        if selected:return [self._entries[i] for i in sorted(selected)]
        return typo_matches(query,self.rows(kinds,context,**filters)) if typos and not explicit else []

    def excluded(self,query,kinds,context,**filters):
        all_matches=self.matches(query,kinds,typos=True,**filters)
        return [{'entity':deepcopy(row),'reason':'country_or_scope_conflict'} for row in all_matches if not self._allowed(row,context)]

    def audit(self,query,kinds,context,*,explicit=False,**filters):
        def identity(row):
            return {k:deepcopy(row[k]) for k in ('kind','id','scope','name','event_id','slot','group') if k in row}
        selected=self.matches(query,kinds,context,explicit=explicit,**filters)
        all_matches=self.matches(query,kinds,explicit=explicit)
        excluded=[]
        for row in all_matches:
            reason=('country_or_scope_conflict' if not self._allowed(row,context) else
                    'declared_parent_or_role_conflict' if any(row.get(k)!=v for k,v in filters.items()) else None)
            if reason:excluded.append({'entity':identity(row),'reason':reason})
        return {'query':query,'country':context.country,'scope':context.scope,'filters':filters,
                'candidates':[identity(row) for row in selected],'excluded_candidates':excluded}


def catalog_index(entries):
    return entries if isinstance(entries,CatalogIndex) else CatalogIndex(entries)
