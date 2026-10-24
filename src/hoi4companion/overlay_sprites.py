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
        self.folders={};self.images={};self.captions={};self.tabs=None

    def load_tabs(self):
        if self.tabs is not None:return self.tabs
        tabs=[]
        for folder in self.get_json('/api/technologies').get('folders',[]):
            tab=folder.get('tab') or {};sprite=tab.get('inactive_sprite') or tab.get('sprite') or {};url=sprite.get('url','')
            order=tab.get('order')
            if not url.startswith('/api/sprite/') or type(order) not in (int,float):continue
            if url not in self.images:
                self.images[url]=Image.open(io.BytesIO(self.get_bytes(url))).convert('RGBA')
            asset=self.images[url]
            if not 12<=min(asset.size)<=256:continue
            # The sheet clips/overlaps the tab border. Match its inner glyph,
            # using the actual inactive atlas frame rather than changing colours.
            crop=(round(asset.width*.16),round(asset.height*.2),round(asset.width*.84),round(asset.height*.77))
            tabs.append({'id':folder['id'],'name':folder['name'],'order':order,'image':asset.crop(crop),'offset':crop[:2],
                         'digest':hashlib.sha256(asset.tobytes()).digest()})
        self.tabs=tabs
        return tabs

    def navigation(self,image,proposals,node_matches):
        """Recognize each requested tab and two peers in its declared GUI row.

        A folder ID alone never supplies a screen rectangle. The tab itself and
        its peers must be visible; occlusion or duplicate layouts fail closed.
        Several recommendations in one folder produce one navigation marker.
        """
        requested={}
        visible={m.get('folder') for m in node_matches if m['panel']=='research'}
        for proposal in proposals:
            folder=proposal.get('folder')
            if proposal['panel']=='research' and folder and folder not in visible:
                requested.setdefault(folder,[]).append(proposal)
        if not requested:return []
        try:
            import cv2
            import numpy as np
        except ImportError:return []
        tabs=self.load_tabs();frame=np.asarray(image.convert('RGB'))
        # Native tabs are above the tree, in the upper part of the client area.
        # Bound the search, but infer translation/scale only from visible pixels.
        height,width=frame.shape[:2];region=frame[:max(1,round(height*.3))]
        gray=cv2.cvtColor(region,cv2.COLOR_RGB2GRAY);candidates={}
        for scale in (1.0,.8,1.25,1.5):
            discovered={}
            def hits_for(tab):
                key=id(tab)
                if key in discovered:return discovered[key]
                hits=[];discovered[key]=hits
                if any(t['id']!=tab['id'] and t['digest']==tab['digest'] for t in tabs):return hits
                size=tuple(max(1,round(v*scale)) for v in tab['image'].size)
                data=np.asarray(tab['image'].resize(size,Image.Resampling.BICUBIC))
                th,tw=data.shape[:2];mask=(data[:,:,3]>245).astype('uint8')*255
                if th>=region.shape[0] or tw>=width or np.count_nonzero(mask)<80:return hits
                template=data[:,:,:3]
                scores=cv2.matchTemplate(gray,cv2.cvtColor(template,cv2.COLOR_RGB2GRAY),cv2.TM_SQDIFF_NORMED,mask=mask)
                scores[~np.isfinite(scores)]=1
                for _ in range(3):
                    score,_,(x,y),_=cv2.minMaxLoc(scores)
                    if score>.006:break
                    delta=region[y:y+th,x:x+tw].astype('float32')-template
                    if float(np.mean(delta[mask>0]**2))<160:
                        hits.append({**tab,'rect':[x,y,x+tw,y+th],
                                     'dx':x-(tab['order']+tab['offset'][0])*scale,
                                     'dy':y-tab['offset'][1]*scale})
                    scores[max(0,y-th//2):y+th//2+1,max(0,x-tw//2):x+tw//2+1]=1
                return hits
            # Discover every requested target candidate, preserving duplicate
            # bar rejection. Other tabs only supply the two required peers;
            # defer their full-region scans until a target can use them.
            hits=[hit for tab in tabs if tab['id'] in requested for hit in hits_for(tab)]
            for hit in hits:
                peers={}
                for tab in tabs:
                    if tab['digest']==hit['digest']:continue
                    for other in hits_for(tab):
                        if abs(other['dx']-hit['dx'])<=2 and abs(other['dy']-hit['dy'])<=2:
                            peers.setdefault(other['digest'],other['rect'])
                    if len(peers)>=2:break
                if len(peers)<2:continue
                candidates.setdefault(hit['id'],[]).append({
                    'key':'research-tab:'+hit['id'],'panel':'research_tab','identity':hit['id'],
                    'folder':hit['id'],'title':hit['name'],'status':'ready','rect':hit['rect'],
                    'step_keys':[p['key'] for p in requested[hit['id']]],'count':len(requested[hit['id']]),
                    'basis':'tab_sprite_and_two_gui_peers','evidence_rects':list(peers.values())[:2]})
            # One GUI scale per sheet; retain ambiguity checks within that scale.
            if candidates:break
        return [matches[0] for matches in candidates.values() if len(matches)==1]

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
            if not position or not url.startswith('/api/sprite/') or not icon:continue
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
        frame=np.asarray(image.convert('RGB'))
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
                # Discovery uses the same absolute RGB bound as admission:
                # normalized grayscale scores disproportionately reject dark icons.
                limit=160*np.count_nonzero(mask)*3
                scores=cv2.matchTemplate(frame,template,cv2.TM_SQDIFF,mask=mask)
                scores[~np.isfinite(scores)]=float('inf')
                locations=[]
                for _ in range(3):
                    score,_,location,_=cv2.minMaxLoc(scores)
                    if score>=limit:break
                    x,y=location;locations.append((x,y,score/(np.count_nonzero(mask)*3)))
                    scores[max(0,y-th//2):y+th//2+1,max(0,x-tw//2):x+tw//2+1]=float('inf')
                for x,y,score in locations:
                    opaque=mask>0
                    target_delta=frame[y:y+th,x:x+tw].astype('float32')-template
                    if float(np.mean(target_delta[opaque]**2))>=160:continue
                    pos=target['position'];icon=target['icon']
                    dx=x-(pos['x']+icon['x']-(target['image'].width/2 if icon.get('centered') else 0))*scale
                    dy=y-(pos['y']+icon['y']-(target['image'].height/2 if icon.get('centered') else 0))*scale
                    landmarks=set();evidence=[]
                    for other in nodes:
                        if other['digest']==target['digest'] or other['digest'] in landmarks:continue
                        op=other['position'];oi=other['icon'];sample,valid=pixels(other);oh,ow=sample.shape[:2]
                        ox=round(dx+(op['x']+oi['x']-(other['image'].width/2 if oi.get('centered') else 0))*scale)
                        oy=round(dy+(op['y']+oi['y']-(other['image'].height/2 if oi.get('centered') else 0))*scale)
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
