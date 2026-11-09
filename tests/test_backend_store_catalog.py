from pathlib import Path
from tempfile import TemporaryDirectory
from threading import RLock
from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import patch
from dataclasses import is_dataclass,fields
from hoi4companion.catalog_generation import CatalogGeneration
from hoi4companion.guide_compatibility import GuideCompatibility
from hoi4companion._catalog_refresh_reference import refresh
from hoi4companion.clausewitz import Node
from hoi4companion.script_document import encode

def wire(value):
    if isinstance(value,Node):return encode(value)
    if is_dataclass(value):return {f.name:wire(getattr(value,f.name)) for f in fields(value)}
    if isinstance(value,dict):return {k:wire(v) for k,v in value.items()}
    if isinstance(value,(list,tuple)):return list(map(wire,value))
    if isinstance(value,Path):return str(value)
    return value

class BackendStoreCatalogTests(TestCase):
    def test_native_cohort_adapters_and_dlc_use_the_same_source_owner(self):
        import hoi4companion.api as api
        with TemporaryDirectory() as folder:
            root=Path(folder);game=root/'game';docs=root/'docs';docs.mkdir()
            focus=game/'common/national_focus/f.txt';focus.parent.mkdir(parents=True)
            focus.write_text('focus_tree={id=tree country={factor=1} focus={id=first} focus={id=blocked allow_branch={has_dlc=Pack}}}',encoding='utf8')
            (docs/'dlc_load.json').write_text('{"enabled_mods":[],"disabled_dlcs":[]}')
            settings={'game_dir':str(game),'docs_dir':str(docs),'country':'TST','tree_id':'','snapshot_path':'','language':'english','ui_language':'english','mods':'auto'}
            old=SimpleNamespace(settings=settings.copy(),lock=RLock())
            with patch('hoi4companion._catalog_refresh_reference.SETTINGS',root/'settings.json'):
                refresh(old)
            owner=CatalogGeneration(settings,root/'sprite-cache',api.DEMO)
            actual,config=owner.attributes(root/'sprite-cache')
            for key,value in actual.items():
                if key!='sprites':self.assertEqual(wire(value),wire(getattr(old,key)),key)
            self.assertEqual(config,old.settings)
            self.assertIs(actual['trees']['tree'],actual['all_trees']['tree'])
            self.assertIs(actual['sprites']._catalog_owner,owner)
            # No copy of localization/source tables is sent back into Rust.
            store=SimpleNamespace(_catalog_generation=owner)
            check=GuideCompatibility(store)
            guide={'title':'Fixture','applies_to':{'country':'TST'},'tracks':{'focus':[{'key':'f','action':{'kind':'select_focus','entity':{'kind':'focus','id':'blocked'}}}]}}
            self.assertTrue(check.assess(guide)['blocked'])
