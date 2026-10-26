import json
import os
from pathlib import Path
import tempfile
import time
import unittest

from hoi4companion.overlay_diagnostics import HudStatus
from hoi4companion.stream import state_signature


class HudStatusTests(unittest.TestCase):
    def test_current_semantic_status_is_shared_without_changing_on_frame_metrics(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'hud.json';reader=HudStatus(path)
            value={'guide_id':'g','guide_server_id':'s','status_code':'different_folder',
                   'status_message':'Apri Industria','geometry_ms':2.5,'explanations':[
                       {'code':'different_folder','message':'Ricerca in Industria','panel':'research','folder':'industry','step_keys':['r']} ]}
            path.write_text(json.dumps(value));first=reader.read('g','s')
            self.assertEqual(first['status'],'observed');self.assertEqual(first['items'][0]['folder'],'industry')
            value['geometry_ms']=42;path.write_text(json.dumps(value));second=reader.read('g','s')
            self.assertEqual(state_signature({'overlay':first}),state_signature({'overlay':second}))
            self.assertNotIn('geometry_ms',first)
            # Cached decoding must not keep exposing a stopped HUD as current.
            old=time.time()-10;os.utime(path,(old,old))
            self.assertEqual(reader.read('g','s')['code'],'stale')

    def test_other_guide_other_server_missing_invalid_or_oversized_cannot_expose_old_messages(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'hud.json';reader=HudStatus(path)
            self.assertEqual(reader.read('g','s')['status'],'unavailable')
            path.write_text(json.dumps({'guide_id':'old','guide_server_id':'s','status_message':'OLD_SECRET'}))
            self.assertNotIn('OLD_SECRET',json.dumps(reader.read('g','s')))
            path.write_text(json.dumps({'guide_id':'g','guide_server_id':'old','status_message':'OLD_SECRET'}))
            self.assertNotIn('OLD_SECRET',json.dumps(reader.read('g','s')))
            for text in ('{','[]','x'*262145):
                path.write_text(text);self.assertEqual(reader.read('g','s')['status'],'unavailable')

    def test_explanation_count_and_text_are_bounded(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'hud.json'
            path.write_text(json.dumps({'guide_id':'g','guide_server_id':'s','status_message':'x'*1000,
                                       'explanations':[{'code':'x','message':'m'}]*20}))
            value=HudStatus(path).read('g','s');self.assertEqual(len(value['message']),500)
            self.assertEqual(len(value['items']),8)
