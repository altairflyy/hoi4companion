"""Native DLC preflight; unavailable content is separate from import errors."""
from .clausewitz import Node


def import_availability(report, compatibility=None):
    """Content eligibility is separate from import errors and manual review."""
    blocked=isinstance(compatibility,dict) and compatibility.get('blocked') is True
    return {'status':'dlc_unavailable' if blocked else 'ready' if report.get('valid') is True else 'needs_revision',
            'requires_review':not blocked and report.get('valid') is not True,
            'issue_count':0 if blocked else len(report.get('errors',[]))}



class GuideCompatibility:
    """An owned native generation with a bounded per-definition preflight cache."""
    def __init__(self,store):
        import weakref
        from .backend_core import _request
        from .script_document import encode
        self._catalog_owner=getattr(store,'_catalog_generation',None)
        if self._catalog_owner is not None:return
        trees=getattr(store,'all_trees',store.trees)
        def focus(f):return {'name':f.name,'localization_key':getattr(f,'localization_key',''),
            'prerequisites':f.prerequisites,'mutually_exclusive':f.mutually_exclusive,
            'bypass_possible':getattr(f,'bypass_possible',None),'sequence_effects_safe':getattr(f,'sequence_effects_safe',None),
            'external_completion_possible':getattr(f,'external_completion_possible',None),'repeatable':getattr(f,'repeatable',False)}
        sources={'all_trees':{key:{'focuses':{k:focus(f) for k,f in tree.focuses.items()},
                'eligible_countries':getattr(tree,'eligible_countries',[]),'country_selector':encode(tree.country_selector) if isinstance(tree.country_selector,Node) else None,
                'branch_conditions':{k:encode(v) for k,v in tree.branch_conditions.items()}} for key,tree in trees.items()},
            'continuous_focuses':{key:focus(f) for key,f in getattr(store,'continuous_focuses',{}).items()},
            'technology_definitions':{kind:{k:encode(v) for k,v in getattr(store,'technology_definitions',{}).get(kind,{}).items()} for kind in ('technologies','folders')},
            'labels':store.labels,'reference_labels':store.reference_labels,'scripts':{k:encode(v) for k,v in store.scripts.items()},
            'scripted_names':store.scripted_names,'dlcs':{'items':store.dlcs.items,'complete':store.dlcs.complete}}
        context=store.resolution_index()._national_metadata
        metadata={'country_names':{key:sorted(values) for key,values in context['country_names'].items()},'adjectives':list(context['adjectives'])}
        self._handle=_request(0,'compatibility_open',sources=sources,**metadata)['handle']
        self._release=weakref.finalize(self,_request,0,'compatibility_release',handle=self._handle)

    def assess(self,guide):
        if self._catalog_owner is not None:return self._catalog_owner.query('compatibility',guide=guide)
        from .backend_core import _request
        return _request(0,'compatibility_assess',handle=self._handle,guide=guide)
