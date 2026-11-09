"""Installed, static geography at the importer seam; no live ownership inference."""
from collections import defaultdict

from .clausewitz import Node
from .research_upgrades import normal_name


def _id(value):
    if type(value) is int and value > 0:
        return value
    if isinstance(value,str) and value.isascii() and value.isdigit() and int(value)>0:
        return int(value)
    return None


def state_geography(state, names):
    """Province membership and named victory points, including dated history.

    Localization supplies names, never comments or a hardcoded city table.
    A historical victory point is usable only inside this state's province set.
    """
    provinces=sorted({_id(value) for block in state.blocks('provinces')
                      for key,value in block.entries if not key and _id(value) is not None})
    cities={}
    def visit(node):
        for key,value in node.entries:
            if key=='victory_points' and isinstance(value,Node):
                items=[v for k,v in value.entries if not k]
                province=_id(items[0]) if len(items)==2 else None
                if province in provinces:
                    label=names('VICTORY_POINTS_'+str(province))
                    # Do not mistake a missing localization key for a city name.
                    if label['name']!='VICTORY_POINTS_'+str(province) or label['aliases']:
                        cities[province]={'province_id':province,**label}
            elif isinstance(value,Node):
                visit(value)
    for history in state.blocks('history'):
        visit(history)
    return {'province_ids':provinces,'cities':[cities[p] for p in sorted(cities)]}


def building_scope(node):
    caps=node.blocks('level_cap')
    if any(cap.first('province_max') is not None for cap in caps):
        return 'province'
    if any(cap.first('state_max') is not None for cap in caps):
        return 'state'
    return None


# Source readers above remain compatibility helpers; resolution is native.
from .native_geography import GeographyIndex
