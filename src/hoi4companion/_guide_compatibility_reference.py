"""Evidence-based DLC preflight; never resolves ambiguous guide identities."""
from copy import deepcopy
from itertools import combinations
from threading import RLock
import hashlib
import json

from .clausewitz import Node
from .dlc import normalise
from .eligibility import condition, score, selector_tags, _and, _or
from ._guide_importer_reference import catalog_entities
from .resolution import CatalogIndex, ResolutionContext
from .scripted_names import NameExpander


def import_availability(report, compatibility=None):
    """Content eligibility is separate from import errors and manual review."""
    blocked=isinstance(compatibility,dict) and compatibility.get('blocked') is True
    return {'status':'dlc_unavailable' if blocked else 'ready' if report.get('valid') is True else 'needs_revision',
            'requires_review':not blocked and report.get('valid') is not True,
            'issue_count':0 if blocked else len(report.get('errors',[]))}


def _logic(node, country):
    return Node([(key.upper() if key.upper() in ('AND','OR','NOT','NAND','NOR') else
                  'unobserved_country' if not country and key in ('tag','original_tag') else key,
                  _logic(value,country) if isinstance(value,Node) else value) for key,value in node.entries])


def _dlcs(node,scripts,seen=()):
    if not isinstance(node,Node):return set()
    names=set()
    for key,value in node.entries:
        if key=='has_dlc' and isinstance(value,str):names.add(value)
        elif isinstance(value,Node):names.update(_dlcs(value,scripts,seen))
        elif key in scripts and key not in seen and len(seen)<30:names.update(_dlcs(scripts[key],scripts,(*seen,key)))
    return names


class _Inventory:
    def __init__(self,base,changes):self.base,self.changes=base,changes
    def enabled(self,name):return self.changes.get(normalise(name),self.base.enabled(name))


class GuideCompatibility:
    def __init__(self,store):
        self.trees=getattr(store,'all_trees',store.trees)
        self.techs=getattr(store,'technology_definitions',{}).get('technologies',{})
        self.folders=getattr(store,'technology_definitions',{}).get('folders',{})
        self.dlcs=store.dlcs;self.scripts=store.scripts
        raw={key:{'name':store.labels.get(key,key),'unlocks':[
            {'kind':'Equipaggiamento','id':value} for block in node.blocks('enable_equipments')
            for _,value in block.entries if isinstance(value,str)]} for key,node in self.techs.items()}
        rows=catalog_entities(self.trees,raw,store.labels,store.reference_labels,
                             continuous_focuses=getattr(store,'continuous_focuses',{}))
        names=NameExpander(store.scripted_names,store.reference_labels)
        for row in rows:
            if row['kind']=='focus' and not row.get('continuous'):
                tree=self.trees[row['scope']]
                if not row['countries'] and isinstance(tree.country_selector,Node):
                    row['countries']=sorted(selector_tags(tree.country_selector,self.scripts))
            aliases=names.expand(row['name'],countries=row.get('countries',()))
            row['aliases']=list(dict.fromkeys([*row.get('aliases',[]),*aliases]))
        base_index=store.resolution_index()
        self.index=base_index.with_entries(rows)
        self.cache={};self.lock=RLock()

    def _focus(self,entry,country,inventory):
        if entry.get('continuous'):return True
        tree=self.trees.get(entry.get('scope'))
        if not tree:return None
        selector=tree.country_selector
        tags=[country] if country else sorted(selector_tags(selector,self.scripts)) if isinstance(selector,Node) else []
        scores=[score(_logic(selector,tag),tag,inventory,self.scripts) for tag in tags or ['']] if isinstance(selector,Node) else []
        root=_or([None if value is None else value>0 for value in scores]) if scores else None
        memo={}
        def visit(key,seen=()):
            if key in memo:return memo[key]
            focus=tree.focuses.get(key)
            if focus is None or key in seen:return None
            node=tree.branch_conditions.get(key)
            own=condition(_logic(node,country),country or '',inventory,self.scripts) if isinstance(node,Node) else True
            answer=_and([own,*[_or([visit(parent,(*seen,key)) for parent in group]) for group in focus.prerequisites]])
            memo[key]=answer
            return answer
        return _and([root,visit(entry['id'])])

    def _tech(self,entry,country,inventory):
        node=self.techs.get(entry['id'])
        if node is None:return None
        branch=node.first('allow_branch')
        own=condition(_logic(branch,country),country or '',inventory,self.scripts) if isinstance(branch,Node) else True
        folders=[]
        for folder in node.blocks('folder'):
            item=self.folders.get(folder.scalar('name'))
            rule=item.first('available') if isinstance(item,Node) else None
            folders.append(condition(_logic(rule,country),country or '',inventory,self.scripts) if isinstance(rule,Node)
                           else True if item is not None else None)
        return _and([own,_or(folders) if folders else True])

    def _names(self,entry):
        if entry['kind']=='technology':
            node=self.techs.get(entry['id']);names=_dlcs(node.first('allow_branch'),self.scripts) if node else set()
            for folder in node.blocks('folder') if node else []:
                item=self.folders.get(folder.scalar('name'))
                if item:names.update(_dlcs(item.first('available'),self.scripts))
            return names
        tree=self.trees.get(entry.get('scope'))
        if not tree:return set()
        names=_dlcs(tree.country_selector,self.scripts);pending=[entry['id']];seen=set()
        while pending:
            key=pending.pop()
            if key in seen:continue
            seen.add(key);names.update(_dlcs(tree.branch_conditions.get(key),self.scripts))
            focus=tree.focuses.get(key)
            if focus:pending.extend(parent for group in focus.prerequisites for parent in group)
        return names

    def assess(self,guide):
        refs=[(r['key'],r['action']['entity']) for rows in guide.get('tracks',{}).values() for r in rows
              if r.get('action',{}).get('kind') in ('select_focus','research')]
        scope=guide.get('applies_to',{})
        cache_key=hashlib.sha256(json.dumps([guide.get('title'),scope,refs],sort_keys=True).encode()).hexdigest()
        with self.lock:
            if cache_key in self.cache:return deepcopy(self.cache[cache_key])
        country=scope.get('country') or self.index.country_hint(guide.get('title',''),guide)
        result={'status':'no_known_missing_dlc','blocked':False,'requirements':[],
                'basis':'declared_identities_and_local_dlc_conditions','unknown':not bool(refs)}
        for key,ref in refs:
            context=ResolutionContext(country=country,scope=ref.get('scope') or (scope.get('tree_id') if ref['kind']=='focus' else None))
            matches=self.index.matches(ref['id'],(ref['kind'],),context)
            if not matches:result['unknown']=True;continue
            evaluate=lambda changes:_or([(self._focus if entry['kind']=='focus' else self._tech)(entry,country,_Inventory(self.dlcs,changes)) for entry in matches])
            current=evaluate({})
            if current is not False:result['unknown']=result['unknown'] or current is None;continue
            # A failed gate is a DLC incompatibility only if changing known DLC
            # states actually removes the failure. Never blame unknown state,
            # an unavailable mod tree or an unrelated always=no condition.
            names=sorted({name for entry in matches for name in self._names(entry) if self.dlcs.enabled(name) is not None})
            if not 1<=len(names)<=7:result['unknown']=True;continue
            alternatives=[]
            for size in range(1,len(names)+1):
                for group in combinations(names,size):
                    changes={normalise(name):not self.dlcs.enabled(name) for name in group}
                    if evaluate(changes) is not False:
                        alternatives.append({'enable':[n for n in group if not self.dlcs.enabled(n)],
                                             'disable':[n for n in group if self.dlcs.enabled(n)]})
                if alternatives:break
            if alternatives:
                result.update(status='dlc_incompatible',blocked=True,requirements=alternatives,
                              witness={'key':key,'query':ref['id'],'candidate_ids':[r['id'] for r in matches]})
                break
            result['unknown']=True
        with self.lock:
            if len(self.cache)>=256:self.cache.pop(next(iter(self.cache)))
            self.cache[cache_key]=deepcopy(result)
        return result
