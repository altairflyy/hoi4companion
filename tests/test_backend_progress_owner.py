"""Native store owns both transitions and persistence, with conflict rollback."""
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch
from hoi4companion.backend_core import _request


class BackendProgressOwnerTests(unittest.TestCase):
    def test_default_application_store_uses_backend_owned_transactions(self):
        from hoi4companion.guide_progress import RunStore
        with patch('hoi4companion.progress_core._library',side_effect=AssertionError('legacy transaction adapter')):
            with RunStore() as store:
                created=store.ensure('one')
                self.assertEqual(store.get('one'),created)
                self.assertIsNotNone(store._owner)

    def test_native_restart_campaign_binding_and_conflicting_write_are_atomic(self):
        with TemporaryDirectory() as folder:
            path=str(Path(folder)/'progress.sqlite3')
            handle=_request(0,'runs_open',path=path)
            second=None
            def call(op,**args):return _request(0,'runs_call',handle=handle,operation=op,args=args)
            try:
                original=call('ensure',guide_id='guide',checks={'one':True})
                campaign='hoi4:11111111-1111-4111-8111-111111111111:00000001'
                call('bind_campaign',campaign_id=campaign,country='HUN',session_id='one',tick=100)
                call('update',guide_id='guide',checks={'one':True},started={'build':True})
                current=call('get',guide_id='guide')
                second=_request(0,'runs_open',path=path)
                restored=_request(0,'runs_call',handle=second,operation='get',args={'guide_id':'guide'})
                self.assertEqual(restored['run_id'],original['run_id'])
                self.assertEqual(restored['started'],{'build':True})
                call('update',guide_id='guide',checks={'first':True})
                with self.assertRaisesRegex(ValueError,'conflict|altra istanza'):
                    _request(0,'runs_call',handle=second,operation='update',args={'guide_id':'guide','checks':{'second':True}})
                self.assertEqual(_request(0,'runs_call',handle=second,operation='get',args={'guide_id':'guide'}),restored)
                self.assertEqual(call('get',guide_id='guide')['checks'],{'first':True})
                self.assertGreater(current['run_revision'],0)
            finally:
                _request(0,'runs_release',handle=handle)
                if second is not None:_request(0,'runs_release',handle=second)
