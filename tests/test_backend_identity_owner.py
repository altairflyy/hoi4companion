"""Normal resolution shares the backend-owned immutable catalog service."""
from unittest import TestCase
from unittest.mock import patch
from hoi4companion.catalog_core import NativeCatalog,engine_info

class BackendIdentityOwnerTests(TestCase):
    def test_no_separate_catalog_dll_is_loaded_for_normal_resolution(self):
        with patch('hoi4companion.catalog_core._library',side_effect=AssertionError('legacy DLL transport')):
            self.assertEqual(engine_info()['engine'],'rust-catalog-core')
            with NativeCatalog([{'kind':'technology','id':'t','name':'Tools'}]) as index:
                self.assertEqual(index.query('matches',query='Tools',kinds=['technology']),[0])
                self.assertEqual(index.query('stats')['rows'],1)
