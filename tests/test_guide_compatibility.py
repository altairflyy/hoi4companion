import io
import tempfile
import unittest
import zipfile
from pathlib import Path
from threading import RLock
from types import SimpleNamespace
from unittest.mock import patch

from hoi4companion.api import Store
from hoi4companion.clausewitz import parse
from hoi4companion.dlc import DlcInventory
from hoi4companion.focus import load_trees
from hoi4companion.guide_compatibility import GuideCompatibility
from hoi4companion.resolution import CatalogIndex
from test_word_guides import word


def guide(*names,kind='focus',country='AAA'):
    return {'id':'example','title':'Example','applies_to':{'country':country},'tracks':{'focus':[
        {'key':str(i),'action':{'kind':'select_focus' if kind=='focus' else 'research',
                              'entity':{'kind':kind,'id':name}}} for i,name in enumerate(names)]}}


def engine(source,inventory=None,techs=None,folders=None):
    with tempfile.TemporaryDirectory() as root:
        path=Path(root)/'focus.txt';path.write_text(source,encoding='utf8');trees,warnings=load_trees([path],{})
    assert not warnings,warnings
    for tree in trees.values():tree.eligible_countries=['AAA']
    store=SimpleNamespace(trees=trees,all_trees=trees,labels={},reference_labels={},
        technology_definitions={'technologies':techs or {},'folders':folders or {}},
        dlcs=inventory or DlcInventory(),scripts={},scripted_names={},
        resolution_index=lambda:CatalogIndex([],country_names={'AAA':{'Example'}}))
    return GuideCompatibility(store)


class GuideCompatibilityTests(unittest.TestCase):
    def test_missing_tree_dlc_is_reported_without_turning_missing_ids_into_dlc_failures(self):
        check=engine('''focus_tree={id=tree country={factor=0 modifier={add=10 tag=AAA has_dlc=Pack}}
                    focus={id=first} focus={id=second}}''')
        result=check.assess(guide('first','second'))
        self.assertTrue(result['blocked'])
        self.assertEqual(result['requirements'],[{'enable':['Pack'],'disable':[]}])
        result['requirements'][0]['enable'].append('Corruption')
        self.assertEqual(check.assess(guide('first','second'))['requirements'][0]['enable'],['Pack'])
        self.assertFalse(check.assess(guide('Unresolved typo'))['blocked'])

    def test_unknown_dlc_state_and_unrelated_unavailable_content_stay_selectable(self):
        source='''focus_tree={id=tree country={factor=0 modifier={add=10 tag=AAA has_dlc=Pack}} focus={id=first}}'''
        self.assertFalse(engine(source,DlcInventory(complete=False)).assess(guide('first'))['blocked'])
        unrelated=engine('''focus_tree={id=tree country={factor=1} focus={id=first allow_branch={always=no has_dlc=Pack}}}''')
        self.assertFalse(unrelated.assess(guide('first'))['blocked'])

    def test_country_not_inferred_from_a_title_still_checks_all_scripted_selector_tags(self):
        check=engine('''focus_tree={id=tree country={factor=0 modifier={add=10 tag=AAA has_dlc=Pack}}
            focus={id=first}}''')
        document=guide('first');document['applies_to']={};document['title']='China — a warlord path'
        self.assertTrue(check.assess(document)['blocked'])

    def test_branch_alternatives_and_not_dlc_requirements(self):
        inventory=DlcInventory(items=[{'name':'Pack','status':'enabled'}])
        check=engine('''focus_tree={id=tree country={factor=1}
          focus={id=old allow_branch={NOT={has_dlc=Pack}}}
          focus={id=new allow_branch={has_dlc=Pack}}
          focus={id=child prerequisite={focus=old focus=new}}}''',inventory)
        self.assertEqual(check.assess(guide('old'))['requirements'],[{'enable':[],'disable':['Pack']}])
        self.assertFalse(check.assess(guide('new','child'))['blocked'])

    def test_and_or_requirements_are_alternatives_not_flat_missing_lists(self):
        check=engine('''focus_tree={id=tree country={factor=1}
          focus={id=both allow_branch={has_dlc=A has_dlc=B}}
          focus={id=either allow_branch={OR={has_dlc=A has_dlc=B}}}}''')
        self.assertEqual(check.assess(guide('both'))['requirements'],[{'enable':['A','B'],'disable':[]}])
        self.assertEqual(check.assess(guide('either'))['requirements'],[
            {'enable':['A'],'disable':[]},{'enable':['B'],'disable':[]}])

    def test_technology_folder_variants_are_checked_without_enabling_the_wrong_variant(self):
        techs={'legacy':parse('allow_branch={NOT={has_dlc=Pack}}'),
               'modern':parse('folder={name=modern}')}
        folders={'modern':parse('available={has_dlc=Pack}')}
        check=engine('',techs=techs,folders=folders)
        self.assertTrue(check.assess(guide('modern',kind='technology'))['blocked'])
        self.assertFalse(check.assess(guide('legacy',kind='technology'))['blocked'])

    def test_archive_metadata_is_cached_and_does_not_rewrite_documents(self):
        payload=io.BytesIO()
        with zipfile.ZipFile(payload,'w') as z:z.writestr('Country Guides/Test.docx',word([[],[],['first']]))
        source=payload.getvalue()
        store=Store.__new__(Store);store.lock=RLock()
        check=engine('focus_tree={id=tree country={factor=1} focus={id=first allow_branch={has_dlc=Pack}}}')
        # The extracted source title has no qualified country; the direct DLC
        # predicate still proves an unmet requirement, independent of country.
        with patch.object(store,'guide_compatibility',return_value=check):
            result=store.word_inventory(io.BytesIO(source))
            self.assertTrue(result['compatibility']['Country Guides/Test.docx']['blocked'])
            result['documents'].clear()
            self.assertEqual(store.word_inventory(io.BytesIO(source))['documents'],['Country Guides/Test.docx'])
        self.assertEqual(payload.getvalue(),source)
