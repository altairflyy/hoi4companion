import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from hoi4companion.guide_progress import RunStore
from hoi4companion.progress_storage import ProgressStorageError,ProgressDatabase
from hoi4companion.campaign import SOURCE,observed_id
from hoi4companion.contracts import normalise_state
from hoi4companion.guide_runtime import evaluate


A='hoi4:11111111-1111-4111-8111-111111111111:00000001'
B='hoi4:22222222-2222-4222-8222-222222222222:00000002'


class CampaignProgressTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.path=Path(self.temp.name)/'progress.sqlite3'
        self.store=RunStore(path=self.path);self.store.ensure('guide')

    def test_restart_preserves_checks_started_baselines_and_rebinds_same_campaign(self):
        self.store.bind_campaign(A,'TST','process-one',100)
        original=self.store.update('guide',{'done':True,'not_done':False},started={'build':True})
        # Use the same validated snapshot seam as the native baseline observer.
        value=self.store.snapshot();value['runs'][original['run_id']]['baselines']={'build':{
            'fact_key':'country.states.items.1.military_factories','value':5,'quantity':2,'country':'TST','source':'process_memory'}}
        database=ProgressDatabase(self.path);database.load();database.save(value)
        reopened=RunStore(path=self.path);reopened.bind_campaign(A,'TST','process-two',110)
        restored=reopened.get('guide')
        self.assertEqual(restored['run_id'],original['run_id'])
        self.assertEqual(restored['checks'],{'done':True,'not_done':False})
        self.assertEqual(restored['started'],{'build':True})
        self.assertEqual(restored['baselines']['build']['value'],5)
        self.assertEqual(restored['session_id'],'process-two')
        self.assertEqual(RunStore(path=self.path).get('guide'),restored)

    def test_campaign_and_country_switches_restore_their_own_progress(self):
        self.store.bind_campaign(A,'TST','same-process',100)
        first=self.store.update('guide',{'a':True})
        self.store.bind_campaign(B,'TST','same-process',100)
        self.assertEqual(self.store.get('guide')['checks'],{})
        second=self.store.update('guide',{'b':True})
        self.store.bind_campaign(A,'OTH','same-process',100)
        self.assertEqual(self.store.get('guide')['checks'],{})
        self.store.bind_campaign(A,'TST','same-process',101)
        self.assertEqual(self.store.get('guide')['run_id'],first['run_id'])
        self.assertEqual(self.store.get('guide')['checks'],{'a':True})
        reopened=RunStore(path=self.path);reopened.bind_campaign(B,'TST','new-process',102)
        self.assertEqual(reopened.get('guide')['run_id'],second['run_id'])
        self.assertEqual(reopened.get('guide')['checks'],{'b':True})
        with self.assertRaises(ValueError):reopened.select('guide',first['run_id'])

    def test_rewind_forks_once_and_preserves_future_history(self):
        self.store.bind_campaign(A,'TST','session',100)
        original=self.store.update('guide',{'future':True})
        self.store.bind_campaign(A,'TST','session',120)
        self.store.bind_campaign(A,'TST','session',90)
        branch=self.store.get('guide')
        self.assertNotEqual(branch['run_id'],original['run_id'])
        self.assertEqual(branch['last_tick'],90)
        self.assertEqual(branch['checks'],{})
        self.assertEqual(branch['branch_reason'],'timeline_rewind')
        self.assertEqual(self.store.get('guide',original['run_id'])['checks'],{'future':True})
        self.assertFalse(self.store.bind_campaign(A,'TST','session',91))
        with self.assertRaises(ValueError):self.store.select('guide',original['run_id'])
        reopened=RunStore(path=self.path);reopened.bind_campaign(A,'TST','other-session',92)
        self.assertEqual(reopened.get('guide')['run_id'],branch['run_id'])

    def test_restart_at_older_tick_does_not_reuse_later_manual_evidence(self):
        self.store.bind_campaign(A,'TST','one',100);self.store.update('guide',{'done':True})
        reopened=RunStore(path=self.path);reopened.bind_campaign(A,'TST','two',80)
        self.assertEqual(reopened.get('guide')['checks'],{})
        self.assertEqual(len(reopened.history('guide')),2)

    def test_restored_legacy_run_is_not_assumed_to_belong_to_first_observed_campaign(self):
        legacy=self.store.update('guide',{'not_identified':True})
        reopened=RunStore(path=self.path);reopened.bind_campaign(A,'TST','session',100)
        self.assertNotEqual(reopened.get('guide')['run_id'],legacy['run_id'])
        self.assertEqual(reopened.get('guide')['checks'],{})
        self.assertEqual(reopened.get('guide',legacy['run_id'])['checks'],{'not_identified':True})
        historical=reopened.get('guide',legacy['run_id'])
        guide={'id':'guide','tracks':{'operations':[{'key':'not_identified','action':{'kind':'manual'}}]}}
        state={'usable':True,'sample':{'country':'TST','session_id':historical['session_id']}}
        self.assertEqual(evaluate(guide,state,historical)['progress_status'],'campaign_unverified')

    def test_no_write_for_forward_time_and_failed_write_rolls_back(self):
        self.store.bind_campaign(A,'TST','one',100)
        revision=self.store._database.revision
        self.assertFalse(self.store.bind_campaign(A,'TST','one',101))
        self.assertEqual(self.store._database.revision,revision)
        before=self.store.snapshot()
        with patch.object(self.store._database,'save',side_effect=ProgressStorageError('disk full')):
            with self.assertRaises(ProgressStorageError):self.store.update('guide',{'done':True})
        self.assertEqual(self.store.snapshot(),before)
        self.assertEqual(RunStore(path=self.path).snapshot(),before)

    def test_another_server_cannot_overwrite_newer_progress(self):
        other=RunStore(path=self.path)
        self.store.update('guide',{'new':True})
        before=other.snapshot()
        with self.assertRaises(ProgressStorageError):other.update('guide',{'old':True})
        self.assertEqual(other.snapshot(),before)
        self.assertEqual(RunStore(path=self.path).get('guide')['checks'],{'new':True})

    def test_corrupt_and_future_archives_are_preserved(self):
        bad=Path(self.temp.name)/'bad.sqlite3';bad.write_bytes(b'not a database')
        with self.assertRaises(ProgressStorageError):RunStore(path=bad)
        self.assertEqual(bad.read_bytes(),b'not a database')
        database=ProgressDatabase(self.path);database.load();database.save({'schema_version':2,'runs':{},'active':{},'campaign_active':{}})
        before=self.path.read_bytes()
        with self.assertRaises(ValueError):RunStore(path=self.path)
        self.assertEqual(self.path.read_bytes(),before)

    def test_identity_requires_native_observed_provenance(self):
        sample={'country':'TST','campaign_identity':{'status':'observed','id':A,'source':SOURCE}}
        self.assertEqual(observed_id(sample),A)
        self.assertEqual(normalise_state({'usable':True,'sample':sample})['facts']['campaign.id']['value'],A)
        for invalid in ({'status':'calculated'},{'source':'country_and_date'},{'id':'TST-1936'},{'id':'hoi4:00000000-0000-0000-0000-000000000000:00000000'}):
            altered=sample|{'campaign_identity':sample['campaign_identity']|invalid}
            self.assertIsNone(observed_id(altered))
            self.assertNotIn('campaign.id',normalise_state({'usable':True,'sample':altered})['facts'])

    def test_runtime_rejects_other_or_unverified_campaign_even_with_same_session(self):
        guide={'id':'guide','tracks':{'operations':[{'key':'done','action':{'kind':'manual'}}]}}
        self.store.bind_campaign(A,'TST','session',100)
        progress=self.store.update('guide',{'done':True})
        def state(campaign):
            return {'usable':True,'sample':{'country':'TST','session_id':'session',
                'campaign_identity':{'status':'observed','id':campaign,'source':SOURCE}}}
        self.assertEqual(evaluate(guide,state(A),progress)['steps'][0]['status'],'completed')
        self.assertEqual(evaluate(guide,state(B),progress)['progress_status'],'wrong_campaign')
        self.assertNotEqual(evaluate(guide,state(B),progress)['steps'][0]['status'],'completed')
        self.assertEqual(evaluate(guide,{'usable':False},progress)['progress_status'],'campaign_unverified')


if __name__=='__main__':unittest.main()
