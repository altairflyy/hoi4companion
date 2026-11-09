"""Bounded, deterministic identity constraints over source slots, never live state.

Complete coverage is an explicit source assertion, not a property inferred from
the number of recognized names. Possible bypasses over-approximate solutions;
they can weaken a proof but can never manufacture one.
"""
from copy import deepcopy

from .resolution import ResolutionContext, catalog_index


CLASSES = ('direct', 'reconstructed', 'probable', 'unresolved')


def validate_metadata(guide):
    policy = guide.get('focus_sequence')
    if policy is not None:
        if (not isinstance(policy, dict) or set(policy) - {'schema_version', 'coverage', 'initial_focuses'}
                or type(policy.get('schema_version')) is not int or policy['schema_version'] != 1
                or policy.get('coverage') not in ('partial', 'complete')):
            raise ValueError('Invalid focus sequence policy')
        initial = policy.get('initial_focuses', [])
        if (policy['coverage'] == 'complete' and 'initial_focuses' not in policy
                or not isinstance(initial, list) or len(initial) > 4096
                or any(not isinstance(x, str) or not x or len(x) > 512 for x in initial)
                or len(set(initial)) != len(initial)):
            raise ValueError('Invalid initial focus list')
    positions = set()
    from .contracts import validate_entity
    for rows in guide['tracks'].values():
        previous = 0
        for row in rows:
            if 'focus_resolution' not in row:
                continue
            r = row['focus_resolution']
            if (row['action']['kind'] != 'select_focus' or not isinstance(r, dict)
                    or set(r) - {'schema_version', 'position', 'query', 'classification', 'entity', 'locked'}
                    or type(r.get('schema_version')) is not int or r['schema_version'] != 1
                    or type(r.get('position')) is not int or not 1 <= r['position'] <= 4096
                    or r['position'] in positions or r['position'] <= previous
                    or not isinstance(r.get('query'), str) or not 0 < len(r['query']) <= 2048
                    or r.get('classification') not in CLASSES
                    or 'locked' in r and type(r['locked']) is not bool):
                raise ValueError('Invalid focus source position or resolution provenance')
            validate_entity(r.get('entity'))
            if r['entity']['kind'] != 'focus':
                raise ValueError('Invalid focus provenance entity')
            positions.add(r['position']); previous = r['position']


def source_guide(guide):
    """Revalidate automatic bindings against their original query, not their ID."""
    result = deepcopy(guide)
    position = 0
    for rows in result['tracks'].values():
        focus_lane=any(r['action']['kind']=='select_focus' for r in rows)
        for row in rows:
            if row['action']['kind'] != 'select_focus' and not (focus_lane and row['action']['kind']=='free'):
                continue
            position += 1
            old = row.get('focus_resolution', {})
            ref = row['action'].get('entity')
            if old and old.get('entity') == ref and not old.get('locked'):
                ref['id'] = old['query']
            row['_focus_position'] = old.get('position', position)
    return result


class _Limit(Exception):
    pass


class _Budget:
    def __init__(self, limit):
        self.limit, self.used = limit, 0

    def spend(self):
        self.used += 1
        if self.used > self.limit:
            raise _Limit


def reconstruct(guide, catalog, named, *, work_limit=2_000_000):
    """Prove slot backbones by searching for counterexamples, not by scoring.

    No proof is published after a resource limit. Missing graph definitions,
    conditional slots and multiple independent source tracks remain explicit
    limitations. Availability/effects are not simulated or claimed verified.
    """
    index = catalog_index(catalog)
    focus_lanes=[lane for lane in guide['tracks'].values() if any(r['action']['kind']=='select_focus' for r in lane)]
    rows = [r for lane in focus_lanes for r in lane if r['action']['kind'] in ('select_focus','free')]
    policy = guide.get('focus_sequence', {'coverage': 'partial'})
    report = {'schema_version': 1, 'coverage': policy['coverage'], 'status': 'partial',
              'slot_count': len(rows), 'reconstructed_count': 0, 'corrections': [],
              'limitations': ['availability_and_effects_not_evaluated'], 'work': 0}
    if policy['coverage'] != 'complete' or not rows:
        return {}, report, []
    if len(rows) > 256:
        report.update(status='limit', limitations=['sequence_slot_limit']); return {}, report, []
    phases = {p['id']: p for p in guide.get('phases', [])}
    all_rows = {r['key']: r for lane in guide['tracks'].values() for r in lane}
    def conditional(row, seen=frozenset()):
        if row['key'] in seen: return True
        return ('when' in row or 'skip_when' in row or row.get('optional')
                or row['action'].get('params', {}).get('focus_optional')
                or 'when' in phases.get(row.get('phase'), {})
                or any(conditional(all_rows[k], seen | {row['key']}) for k in
                       [*row.get('depends_on', []), *row.get('depends_on_started', [])]))
    if len(focus_lanes) != 1 or any(r['action']['kind']=='research' for lane in focus_lanes for r in lane):
        report.update(status='incomplete', limitations=['multiple_focus_tracks'])
        return {}, report, []
    free = {r['key'] for r in rows if r['action']['kind']=='free'}
    optional = free | {r['key'] for r in rows if conditional(r)}
    report['conditional_slot_count'] = len(optional-free)
    report['free_slot_count'] = len(free)
    if optional: report['limitations'].append('conditional_steps_overapproximated')
    country = guide['applies_to'].get('country'); scope = guide['applies_to'].get('tree_id')
    if not scope:
        scopes = {x.get('scope') for r in rows for x in named.get(r['key'],{}).get('candidate_domain',[]) if x.get('scope')}
        if len(scopes) == 1: scope = next(iter(scopes))
    if not scope:
        report.update(status='incomplete', limitations=['focus_tree_unknown']); return {}, report, []
    entries = index.rows(('focus',), ResolutionContext(country=country, scope=scope))
    identity = lambda x: (x.get('scope'), x['id'])
    nodes = {identity(x): x for x in entries}
    if len(nodes) > 1200:
        report.update(status='limit', limitations=['sequence_catalog_limit']); return {}, report, []
    initial = {(scope, x) for x in policy.get('initial_focuses', [])}
    if not initial <= nodes.keys():
        report.update(status='conflict', limitations=['initial_focus_not_in_catalog'])
        return {}, report, ['Focus sequence: an initial focus is absent from the selected tree']
    parents = {}; opposites = {k: set() for k in nodes}; repeat = set(); bypass = set()
    for k, n in nodes.items():
        if 'prerequisites' not in n or 'mutually_exclusive' not in n:
            report.update(status='incomplete', limitations=['missing_focus_relations']); return {}, report, []
        parents[k] = [set((k[0], p) for p in group) for group in n['prerequisites']]
        if any(not group or not group <= nodes.keys() for group in parents[k]):
            report.update(status='incomplete', limitations=['missing_focus_relations']); return {}, report, []
        if n.get('continuous') or n.get('repeatable'): repeat.add(k)
        if n.get('bypass_possible') is not False or n.get('external_completion_possible'): bypass.add(k)
        if n.get('sequence_effects_safe') is False:
            report.update(status='incomplete', limitations=['focus_tree_reset_effects']); return {}, report, []
        for other in n['mutually_exclusive']:
            q = (k[0], other)
            if q in nodes: opposites[k].add(q); opposites[q].add(k)
    # A cycle/missing relation is uncertainty, never a convenient absent edge.
    visiting, done = set(), set()
    def visit(k):
        if k in visiting: raise ValueError
        if k in done: return
        visiting.add(k)
        for group in parents[k]:
            for p in sorted(group,key=str): visit(p)
        visiting.remove(k); done.add(k)
    try:
        for k in nodes: visit(k)
    except (ValueError, RecursionError):
        report.update(status='incomplete', limitations=['cyclic_focus_relations']); return {}, report, []
    # An optional source slot may contribute no completed focus at all. Keep
    # its source position, without guessing the truth of its live predicate.
    skip = (None, '')
    parents[skip] = []; opposites[skip] = set(); repeat.add(skip)
    domains = []
    for row in rows:
        choices = named.get(row['key'],{}).get('candidate_domain',[])
        ref = row['action'].get('entity',{'id':''})
        explicit = row.get('focus_resolution', {}).get('locked') or '_' in ref['id']
        domain = {identity(x) for x in choices} if choices else set() if explicit else set(nodes)
        domain &= nodes.keys()
        alternatives = row['action'].get('params', {}).get('conditional_focus', {})
        if alternatives:
            for field in ('bypass_id', 'fallback_id'):
                k = (scope, alternatives.get(field))
                if k in nodes: domain.add(k)
        if row['key'] in optional: domain.add(skip)
        domains.append(domain)
    budget = _Budget(work_limit)

    def propagate(ds):
        changed = True
        while changed:
            changed = False
            if any(not d for d in ds): return None
            fixed = [next(iter(d)) for d in ds if len(d) == 1]
            unique = [k for k in fixed if k not in repeat]
            if len(set(unique)) != len(unique) or set(unique) & initial: return None
            all_fixed = set(fixed) | initial
            if any(opposites[k] & all_fixed for k in all_fixed): return None
            prefix = set(initial)
            for i, domain in enumerate(ds):
                keep = set()
                for k in sorted(domain,key=str):
                    budget.spend()
                    if k not in repeat and (k in initial or len(domain) > 1 and k in all_fixed): continue
                    if opposites[k] & (all_fixed - {k}): continue
                    if k in bypass or all(group & (prefix | bypass) for group in parents[k]): keep.add(k)
                if not keep: return None
                if keep != domain: ds[i] = keep; changed = True
                prefix |= keep
            # A compulsory group must occupy an earlier slot; one remaining
            # position gets the whole OR group, not an arbitrary member.
            for i, domain in enumerate(ds):
                if len(domain) != 1: continue
                k = next(iter(domain))
                if k in bypass: continue
                for group in parents[k]:
                    if group & (initial | bypass): continue
                    positions = [j for j in range(i) if ds[j] & group]
                    if not positions: return None
                    if len(positions) == 1:
                        j = positions[0]; keep = ds[j] & group
                        if keep != ds[j]: ds[j] = keep; changed = True
        return ds

    def find(ds):
        budget.spend()
        ds = propagate([set(d) for d in ds])
        if ds is None: return None
        variables = [i for i, d in enumerate(ds) if len(d) > 1]
        if not variables: return [next(iter(d)) for d in ds]
        i = min(variables, key=lambda j: (len(ds[j]), j))
        for k in sorted(ds[i], key=str):
            branch = ds.copy(); branch[i] = {k}
            solution = find(branch)
            if solution is not None: return solution
        return None

    try:
        solution = find(domains)
        if solution is None:
            report.update(status='conflict',work=budget.used)
            issues=[];prefix=set(initial)
            for i,(row,domain) in enumerate(zip(rows,domains)):
                if len(domain)==1:
                    k=next(iter(domain))
                    if k not in bypass:
                        for group in parents[k]:
                            if not group & (prefix|bypass):
                                issues.append({'key':row['key'],'position':row.get('_focus_position',i+1),
                                               'id':k[1],'reason':'missing_earlier_prerequisite',
                                               'required_any_of':sorted(p[1] for p in group)})
                prefix|=domain
            report['conflicts']=issues[:12]
            errors=[f'Focus sequence: source position {x["position"]} ({x["id"]}) requires one of '
                    +', '.join(x['required_any_of'])+' before this slot' for x in issues[:12]]
            if not errors:errors=['Focus sequence has no compatible ordering: check names, coverage, initial focuses and source positions']
            # A repair witness is useful for review, but is not proof that the
            # source name was wrong rather than the coverage assertion.
            repairs=[]
            for i,row in enumerate(rows):
                q=row['action'].get('entity',{}).get('id','')
                if row['action']['kind']=='free':continue
                if ('_' in q or row.get('focus_resolution',{}).get('locked')
                        or len(domains[i])!=1 or any(q==k[1] for k in domains[i])):continue
                relaxed=[set(d) for d in domains];relaxed[i]=set(nodes)-domains[i]
                if row['key'] in optional:relaxed[i].add(skip)
                try:alternative=find(relaxed)
                except _Limit:
                    report['limitations'].append('repair_search_limit');break
                if alternative is not None and alternative[i]!=skip:
                    item=nodes[alternative[i]]
                    repairs.append({'key':row['key'],'position':row.get('_focus_position',i+1),
                                    'query':q,'entity':item,'classification':'probable','exhaustive':False})
                    errors.append(f'Focus sequence: review source position {row.get("_focus_position",i+1)} '
                                  +f'({q}); compatible candidate {item["name"]} ({item["id"]}), not applied')
                if len(repairs)==8:break
            report.update(probable_repairs=repairs,work=budget.used)
            return {},report,errors
        forced = {}
        witnesses = [{k} for k in solution]
        for i, row in enumerate(rows):
            other = [set(d) for d in domains]; other[i].discard(solution[i])
            alternative = find(other) if other[i] else None
            if (alternative is None and solution[i] != skip
                    and row['action']['kind']=='select_focus'
                    and not row['action'].get('params', {}).get('conditional_focus')):
                forced[row['key']] = nodes[solution[i]]
            elif alternative is not None:
                for j,k in enumerate(alternative): witnesses[j].add(k)
        report.update(status='validated', forced_count=len(forced), work=budget.used,
                      bypass_uncertainty_count=len(bypass),
                      external_completion_count=sum(bool(n.get('external_completion_possible')) for n in nodes.values()),
                      ambiguous_slots=[{'key':r['key'],'candidates':[nodes[k] for k in sorted(witnesses[i]-{skip},key=str)[:12]],
                                        'exhaustive':False} for i,r in enumerate(rows)
                                       if r['key'] not in forced and r['action']['kind']=='select_focus'])
        # "validated" means these structural constraints are satisfiable,
        # not that the guide is playable under all dynamic game conditions.
        return forced, report, []
    except (_Limit, RecursionError):
        report.update(status='limit', work=budget.used, limitations=['sequence_search_limit'])
        return {}, report, []
