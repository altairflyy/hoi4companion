"""Import-time interpretation of UP; no live state, nation policy or strategy."""
from __future__ import annotations

import re
import unicodedata


def normal_name(value):
    value = re.sub(r'\bmachine\s*guns?\b','mg',value,flags=re.I)
    return ''.join(c for c in unicodedata.normalize('NFKD', value).casefold() if c.isalnum())


def exact_matches(query, candidates):
    exact = [item for item in candidates if item['id'] == query]
    needle = normal_name(query)
    # Native identity first, then the English reference, then installed aliases.
    names = [item for item in candidates if normal_name(item['name']) == needle]
    aliases = [item for item in candidates if needle in {
        normal_name(name) for name in [item['id'], *item.get('aliases', [])]}]
    if exact or names or aliases:
        return exact or names or aliases
    # Whole leading words only: Kb PPanc -> Kb Ppanc wz. 35 Ur. No edit distance.
    words = re.findall(r'\w+',query.casefold())
    return [item for item in candidates if len(words)>=2 and any(
        re.findall(r'\w+',name.casefold())[:len(words)] == words
        for name in [item['name'],*item.get('aliases',[])])]


def typo_matches(query, candidates):
    """One edit in one substantial word; no fuzzy IDs, short names or levels."""
    if '_' in query or len(normal_name(query)) < 10:
        return []
    words = re.findall(r'[^\W_]+',query.casefold())

    def one_edit(a,b):
        if abs(len(a)-len(b))>1: return False
        if len(a)==len(b):
            positions=[i for i in range(len(a)) if a[i]!=b[i]]
            return len(positions)==1 or len(positions)==2 and positions[1]==positions[0]+1 and a[positions[0]]==b[positions[1]] and a[positions[1]]==b[positions[0]]
        if len(a)>len(b): a,b=b,a
        first=next((i for i in range(len(a)) if a[i]!=b[i]),len(a))
        return a[first:]==b[first+1:]

    def close(name):
        other=re.findall(r'[^\W_]+',name.casefold())
        if len(words)!=len(other): return False
        different=[(a,b) for a,b in zip(words,other) if a!=b]
        return len(different)==1 and all(min(len(a),len(b))>=4 and a.isalpha() and b.isalpha() and one_edit(a,b) for a,b in different)
    return [item for item in candidates if any(close(name) for name in [item['name'],*item.get('aliases',[])])]


def name_matches(query,candidates):
    return exact_matches(query,candidates) or typo_matches(query,candidates)


def tokens(value):
    words = set(re.findall(r'[^\W\d_]+', unicodedata.normalize('NFKD', value).casefold()))
    if 'equip' in words: words.remove('equip'); words.add('equipment')
    if 'aa' in words: words.remove('aa'); words.update(('anti','air'))
    return words


def first_useful(family, by_id, context):
    """A declared family selects the first unfinished level in a single lane."""
    if not context:
        return None, None, 'upgrade_baseline_unknown'
    matches = [key for key,t in by_id.items() if family <= tokens(key+' '+t['name'])]
    levels = {'i','ii','iii','iv','v','vi','basic','improved','advanced','upgrade'}
    tight = [key for key in matches if tokens(by_id[key]['name'])-levels == family]
    matches = tight or matches
    parents = {}
    for key,t in by_id.items():
        for target in t.get('upgrade_successors',[]): parents.setdefault(target,[]).append(key)
    roots = set()
    for key in matches:
        seen = set()
        while key not in seen:
            seen.add(key)
            previous = parents.get(key,[])
            if not previous: roots.add(key); break
            if len(previous)!=1: return None,None,'upgrade_branch_ambiguous'
            key = previous[0]
    if len(roots)!=1: return None,None,'upgrade_branch_ambiguous' if roots else 'upgrade_baseline_unknown'
    key = next(iter(roots)); seen = set(); anchor = None
    while key in by_id and key not in seen:
        seen.add(key)
        if key in context.get('unknown',[]): return None,anchor,'upgrade_baseline_unknown'
        if key not in context['researched']: return key,anchor,'upgrade_resolved'
        links = by_id[key].get('upgrade_successors',[])
        if len(links)!=1: return None,key,'upgrade_end_of_line' if not links else 'upgrade_branch_ambiguous'
        anchor,key = key,links[0]
    return None,anchor,'upgrade_target_missing'


def upgrade_successors(tech, technologies):
    targets = list(dict.fromkeys(p['target'] for p in tech.get('paths', []) if isinstance(p.get('target'), str)))
    def same_line(a, b):
        for left in a.get('folders', []):
            x = left.get('layout', {})
            axis = 'anchor_x' if x.get('direction') == 'UP' else 'anchor_y' if x.get('direction') == 'LEFT' else None
            if not axis or not x.get('tree_id') or not isinstance(x.get(axis), (int, float)):
                continue
            for right in b.get('folders', []):
                y = right.get('layout', {})
                if (left['id'] == right['id'] and x['tree_id'] == y.get('tree_id') and
                        x['direction'] == y.get('direction') and x[axis] == y.get(axis)):
                    return True
        return False
    # Directed game paths determine progression. Existing GUI lane identity can
    # distinguish the continuing branch from AA/AT, conversion or rocket forks.
    aligned = [key for key in targets if key in technologies and same_line(tech, technologies[key])]
    return aligned or targets


def resolve_research_upgrades(guide, entities, research_context=None):
    catalog = [item for item in entities if item['kind'] == 'technology']
    by_id = {item['id']: item for item in catalog}
    country = guide.get('applies_to',{}).get('country')
    if research_context and country and country != research_context.get('country'):
        raise ValueError('Contesto iniziale ricerca di una nazione diversa dalla guida')
    baseline = guide.get('research_baseline', [])
    if not isinstance(baseline, list) or any(not isinstance(key, str) or key not in by_id for key in baseline):
        raise ValueError('research_baseline deve contenere ID tecnologici presenti nel catalogo')
    resolutions = {}
    def follows(start, end):
        seen = set()
        while start in by_id and start not in seen:
            if start == end:
                return True
            seen.add(start)
            links = by_id[start].get('upgrade_successors', [])
            if len(links) != 1:
                break
            start = links[0]
        return False
    for rows in guide['tracks'].values():
        history = []
        families = {}
        for step in rows:
            if step['action']['kind'] != 'research':
                continue
            ref = step['action']['entity']; query = ref['id']
            relative = re.fullmatch(r'\s*(.+?)\s+UP\s*(?:\([^)]*\))?\s*', query, re.I)
            params = step['action'].get('params', {})
            explicit = params.get('upgrade_from')
            matched = name_matches(query, catalog)
            target = matched[0]['id'] if len(matched) == 1 else None
            family = tokens(relative[1] if relative else query)
            family_key = frozenset(family)
            if research_context and not relative and query != target and (
                    family_key in families or target in research_context['researched'] and
                    normal_name(query) != normal_name(by_id[target]['name'])):
                target = None
            if target:
                for known_family, anchor in families.items():
                    if anchor and (follows(anchor, target) or follows(target, anchor)):
                        families[known_family] = target
            if not target and (relative or 'upgrade_from' in params or research_context):
                anchor = None
                use_start = False
                if 'upgrade_from' in params:
                    if isinstance(explicit, str):
                        previous = next((row for row in reversed(history) if row['key'] == explicit), None)
                        anchor = previous['target'] if previous else explicit if explicit in by_id else None
                else:
                    previous = [row for row in reversed(history) if family <= row['tokens']]
                    direct = next((row for row in previous if family == row['family']), None)
                    if family_key in families:
                        anchor = families[family_key]
                    elif direct:
                        anchor = direct['target']
                    elif previous:
                        proposed = previous[0]['target']
                        if proposed and all(row['target'] and (follows(proposed,row['target']) or follows(row['target'],proposed)) for row in previous):
                            anchor = proposed
                    else:
                        candidates = [key for key in baseline if family <= tokens(key + ' ' + by_id[key]['name'])]
                        if len(candidates) == 1:
                            anchor = candidates[0]
                        elif research_context and 'research_baseline' not in guide:
                            use_start = True
                reason = 'upgrade_baseline_unknown'
                successors = by_id[anchor].get('upgrade_successors', []) if anchor in by_id else []
                if use_start:
                    target,anchor,reason = first_useful(family,by_id,research_context)
                elif anchor:
                    reason = 'upgrade_end_of_line' if not successors else 'upgrade_branch_ambiguous' if len(successors) != 1 else 'upgrade_target_missing'
                    if len(successors) == 1 and successors[0] in by_id:
                        target = successors[0]
                        if target == anchor or target in baseline or any(row['target'] == target for row in history):
                            target, reason = None, 'upgrade_cycle_or_repeated_target'
                info = {'query': query, 'interpretation': 'next_upgrade', 'upgrade_from': anchor,
                        'reason': 'upgrade_resolved' if target else reason,
                        'candidates': [by_id[key] for key in successors if key in by_id]}
                resolutions[step['key']] = info
                families[family_key] = target
                if target:
                    ref['id'] = target
                    step['research_upgrade'] = {'query':query, 'from_id':anchor,
                                                'basis':'catalog_paths_and_guide_order'}
                    if use_start:
                        step['research_upgrade'].update(basis='country_history_and_catalog_paths',
                            country=research_context['country'],date=research_context['date'])
            # Retain the authored family through renamed successors such as
            # Tools -> Assembly Line Production, including after a preview reload.
            labels = query + ' ' + step.get('research_upgrade', {}).get('query', '')
            if isinstance(step.get('title'), str):
                labels += ' ' + step['title']
            if target:
                labels += ' ' + target + ' ' + by_id[target]['name']
            history.append({'key':step['key'], 'target':target, 'tokens':tokens(labels), 'family':family})
    return resolutions
