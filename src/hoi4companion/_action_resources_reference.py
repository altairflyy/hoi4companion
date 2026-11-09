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
    """Enrich the response only; never write costs back to the installed guide."""
    projects={}
    for entry in catalog:
        if entry.get('kind')=='project':projects.setdefault(entry['id'],[]).append(entry)
    for rows in guide['tracks'].values():
        for step in rows:
            action=step['action']
            if action['kind']!='special_project' or step.get('tracking')=='reminder':continue
            entries=projects.get(action.get('entity',{}).get('id'),[])
            if len(entries)!=1:continue
            costs=entries[0].get('resource_costs')
            if isinstance(costs,dict):
                # The installed game defines its prices; authored conditions
                # and dependencies remain unchanged.
                step['resource_costs']=dict(costs)
    return guide
