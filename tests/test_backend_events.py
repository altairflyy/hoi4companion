import unittest
from hoi4companion.backend_core import NativeEvents,state_signature
from hoi4companion._stream_reference import state_signature as reference_signature


class BackendEventsTests(unittest.TestCase):
    def test_semantic_changes_calendar_and_metadata_match_reference(self):
        samples=[{}, {'usable':True,'metrics':{'v':1},'sample':{'current_focus':'a','native_tick':1}},
            {'usable':True,'sample':{'country_identity_validation':'primary_selector_and_country_map',
            'session_id':'s','executable_sha256':'a'*64,'native_tick':60768000}},
            {'usable':True,'a':{'observed_at':'x','list':[{'age_seconds':3,'x':'Łódź'}]}}]
        for value in samples:
            with self.subTest(value=value):self.assertEqual(state_signature(value),reference_signature(value))

    def test_revision_coalescing_expiration_recovery_and_reconnect(self):
        with NativeEvents(.25,2) as hub:
            first=hub.subscribe(100);second=hub.subscribe(100)
            with self.assertRaises(ValueError):hub.subscribe(100)
            value={'usable':True,'status':'observed','sample':{'focus':'a'}}
            hub.publish(value,100);original=hub.wait(first,0,100)
            self.assertEqual(original,hub.wait(second,0,100))
            hub.publish({**value,'metrics':{'bytes':9}},101)
            self.assertIsNone(hub.wait(first,0,101))
            expired=hub.wait(second,0,106)
            self.assertFalse(expired['value']['usable']);self.assertEqual(expired['value']['status'],'stale')
            hub.publish(value,107);recovered=hub.wait(first,0,107)
            self.assertTrue(recovered['value']['usable']);self.assertGreater(recovered['revision'],expired['revision'])
            hub.unsubscribe(first);hub.unsubscribe(second)
            third=hub.subscribe(108);self.assertIsNone(hub.wait(third,0,108))
            hub.publish(value,108);self.assertGreater(hub.wait(third,0,108)['revision'],recovered['revision'])
