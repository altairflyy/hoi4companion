"""OCR can locate labels, never complete guide actions or override identity."""
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.screen_overlay import rectangles

def line(text,x,y,width=90,height=20):
    return {'text':text,'words':[{'text':text,'x':x,'y':y,'width':width,'height':height}]}

class NativeHudOcrTests(unittest.TestCase):
    def test_exact_multiline_duplicates_panel_event_and_cross_entity_guards_match(self):
        targets=[{'key':'f','panel':'focus','identity':'F','labels':['Royal Guard Divisions']},
                 {'key':'r','panel':'research','identity':'R','labels':['Industry II']},
                 {'key':'e','panel':'event','identity':'E:A','event_title':'A Royal Event','labels':['Accept'],'watch':True,'live_visible':True}]
        cases=[[],[line('National Focus',10,20),line('Royal Guard',100,150),line('Divisions',102,175)],
               [line('National Focus',10,20),line('Royal Guard Divisions',100,150),line('Royal Guard Divisions',300,150)],
               [line('Research',10,20),line('Industry II',100,150)],
               [line('A Royal Event',10,20),line('Accept',100,150)],
               [line('Focus',10,20),line('nazionali',100,25),line('Royal Guard Divisions',100,150)],
               [line('Research',10,20),line('Industry II',-10,150)]]
        for lines in cases:
            ocr={'width':800,'height':600,'lines':lines}
            self.assertEqual(_request(0,'hud_ocr_rectangles',ocr=ocr,proposals=targets),rectangles(ocr,targets))
        ocr={'width':800,'height':600,'lines':[line('Research',10,20),line('Industry II',100,150)]}
        targets.append({**targets[1],'identity':'OTHER'})
        self.assertEqual(_request(0,'hud_ocr_rectangles',ocr=ocr,proposals=targets),[])
