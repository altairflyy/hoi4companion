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


if __name__=='__main__':unittest.main()
