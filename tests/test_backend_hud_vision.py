"""Independent screenshot fixtures compare the native visual fallback."""
import unittest
from hoi4companion.backend_core import _request

def recognize(fixture,frame=None,proposals=None,ocr=None,node_matches=None):
    frame=frame or fixture.frame
    return _request(0,'hud_vision_fixture',frame={'width':frame.width,'height':frame.height,'rgb':frame.tobytes().hex()},
                    assets={k:v.hex() for k,v in fixture.assets.items()},
                    trees={'test':fixture.tree} if hasattr(fixture,'tree') else {},
                    catalog=getattr(fixture,'catalog',{}),proposals=fixture.proposals if proposals is None else proposals,
                    ocr=getattr(fixture,'ocr',{}) if ocr is None else ocr,node_matches=node_matches or [])

class NativeHudVisionTests(unittest.TestCase):
    def test_binary_frame_cache_is_bound_to_guide_session_view_and_exact_payload(self):
        import ctypes,json
        from hoi4companion.backend_core import _library,_reply
        from PIL import Image
        from test_backend_hud_ocr import line
        handle=_request(0,'vision_open',assets={},trees={},catalog={})
        self.addCleanup(lambda:_request(0,'vision_release',handle=handle))
        dll=_library();function=dll.hoi4_backend_hud_frame
        function.argtypes=[ctypes.c_uint64,ctypes.c_void_p,ctypes.c_size_t,ctypes.c_void_p,ctypes.c_size_t]
        function.restype=ctypes.c_void_p
        pixels=Image.new('RGB',(200,200),(20,30,40)).tobytes('raw','BGRX')
        args={'width':200,'height':200,'raw_len':len(pixels),'version':7,'guide_id':'one','guide_server_id':'server',
              'view':[11,42,0,0,200,200],'proposals':[{'key':'f','panel':'focus','identity':'F','labels':['Test Focus']}],
              'ocr':{'width':200,'height':200,'lines':[line('National Focus',5,5),line('Test Focus',20,100)]}}
        def render(raw=pixels):
            packet=json.dumps(args).encode();p=ctypes.create_string_buffer(packet);b=ctypes.create_string_buffer(raw)
            return _reply(dll,function(handle,p,len(packet),b,len(raw)))
        first=render();self.assertFalse(first['cache_hit']);self.assertEqual(len(first['matches']),1)
        self.assertTrue(render()['cache_hit'])
        args['guide_id']='two';self.assertFalse(render()['cache_hit'])
        args['guide_server_id']='restarted';self.assertFalse(render()['cache_hit'])
        args['view'][1]=99;self.assertFalse(render()['cache_hit'])
        with self.assertRaises(ValueError):render(pixels[:-4])
        self.assertTrue(render()['cache_hit'])

    def test_exact_sprite_and_two_independent_landmarks_match_existing_reader(self):
        from test_overlay_sprites import ResearchSpriteTests
        fixture=ResearchSpriteTests();fixture.setUp()
        expected=fixture.matcher.match(fixture.frame,fixture.proposals,fixture.ocr)
        actual=recognize(fixture)['matches']
        self.assertEqual(actual,expected)
        self.assertEqual(recognize(fixture,ocr={'lines':[]})['matches'],[])
        fixture.tree['technologies'].append({**fixture.tree['technologies'][0],'id':'duplicate'})
        self.assertEqual(recognize(fixture)['matches'],[])

    def test_research_tabs_preserve_choice_order_and_require_two_distinct_peers(self):
        from test_overlay_sprites import ResearchTabTests
        fixture=ResearchTabTests();fixture.setUp()
        for direct in ([],[{'panel':'research','folder':'0'}]):
            expected=fixture.matcher.navigation(fixture.frame,fixture.proposals,direct)
            self.assertEqual(recognize(fixture,node_matches=direct)['navigation'],expected)
