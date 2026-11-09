"""Pure recipe comparison; labels, MIOs and derived combat statistics are not IDs."""
import re
import math


def _identifier(value):
    return isinstance(value, str) and re.fullmatch(r'[A-Za-z0-9_.-]{1,160}', value) is not None


def configuration(value):
    """Normalize complete occupied slots and nonzero upgrade levels.

    Absence means empty/zero only inside a complete qualified configuration.
    Recipe scripts store upgrade levels as decimal strings; live data uses ints.
    """
    if not isinstance(value, dict):
        return None
    modules, upgrades = value.get('modules'), value.get('upgrades')
    if (not isinstance(modules, dict) or not isinstance(upgrades, dict) or
            len(modules) > 256 or len(upgrades) > 256):
        return None
    result = {'modules': {}, 'upgrades': {}}
    for key, module in modules.items():
        if not _identifier(key) or not _identifier(module):
            return None
        if module != 'empty':
            result['modules'][key] = module
    for key, level in upgrades.items():
        if not _identifier(key):
            return None
        if isinstance(level, str) and re.fullmatch(r'(?:0|[1-9][0-9]{0,4})', level):
            level = int(level)
        if type(level) not in (int, float) or not 0 <= level <= 10000 or not math.isfinite(level) or level != int(level):
            return None
        if level:
            result['upgrades'][key] = int(level)
    return result


def valid_live_design(value):
    if not isinstance(value, dict):
        return False
    if value.get('status') == 'unavailable':
        return not any(k in value for k in ('modules', 'upgrades'))
    if value.get('status') != 'observed':
        return False
    # Live levels must already be numeric; permissive script conversion is
    # confined to reference recipes and must not qualify malformed observations.
    return (isinstance(value.get('upgrades'), dict) and
            all(type(level) in (int, float) for level in value['upgrades'].values()) and
            configuration(value) is not None)


def compare(expected, observed):
    wanted = configuration(expected)
    if wanted is None or not valid_live_design(observed) or observed.get('status') != 'observed':
        return {'status': 'unknown', 'differences': []}
    actual = configuration(observed)
    differences = []
    for kind in ('modules', 'upgrades'):
        default = None if kind == 'modules' else 0
        for key in sorted(set(wanted[kind]) | set(actual[kind])):
            before, after = wanted[kind].get(key, default), actual[kind].get(key, default)
            if before != after:
                differences.append({'kind': kind, 'key': key, 'expected': before, 'actual': after})
    return {'status': 'different' if differences else 'match', 'differences': differences}


def matching(selector, lines):
    """Return matching rows, complete count evidence and a separate explanation."""
    if not selector:
        return [], False, None
    candidates = [row for row in lines if row['equipment'] in selector['equipment_ids'] and
                  selector['category'] in (None, row['category'])]
    if 'design' not in selector:
        from .production_progress import _name
        names = {_name(name) for name in selector['variant_names']}
        return [row for row in candidates if not names or _name(row['name']) in names], True, None
    comparisons = [{'line_id': row['id'], 'name': row['name'],
                    **compare(selector['design'], row.get('design'))} for row in candidates]
    complete = all(row['status'] != 'unknown' for row in comparisons)
    identities = {row['line_id'] for row in comparisons if row['status'] == 'match'}
    explanation = {'status': 'unknown' if not complete else 'match' if identities else 'different' if comparisons else 'missing',
                   'candidates': comparisons}
    return [row for row in candidates if row['id'] in identities], complete, explanation
