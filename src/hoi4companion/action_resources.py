"""Game catalog costs, separate from authored guide instructions and live balances."""
import re

from .clausewitz import Node


def project_costs(node):
    """Only literal, unconditional breakthrough costs are resolved here."""
    blocks=list(node.blocks('breakthrough_cost'))
    if len(blocks)!=1:return None
    result={}
    for key,value in blocks[0].entries:
        if (not key or key in result or not isinstance(value,str)
                or not re.fullmatch(r'\d+',value)):
            return None
        result[key]=int(value)
    return {'projects.points.'+key:value for key,value in result.items()}


def enrich_resource_costs(guide, catalog):
    from .backend_core import _request
    value=_request(0,'source_project_costs',guide=guide,catalog=list(catalog))
    for track,rows in guide['tracks'].items():
        for row,enriched in zip(rows,value['tracks'][track]):
            if 'resource_costs' in enriched:row['resource_costs']=enriched['resource_costs']
    return guide
