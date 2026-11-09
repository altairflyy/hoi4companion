"""Import-time interpretation of UP; no live state, nation policy or strategy."""
from __future__ import annotations

import re
import unicodedata
import json
from functools import lru_cache
from pathlib import Path


@lru_cache(maxsize=1)
def guide_language():
    return json.loads(Path(__file__).with_name('guide_language.json').read_text(encoding='utf-8'))


def expanded_research_name(query):
    query=re.sub(r'(?<=[A-Za-z])(?=\d+$)',' ',query)
    match = re.fullmatch(r'\s*(.+?)(\s+(?:\d+|I|II|III|IV|V|VI))?\s*',query,re.I)
    if not match:return query
    family=guide_language()['research_families'].get(match[1].casefold())
    return family+(match[2] or '') if family else query


def normal_name(value):
    value=value.translate(str.maketrans({'ı':'i','İ':'I','ł':'l','Ł':'L','ø':'o','Ø':'O','æ':'ae','Æ':'AE','œ':'oe','Œ':'OE','ß':'ss'}))
    value=value.replace('&',' and ')
    value = re.sub(r'\bmachine\s*guns?\b','mg',value,flags=re.I)
    value = re.sub(r'\banti[\s-]*aircraft\b','anti-air',value,flags=re.I)
    # Numeric and Roman suffixes describe the same level, but never collapse
    # different levels or a numeral embedded in the middle of a name.
    roman={'i':'1','ii':'2','iii':'3','iv':'4','v':'5','vi':'6','vii':'7','viii':'8','ix':'9','x':'10'}
    value = re.sub(r'(?<=\s)(i|ii|iii|iv|v|vi|vii|viii|ix|x)\s*$',
                   lambda match: roman[match.group(1).casefold()], value, flags=re.I)
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


@lru_cache(maxsize=65536)
def _spelling_words(value):
    return tuple(normal_name(w) for w in re.findall(r'[^\W_]+',value.replace('&',' and ')))


def typo_matches(query, candidates, *, max_edits=2):
    """Bounded word spelling; the larger focus budget needs an unchanged anchor."""
    if '_' in query or len(normal_name(query)) < 10:
        return []
    source=_spelling_words(query)

    def distance(a,b):
        rows=[list(range(len(b)+1))]
        for i,x in enumerate(a,1):
            row=[i]
            for j,y in enumerate(b,1):
                value=min(row[-1]+1,rows[-1][j]+1,rows[-1][j-1]+(x!=y))
                if i>1 and j>1 and x==b[j-2] and a[i-2]==y:value=min(value,rows[-2][j-2]+1)
                row.append(value)
            rows.append(row)
        return rows[-1][-1]

    def close(name):
        other=_spelling_words(name)
        if len(source)!=len(other): return False
        different=[(a,b) for a,b in zip(source,other) if a!=b]
        if not different or len(different)>2:return False
        protected={'east','eastern','eastward','west','western','westward','north','northern','northward',
                   'south','southern','southward','basic','improved','advanced','modern','medium','heavy','light',
                   'communism','communist','fascism','fascist','democratic','neutrality'}
        if any(a in protected or b in protected for a,b in different):return False
        plural=lambda a,b:min(len(a),len(b))>=3 and (a==b+'s' or b==a+'s')
        if any((min(len(a),len(b))<4 and not plural(a,b)) or abs(len(a)-len(b))>2 or not a.isalpha() or not b.isalpha() for a,b in different):return False
        edits=[distance(a,b) for a,b in different]
        budget=sum(edits)
        if budget>max_edits or any(cost>2 or cost>1 and min(len(a),len(b))<(4 if max_edits>2 else 5) for (a,b),cost in zip(different,edits)):return False
        if budget>2 and not any(a==b and len(a)>=5 for a,b in zip(source,other)):return False
        return budget
    scored=[]
    for item in candidates:
        costs=[cost for name in [item['name'],*item.get('aliases',[])] if (cost:=close(name))]
        if costs:scored.append((min(costs),item))
    best=min((cost for cost,_ in scored),default=None)
    return [item for cost,item in scored if cost==best]


def name_matches(query,candidates):
    return exact_matches(query,candidates) or typo_matches(query,candidates)


def tokens(value):
    value = guide_language()['research_families'].get(value.strip().casefold(), value)
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


def named_progression(query,context=''):
    """Documented shorthand progressions; full native IDs remain exact."""
    if '_' in query:return None
    naval=bool(re.search(r'\(\s*(?:navy|naval)\s*\)',query+' '+context,re.I))
    abbreviated='…' in query or '...' in query
    base=re.sub(r'\(\s*(?:navy|naval)\s*\)','',query,flags=re.I)
    base=re.sub(r'\s+(?:UP|UPGRADES)\s*$','',base.strip(),flags=re.I)
    base=base.replace('…','').replace('...','').strip()
    for definition in guide_language().get('named_research_progressions',{}).values():
        if normal_name(base) not in {normal_name(value) for value in definition['aliases']}:continue
        if definition.get('requires_naval_qualifier_or_ellipsis') and not (naval or abbreviated):continue
        return definition
    return None


def relative_chain(query, by_id):
    """Find one complete, non-branching catalog lane; never guess a fork."""
    named=named_progression(query)
    if named:
        members=named['members']
        return list(members) if all(key in by_id for key in members) else None
    family = tokens(query)
    if not family:
        return None
    matches = [key for key, item in by_id.items() if any(
        family <= tokens(label) for label in [key, item['name'], *item.get('aliases', [])])]
    levels = {'i','ii','iii','iv','v','vi','basic','improved','advanced','upgrade'}
    canonical = [key for key in matches if tokens(by_id[key]['name']) - levels == family]
    tight = canonical or [key for key in matches if any(tokens(label) - levels == family
              for label in by_id[key].get('aliases', []))]
    # The first matching technology, not its generic prerequisite, starts the
    # family (e.g. support equipment is not an engineer company upgrade).
    continuation = {'assembly_line_production'} if family == {'machine','tools'} else set()
    selected = tight or matches
    return (chains_for_matches(selected, by_id, set(matches) | continuation)
            or detached_level_progression(selected, by_id, family))


def detached_level_progression(matches, by_id, family):
    """Order explicit levels where the game declares no paths between them.

    This is a research-family progression, not a dependency graph: Excavation
    levels, for example, each depend on the corresponding Construction level.
    Use only canonical, consecutive, unambiguous level names. Never repair or
    override an explicit path, a missing path target, or a branch with this rule.
    """
    if not 2 <= len(matches) <= 128:
        return None
    ranks = {}
    scheme = None
    roman = {'i':1, 'ii':2, 'iii':3, 'iv':4, 'v':5, 'vi':6,
             'vii':7, 'viii':8, 'ix':9, 'x':10}
    quality = {'basic':1, 'improved':2, 'advanced':3}
    for key in matches:
        item = by_id[key]
        if item.get('upgrade_successors'):
            return None
        name = item['name'].strip()
        numbered = re.fullmatch(r'(.+?)\s+(\d+|VIII|VII|VI|IV|IX|III|II|I|V|X)', name, re.I)
        qualified = re.fullmatch(r'(Basic|Improved|Advanced)\s+(.+)', name, re.I)
        if numbered:
            stem, level = numbered.groups()
            rank = int(level) if level.isdigit() else roman[level.casefold()]
            kind = 'numbered'
        elif qualified:
            level, stem = qualified.groups()
            rank = quality[level.casefold()]
            kind = 'qualified'
        else:
            return None
        if tokens(stem) != family or rank in ranks or scheme not in (None, kind):
            return None
        ranks[rank] = key
        scheme = kind
    if sorted(ranks) != list(range(1, len(ranks) + 1)):
        return None
    return [ranks[rank] for rank in sorted(ranks)]


def chains_for_matches(matches, by_id, family_ids=None):
    parents = {}
    for key, item in by_id.items():
        for target in item.get('upgrade_successors', []):
            parents.setdefault(target, []).append(key)
    roots = set()
    for key in matches:
        seen = set()
        while key not in seen:
            seen.add(key)
            previous = [p for p in parents.get(key, []) if family_ids is None or p in family_ids]
            if not previous:
                roots.add(key); break
            if len(previous) != 1:
                return None
            key = previous[0]
        else:
            return None
    if len(roots) != 1:
        return None
    key = next(iter(roots)); chain = []
    while key in by_id and key not in chain:
        chain.append(key)
        links = by_id[key].get('upgrade_successors', [])
        if any(link not in by_id for link in links):
            return None
        if family_ids is not None:
            links = [link for link in links if link in family_ids]
        if not links:
            return chain
        if len(links) != 1:
            return None
        key = links[0]
    return None


def resolve_research_upgrades(guide, entities, research_context=None, resolver=None):
    # Local import keeps the low-level name helpers independent of the index.
    from .resolution import catalog_index
    resolver=resolver or catalog_index(entities)
    catalog = resolver.rows(('technology',))
    by_id = {item['id']: item for item in catalog}
    from ._research_choices_reference import compile_choices
    compile_choices(guide,by_id,resolver)
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
            if step.get('research_choice'):
                from ._research_choices_reference import validate_choice
                validate_choice(step)
                if any(identity not in by_id for chain in step['research_choice']['branches'].values() for identity in chain):
                    raise ValueError('Un ramo della scelta di ricerca non è presente nel catalogo compatibile')
                resolutions[step['key']]={'query':step['research_choice']['query'],'interpretation':'branch_choice',
                    'reason':'upgrade_live','upgrade_from':None,'candidates':[]}
                continue
            ref = step['action']['entity']
            query = step.get('relative_research', {}).get('query', ref['id'])
            relative = re.fullmatch(r'\s*(.+?)\s+(?:UP|UPGRADES)\s*(?:\([^)]*\))?\s*', query, re.I)
            params = step['action'].get('params', {})
            family_query = relative[1] if relative else query if params.get('interpretation') == 'next_upgrade' else None
            if family_query:
                unqualified=re.sub(r'^(?:basic|improved|advanced)\s+','',family_query,flags=re.I)
                if unqualified!=family_query and expanded_research_name(unqualified)!=unqualified:
                    family_query=expanded_research_name(unqualified)
            named=named_progression(query,' '.join(str(step.get(field,'')) for field in ('title','note')))
            if named:family_query=family_query or query
            # A family name without a level can describe the next useful
            # research too. Require a single catalog lane and do not reinterpret
            # explicit IDs or numbered/versioned names.
            if not family_query and '_' not in query and not re.search(r'\b(?:\d+|i|ii|iii|iv|v)\s*$',query,re.I):
                matches = resolver.matches(query,('technology',),typos=True)
                if len(matches) != 1 and relative_chain(query,by_id):
                    family_query = query
            if family_query:
                chain = (list(named['members']) if all(key in by_id for key in named['members']) else None) if named else relative_chain(family_query, by_id)
                if 'upgrade_from' in params and (chain or not named):
                    explicit = params['upgrade_from']
                    previous = next((row for row in reversed(history) if row['key'] == explicit), None) if isinstance(explicit,str) else None
                    anchor = previous.get('target') if previous else explicit if isinstance(explicit,str) and explicit in by_id else None
                    prior_chain = previous.get('chain') if previous else None
                    if prior_chain:
                        if chain and chain != prior_chain: chain = None
                        else: chain = prior_chain
                    elif not anchor or chain and anchor not in chain:
                        chain = None
                    elif not chain:
                        chain = chains_for_matches([anchor],by_id)
                elif not chain and not named:
                    family = tokens(family_query)
                    earlier = next((row for row in reversed(history) if family <= row['tokens'] and row.get('target')), None)
                    if earlier:
                        chain = chains_for_matches([earlier['target']], by_id)
                if chain:
                    step['relative_research'] = {'query': query, 'chain': chain}
                    ref['id'] = chain[0]
                    step.pop('research_upgrade', None)
                    resolutions[step['key']] = {'query':query, 'interpretation':'next_upgrade',
                        'upgrade_from':None, 'reason':'upgrade_live', 'candidates':[]}
                else:
                    step.pop('relative_research', None)
                    resolutions[step['key']] = {'query':query, 'interpretation':'next_upgrade',
                        'upgrade_from':None, 'reason':'upgrade_branch_ambiguous', 'candidates':[]}
                history.append({'key':step['key'], 'target':None, 'chain':chain, 'tokens':tokens(query), 'family':tokens(family_query)})
                continue
            explicit = params.get('upgrade_from')
            matched = resolver.matches(query,('technology',),typos=True)
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
