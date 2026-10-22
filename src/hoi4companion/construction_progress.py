"""Pure run-relative construction evidence, using existing qualified counters."""
from .contracts import fact


BUILDING_FACTS = {'infrastructure': 'infrastructure', 'arms_factory': 'military_factories',
                  'industrial_complex': 'civilian_factories', 'dockyard': 'dockyards'}
MAX_INTEGER = 9007199254740991


def integer(value, minimum=0):
    return type(value) is int and minimum <= value <= MAX_INTEGER


def relative_spec(step):
    params = step['action'].get('params', {})
    building = params.get('building_id')
    field = BUILDING_FACTS.get(building) if isinstance(building, str) else None
    if (step['action']['kind'] not in ('manual', 'special_project') or not field
            or params.get('category') != 'construction' or params.get('quantity_mode') != 'additional'
            or not integer(params.get('state_id'), 1) or not integer(params.get('quantity'), 1)):
        return None
    key = 'guide.construction.' + step['key'] + '.added'
    condition = {'op': 'fact', 'key': key, 'test': 'gte', 'value': params['quantity']}
    if step.get('completion') != condition:
        return None
    return {'fact_key': f"country.states.items.{params['state_id']}.{field}",
            'quantity': params['quantity'], 'derived_key': key,
            'queue_key': f"state:{params['state_id']}:{params['building_id']}"}


def validate_baselines(value):
    if not isinstance(value, dict) or len(value) > 4096:
        raise ValueError('Conteggi iniziali della partita non validi')
    for key, entry in value.items():
        if (not isinstance(key, str) or not key or not isinstance(entry, dict)
                or set(entry) != {'fact_key', 'value', 'country', 'quantity', 'source'}
                or any(not isinstance(entry.get(field), str) or not entry[field]
                       for field in ('fact_key', 'country', 'source'))
                or not integer(entry.get('value')) or not integer(entry.get('quantity'), 1)):
            raise ValueError('Conteggio iniziale della partita non valido')
    return value


def observed_count(state, key):
    item = state['facts'].get(key, {})
    return item if (state['usable'] and item.get('status') == 'observed'
                    and item.get('source') and integer(item.get('value'))) else None


def construction_context(guide, state, baselines):
    facts, details = dict(state['facts']), {}
    country = facts.get('country', {}).get('value') if state['usable'] else None
    for rows in guide['tracks'].values():
        for step in rows:
            spec = relative_spec(step)
            if spec is None:
                continue
            baseline = baselines.get(step['key'])
            current = observed_count(state, spec['fact_key'])
            valid = (baseline is not None and baseline['country'] == country
                     and baseline['fact_key'] == spec['fact_key'] and baseline['quantity'] == spec['quantity']
                     and baseline['value'] + spec['quantity'] <= MAX_INTEGER)
            delta = current['value'] - baseline['value'] if valid and current else None
            # This namespace belongs to the runtime, never to external live input.
            facts[spec['derived_key']] = fact(delta, 'calculated', 'run_baseline:' + step['key']) if delta is not None else fact(reason='Conteggio iniziale non verificabile')
            details[step['key']] = {'baseline': baseline['value'] if valid else None,
                                    'current': current['value'] if current else None,
                                    'target': baseline['value'] + spec['quantity'] if valid else None}
    return {**state, 'facts': facts}, details
