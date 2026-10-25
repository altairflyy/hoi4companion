"""Deterministic propagation of authored focus routes; no live facts or strategy."""
from .resolution import catalog_index, ResolutionContext


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
            choices = [self.check((key[0],parent),stack|{key}) for parent in group]
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
        for group in node.get('prerequisites',[]):
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


def resolve_focus_routes(guide, catalog):
    """Bind names from the whole declared route, independent of track/UI order.

    Conditional steps/phases/dependencies cannot assert a mandatory branch.
    Missing ancestors and cycles remain unknown; omitted focuses are not false.
    """
    index=catalog_index(catalog)
    rows = {row['key']:row for lane in guide['tracks'].values() for row in lane}
    phases = {phase['id']:phase for phase in guide.get('phases',[])}
    nodes = {_identity(entry):entry for entry in index.rows(('focus',))}
    domains, mandatory = {}, {}

    def conditional(key, stack=frozenset()):
        row = rows[key]
        return (key in stack or 'when' in row or 'skip_when' in row or
                'when' in phases.get(row.get('phase'),{}) or
                any(conditional(dep,stack|{key}) for dep in row.get('depends_on',[])))

    for key,row in rows.items():
        if row['action']['kind']!='select_focus':
            continue
        ref = row['action']['entity']
        scope = ref.get('scope') or guide['applies_to'].get('tree_id')
        country = guide['applies_to'].get('country')
        domains[key] = index.matches(ref['id'],('focus',),ResolutionContext(country=country,scope=scope),typos=True)
        mandatory[key] = not conditional(key)

    anchors = {_identity(matches[0]) for key,matches in domains.items() if mandatory[key] and len(matches)==1}
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
        result[key] = {'matches':selected,'excluded_candidates':excluded,
                       'supporting_focus_ids':sorted(support),'unresolved_route':unresolved,
                       'has_name_matches':bool(matches),'viable_candidates':viable,
                       'inferred':len(matches)>1 and len(selected)==1}
    return result,route.summary(),errors
