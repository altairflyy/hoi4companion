import io
import random
import unittest
from importlib.util import find_spec

from PIL import Image
from hoi4companion.overlay_sprites import ResearchSprites


@unittest.skipUnless(find_spec('cv2') and find_spec('numpy'),'optional overlay sprite dependencies unavailable')
class ResearchSpriteTests(unittest.TestCase):
    def setUp(self):
        rng=random.Random(42);self.assets={};nodes=[]
        self.frame=Image.new('RGB',(600,400),(30,30,30))
        for index,(x,y) in enumerate(((50,80),(200,80),(50,220))):
            icon=Image.new('RGBA',(24,24))
            icon.putdata([tuple(rng.randrange(40,240) for _ in range(3))+(255,) for _ in range(576)])
            data=io.BytesIO();icon.save(data,format='PNG');url='/api/sprite/'+str(index)+'.png';self.assets[url]=data.getvalue()
            nodes.append({'id':str(index),'folders':[{'id':'test','layout':{'x':x,'y':y,'width':40,'height':40}}],
                          'sprite':{'url':url},'visual':{'icon':{'centered':True,'x':20,'y':20}}})
            self.frame.paste(icon,(x+8+12,y+8+16),icon)
        self.tree={'technologies':nodes,'layout':{'captions':[{'kind':'title','text':'Industry'}]}}
        self.matcher=ResearchSprites(lambda path:self.tree,lambda path:self.assets[path])
        self.proposals=[{'key':'r','identity':'0','panel':'research','folder':'test','labels':['Target'],'status':'ready'}]
        self.ocr={'lines':[{'text':'Industry'}]}

    def test_target_and_two_independent_landmarks_recover_translation(self):
        result=self.matcher.match(self.frame,self.proposals,self.ocr)
        self.assertEqual(len(result),1)
        self.assertEqual(result[0]['rect'],[62,96,102,136])
        self.assertEqual(result[0]['basis'],'sprite_and_two_gui_landmarks')
        self.assertEqual(result[0]['landmarks'],2)

    def test_missing_context_changed_landmark_and_duplicate_target_fail_closed(self):
        self.assertEqual(self.matcher.match(self.frame,self.proposals,{'lines':[]}),[])
        changed=self.frame.copy();changed.paste((0,0,0),(220,104,244,128))
        self.assertEqual(self.matcher.match(changed,self.proposals,self.ocr),[])
        # Copy the entire layout, not just the target: two complete copies are ambiguous.
        duplicated=Image.new('RGB',(1200,400),(30,30,30));duplicated.paste(self.frame,(0,0));duplicated.paste(self.frame,(600,0))
        self.assertEqual(self.matcher.match(duplicated,self.proposals,self.ocr),[])

    def test_asset_loader_never_fetches_external_urls(self):
        self.tree['technologies'][0]['sprite']['url']='https://example.invalid/asset.png'
        self.assertEqual(self.matcher.match(self.frame,self.proposals,self.ocr),[])

    def test_reused_icon_cannot_identify_a_technology(self):
        self.tree['technologies'].append(self.tree['technologies'][0]|{'id':'another_level'})
        self.assertEqual(self.matcher.match(self.frame,self.proposals,self.ocr),[])

    def test_non_centered_and_mixed_gui_icon_origins_are_respected(self):
        # The artillery small-item GUI anchors its icon at the upper left,
        # whereas other item templates center the sprite.
        self.tree['technologies'][0]['visual']['icon']={'centered':False,'x':8,'y':8}
        result=self.matcher.match(self.frame,self.proposals,self.ocr)
        self.assertEqual(result[0]['rect'],[62,96,102,136])
        for node in self.tree['technologies']:
            node['visual']['icon']={'centered':False,'x':8,'y':8}
        self.matcher.folders={}
        self.assertEqual(self.matcher.match(self.frame,self.proposals,self.ocr)[0]['rect'],[62,96,102,136])

    def test_dark_icon_discovery_uses_the_same_rgb_bound_as_admission(self):
        rng=random.Random(18);icon=Image.new('RGBA',(24,24))
        pixels=[tuple(rng.randrange(5,35) for _ in range(3))+(255,) for _ in range(576)]
        icon.putdata(pixels);data=io.BytesIO();icon.save(data,format='PNG');self.assets['/api/sprite/0.png']=data.getvalue()
        for delta,expected in ((10,1),(14,0)):
            observed=Image.new('RGBA',icon.size);observed.putdata([tuple(v+delta for v in p[:3])+(255,) for p in pixels])
            frame=self.frame.copy();frame.paste(observed,(70,104),observed)
            self.assertEqual(len(self.matcher.match(frame,self.proposals,self.ocr)),expected)


@unittest.skipUnless(find_spec('cv2') and find_spec('numpy'),'optional overlay sprite dependencies unavailable')
class ResearchTabTests(unittest.TestCase):
    def setUp(self):
        rng=random.Random(7);self.assets={};folders=[]
        self.frame=Image.new('RGB',(600,400),(30,30,30))
        for index,x in enumerate((20,95,170,245)):
            asset=Image.new('RGBA',(60,28))
            asset.putdata([tuple(rng.randrange(40,240) for _ in range(3))+(255,) for _ in range(60*28)])
            data=io.BytesIO();asset.save(data,format='PNG');url='/api/sprite/tab'+str(index)+'.png'
            self.assets[url]=data.getvalue();self.frame.paste(asset,(x+10,40),asset)
            folders.append({'id':str(index),'name':'Folder '+str(index),'tab':{'order':x,'sprite':{'url':url}}})
        self.catalog={'folders':folders}
        self.matcher=ResearchSprites(lambda path:self.catalog,lambda path:self.assets[path])
        self.proposals=[{'key':'r'+str(i),'panel':'research','folder':str(i%2),'status':'ready'} for i in range(3)]

    def test_two_folders_represent_three_research_choices_and_direct_nodes_take_over(self):
        result=self.matcher.navigation(self.frame,self.proposals,[])
        self.assertEqual([m['identity'] for m in result],['0','1'])
        self.assertEqual([m['count'] for m in result],[2,1])
        self.assertEqual(result[0]['rect'],[40,46,80,62])
        self.assertEqual(result[0]['step_keys'],['r0','r2'])
        self.assertEqual(result[0]['basis'],'tab_sprite_and_two_gui_peers')
        nodes=[{'panel':'research','folder':'0'}]
        self.assertEqual([m['identity'] for m in self.matcher.navigation(self.frame,self.proposals,nodes)],['1'])

    def test_absent_requested_icons_do_not_scan_unneeded_peer_templates(self):
        import cv2
        from unittest.mock import patch
        with patch.object(cv2,'matchTemplate',wraps=cv2.matchTemplate) as match:
            self.assertEqual(self.matcher.navigation(Image.new('RGB',self.frame.size),self.proposals,[]),[])
        # Two requested icons over four scales; peers cannot establish a
        # missing target, so they must not add full-region template scans.
        self.assertLessEqual(match.call_count,8)

    def test_two_independent_peers_are_sufficient_without_scanning_unused_tabs(self):
        import cv2
        from unittest.mock import patch
        with patch.object(cv2,'matchTemplate',wraps=cv2.matchTemplate) as match:
            result=self.matcher.navigation(self.frame,[self.proposals[0]],[])
        self.assertEqual(result[0]['evidence_rects'],[[115,46,155,62],[190,46,230,62]])
        self.assertEqual(match.call_count,3)

    def test_no_guidance_no_context_occlusion_duplicate_rows_and_external_urls(self):
        self.assertEqual(self.matcher.navigation(self.frame,[],[]),[])
        self.assertEqual(self.matcher.navigation(Image.new('RGB',self.frame.size),self.proposals,[]),[])
        changed=self.frame.copy();changed.paste((0,0,0),(105,40,315,68))
        self.assertEqual(self.matcher.navigation(changed,self.proposals,[]),[])
        duplicate=Image.new('RGB',(1200,400));duplicate.paste(self.frame,(0,0));duplicate.paste(self.frame,(600,0))
        self.assertEqual(self.matcher.navigation(duplicate,self.proposals,[]),[])
        self.catalog['folders'][0]['tab']['sprite']['url']='https://example.invalid/icon.png'
        self.matcher.tabs=None
        self.assertNotIn('0',[m['identity'] for m in self.matcher.navigation(self.frame,self.proposals,[])])

    def test_repeated_sprite_cannot_name_a_tab(self):
        self.catalog['folders'].append(self.catalog['folders'][0]|{'id':'duplicate'})
        self.assertNotIn('0',[m['identity'] for m in self.matcher.navigation(self.frame,self.proposals,[])])

    def test_inactive_atlas_frame_is_used_instead_of_the_active_frame(self):
        for folder in self.catalog['folders']:
            tab=folder['tab'];tab['inactive_sprite']=tab['sprite']
            tab['sprite']={'url':'/api/sprite/active.png'}
        data=io.BytesIO();Image.new('RGBA',(60,28),'red').save(data,format='PNG')
        self.assets['/api/sprite/active.png']=data.getvalue()
        self.assertEqual(len(self.matcher.navigation(self.frame,self.proposals,[])),2)


if __name__=='__main__':unittest.main()
