"""One owned native installation generation; adapters never reload its sources."""
from pathlib import Path
import weakref
from .backend_core import _request


class CatalogGeneration:
    def __init__(self,settings,cache,demo):
        self.handle=_request(0,'catalog_open',settings=settings,cache=str(cache),
                             demo_file=str(demo),demo_labels=str(demo.parent/'demo_labels.json'),
                             vocabulary_path=str(Path(__file__).with_name('guide_language.json')))
        self._release=weakref.finalize(self,_request,0,'catalog_release',handle=self.handle)

    def query(self,operation,**args):
        return _request(0,'catalog_'+operation,handle=self.handle,**args)

    def attributes(self,cache):
        from .focus import Focus,Tree
        from .dlc import DlcInventory
        from .sprites import SpriteLibrary
        from .script_document import decode
        values=self.query('sources')
        def trees(rows):
            return {key:Tree(**{**item,'focuses':{k:Focus(**v) for k,v in item['focuses'].items()},
                'country_selector':decode(item['country_selector']),
                'branch_conditions':{k:decode(v) for k,v in item['branch_conditions'].items()}}) for key,item in rows.items()}
        # Preserve shared definitions between the filtered and unfiltered views.
        visible=trees(values['trees']);other=trees({key:value for key,value in values['all_trees'].items() if key not in visible})
        all_trees={key:visible[key] if key in visible else other[key] for key in values['all_trees']}
        result={key:values[key] for key in ('demo','warnings','labels','reference_labels','scripted_names','mod_inventory',
            'excluded_trees','excluded_focuses','operation_entities','technologies','technology_visuals','focus_visuals')}
        result.update(trees=visible,all_trees=all_trees,
            continuous_focuses={key:Focus(**v) for key,v in values['continuous_focuses'].items()},
            dlcs=DlcInventory(**values['dlcs']) if values['dlcs'] is not None else None,
            game_root=Path(values['game_root']) if values['game_root'] is not None else None,
            mod_roots=list(map(Path,values['mod_roots'])),scripts={key:decode(v) for key,v in values['scripts'].items()},
            technology_definitions={kind:{key:decode(v) for key,v in nodes.items()} for kind,nodes in values['technology_definitions'].items()},
            sprites=SpriteLibrary.from_catalog(self,cache) if not values['demo'] else None)
        return result,values['settings']
