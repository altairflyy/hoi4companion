"""Pure, run-scoped campaign conditions derived from qualified observations."""
import re
from .contracts import fact


AFTER_WAR = 'guide.campaign.after_war'
_AFTER_WAR = frozenset(('after war','after the war','once war is over','once the war is over',
                        'when war is over','when the war is over','once the war ends','when the war ends',
                        'dopo la guerra','dopo guerra','quando la guerra finisce','una volta finita la guerra'))


def timing_condition(text,catalog=None,country=None,*,scope=None,currency=None):
    """Only complete generic phrases; named wars and extra qualifiers stay unknown."""
    if isinstance(text,str) and ' '.join(text.casefold().strip(' .:').split()) in _AFTER_WAR:
        return {'op':'fact','key':AFTER_WAR,'test':'eq','value':True}
    from .timing_conditions import compile_timing
    return compile_timing(text,catalog,country,scope=scope,currency=currency)


def validate_observations(value):
    if (not isinstance(value,dict) or value and (set(value)-{'country','war_seen','milestones'}
            or not isinstance(value.get('country'),str) or not re.fullmatch('[A-Z0-9]{3}',value['country'])
            or 'war_seen' in value and value['war_seen'] is not True
            or 'war_seen' not in value and not value.get('milestones')
            or 'milestones' in value and (not isinstance(value['milestones'],dict) or not 1<=len(value['milestones'])<=4096
                or any(not isinstance(k,str) or not 1<=len(k)<=2048 or v is not True for k,v in value['milestones'].items())))):
        raise ValueError('Osservazioni campagna non valide')
    return value


def uses_after_war(guide):
    def contains(node):
        return isinstance(node,dict) and (node.get('op')=='fact' and node.get('key')==AFTER_WAR
                or any(contains(v) for v in node.values()) or any(contains(v) for values in node.values() if isinstance(values,list) for v in values))
    return any(contains(step.get(field)) for rows in guide['tracks'].values() for step in rows
               for field in ('when','skip_when','completion','started_when','priority_when','requirements')) or any(contains(phase.get('when')) for phase in guide.get('phases',[]))


def war_value(state):
    item=state.get('facts',{}).get('country.diplomacy.at_war',{})
    country=state.get('facts',{}).get('country',{})
    if (state.get('usable') and item.get('status')=='observed' and item.get('source')
            and type(item.get('value')) is bool and country.get('status') in ('observed','calculated')
            and country.get('source') and isinstance(country.get('value'),str)
            and re.fullmatch('[A-Z0-9]{3}',country['value'])):
        return country['value'],item['value']
    return None


def context(state,observations):
    from .backend_core import _request
    return _request(0,'domain_context',kind='campaign',state=state,observations=observations)
