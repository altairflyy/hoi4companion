import json
import unittest
from unittest.mock import patch

from hoi4companion.overlay_windows import Feed


class OverlayFeedTests(unittest.TestCase):
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


if __name__=='__main__':unittest.main()
