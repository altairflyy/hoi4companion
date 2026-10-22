"""Read-only scenario research context; never reads a running game or a save."""
from __future__ import annotations

from copy import deepcopy
from pathlib import Path
import re
import math

from .clausewitz import Node, parse
from .eligibility import condition
from .installation import collect_files


def _date(value):
    if not isinstance(value, str) or not re.fullmatch(r'\d{4}\.\d{1,2}\.\d{1,2}(?:\.\d{1,2})?', value):
        raise ValueError('Data iniziale ricerca non valida')
    parts = tuple(map(int, value.split('.')))
    if not 1 <= parts[1] <= 12 or not 1 <= parts[2] <= 31:
        raise ValueError('Data iniziale ricerca non valida')
    return parts + (0,) * (4-len(parts))


def read_research_start(game, mods, country, dlcs, scripts, start_date=None):
    if not isinstance(country, str) or not re.fullmatch(r'[A-Z0-9]{3}', country):
        return None
    files, _ = collect_files(game, mods, 'history/countries', '.txt')
    files = [p for p in files if p.name.startswith(country + ' - ')]
    if len(files) != 1:
        return None
    if start_date is None:
        bookmarks, _ = collect_files(game, mods, 'common/bookmarks', '.txt')
        dates = []
        for path in bookmarks:
            for group in parse(path.read_text(encoding='utf-8-sig')).blocks('bookmarks'):
                dates += [b.scalar('date') for b in group.blocks('bookmark') if b.scalar('date')]
        if not dates:
            return None
        start_date = min(dates, key=_date)
    date = _date(start_date)
    root = parse(files[0].read_text(encoding='utf-8-sig'))

    def limits(node):
        return Node([(k.upper() if k.upper() in ('AND','OR','NOT','NOR','NAND') else k,
                      limits(v) if isinstance(v,Node) else v) for k,v in node.entries])

    def apply(node, state, depth=0):
        if depth > 32:
            raise ValueError('Storia ricerca troppo profonda')
        for key, value in node.entries:
            if key == 'set_technology' and isinstance(value, Node):
                for tech, level in value.entries:
                    if tech == 'popup':
                        continue
                    try:
                        number = float(level) if isinstance(level,str) else float('nan')
                        state[tech] = number > 0 if math.isfinite(number) else None
                    except ValueError:
                        state[tech] = None
            elif key == 'if' and isinstance(value, Node):
                limit = value.first('limit')
                answer = condition(limits(limit), country, dlcs, scripts) if isinstance(limit, Node) else None
                yes = Node([(k,v) for k,v in value.entries if k not in ('limit','else','else_if')])
                no = value.first('else', Node())
                if not isinstance(no, Node): no = Node()
                for alternate in reversed(value.blocks('else_if')):
                    no = Node([('if',Node([*alternate.entries,('else',no)]))])
                # Unknown conditions merge possible outcomes, never assume false.
                if answer is True:
                    apply(yes,state,depth+1)
                elif answer is False:
                    apply(no,state,depth+1)
                else:
                    left, right = deepcopy(state), deepcopy(state)
                    apply(yes,left,depth+1); apply(no,right,depth+1)
                    for tech in left.keys() | right.keys():
                        a,b = left.get(tech,False),right.get(tech,False)
                        state[tech] = a if a == b else None
            elif isinstance(value, Node) and key not in ('limit','else') and not re.match(r'^\d{4}\.',key):
                # Effects in an unsupported scope cannot establish initial facts.
                possible = {}; apply(value,possible,depth+1)
                for tech in possible: state[tech] = None

    state = {}
    apply(root,state)
    dated = sorted(((k,v) for k,v in root.entries if isinstance(v,Node) and re.fullmatch(r'\d{4}\.\d{1,2}\.\d{1,2}',k)),key=lambda pair:_date(pair[0]))
    for key, node in dated:
        if _date(key) <= date: apply(node,state)
    return {'country':country,'date':start_date,'researched':sorted(k for k,v in state.items() if v is True),
            'unknown':sorted(k for k,v in state.items() if v is None),
            'basis':'country_history','source':str(files[0])}
