import copy
import unittest
from types import SimpleNamespace

from hoi4companion.overlay_geometry import NativeGeometry


class Reader:
    def __init__(self, path):
        self.value={'available':True,'pid':42,'country':'GER','scope':'german_focus',
                    'resolution':[400,300],'active_panel':'focus',
                    'panels':{'focus':{'visible':True,'clip':[0,60,400,240]},
                              'research':{'visible':False,'clip':[0,60,400,240]}},
                    'nodes':[{'panel':'focus','identity':'F','scope':'german_focus',
                              'visible':True,'rect':[100,90,100,80],'clip':[0,60,400,240],'scale':1}]}
        self.closed=False
    def snapshot(self,**kwargs):return copy.deepcopy(self.value)
    def close(self):self.closed=True


class NativeGeometryProviderTests(unittest.TestCase):
    def setUp(self):
        self.reader=Reader('unused')
        self.provider=NativeGeometry(lambda path:{'game_dir':'C:/fixture'},reader_factory=lambda path:self.reader)
        self.proposals=[{'key':'f','panel':'focus','identity':'F','country':'GER','scope':'german_focus'}]
        self.view=[99,42,0,0,400,300]
    def read(self):return self.provider.read(self.view,self.proposals)

    def test_id_geometry_tracks_pan_and_zoom_without_text(self):
        self.assertEqual(self.read()['matches'][0]['rect'],[100,90,200,170])
        self.reader.value['nodes'][0]['rect']=[140,110,50,40]
        self.assertEqual(self.read()['matches'][0]['rect'],[140,110,190,150])
        self.assertEqual(self.read()['active_panel'],'focus')

    def test_process_country_scope_and_resolution_are_required(self):
        original=copy.deepcopy(self.reader.value)
        for field,value in [('pid',43),('country','POL'),('scope','another_tree'),('resolution',[800,600])]:
            with self.subTest(field=field):
                self.reader.value=copy.deepcopy(original);self.reader.value[field]=value
                self.assertEqual(self.read()['matches'],[])
        self.reader.value=original;self.reader.value['nodes'][0]['scope']='another_tree'
        self.assertEqual(self.read()['matches'],[])

    def test_hidden_duplicate_offscreen_and_clipped_widgets(self):
        node=self.reader.value['nodes'][0]
        node['visible']=False;self.assertEqual(self.read()['matches'],[])
        node['visible']=True;self.reader.value['nodes'].append(copy.deepcopy(node))
        self.assertEqual(self.read()['matches'],[])
        self.reader.value['nodes'].pop();node['rect']=[390,90,100,80]
        self.assertEqual(self.read()['matches'][0]['rect'],[390,90,400,170])
        node['rect']=[410,90,100,80];self.assertEqual(self.read()['matches'],[])
        node['rect']=[100,90,float('nan'),80];self.assertEqual(self.read()['matches'],[])

    def test_closed_panel_and_transient_error_never_reuse_coordinates(self):
        self.assertEqual(len(self.read()['matches']),1)
        self.reader.value['panels']['focus']['visible']=False
        self.reader.value['active_panel']=None
        self.assertEqual(self.read()['matches'],[])
        self.reader.snapshot=lambda **kwargs: (_ for _ in ()).throw(ValueError('GUI changing'))
        result=self.read();self.assertEqual(result['matches'],[])
        self.assertTrue(result['suspend_visual'])
        self.provider.close();self.assertTrue(self.reader.closed)

    def research(self):
        self.reader.value['active_panel']='research'
        self.reader.value['panels']['focus']['visible']=False
        self.reader.value['panels']['research'].update(visible=True,folder='industry_folder')
        self.proposals=[{'key':'r','panel':'research','identity':'T','country':'GER','folder':'industry_folder'}]
        self.reader.value['nodes']=[{'panel':'research','identity':'T','visible':True,
                                   'folder':'industry_folder','rect':[10,90,100,80],'clip':[0,60,400,240]}]

    def test_research_uses_same_identity_geometry_and_clipping_as_focus(self):
        self.research()
        result=self.read()
        self.assertEqual(result['active_panel'],'research')
        self.assertIn('research',result['handled_panels'])
        self.assertEqual(result['matches'][0]['identity'],'T')
        self.assertEqual(result['matches'][0]['rect'],[10,90,110,170])
        self.reader.value['nodes'][0]['rect']=[40,130,50,40]
        self.assertEqual(self.read()['matches'][0]['rect'],[40,130,90,170])

    def test_folder_navigation_uses_exact_native_tab_and_groups_free_slot_steps(self):
        self.research()
        self.reader.value['panels']['research']['folder']='electronics_folder'
        self.reader.value['nodes']=[]
        self.proposals.append({**self.proposals[0],'key':'r2','identity':'T2'})
        self.reader.value['tabs']=[{'panel':'research_tab','identity':'industry_folder',
            'visible':True,'rect':[300,60,40,30],'clip':[0,60,400,240]}]
        result=self.read()
        self.assertEqual(len(result['matches']),1)
        tab=result['matches'][0]
        self.assertEqual(tab['panel'],'research_tab');self.assertEqual(tab['identity'],'industry_folder')
        self.assertEqual(tab['count'],2);self.assertEqual(tab['step_keys'],['r','r2'])
        self.reader.value['tabs'].append(copy.deepcopy(self.reader.value['tabs'][0]))
        self.assertEqual(self.read()['matches'],[])

    def test_selected_folder_never_highlights_hidden_node_or_redirects_back_to_same_tab(self):
        self.research()
        self.reader.value['nodes'][0]['visible']=False
        self.reader.value['tabs']=[{'panel':'research_tab','identity':'industry_folder',
            'visible':True,'rect':[300,60,40,30],'clip':[0,60,400,240]}]
        result=self.read()
        self.assertEqual(result['matches'],[])
        self.reader.value['nodes'][0].update(visible=True,folder='electronics_folder')
        self.assertEqual(self.read()['matches'],[])

    def test_research_cannot_highlight_a_different_country(self):
        self.research();self.reader.value['country']='POL'
        self.assertEqual(self.read()['matches'],[])

    def test_game_restart_reopens_before_observing_the_previous_process(self):
        self.read()
        old=self.reader;old.process=SimpleNamespace(pid=42)
        def forbidden(**kwargs):raise AssertionError('Old process must not be sampled')
        old.snapshot=forbidden
        self.reader=Reader('new');self.reader.value['pid']=43
        self.provider.factory=lambda path:self.reader
        self.view[1]=43
        self.assertEqual(len(self.read()['matches']),1)
        self.assertTrue(old.closed)


if __name__=='__main__':unittest.main()
