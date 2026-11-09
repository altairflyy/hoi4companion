"""Identity corrections retain independent source definitions and campaign records."""
from copy import deepcopy
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.resolution import catalog_index
from hoi4companion._guide_associations_reference import change_association, read_associations
from test_guide_associations import guide, CATALOG

class NativeAssociationTests(unittest.TestCase):
    def test_native_editor_matches_existing_edits_restore_and_stale_guards(self):
        index=catalog_index(CATALOG)
        with TemporaryDirectory() as native, TemporaryDirectory() as reference:
            source=guide();original=deepcopy(source)
            old=source['tracks']['research'][0]['action']['entity'];new={**old,'id':'construction'}
            for expected,replacement in [(old,new),(new,None)]:
                want=change_association(source,'r',expected,replacement,CATALOG,reference)
                got=_request(0,'guide_association_change',guide=source,key='r',expected=expected,entity=replacement,directory=native,**index.native_import_args())
                self.assertEqual(got,want)
                self.assertEqual(read_associations('example',native),read_associations('example',reference))
            before=(Path(native)/'example.json').read_bytes()
            with self.assertRaises(ValueError):
                _request(0,'guide_association_change',guide=source,key='r',expected=new,entity=old,directory=native,**index.native_import_args())
            self.assertEqual(before,(Path(native)/'example.json').read_bytes());self.assertEqual(source,original)

    def test_native_editor_rejects_route_conflict_and_invalid_replacement_before_write(self):
        source=guide();source['tracks']['focus'].append({'key':'other','action':{'kind':'select_focus','entity':{'kind':'focus','id':'one','scope':'tree'}}})
        catalog=deepcopy(CATALOG);catalog[1]['mutually_exclusive']=['one'];index=catalog_index(catalog)
        old=source['tracks']['focus'][0]['action']['entity']
        with TemporaryDirectory() as folder:
            for replacement in [{**old,'id':'two'},{**old,'scope':'other'},{**old,'id':'unknown'},{**old,'extra':True}]:
                with self.assertRaises(ValueError):
                    _request(0,'guide_association_change',guide=source,key='f',expected=old,entity=replacement,directory=folder,**index.native_import_args())
            self.assertEqual(list(Path(folder).iterdir()),[])

    def test_application_keeps_original_source_after_association_restart(self):
        index=catalog_index(CATALOG)
        with TemporaryDirectory() as folder:
            source=guide();old=source['tracks']['research'][0]['action']['entity'];new={**old,'id':'construction'}
            _request(0,'guide_association_change',guide=source,key='r',expected=old,entity=new,directory=folder,**index.native_import_args())
            handle=_request(0,'app_open',guides=[source],association_directory=folder)
            try:
                self.assertEqual(_request(0,'app_source',handle=handle,guide_id='example')['tracks']['research'][0]['action']['entity'],old)
                self.assertEqual(_request(0,'app_definition',handle=handle,guide_id='example')['tracks']['research'][0]['action']['entity'],new)
            finally:_request(0,'app_release',handle=handle)

