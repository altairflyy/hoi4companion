"""Native GUI identity/viewport contracts: no OCR, no guessed tree transform."""
import copy
import unittest

from hoi4companion.overlay_bridge import Bridge
from test_overlay_native import FakeFeed,FakeSprites


class GeometryProvider:
    def __init__(self):
        self.calls=[]
        self.result={'available':True,'matches':[{'key':'focus','panel':'focus','identity':'F',
            'labels':['Focus'],'title':'Focus','rect':[50,80,170,160],
            'basis':'native_gui_identity'}],'handled_panels':['focus','research'],'read_ms':1}

    def read(self,view,proposed):
        self.calls.append((view,proposed))
        return copy.deepcopy(self.result)

    def close(self):pass


class OverlayGeometryTests(unittest.TestCase):
    def bridge(self):
        feed=FakeFeed()
        feed.value=[{'key':'focus','panel':'focus','identity':'F','labels':['Focus'],'title':'Focus','status':'ready'}]
        geometry=GeometryProvider()
        bridge=Bridge(feed,sprites=FakeSprites(),geometry=geometry)
        return bridge,feed,geometry

    def request(self,bridge,version=7,view=None):
        return bridge.handle({'schema_version':1,'kind':'geometry','token':1,'version':version,
                              'view':view or [99,42,0,0,400,300]})

    def test_current_native_rectangles_do_not_require_ocr_or_bgra(self):
        bridge,feed,geometry=self.bridge()
        first=self.request(bridge)
        self.assertTrue(first['usable']);self.assertTrue(first['native_available'])
        self.assertEqual(first['matches'][0]['rect'],[50,80,170,160])
        geometry.result['matches'][0]['rect']=[125,100,185,140]
        second=self.request(bridge)
        self.assertEqual(second['matches'][0]['rect'],[125,100,185,140])
        self.assertEqual(len(geometry.calls),2)

    def test_revision_and_live_failure_cannot_reuse_old_native_rectangles(self):
        bridge,feed,geometry=self.bridge()
        self.assertFalse(self.request(bridge,version=6)['usable'])
        self.assertEqual(len(geometry.calls),0)
        original=geometry.read
        def changed(*args):
            result=original(*args);feed.version+=1;return result
        geometry.read=changed
        result=self.request(bridge)
        self.assertFalse(result['usable']);self.assertEqual(result['matches'],[])
        feed.updated=0
        self.assertFalse(self.request(bridge,version=8)['usable'])

    def test_invalid_view_cannot_authorize_a_memory_observation(self):
        bridge,feed,geometry=self.bridge()
        for view in ([99,0,0,0,400,300],[99,42,0,0,-1,300],[99,42,0,0,90000,300]):
            with self.assertRaises(ValueError):self.request(bridge,view=view)
        self.assertEqual(geometry.calls,[])

    def test_native_failure_is_explicit_and_discards_marks(self):
        bridge,feed,geometry=self.bridge()
        geometry.result={'available':False,'matches':[],'error':'GUI non qualificata','handled_panels':[]}
        result=self.request(bridge)
        self.assertFalse(result['native_available']);self.assertEqual(result['matches'],[])

    def test_native_category_can_only_reference_current_research_proposals(self):
        bridge,feed,geometry=self.bridge()
        feed.value=[{'key':'r1','panel':'research','identity':'T1','country':'GER','folder':'industry_folder'},
                    {'key':'r2','panel':'research','identity':'T2','country':'GER','folder':'industry_folder'}]
        category={'key':'research-tab:industry_folder','panel':'research_tab','identity':'industry_folder',
                  'folder':'industry_folder','step_keys':['r1','r2'],'count':2,'rect':[10,80,50,110]}
        geometry.result['matches']=[category]
        self.assertEqual(self.request(bridge)['matches'][0]['count'],2)
        for mutation in ({'identity':'other_folder'},{'step_keys':['focus']},{'count':4},
                         {'step_keys':['r1','r1']},{'step_keys':[]}):
            geometry.result['matches']=[{**category,**mutation}]
            self.assertEqual(self.request(bridge)['matches'],[])


if __name__=='__main__':unittest.main()
