"""Conservative recipient constraints from event scripts, not event prediction.

Only THIS-country tag tests and Boolean groups are interpreted for events.
Political exports can also carry original-tag tests, evaluated only with an
explicit original-country assumption. Other triggers and scope changes remain
unknown. A false result can prune a candidate; unknown must retain it.
"""
import re

from .clausewitz import Node


def combine(op, args):
    identity, decisive = ('true', 'false') if op == 'all' else ('false', 'true')
    kept = []
    for arg in [child for item in args for child in (item['args'] if item['op']==op else [item])]:
        if arg['op'] == decisive:
            return {'op': decisive}
        if arg['op'] != identity and arg not in kept:
            kept.append(arg)
    return {'op': identity} if not kept else kept[0] if len(kept) == 1 else {'op': op, 'args': kept}


def country_condition(block, op='all', depth=0, *, political=False):
    if not isinstance(block, Node) or depth > 32:
        return {'op': 'unknown'}
    args = []
    for key, value in block.entries:
        name = key.upper()
        tag_key = key in ('tag', 'tag !=') or political and key in ('original_tag', 'original_tag !=')
        if tag_key and isinstance(value, str) and re.fullmatch(r'[A-Z0-9]{3}', value):
            item = {'op': 'original_tag' if key.startswith('original_tag') else 'tag', 'value': value}
            if key.endswith(' !='):
                item = {'op': 'not', 'arg': item}
        elif not political and key == 'always' and value in ('yes', 'no'):
            item = {'op': 'true' if value == 'yes' else 'false'}
        elif name in ('AND', 'OR') and isinstance(value, Node):
            item = country_condition(value, 'all' if name == 'AND' else 'any', depth+1, political=political)
        elif name == 'NOT' and isinstance(value, Node) and len(value.entries) == 1:
            inner = country_condition(value, depth=depth+1, political=political)
            item = {'op': {'true': 'false', 'false': 'true'}.get(inner['op'], 'unknown')} if inner['op'] in ('true', 'false', 'unknown') else {'op': 'not', 'arg': inner}
        else:
            item = {'op': 'unknown'}
        args.append(item)
    return combine(op, args)


def political_condition(*nodes):
    """Project declared national gates, not runtime selection availability.

    `always=no` can denote a law/spirit assigned by script. It is not proof
    that the nation can never possess that identity. Prefixes are not gates.
    Original-tag predicates stay explicit; the exporter declares its origin
    assumption separately instead of treating them as current-tag predicates.
    """
    blocks=[block for node in nodes for field in ('allowed','visible','available')
            for block in node.blocks(field)]
    return combine('all',[country_condition(block,political=True) for block in blocks]) if blocks else {'op':'unknown'}


def event_condition(kind, event):
    if kind not in ('country_event', 'news_event'):
        return {'op': 'unknown'}
    triggers = event.blocks('trigger')
    condition = combine('all', [country_condition(block) for block in triggers]) if triggers else {'op': 'unknown'}
    # Major/news events can also be shown to countries other than the original
    # recipient. Never apply that recipient's tag restriction to the broadcast.
    if event.scalar('major') == 'yes' or kind == 'news_event':
        show = combine('all', [country_condition(block) for block in event.blocks('show_major')])
        condition = combine('any', [condition, show])
    return condition
