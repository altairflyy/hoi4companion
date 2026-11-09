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
    def test_combined_observation_keeps_legacy_admission_with_one_request(self):
        bridge,feed,geometry=self.bridge()
        request={'schema_version':1,'kind':'observe','token':9,'view':[99,42,0,0,400,300]}
        result=bridge.handle(request)
        self.assertTrue(result['usable']);self.assertEqual(result['version'],feed.version)
        self.assertEqual(result['kind'],'observe');self.assertEqual(result['token'],9)
        self.assertEqual(result['guides'][0]['id'],'test')
        self.assertEqual(len(geometry.calls),1)
        legacy=self.request(bridge)
        for key in ('matches','proposed','version','view','usable','native_available'):
            self.assertEqual(result[key],legacy[key])
        # The combined reply omits the redundant second full proposal packet.
        import json
        old_snapshot=bridge.handle({'schema_version':1,'kind':'snapshot','token':8})
        self.assertLess(len(json.dumps(result)),len(json.dumps(old_snapshot))+len(json.dumps(legacy)))

    def test_combined_observation_never_reuses_geometry_across_revision_or_outage(self):
        bridge,feed,geometry=self.bridge()
        request={'schema_version':1,'kind':'observe','view':[99,42,0,0,400,300]}
        original=geometry.read
        def changing(*args):
            result=original(*args);feed.version+=1;return result
        geometry.read=changing
        result=bridge.handle(request)
        self.assertFalse(result['usable']);self.assertEqual(result['matches'],[])
        geometry.read=original;feed.updated=0
        self.assertFalse(bridge.handle(request)['usable'])
        self.assertEqual(len(geometry.calls),1)
        bridge.guides=None;feed.get=lambda path:(_ for _ in ()).throw(ConnectionError('offline'))
        result=bridge.handle(request)
        self.assertFalse(result['usable']);self.assertEqual(result['matches'],[])
        self.assertEqual(len(geometry.calls),1)
        for view in ([99,0,0,0,400,300],[99,42,0,0,90000,300]):
            with self.assertRaises(ValueError):bridge.handle({**request,'view':view})

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

    def test_one_production_proposal_keeps_both_its_category_and_exact_product(self):
        bridge,feed,geometry=self.bridge()
        feed.value=[{'key':'product','panel':'production','identity':'tank','title':'Required tank','status':'ready'}]
        geometry.result['matches']=[
            {'key':'product','panel':'production','identity':'tank','navigation_stage':'add','rect':[10,20,60,50]},
            {'key':'product','panel':'production','identity':'tank','navigation_stage':'equipment','rect':[100,80,200,140]}]
        result=self.request(bridge)
        self.assertEqual([m['navigation_stage'] for m in result['matches']],['add','equipment'])
        feed.version+=1
        self.assertEqual(self.request(bridge)['matches'],[])

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
