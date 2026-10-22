import json
from copy import deepcopy
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from hoi4companion.guide_associations import apply_associations, change_association, read_associations


def guide():
    return {'schema_version':1,'id':'example','title':'Example','applies_to':{'country':'TST','tree_id':'tree'},'tracks':{
        'focus':[{'key':'f','action':{'kind':'select_focus','entity':{'kind':'focus','id':'one','scope':'tree'}}}],
        'research':[{'key':'r','action':{'kind':'research','entity':{'kind':'technology','id':'tools'}}}]}}


CATALOG=[{'kind':'focus','id':key,'scope':'tree','name':key,'countries':['TST'],'prerequisites':[],'mutually_exclusive':[]} for key in ('one','two')]+[
    {'kind':'technology','id':key,'name':key} for key in ('tools','construction')]


class AssociationTests(unittest.TestCase):
    def test_correction_persists_without_modifying_original_definition_or_progress(self):
        with TemporaryDirectory() as directory:
            source=guide();before=deepcopy(source);old=source['tracks']['focus'][0]['action']['entity'];new={**old,'id':'two'}
            change_association(source,'f',old,new,CATALOG,Path(directory))
            current=apply_associations(source,Path(directory));self.assertEqual(current['tracks']['focus'][0]['target'],'two')
            self.assertEqual(source,before);self.assertEqual(read_associations('example',Path(directory))['changes'][0]['original'],old)
            change_association(source,'f',new,None,CATALOG,Path(directory))
            self.assertEqual(apply_associations(source,Path(directory))['tracks']['focus'][0]['action']['entity'],old)

    def test_invalid_identity_type_scope_country_and_stale_edit_never_write(self):
        with TemporaryDirectory() as directory:
            root=Path(directory);source=guide();old=source['tracks']['focus'][0]['action']['entity']
            variants=[{**old,'id':'missing'},{**old,'scope':'wrong'},{'kind':'technology','id':'tools'}]
            for new in variants:
                with self.assertRaises(ValueError):change_association(source,'f',old,new,CATALOG,root)
            with self.assertRaises(ValueError):change_association(source,'f',{**old,'id':'stale'},{**old,'id':'two'},CATALOG,root)
            self.assertEqual(list(root.iterdir()),[])

    def test_conflicting_route_is_rejected(self):
        with TemporaryDirectory() as directory:
            source=guide();source['tracks']['focus'].append({'key':'f2','action':{'kind':'select_focus','entity':{'kind':'focus','id':'one','scope':'tree'}}})
            catalog=deepcopy(CATALOG);catalog[1]['mutually_exclusive']=['one'];old=source['tracks']['focus'][0]['action']['entity']
            with self.assertRaisesRegex(ValueError,'incompatibile'):change_association(source,'f',old,{**old,'id':'two'},catalog,Path(directory))

    def test_source_changes_invalidate_only_obsolete_overrides(self):
        with TemporaryDirectory() as directory:
            source=guide();old=source['tracks']['focus'][0]['action']['entity']
            change_association(source,'f',old,{**old,'id':'two'},CATALOG,Path(directory))
            source['tracks']['focus'][0]['action']['entity']['id']='new-source'
            result=apply_associations(source,Path(directory));self.assertEqual(result['tracks']['focus'][0]['target'],'new-source')
            self.assertTrue(result['association_warnings'])
