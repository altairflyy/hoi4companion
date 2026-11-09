"""Coherent native application packets and guarded run writes."""
from tempfile import TemporaryDirectory
from pathlib import Path
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.guide import normalise_guide
from test_guide_runtime import state


class BackendApplicationTests(unittest.TestCase):
    def test_hud_stamp_changes_for_observation_progress_and_selection_without_carrying_state(self):
        app=self.open()
        first=app('overlay_stamp')
        self.assertEqual(app('overlay_stamp'),first)
        self.assertNotIn('sample',first)
        app('state',value={'usable':False,'sample':None})
        observed=app('overlay_stamp');self.assertNotEqual(observed,first)
        app('active',update={'guide_id':'two'})
        self.assertNotEqual(app('overlay_stamp'),observed)
        self.assertEqual(app('overlay_packet')['guide_id'],'two')

    def test_default_selection_preserves_registry_input_order_without_explicit_default(self):
        guides=[normalise_guide({'id':name,'tracks':{}}) for name in ['z-last-alphabetically','a-first-alphabetically']]
        handle=_request(0,'app_open',guides=guides)
        self.addCleanup(lambda:_request(0,'app_release',handle=handle))
        self.assertEqual(_request(0,'app_active',handle=handle)['guide_id'],'z-last-alphabetically')
    def open(self,**options):
        guide={'id':'one','tracks':{'operations':[
            {'key':'project','action':{'kind':'special_project','entity':{'kind':'project','id':'p'}},
             'started_when':{'op':'fact','key':'projects.active','test':'contains','value':'p'},
             'completion':{'op':'fact','key':'projects.completed','test':'contains','value':'p'}},
            {'key':'advisor','after':[{'key':'project','state':'started'}],
             'action':{'kind':'manual','params':{'category':'politics'}},
             'completion':{'op':'fact','key':'politics.ideas','test':'contains','value':'advisor'}},
            {'key':'note','action':{'kind':'manual'}}]}}
        handle=_request(0,'app_open',guides=[normalise_guide(guide),normalise_guide({'id':'two','tracks':{}})],**options)
        self.addCleanup(lambda:_request(0,'app_release',handle=handle))
        return lambda op,**args:_request(0,'app_'+op,handle=handle,**args)

    def test_projection_uses_same_guide_state_run_and_start_dependency(self):
        app=self.open()
        app('state',value=state(**{'projects.active':['p'],'projects.completed':[],'politics.ideas':[]}))
        packet=app('projection',guide_id='one',live=True)
        self.assertEqual(packet['guide_id'],packet['progress']['guide_id'])
        self.assertEqual(packet['guide_id'],packet['evaluation']['guide_id'])
        self.assertEqual(packet['guide']['runtime_definition_revision'],packet['definition_revision'])
        steps={s['key']:s for s in packet['evaluation']['steps']}
        self.assertEqual(steps['project']['status'],'active')
        self.assertEqual(steps['advisor']['status'],'ready')
        app('active',update={'guide_id':'two'})
        self.assertEqual(app('active')['guide_id'],'two')
        self.assertNotEqual(app('projection',guide_id='one',live=False)['evaluation']['mode'],'live')

    def test_restart_and_stale_client_writes_do_not_mix_guides_or_runs(self):
        with TemporaryDirectory() as folder:
            path=str(Path(folder)/'progress.sqlite3')
            app=self.open(path=path)
            old=app('progress',guide_id='one')
            update={'run_id':old['run_id'],'checks':{'note':True},'server_id':old['server_id'],
                    'expected_run_id':old['run_id'],'expected_revision':old['run_revision']}
            current=app('progress',guide_id='one',update=update)
            self.assertTrue(current['checks']['note'])
            with self.assertRaisesRegex(ValueError,'updated|aggiornato'):
                app('progress',guide_id='one',update=update)
            with self.assertRaises(ValueError):app('progress',guide_id='two',update={'run_id':old['run_id'],'checks':{'note':True}})
            reopened=self.open(path=path)
            self.assertEqual(reopened('progress',guide_id='one')['checks'],{'note':True})
            with self.assertRaisesRegex(ValueError,'restarted|riavviato'):
                reopened('progress',guide_id='one',update=update)

    def test_invalid_selection_does_not_fall_back_to_current_guide(self):
        app=self.open()
        for update in ({'guide_id':None},{'other':'one'},{'guide_id':12}):
            with self.assertRaises(ValueError):app('active',update=update)
        self.assertEqual(app('active')['guide_id'],'one')
