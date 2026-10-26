import json
import unittest
from unittest.mock import patch

from hoi4companion.overlay_windows import Feed


class OverlayFeedTests(unittest.TestCase):
    def test_same_targets_in_another_campaign_invalidate_geometry_version(self):
        feed=Feed('http://127.0.0.1:8766','guide')
        guide={'id':'guide','title':'Guide','tracks':{'focus':[{'key':'f','action':{'kind':'select_focus','entity':{'kind':'focus','id':'x'}}}]}}
        state={'usable':True,'sample':{'country':'TST','session_id':'s'}}
        runtime={'guide_id':'guide','country':'TST','mode':'live','steps':[{'key':'f','status':'ready'}],
                 'parallel':{'focus':{'steps':['f']}},'action_context':{'session_id':'s'}}
        feed.get=lambda path: {'entities':[{'kind':'focus','id':'x','name':'A Focus'}]} if path=='/api/import/catalog' else runtime if '/runtime' in path else guide
        snapshots=[]
        def events():
            for number in (1,2):
                identity=f'hoi4:00000000-0000-0000-0000-{number:012d}:00000001'
                state['sample']['campaign_identity']={'status':'observed','id':identity,'source':'native_game_unique_id_and_seed'}
                runtime['action_context']['campaign_id']=identity
                yield b'event: state\n'
                yield ('data: '+json.dumps(state)+'\n').encode()
                snapshots.append(feed.snapshot())
            feed.closed.set()
        class Stream:
            def __enter__(self):return iter(events())
            def __exit__(self,*args):pass
        with patch('hoi4companion.overlay_windows.urlopen',return_value=Stream()):feed.run()
        self.assertEqual(snapshots[0][1],snapshots[1][1])
        self.assertGreater(snapshots[1][0],snapshots[0][0])

    def test_unusable_heartbeat_clears_and_recovery_fetches_fresh_state(self):
        feed=Feed('http://127.0.0.1:8766','guide')
        guide={'id':'guide','title':'Generic Guide','applies_to':{},'tracks':{'focus':[
            {'key':'f','action':{'kind':'select_focus','entity':{'kind':'focus','id':'x'}}}]}}
        state={'usable':True,'sample':{'country':'TST'}}
        runtime={'guide_id':'guide','country':'TST','mode':'live','steps':[{'key':'f','status':'ready'}],
                 'parallel':{'focus':{'steps':['f']}}}
        fresh_reads=[];snapshots=[]
        def get(path):
            if path=='/api/import/catalog':return {'entities':[{'kind':'focus','id':'x','name':'A Focus'}]}
            if path=='/api/guide?id=guide':return guide
            if path=='/api/guide/runtime?id=guide':return runtime
            if path=='/api/state':fresh_reads.append(path);return state
            self.fail(path)
        feed.get=get
        def events():
            for event,payload in [('state',state),('heartbeat',{'usable':False}),('heartbeat',{'usable':True})]:
                yield ('event: '+event+'\n').encode()
                yield ('data: '+json.dumps(payload)+'\n').encode()
                snapshots.append(feed.snapshot())
            feed.closed.set()
        class Stream:
            def __enter__(self):return iter(events())
            def __exit__(self,*args):pass
        with patch('hoi4companion.overlay_windows.urlopen',return_value=Stream()):feed.run()
        self.assertEqual(len(snapshots[0][1]),1)
        self.assertEqual(snapshots[1][1],[]);self.assertTrue(snapshots[1][3])
        self.assertEqual(len(snapshots[2][1]),1);self.assertEqual(snapshots[2][3],'')
        self.assertEqual(fresh_reads,['/api/state'])

    def test_switching_guides_invalidates_pending_rectangles(self):
        feed=Feed('http://127.0.0.1:8766','old');feed.value=[{'key':'old'}];feed.updated=10
        version=feed.version;feed.select('new')
        self.assertGreater(feed.version,version)
        self.assertEqual(feed.snapshot()[1],[]);self.assertEqual(feed.snapshot()[2],0)

    def test_service_restart_invalidates_same_guide_and_catalog_context(self):
        feed=Feed('http://127.0.0.1:8766','guide')
        feed.sync_selection({'active_guide_id':'guide','active_guide_revision':0,'guide_server_id':'old'})
        feed.value=[{'key':'old'}];feed.updated=10;version=feed.version
        feed.sync_selection({'active_guide_id':'guide','active_guide_revision':0,'guide_server_id':'new'})
        self.assertGreater(feed.version,version)
        self.assertEqual(feed.value,[])
        self.assertEqual(feed.updated,0)

    def test_server_selection_invalidates_old_targets_and_rejects_older_updates(self):
        feed=Feed('http://127.0.0.1:8766','a');feed.value=[{'key':'old'}];feed.updated=10
        feed.sync_selection({'active_guide_id':'b','active_guide_revision':2,'guide_server_id':'server'})
        self.assertEqual(feed.guide_id,'b');self.assertEqual(feed.snapshot()[1],[])
        version=feed.version
        feed.sync_selection({'active_guide_id':'a','active_guide_revision':1,'guide_server_id':'server'})
        self.assertEqual(feed.guide_id,'b');self.assertEqual(feed.version,version)
        feed.sync_selection({'active_guide_id':'a','active_guide_revision':0,'guide_server_id':'new-server'})
        self.assertEqual(feed.guide_id,'a')

    def test_hud_selection_is_confirmed_by_server_before_local_activation(self):
        feed=Feed('http://127.0.0.1:8766','a');feed.value=[{'key':'old'}]
        class Response:
            def __enter__(self):return self
            def __exit__(self,*args):pass
            def read(self):return b'{"guide_id":"b","revision":3,"server_id":"s"}'
        with patch('hoi4companion.overlay_windows.urlopen',return_value=Response()) as call:feed.activate('b')
        request=call.call_args.args[0]
        self.assertEqual(request.method,'POST');self.assertEqual(json.loads(request.data),{'guide_id':'b'})
        self.assertEqual(feed.guide_id,'b');self.assertEqual(feed.value,[])
        self.assertEqual(feed.selection_revision,3)


if __name__=='__main__':unittest.main()
