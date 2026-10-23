import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from test_native_fixture import Fixture
from hoi4companion.live import NativeSample,read_native_state,watch_changed,compile_watch_reads,SUPPORTED_HASH,LiveReadError
from hoi4companion.campaign import read_campaign
from hoi4companion.guide_runtime import evaluate


class CampaignNativeTests(unittest.TestCase):
    def fixture(self):
        game=Fixture();game.profile.sha256=SUPPORTED_HASH
        game.profile.bytes=lambda address,size:bytes.fromhex('4c8d86280700004533c9ba803b0000488bcb')
        game.put(game.base+0x3423158,'<I',0x3b80)
        game.put(game.world+0x720,'<I',1234)
        game.string(game.world+0x728,'11111111-1111-4111-8111-111111111111')
        return game

    def test_uuid_and_seed_are_witnessed_and_changes_are_in_existing_watch_plan(self):
        game=self.fixture();plan={}
        with patch('hoi4companion.country.collect_country',return_value={}):state=read_native_state(game,watch_plan=plan)
        self.assertEqual(state['campaign_identity']['id'],'hoi4:11111111-1111-4111-8111-111111111111:000004d2')
        reads=compile_watch_reads(plan);self.assertFalse(watch_changed(game,reads))
        game.put(game.world+0x720,'<I',4321)
        self.assertTrue(watch_changed(game,reads))

    def test_invalid_uuid_or_token_only_marks_campaign_unknown(self):
        game=self.fixture();game.put(game.base+0x3423158,'<I',0)
        with patch('hoi4companion.country.collect_country',return_value={}):state=read_native_state(game)
        self.assertEqual(state['campaign_identity']['status'],'unknown')
        self.assertEqual(state['country'],'ROM')
        game=self.fixture();game.string(game.world+0x728,'00000000-0000-0000-0000-000000000000')
        self.assertEqual(read_campaign(NativeSample(game),game.world)['status'],'unknown')

    def test_uuid_change_is_detected_even_at_same_tick_country_and_seed(self):
        game=self.fixture();plan={}
        with patch('hoi4companion.country.collect_country',return_value={}):read_native_state(game,watch_plan=plan)
        reads=compile_watch_reads(plan)
        game.string(game.world+0x728,'22222222-2222-4222-8222-222222222222')
        self.assertTrue(watch_changed(game,reads))

    def test_shared_campaign_runtime_fixtures(self):
        data=json.loads((Path(__file__).parent/'fixtures/campaign-progress.json').read_text())
        for case in data['cases']:
            state=data['state']|{'sample':data['state']['sample']|case['sample']}
            result=evaluate(data['guide'],state,data['progress'])
            with self.subTest(case['name']):
                self.assertEqual(result['progress_status'],case['progress_status'])
                self.assertEqual(result['steps'][0]['status'],case['step_status'])


if __name__=='__main__':unittest.main()
