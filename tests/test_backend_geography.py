from unittest import TestCase
from hoi4companion.backend_core import _request
from hoi4companion._geography_reference import GeographyIndex
from hoi4companion.native_geography import GeographyIndex as NativeGeography
from test_geography import catalog

class NativeGeographyTests(TestCase):
    def test_locations_and_conflicts_match_independent_geography(self):
        entries=catalog();expected=GeographyIndex(entries)
        native=NativeGeography(entries)
        self.assertEqual(native.states,expected.states)
        self.assertEqual(native.names,expected.names)
        self.assertEqual(native.provinces,expected.provinces)
        self.assertEqual(native.neighbors,expected.neighbors)
        handle=_request(0,'geography_open',entries=entries)['handle']
        self.addCleanup(lambda:_request(0,'geography_release',handle=handle))
        for params in ({'state_name':'Berlin'},{'state_name':'Berlino'}, {'state_id':64},
                       {'state_id':77,'state_name':'Berlin'}, {'province_id':6521},
                       {'province_id':True},{'state_name':'Unknown'}):
            for provincial in (False,True):
                self.assertEqual(_request(0,'geography_resolve',handle=handle,params=params,provincial=provincial),expected.resolve(params,provincial))
                self.assertEqual(native.resolve(params,provincial),expected.resolve(params,provincial))
