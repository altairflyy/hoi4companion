"""Optional local sprite recognition, qualified by the installed GUI layout.

No screen coordinates are inferred from a technology ID alone. The target icon
must match uniquely and two different neighbouring sprites must corroborate the
same translation and scale. All assets come from the existing local asset API.
"""
import hashlib
import io
from urllib.parse import quote

from PIL import Image
from .screen_overlay import text_key


class ResearchSprites:
    def __init__(self,get_json,get_bytes):
        self.get_json,self.get_bytes=get_json,get_bytes
        self.folders={};self.images={};self.captions={}

    def load(self,folder):
        if folder in self.folders:return self.folders[folder]
        tree=self.get_json('/api/technology-tree?id='+quote(folder,safe=''))
        self.captions[folder]={text_key(c['text']) for c in tree.get('layout',{}).get('captions',[]) if c.get('kind')=='title'}
        nodes=[]
        for node in tree.get('technologies',[]):
            position=next((p.get('layout') for p in node.get('folders',[]) if p['id']==folder),None)
            sprite=node.get('sprite') or {};visual=node.get('visual') or {};icon=visual.get('icon') or {}
            url=sprite.get('url','')
            # Never follow an arbitrary URL supplied by a guide or remote site.
            if not position or not url.startswith('/api/sprite/') or not icon.get('centered'):continue
            if url not in self.images:
                image=Image.open(io.BytesIO(self.get_bytes(url))).convert('RGBA')
                self.images[url]=image
            image=self.images[url]
            if not 12<=min(image.size)<=256:continue
            nodes.append({'id':node['id'],'position':position,'icon':icon,'image':image,
                          'digest':hashlib.sha256(image.tobytes()).digest()})
        self.folders[folder]=nodes
        return nodes

    def match(self,image,proposals,ocr):
        try:
            import cv2
            import numpy as np
        except ImportError:return []
        frame=np.asarray(image.convert('RGB'));gray=cv2.cvtColor(frame,cv2.COLOR_RGB2GRAY)
        height,width=frame.shape[:2];found=[]
        for proposal in proposals:
            if proposal['panel']!='research' or not proposal.get('folder'):continue
            nodes=self.load(proposal['folder'])
            keys={text_key(line.get('text','')) for line in ocr.get('lines',[])}
            if not keys & self.captions[proposal['folder']]:continue
            target=next((n for n in nodes if n['id']==proposal['identity']),None)
            if not target:continue
            if any(n['id']!=target['id'] and n['digest']==target['digest'] for n in nodes):continue
            candidates=[]
            for scale in (1.0,.8,1.25,1.5):
                def pixels(node):
                    source=node['image'];size=tuple(max(1,round(v*scale)) for v in source.size)
                    data=np.asarray(source.resize(size,Image.Resampling.BICUBIC))
                    return data[:,:,:3],(data[:,:,3]>245).astype('uint8')*255
                template,mask=pixels(target);th,tw=template.shape[:2]
                if th>=height or tw>=width or np.count_nonzero(mask)<80:continue
                scores=cv2.matchTemplate(gray,cv2.cvtColor(template,cv2.COLOR_RGB2GRAY),cv2.TM_SQDIFF_NORMED,mask=mask)
                scores[~np.isfinite(scores)]=1
                locations=[]
                for _ in range(3):
                    score,_,location,_=cv2.minMaxLoc(scores)
                    if score>.006:break
                    x,y=location;locations.append((x,y,score))
                    scores[max(0,y-th//2):y+th//2+1,max(0,x-tw//2):x+tw//2+1]=1
                for x,y,score in locations:
                    opaque=mask>0
                    target_delta=frame[y:y+th,x:x+tw].astype('float32')-template
                    if float(np.mean(target_delta[opaque]**2))>=160:continue
                    pos=target['position'];icon=target['icon']
                    dx=x-(pos['x']+icon['x']-target['image'].width/2)*scale
                    dy=y-(pos['y']+icon['y']-target['image'].height/2)*scale
                    landmarks=set();evidence=[]
                    for other in nodes:
                        if other['digest']==target['digest'] or other['digest'] in landmarks:continue
                        op=other['position'];oi=other['icon'];sample,valid=pixels(other);oh,ow=sample.shape[:2]
                        ox=round(dx+(op['x']+oi['x']-other['image'].width/2)*scale)
                        oy=round(dy+(op['y']+oi['y']-other['image'].height/2)*scale)
                        if ox<0 or oy<0 or ox+ow>width or oy+oh>height:continue
                        # Only opaque sprite pixels participate; backgrounds and
                        # researched/available colours cannot create a match.
                        delta=frame[oy:oy+oh,ox:ox+ow].astype('float32')-sample
                        selected=delta[valid>0]
                        if len(selected)<80:continue
                        if float(np.mean(selected**2))<160:
                            landmarks.add(other['digest']);evidence.append([ox,oy,ox+ow,oy+oh])
                        if len(landmarks)>=2:break
                    if len(landmarks)<2:continue
                    left=dx+pos['x']*scale;top=dy+pos['y']*scale
                    rect=[left,top,left+pos['width']*scale,top+pos['height']*scale]
                    if min(rect[:2])<0 or rect[2]>width or rect[3]>height:continue
                    candidates.append({**proposal,'rect':rect,'evidence_rects':evidence,
                                       'basis':'sprite_and_two_gui_landmarks','score':score,'landmarks':len(landmarks)})
                # A native technology sheet has one GUI scale. Once qualified,
                # searching other scales adds cost without additional evidence.
                if candidates:break
            # Duplicate icons, different scales or multiple visible copies are
            # ambiguous, even if the guide itself requests only one of them.
            if len(candidates)==1:found.extend(candidates)
        return found
