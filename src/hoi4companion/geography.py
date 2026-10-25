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


class GeographyIndex:
    """One index per import, shared by all steps; ambiguity is retained."""
    def __init__(self,catalog):
        self.states=defaultdict(list)
        self.names=defaultdict(set)
        self.provinces=defaultdict(set)
        for row in catalog:
            if row['kind']!='state' or _id(row['id']) is None:
                continue
            state_id=_id(row['id'])
            self.states[state_id].append(row)
            for name in [row['id'],row['name'],row.get('localized_name',''),*row.get('aliases',[])]:
                if name:self.names[normal_name(name)].add((state_id,None))
            geography=row.get('geography',{})
            provinces={p for p in geography.get('province_ids',[]) if type(p) is int and p>0}
            for p in provinces:self.provinces[p].add(state_id)
            for city in geography.get('cities',[]):
                p=city.get('province_id')
                if p not in provinces:continue
                for name in [city['name'],city.get('localized_name',''),*city.get('aliases',[])]:
                    if name:self.names[normal_name(name)].add((state_id,p))

    def resolve(self,params,provincial=False):
        """Return a resolved relation or a reason; explicit IDs never fall back."""
        state_id,province=_id(params.get('state_id')),_id(params.get('province_id'))
        if params.get('state_id') is not None and state_id is None or params.get('province_id') is not None and province is None:
            return {'reason':'location_unresolved'}
        if state_id is not None and state_id not in self.states:
            return {'reason':'location_unresolved'}
        owners=self.provinces.get(province,set()) if province is not None else set()
        if province is not None:
            if not owners:return {'reason':'location_unresolved'}
            if len(owners)!=1:return {'reason':'location_ambiguous'}
            if state_id is not None and state_id not in owners:return {'reason':'location_conflict'}
            state_id=next(iter(owners))
        query=params.get('state_name')
        candidates=self.names.get(normal_name(query),set()) if isinstance(query,str) and query.strip() else set()
        # With explicit IDs, unknown free text is only a label. A recognized,
        # contradictory place is an error, not an invitation to override the ID.
        if state_id is not None:
            if candidates:
                candidates={pair for pair in candidates if pair[0]==state_id
                            and (province is None or pair[1] in (None,province))}
                if not candidates:return {'reason':'location_conflict'}
            else:candidates={(state_id,province)}
        elif not candidates:
            return {'reason':'location_unresolved'}
        states={s for s,p in candidates}
        if len(states)!=1:return {'reason':'location_ambiguous'}
        state_id=next(iter(states))
        if len(self.states[state_id])!=1:return {'reason':'location_ambiguous'}
        city_provinces={p for s,p in candidates if p is not None}
        if any(len(self.provinces.get(p,set()))!=1 for p in city_provinces):
            return {'reason':'location_ambiguous'}
        if province is None and len(city_provinces)==1:province=next(iter(city_provinces))
        if provincial:
            if province is None:return {'reason':'location_ambiguous' if city_provinces else 'province_required'}
            if len(self.provinces.get(province,set()))!=1:return {'reason':'location_ambiguous'}
        region=self.states[state_id][0]
        return {'state_id':state_id,'state_name':region.get('localized_name',region['name']),
                'province_id':province,'via':'city' if city_provinces else 'province' if province else 'state'}
