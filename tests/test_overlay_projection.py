"""HUD navigation consumes the same coherent decision packet as web pages."""
import unittest

from hoi4companion.overlay_windows import Feed


class OverlayProjectionTests(unittest.TestCase):
    def test_native_guide_uses_packet_state_and_never_a_second_runtime_request(self):
        feed=Feed('http://127.0.0.1:1','g');feed.selection_server='server'
        guide={'id':'g','runtime_engine':'rust-guide-core','runtime_definition_revision':'rev'}
        packet={'schema_version':1,'engine':'rust-guide-core','guide_id':'g','server_id':'server',
            'definition_revision':'rev','state':{'sample':{'session_id':'fresh'}},
            'progress':{'guide_id':'g','run_id':'run'},
            'evaluation':{'guide_id':'g','run_id':'run','mode':'live'}}
        calls=[]
        feed.get=lambda path:(calls.append(path) or packet)
        definition,runtime,state=feed.projection(guide,'?id=g',{'sample':{'session_id':'old'}})
        self.assertIs(definition,guide);self.assertIs(runtime,packet['evaluation']);self.assertIs(state,packet['state'])
        self.assertEqual(calls,['/api/guide/projection?id=g'])

    def test_late_other_run_guide_or_server_packets_are_rejected(self):
        for mismatch in ('guide','server','run'):
            with self.subTest(mismatch=mismatch):
                feed=Feed('http://127.0.0.1:1','g');feed.selection_server='server'
                guide={'id':'g','runtime_engine':'rust-guide-core','runtime_definition_revision':'rev'}
                packet={'schema_version':1,'engine':'rust-guide-core','guide_id':'g','server_id':'server',
                    'definition_revision':'rev','state':{},'progress':{'guide_id':'g','run_id':'run'},
                    'evaluation':{'guide_id':'g','run_id':'run'}}
                if mismatch=='guide':packet['evaluation']['guide_id']='other'
                elif mismatch=='server':packet['server_id']='other'
                else:packet['evaluation']['run_id']='other'
                feed.get=lambda path:packet
                with self.assertRaisesRegex(ValueError,'shared guide evaluation'):feed.projection(guide,'?id=g',{})

    def test_changed_definition_is_reloaded_once_and_must_match_the_packet(self):
        feed=Feed('http://127.0.0.1:1','g')
        old={'id':'g','runtime_engine':'rust-guide-core','runtime_definition_revision':'old'}
        new={**old,'runtime_definition_revision':'new'}
        packet={'schema_version':1,'engine':'rust-guide-core','guide_id':'g','server_id':'server',
            'definition_revision':'new','state':{},'progress':{'guide_id':'g','run_id':'run'},
            'evaluation':{'guide_id':'g','run_id':'run'}}
        calls=[]
        feed.get=lambda path:(calls.append(path) or (packet if '/projection' in path else new))
        self.assertIs(feed.projection(old,'?id=g',{})[0],new)
        self.assertEqual(calls,['/api/guide/projection?id=g','/api/guide?id=g'])
        feed.get=lambda path:packet if '/projection' in path else old
        with self.assertRaisesRegex(ValueError,'definition changed'):feed.projection(old,'?id=g',{})

    def test_unverified_campaign_may_withhold_the_run_without_rejecting_the_packet(self):
        feed=Feed('http://127.0.0.1:1','g')
        guide={'id':'g','runtime_engine':'rust-guide-core','runtime_definition_revision':'rev'}
        packet={'schema_version':1,'engine':'rust-guide-core','guide_id':'g','server_id':'server',
            'definition_revision':'rev','state':{'usable':False},'progress':{'guide_id':'g','run_id':'saved'},
            'evaluation':{'guide_id':'g','run_id':None,'progress_status':'campaign_unverified','mode':'preview'}}
        feed.get=lambda path:packet
        self.assertIs(feed.projection(guide,'?id=g',{})[1],packet['evaluation'])

    def test_older_protocol_is_preserved_without_using_a_local_interpreter(self):
        feed=Feed('http://127.0.0.1:1','g');guide={'id':'g'};state={'usable':False};calls=[]
        feed.get=lambda path:(calls.append(path) or {'mode':'preview'})
        self.assertEqual(feed.projection(guide,'?id=g',state),(guide,{'mode':'preview'},state))
        self.assertEqual(calls,['/api/guide/runtime?id=g'])
