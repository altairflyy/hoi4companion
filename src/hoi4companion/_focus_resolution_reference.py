"""Deterministic propagation of authored focus routes; no live facts or strategy."""
from .resolution import catalog_index, ResolutionContext, optional_focus_name
from .research_upgrades import normal_name, guide_language
import re


def _same_qualifiers(query,label):
    """A named level or geographic direction is not a recoverable typo."""
    def levels(value):
        return [normal_name(token) for token in re.findall(r'\b(?:\d+(?:st|nd|rd|th)?|VIII|VII|VI|IV|IX|III|II|I|V|X)\b',value,re.I)]
    if levels(query)!=levels(label):return False
    directions={'east':'east','eastern':'east','eastward':'east','west':'west','western':'west','westward':'west',
                'north':'north','northern':'north','northward':'north','south':'south','southern':'south','southward':'south'}
    def bearing(value):return {directions[word] for word in re.findall(r'\w+',value.casefold()) if word in directions}
    return bearing(query)==bearing(label)


def _word_variant(query, label):
    """One positional word substitution, never a new level or strategic qualifier."""
    if '_' in query or any(mark in query for mark in ('/', '\\', '…', '...')):
        return None
    words=lambda value:[normal_name(word) for word in re.findall(r'[^\W_]+',value)]
    left,right=words(query),words(label)
    if min(len(left),len(right))<2 or abs(len(left)-len(right))>1 or not _same_qualifiers(query,label):return None
    method='one_positional_word_difference'
    if len(left)==len(right):
        different=[(a,b) for a,b in zip(left,right) if a!=b]
        shared=[a for a,b in zip(left,right) if a==b]
        if len(different)!=1 or any(len(word)<2 for pair in different for word in pair):return None
    else:
        longer,shorter=(left,right) if len(left)>len(right) else (right,left)
        removed=next((i for i in range(len(longer)) if longer[:i]+longer[i+1:]==shorter),None)
        if removed is None:return None
        different=[(longer[removed],'') if len(left)>len(right) else ('',longer[removed])]
        shared=shorter;method='one_word_insertion_or_omission'
    # Protect literal gameplay distinctions even when only one word differs.
    protected={'basic','improved','advanced','modern','medium','small','large','light','heavy',
               'democratic','communist','communism','fascist','fascism','monarchist','neutrality',
               'not','no','never','without','against','defense','defence','offense','offence',
               'attack','defend','land','naval','air'}
    if any(word in protected or word.isdecimal() for pair in different for word in pair):return None
    stop={'the','and','from','with','that','this','into','for','of','to','a','an','in','on','at','by','or','as'}
    meaningful=[w for w in shared if w not in stop]
    if not any(len(w)>=4 for w in meaningful) and sum(len(w)>=3 for w in meaningful)<2:return None
    return {'method':method,'matched_label':label,
            'changed_words':[{'source':a,'catalog':b} for a,b in different]}


def infer_focus_scope(guide, catalog):
    """Return a unique shared scope supported by at least two distinct names.

    An unmatched name supplies no evidence. Explicit scopes and countries are
    constraints; multiple possible trees must remain unresolved.
    """
    scope = guide.get('applies_to', {}).get('tree_id')
    if scope:
        return None
    index = catalog_index(catalog)
    country = guide.get('applies_to', {}).get('country')
    allowed, names = None, set()
    for row in (step for lane in guide['tracks'].values() for step in lane):
        if row['action']['kind'] != 'select_focus': continue
        ref = row['action']['entity']; query = ref['id']
        found = index.matches(query, ('focus',), ResolutionContext(country=country, scope=ref.get('scope')))
        candidates = {item['scope'] for item in found if item.get('scope')}
        if not candidates: continue
        names.add(normal_name(query)); allowed = candidates if allowed is None else allowed & candidates
        if not allowed: return None
    return next(iter(allowed)) if len(names) >= 2 and allowed and len(allowed) == 1 else None


def _identity(row):
    return row.get('scope'), row['id']


class _Route:
    def __init__(self, nodes, anchors):
        self.nodes, self.required = nodes, set(anchors)
        self.conflicts, self.cache = [], {}
        self.opposites = {}
        for key, node in nodes.items():
            for other in node.get('mutually_exclusive', []):
                opposite = (key[0], other)
                self.opposites.setdefault(key, set()).add(opposite)
                self.opposites.setdefault(opposite, set()).add(key)
        # Only guaranteed ancestors are added. OR alternatives are intersected.
        while True:
            extra = set()
            for key in sorted(self.required, key=str):
                status, witness = self.check(key)
                if status is False:
                    self.conflicts.append({'entity': {'scope':key[0], 'id':key[1]},
                                           'conflicting_focus_ids': sorted(k[1] for k in witness)})
                else:
                    extra.update(self.guaranteed(key))
            extra -= self.required
            if self.conflicts or not extra:
                break
            self.required.update(extra)
            self.cache.clear()

    def check(self, key, stack=frozenset()):
        if key in self.cache:
            return self.cache[key]
        opposed = self.opposites.get(key,set()) & self.required
        if opposed:
            return False, opposed
        node = self.nodes.get(key)
        if node is None or 'prerequisites' not in node or key in stack:
            return None, set()
        unknown, witnesses = False, set()
        for group in node['prerequisites']:
            choices = []
            for parent in group:
                parent_key=(key[0],parent)
                choice=self.check(parent_key,stack|{key})
                definition=self.nodes.get(parent_key,{})
                if definition.get('bypass_possible') or definition.get('external_completion_possible'):
                    choice=(True,set())  # A possible path, never observed completion.
                choices.append(choice)
            if not any(status is True for status,_ in choices):
                if any(status is None for status,_ in choices):
                    unknown = True
                else:
                    for _, witness in choices:
                        witnesses.update(witness)
                    return False, witnesses
        result = (None if unknown else True), set()
        if not stack:
            self.cache[key] = result
        return result

    def guaranteed(self, key, stack=frozenset()):
        required = {key}
        node = self.nodes.get(key)
        if node is None or key in stack:
            return required
        if node.get('bypass_possible') or node.get('external_completion_possible'):
            return required
        for group in node.get('prerequisites',[]):
            if any(self.nodes.get((key[0],p),{}).get('bypass_possible')
                   or self.nodes.get((key[0],p),{}).get('external_completion_possible') for p in group):
                continue
            viable = [(key[0],parent) for parent in group if self.check((key[0],parent))[0] is not False]
            if viable:
                alternatives = [self.guaranteed(parent,stack|{key}) for parent in viable]
                required.update(set.intersection(*alternatives))
        return required

    def summary(self):
        return {'status':'conflict' if self.conflicts else
                'incomplete' if any(self.check(k)[0] is None for k in self.required) else 'no_conflict_detected',
                'required_focuses':[{'scope':scope,'id':key} for scope,key in sorted(self.required,key=str)],
                'conflicts':self.conflicts}


def _source_branch(guide, key, matches, rows, domains, mandatory, nodes, anchors):
    """Disambiguate identical labels by an explicitly documented normal route.

    Bypasses remain possible in the game. They are not invented as instructions
    to reach a differently named branch when the Word source gives a direct
    parent on its chosen, mutually exclusive route. This is identity context,
    never a completion observation or a claim that a bypass cannot happen.
    """
    if len(matches)<2 or len({normal_name(e['name']) for e in matches})!=1:return None
    row=rows[key]
    if row.get('focus_resolution',{}).get('locked'):return None
    files={s['id'] for s in guide.get('sources',[]) if s.get('type')=='file'
           and '[Table row]' in s.get('text','') and '[Column 3]' in s.get('text','')}
    if not any(e.get('source_id') in files for e in row.get('evidence',[])):return None
    if any(re.search(r'\bbypass(?:ed)?\b',str(r.get('note',''))+' '+str(r.get('title','')),re.I) for r in rows.values()):return None
    # A contradiction must not hide a missing/cyclic ancestor in this fallback.
    visited={};stack=[(_identity(entry),False) for entry in matches]
    while stack:
        identity,leaving=stack.pop()
        if leaving:visited[identity]=2;continue
        if visited.get(identity)==2:continue
        if visited.get(identity)==1 or identity not in nodes or len(visited)>1200:return None
        entry=nodes[identity]
        if 'prerequisites' not in entry:return None
        visited[identity]=1;stack.append((identity,True))
        stack.extend(((identity[0],p),False) for group in entry['prerequisites'] for p in group)
    normal={identity:{**entry,'bypass_possible':False,'external_completion_possible':False} for identity,entry in nodes.items()}
    route=_Route(normal,anchors)
    if route.conflicts:return None
    values=[route.check(_identity(entry))[0] for entry in matches]
    if values.count(True)!=1 or any(value is None for value in values):return None
    candidate=matches[values.index(True)]
    previous=set()
    for k,entries in domains.items():
        if not mandatory.get(k) or rows[k].get('_focus_position',0)>=row.get('_focus_position',0):continue
        bound={_identity(entry) for entry in entries} & anchors
        if len(bound)==1:previous.update(bound)
    parents={(candidate.get('scope'),p) for group in candidate.get('prerequisites',[]) for p in group}
    support=parents & previous
    if not support:return None
    return candidate,{'method':'declared_source_branch','supporting_focus_ids':sorted(p[1] for p in support),
                      'basis':'same_label_direct_predecessor_and_normal_route_exclusions',
                      'bypasses_are_still_possible':True,
                      'excluded_focus_ids':sorted(e['id'] for e,value in zip(matches,values) if value is False)}


def _resolve_name_routes(guide, catalog):
    """Bind names from the whole declared route, independent of track/UI order.

    Conditional steps/phases/dependencies cannot assert a mandatory branch.
    Missing ancestors and cycles remain unknown; omitted focuses are not false.
    """
    index=catalog_index(catalog)
    rows = {row['key']:row for lane in guide['tracks'].values() for row in lane}
    phases = {phase['id']:phase for phase in guide.get('phases',[])}
    nodes = {_identity(entry):entry for entry in index.rows(('focus',))}
    other_kinds=tuple(sorted({entry['kind'] for entry in index if entry['kind']!='focus'}))
    identity_graph=_Route(nodes,set())
    domains, mandatory = {}, {}
    word_choices={}
    government_hints={guide_language().get('government_names',{}).get(label.strip().casefold())
                      for label in re.findall(r'\(([^()]*)\)',guide.get('title',''))}
    government_hints.discard(None)
    government=next(iter(government_hints)) if len(government_hints)==1 else None
    government_choices={};government_excluded={}

    def conditional(key, stack=frozenset()):
        row = rows[key]
        return (key in stack or 'when' in row or 'skip_when' in row or
                'when' in phases.get(row.get('phase'),{}) or
                any(conditional(dep,stack|{key}) for dep in [*row.get('depends_on',[]),*row.get('depends_on_started',[])]))

    for key,row in rows.items():
        if row['action']['kind']!='select_focus':
            continue
        ref = row['action']['entity']
        scope = ref.get('scope') or guide['applies_to'].get('tree_id')
        country = guide['applies_to'].get('country')
        domains[key] = index.matches(ref['id'],('focus',),ResolutionContext(country=country,scope=scope),typos=True)
        competing_names={ref['id'],re.sub(r'\b(?:a|an|the)\s+', '', ref['id'],flags=re.I).strip()}
        competing_entity=not domains[key] and any(index.matches(name,other_kinds,ResolutionContext(country=country))
                                                for name in competing_names)
        if not domains[key] and not competing_entity:
            context=ResolutionContext(country=country,scope=scope)
            variants={}
            for entry,label in index.focus_word_candidates(ref['id'],context):
                evidence=_word_variant(ref['id'],label)
                if evidence and _identity(entry) not in variants and identity_graph.check(_identity(entry))[0] is True:
                    domains[key].append(entry);variants[_identity(entry)]=evidence
            if variants:word_choices[key]=variants
        if len(domains[key])>1 and government:
            excluded=[entry for entry in domains[key] if government in entry.get('excluded_governments',[])]
            domains[key]=[entry for entry in domains[key] if entry not in excluded]
            government_excluded[key]=[{'entity':entry,'reason':'declared_guide_government_conflict',
                'government':government,'basis':'guide_title'} for entry in excluded]
            if excluded and len(domains[key])==1:government_choices[key]={'government':government,'basis':'guide_title'}
        mandatory[key] = not conditional(key) and not optional_focus_name(ref['id'])

    anchors = {_identity(matches[0]) for key,matches in domains.items() if mandatory[key] and len(matches)==1}
    recovery = {}
    name_matches = {key:bool(matches) for key,matches in domains.items()}
    suggestions = {}
    # Every iteration adds a previously unresolved identity; no heuristic score.
    while True:
        route, extra = _Route(nodes,anchors), set()
        if route.conflicts:
            break
        for key,matches in domains.items():
            if not mandatory[key] or len(matches)<2:
                continue
            viable = [entry for entry in matches if route.check(_identity(entry))[0] is not False]
            if len(viable)==1 and route.check(_identity(viable[0]))[0] is True:
                extra.add(_identity(viable[0]))
            elif len(viable)>1:
                contextual=_source_branch(guide,key,matches,rows,domains,mandatory,nodes,anchors)
                if contextual:
                    candidate,evidence=contextual
                    domains[key]=[candidate];recovery[key]=evidence
                    extra.add(_identity(candidate))
        # A later declared focus proves which ancestors its route needs, not
        # which arbitrary unknown word denotes one of those ancestors. Recover
        # only a substantial name with one plausible textual candidate across
        # all compatible branches, including parallel branches. Never use UI
        # adjacency, subtract omitted ancestors, or select an OR prerequisite.
        for key,matches in domains.items():
            ref=rows[key]['action']['entity'];query=ref['id']
            if matches or not mandatory[key] or '_' in query or len(normal_name(query))<10:
                continue
            if key not in suggestions:
                context=ResolutionContext(country=guide['applies_to'].get('country'),
                    scope=ref.get('scope') or guide['applies_to'].get('tree_id'))
                suggestions[key]=index.suggest(query,('focus',),context,limit=128)
            proposed=suggestions[key]
            if len(proposed)>=128:continue
            plausible=[item for item in proposed if item['suggestion']['score']>=0.8
                       and _same_qualifiers(query,item['suggestion']['matched_label'])
                       and route.check(_identity(item))[0] is not False]
            if len(plausible)!=1:continue
            candidate=plausible[0];identity=_identity(candidate)
            if identity not in route.required or identity in anchors or route.check(identity)[0] is not True:
                continue
            supporting=[anchor for anchor in anchors if anchor!=identity
                        and route.check(anchor)[0] is True and identity in route.guaranteed(anchor)]
            if not supporting:continue
            canonical=nodes[identity]
            domains[key]=[canonical]
            recovery[key]={'method':'unique_name_and_required_ancestor',
                           'supporting_focus_ids':sorted({item[1] for item in supporting}),
                           'matched_label':candidate['suggestion']['matched_label']}
            extra.add(identity)
        extra -= anchors
        if not extra:
            break
        anchors.update(extra)

    result, errors = {}, []
    if route.conflicts:
        errors.append('Percorso della guida incompatibile: '+
                      '; '.join(c['entity']['id']+' / '+', '.join(c['conflicting_focus_ids']) for c in route.conflicts))
    for key,matches in domains.items():
        excluded, viable, support = [], [], set()
        audit = list(matches)
        if len(matches)==1:
            # Keep the explanation after the guide has been compiled to native IDs.
            siblings=index.matches(matches[0]['name'],('focus',),ResolutionContext(scope=matches[0].get('scope')))
            for sibling in siblings:
                if _identity(sibling)!=_identity(matches[0]): audit.append(sibling)
        for entry in audit:
            status,witness = route.check(_identity(entry)) if mandatory[key] and not route.conflicts else (None,set())
            if status is False:
                excluded.append({'entity':entry,'reason':'route_mutual_exclusion',
                                 'conflicting_focus_ids':sorted(k[1] for k in witness)})
                support.update(k[1] for k in witness)
            elif entry in matches:
                viable.append(entry)
        selected = matches
        unresolved = False
        if len(matches)>1 and mandatory[key] and not route.conflicts:
            selected = viable
            if len(viable)==1 and route.check(_identity(viable[0]))[0] is None:
                selected,unresolved = [],True
        # A relaxed name alone cannot qualify an incomplete or cyclic route.
        if key in word_choices and len(selected)==1 and route.check(_identity(selected[0]))[0] is not True:
            selected,unresolved=[],True
        if key in recovery:support.update(recovery[key]['supporting_focus_ids'])
        result[key] = {'matches':selected,'candidate_domain':list(matches),'excluded_candidates':[*government_excluded.get(key,[]),*excluded],
                       'supporting_focus_ids':sorted(support),'unresolved_route':unresolved,
                       'has_name_matches':name_matches[key],'viable_candidates':viable,
                       'inferred':bool(key in recovery or len(matches)>1 and len(selected)==1),
                       **({'government_context':government_choices[key]} if key in government_choices else {}),
                       **({'word_variant':word_choices[key][_identity(selected[0])]} if key in word_choices and len(selected)==1 else {}),
                       **({'recovery':recovery[key]} if key in recovery else {})}
    return result,route.summary(),errors


def resolve_focus_routes(guide, catalog):
    """Combine name/branch resolution with opt-in complete source constraints."""
    from copy import deepcopy
    from ._focus_sequence_reference import source_guide, reconstruct
    index = catalog_index(catalog)
    source = source_guide(guide)
    result, route, errors = _resolve_name_routes(source, index)
    rows = {r['key']: r for lane in source['tracks'].values() for r in lane
            if r['action']['kind'] == 'select_focus'}
    for key, row in rows.items():
        ref = row['action']['entity']; query = ref['id']
        # Revisit weak automatic bindings. Exact source names/native IDs and
        # manual overrides stay fixed; a contradiction is exposed for review.
        context = ResolutionContext(country=source['applies_to'].get('country'),
                                    scope=ref.get('scope') or source['applies_to'].get('tree_id'))
        exact = index.matches(query, ('focus',), context)
        if (source.get('focus_sequence',{}).get('coverage')=='complete' and not exact
                and not row.get('focus_resolution', {}).get('locked')):
            candidates = result[key]['candidate_domain']
            for item in index.suggest(query, ('focus',), context, limit=32):
                if (item['suggestion']['score'] >= .8
                        and _same_qualifiers(query, item['suggestion']['matched_label'])
                        and _identity(item) not in {_identity(x) for x in candidates}):
                    candidates.append(item)
    forced, sequence, sequence_errors = reconstruct(source, index, result)
    changed = {key: item for key, item in forced.items()
               if len(result[key]['candidate_domain']) != 1
               or _identity(result[key]['candidate_domain'][0]) != _identity(item)}
    if changed:
        qualified = deepcopy(source)
        for lane in qualified['tracks'].values():
            for row in lane:
                if row['key'] in forced:
                    item = forced[row['key']]
                    row['action']['entity'] = {k: item[k] for k in ('kind', 'id', 'scope') if k in item}
        result, route, errors = _resolve_name_routes(qualified, index)
        for key, item in changed.items():
            result[key].update(matches=[item], inferred=True,
                               recovery={'method': 'unique_global_sequence',
                                         'source_position': rows[key]['_focus_position'],
                                         'coverage': 'complete', 'counterexample_found': False})
        sequence['reconstructed_count'] = len(changed)
        sequence['corrections'] = [{'key': key, 'query': rows[key]['action']['entity']['id'],
                                    'id': item['id']} for key, item in changed.items()]
    probable = {r['key']:r['candidates'] for r in sequence.get('ambiguous_slots',[])}
    for key, item in result.items():
        row = rows[key]; matches = item['matches']
        classification = ('reconstructed' if key in changed or item.get('recovery',{}).get('method')=='declared_source_branch' else 'direct' if len(matches) == 1
                          else 'probable' if item['candidate_domain'] or probable.get(key) else 'unresolved')
        if key in probable and len(matches)!=1:item['probable_candidates']=probable[key]
        item['classification'] = classification
        item['source_position'] = row['_focus_position']
        item['source_query'] = row.get('focus_resolution',{}).get('query',row['action']['entity']['id'])
        item['locked'] = row.get('focus_resolution', {}).get('locked', False)
    sequence['classification_counts']={label:sum(r['classification']==label for r in result.values())
                                      for label in ('direct','reconstructed','probable','unresolved')}
    sequence['reconstructed_count']=sequence['classification_counts']['reconstructed']
    route['sequence'] = sequence
    return result, route, list(dict.fromkeys([*errors, *sequence_errors]))
