"""Legacy catalog test oracle; never a production fallback.

No live availability, progress or strategy enters this module. False national
constraints prune identities; unknown constraints retain all candidates.
"""
from collections import defaultdict, OrderedDict
from copy import deepcopy
from dataclasses import dataclass
import re
from threading import RLock
from difflib import SequenceMatcher

from .research_upgrades import normal_name, typo_matches, expanded_research_name, guide_language


@dataclass(frozen=True)
class ResolutionContext:
    country: str | None = None
    scope: str | None = None


def optional_focus_name(query):
    if not isinstance(query,str):return None
    parts=[x.strip() for x in query.split('/')]
    if len(parts)!=2:return None
    empty={'nothing','none','no focus'}
    if parts[1].casefold() in empty:return parts[0] or None
    if parts[0].casefold() in empty:return parts[1] or None
    return None


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
    def __init__(self,entries,*,country_names=None,adjectives=(),reference_designs=None):
        self.reference_designs=deepcopy(reference_designs or {})
        self._country_names=deepcopy(country_names or {})
        self._adjectives={normal_name(word) for word in adjectives if isinstance(word,str) and word and '[' not in word}
        self._adjective_templates=[]
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
        self._suggestion_words=defaultdict(set)
        self._suggestion_labels={}
        self._focus_word_signatures=defaultdict(set)
        self._focus_full_words=defaultdict(set)
        self._focus_deleted_words=defaultdict(set)
        self._focus_word_labels={}
        self._suggestions=OrderedDict()
        self._contexts=OrderedDict()
        self._lock=RLock()
        for i,row in enumerate(self._entries):
            kind=row['kind'];self._kinds[kind].add(i)
            if kind=='focus' and re.search(r'\[(?:ROOT|[A-Z0-9]{3})\.GetAdjective\]',row['name']):
                before,after=re.split(r'\[(?:ROOT|[A-Z0-9]{3})\.GetAdjective\]',row['name'],maxsplit=1)
                self._adjective_templates.append((i,re.compile('^'+re.escape(before)+r'(.+?)'+re.escape(after)+'$',re.I)))
            self._ids[kind,row['id']].add(i)
            self._names[kind,normal_name(row['name'])].add(i)
            aliases=[row['id'],row.get('localized_name',''),*row.get('aliases',[])]
            aliases += [alias for alias,target in guide_language().get('entity_names',{}).get(kind,{}).items()
                        if normal_name(target)==normal_name(row['name']) or target==row['id']]
            self._suggestion_labels[i]=[(name,normal_name(name)) for name in [row['name'],*row.get('aliases',[])]
                                        if isinstance(name,str) and name]
            if kind=='focus':
                self._focus_word_labels[i]=[(label,tuple(normal_name(word) for word in re.findall(r'[^\W_]+',label)))
                                            for label,_ in self._suggestion_labels[i]]
                for _,words in self._focus_word_labels[i]:
                    if len(words)<2:continue
                    self._focus_full_words[words].add(i)
                    for position in range(len(words)):
                        self._focus_word_signatures[(position,*words[:position],*words[position+1:])].add(i)
                        self._focus_deleted_words[words[:position]+words[position+1:]].add(i)
            for alias in aliases:
                if alias:self._aliases[kind,normal_name(alias)].add(i)
            if kind=='focus' and re.match(r'^To\s+',row['name'],re.I):
                self._aliases[kind,normal_name(re.sub(r'^To\s+','',row['name'],flags=re.I))].add(i)
            for country,values in row.get('country_aliases',{}).items():
                for alias in values:
                    self._country_aliases[kind,country,normal_name(alias)].add(i)
                    words=tuple(re.findall(r'\w+',alias.casefold()))
                    for end in range(2,len(words)+1):self._country_prefixes[kind,country,words[:end]].add(i)
                    if len(words)>1 and len(words[0])>=8 and all(w.isdecimal() for w in words[1:]):
                        self._country_prefixes[kind,country,words[:1]].add(i)
            for name in [row['name'],*aliases]:
                words=tuple(re.findall(r'\w+',name.casefold()))
                for word in words:
                    if len(word)>=3:self._suggestion_words[word].add(i)
                for end in range(2,len(words)+1):self._prefixes[kind,words[:end]].add(i)
                if len(words)>1 and len(words[0])>=8 and all(w.isdecimal() for w in words[1:]):
                    self._prefixes[kind,words[:1]].add(i)
            if kind=='event':self._events[row['id']].append(row)
            if kind=='event_option':self._event_options[row.get('event_id')].add(i)
        self._geography=None

    def __iter__(self):
        return iter(self._entries)

    def __len__(self):
        return len(self._entries)

    def export(self):
        return deepcopy(list(self._entries))

    def resolve_country(self,query):
        """Exact tag or unique installed name; no fuzzy geopolitical guesses."""
        if not isinstance(query,str) or not query.strip():return None
        tag=query.strip().upper()
        if re.fullmatch('[A-Z0-9]{3}',tag) and (not self._country_names or tag in self._country_names):return tag
        needle=normal_name(query)
        matches={tag for tag,names in self._country_names.items() if any(normal_name(n)==needle for n in names)}
        return next(iter(matches)) if len(matches)==1 else None

    def country_hint(self,title,guide):
        """A source title plus independent focus identities can qualify a tag.

        A country word alone is insufficient (e.g. a China guide may follow a
        warlord). Require unique strongest support from at least two distinct
        declared focus names; do not use the currently played country.
        """
        if not isinstance(title,str):return None
        candidates={tag for tag,names in self._country_names.items() if any(
            re.match(re.escape(name)+r'(?=$|[\s_(:—-])',title,re.I) for name in names if name)}
        if not candidates:return None
        support=defaultdict(set)
        for row in (row for lane in guide['tracks'].values() for row in lane):
            if row['action']['kind']!='select_focus':continue
            ref=row['action']['entity'];query=ref['id']
            for entry in self.matches(query,('focus',),ResolutionContext(scope=ref.get('scope')),typos=True):
                for tag in entry.get('countries',[]):support[tag].add(normal_name(query))
        if not support:return None
        maximum=max(map(len,support.values()))
        leaders={tag for tag,values in support.items() if len(values)==maximum}
        qualified=leaders & candidates
        return next(iter(qualified)) if maximum>=2 and len(qualified)==1 else None

    @property
    def geography(self):
        with self._lock:
            if self._geography is None:
                from .geography import GeographyIndex
                self._geography=GeographyIndex(self._entries)
            return self._geography

    def _allowed(self,row,context):
        if context.scope and row['kind']=='focus' and not row.get('scope_independent') and row.get('scope')!=context.scope:return False
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
        optional=optional_focus_name(query) if tuple(kinds)==('focus',) else None
        if optional:return [r for r in self.matches(optional,kinds,context,typos=typos,**filters) if r.get('continuous')]
        needle=normal_name(query)
        names=set().union(*(self._names.get((kind,needle),set()) for kind in kinds)) & domain
        aliases=set().union(*(self._aliases.get((kind,needle),set()) | self._country_aliases.get((kind,context.country,needle),set()) for kind in kinds)) & domain
        words=tuple(re.findall(r'\w+',query.casefold()))
        prefixes=set().union(*(self._prefixes.get((kind,words),set()) | self._country_prefixes.get((kind,context.country,words),set()) for kind in kinds)) & domain if words else set()
        selected=names or aliases or prefixes
        if selected:return [self._entries[i] for i in sorted(selected)]
        if tuple(kinds)==('focus',) and '/' in query:
            variants=[]
            for i,pattern in self._adjective_templates:
                if i not in domain:continue
                match=pattern.fullmatch(query)
                parts=[normal_name(x.strip()) for x in match[1].split('/')] if match else []
                if 2<=len(parts)<=8 and all(part in self._adjectives for part in parts):variants.append(self._entries[i])
            if variants:return variants
        if tuple(kinds)==('focus',) and re.match(r'^To\s+',query,re.I):
            return self.matches(re.sub(r'^To\s+','',query,flags=re.I),kinds,context,typos=typos,**filters)
        if tuple(kinds)==('technology',):
            expanded=expanded_research_name(query)
            if expanded!=query:return self.matches(expanded,kinds,context,typos=typos,**filters)
        if typos and not explicit:
            rows=self.rows(kinds,context,**filters)
            near=typo_matches(query,rows)
            if near:return near
            if tuple(kinds)==('focus',) and context.scope:
                return typo_matches(query,rows,max_edits=3)
        return []

    def excluded(self,query,kinds,context,**filters):
        all_matches=self.matches(query,kinds,typos=True,**filters)
        return [{'entity':deepcopy(row),'reason':'country_or_scope_conflict'} for row in all_matches if not self._allowed(row,context)]

    def focus_word_candidates(self,query,context):
        """Indexed positional word variants; the focus route qualifies each identity."""
        if not context.country and not context.scope:return []
        if not isinstance(query,str) or '_' in query or any(mark in query for mark in ('/','\\','…','...')):return []
        words=tuple(normal_name(word) for word in re.findall(r'[^\W_]+',query))
        if len(words)<2:return []
        nearby=set().union(*(self._focus_word_signatures.get((position,*words[:position],*words[position+1:]),set())
                             for position in range(len(words))))
        nearby.update(self._focus_deleted_words.get(words,set()))
        for position in range(len(words)):nearby.update(self._focus_full_words.get(words[:position]+words[position+1:],set()))
        def close(other):
            if len(other)==len(words):return sum(a!=b for a,b in zip(words,other))==1
            longer,shorter=(other,words) if len(other)>len(words) else (words,other)
            return len(longer)==len(shorter)+1 and any(longer[:i]+longer[i+1:]==shorter for i in range(len(longer)))
        return [(self._entries[i],label) for i in sorted(nearby & self._domain(('focus',),context,{}))
                for label,other in self._focus_word_labels[i]
                if close(other)]

    def suggest(self,query,kinds,context=None,*,limit=6):
        """Review candidates only. Scores never participate in binding IDs."""
        if not isinstance(query,str) or not query.strip():return []
        cache_key=(query,tuple(kinds),context,limit)
        with self._lock:
            if cache_key in self._suggestions:
                self._suggestions.move_to_end(cache_key)
                return deepcopy(self._suggestions[cache_key])
        domain=self._domain(kinds,context or ResolutionContext(),{})
        words=set(re.findall(r'\w+',query.casefold()))-{'the','and','for','with','from','that','this','into'}
        nearby=set().union(*(self._suggestion_words.get(word,set()) for word in words if len(word)>=3)) & domain
        # A single misspelled word can have no exact token overlap. This wider
        # pass remains proposal-only and never changes matches()/native IDs.
        if not nearby:nearby=domain
        needle=normal_name(query)
        scored=[]
        for i in nearby:
            row=self._entries[i]
            scored_labels=[(SequenceMatcher(None,needle,normalized).ratio(),label)
                           for label,normalized in self._suggestion_labels[i]]
            if not scored_labels:continue
            score,label=max(scored_labels)
            if score>=0.55:scored.append((score,row['id'],row.get('scope',''),label,row))
        scored.sort(key=lambda item:(-item[0],item[1],item[2]))
        result=[{**deepcopy(row),'suggestion':{'basis':'text_similarity_only','score':round(score,4),
                 'matched_label':label,'requires_review':True}} for score,_,_,label,row in scored[:limit]]
        with self._lock:
            self._suggestions[cache_key]=result
            if len(self._suggestions)>512:self._suggestions.popitem(last=False)
        return deepcopy(result)

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
