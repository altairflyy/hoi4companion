"""User-supplied design references. Never installs anything into the game."""
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import unicodedata

from .jomini_parser import parse_bytes
from .clausewitz import Node
from .word_guides import _archive, _read, _safe_entry


def name_key(value):
    # Plus denotes a distinct model. Hyphens/spaces are editorial separators.
    return ''.join(c for c in unicodedata.normalize('NFKD',value).casefold() if c.isalnum() or c=='+')


def base_name(value):
    return value.split('(',1)[0].strip()


def _plain(block):
    return {k:v for k,v in block.entries if k and isinstance(v,str)}


def image_metadata(path):
    """Classify declared archive folders, never guess a model's military role."""
    patterns=(('templates',r'(?:division\s+)?templates?'),('armor',r'(?:armor|armour|tank)(?:\s+designs?)?'),
              ('air',r'(?:air|aircraft|aviation)(?:\s+designs?)?'),('navy',r'(?:navy|naval|ship)(?:\s+designs?)?'),
              ('multiplayer',r'multiplayer'))
    parts=path.parent.parts;category='other';start=0
    for i in range(len(parts)-1,-1,-1):
        label=re.sub(r'\s*\([^)]*\)\s*',' ',parts[i]).strip()
        match=next((key for key,pattern in patterns if re.fullmatch(pattern,label,re.I)),None)
        if match:category=match;start=i;break
    tail=' '.join(parts[start:])
    return {'category':category,'tags':[tag for tag in ('situational','outdated') if re.search(r'\b'+tag+r'\b',tail,re.I)]}


class ReferenceLibrary:
    def __init__(self,root):
        self.root=Path(root)

    def manifest(self):
        try:
            data=json.loads((self.root/'manifest.json').read_text(encoding='utf8'))
            if data.get('schema_version')==1 and isinstance(data.get('designs'),list) and isinstance(data.get('images'),list):return data
        except (OSError,ValueError,AttributeError):pass
        return {'schema_version':1,'designs':[],'images':[]}

    def import_archive(self,stream):
        from PIL import Image
        designs=[];images=[];blobs={};budget=180_000_000
        with _archive(stream) as archive:
            names=[i.filename for i in archive.infolist() if not i.is_dir()]
            if len(names)!=len(set(names)):raise ValueError('Duplicate reference archive entries')
            for info in archive.infolist():
                if info.is_dir() or not _safe_entry(info.filename):continue
                path=PurePosixPath(info.filename);suffix=path.suffix.lower()
                if suffix not in ('.txt','.png','.jpg','.jpeg'):continue
                budget-=info.file_size
                if budget<0:raise ValueError('Reference archive exceeds its decoded size limit')
                raw=_read(archive,info,16_000_000)
                if suffix=='.txt':
                    root=parse_bytes(raw)
                    for _,group in root.entries:
                        if not isinstance(group,Node):continue
                        for equipment in group.blocks('equipment'):
                            title=equipment.scalar('name');variants=equipment.blocks('target_variant')
                            if not title or len(title)>200 or len(variants)!=1:continue
                            variant=variants[0];identifier=variant.scalar('type')
                            if not re.fullmatch(r'[A-Za-z0-9_]+',identifier):continue
                            row={'name':title,'equipment_id':identifier,'category':group.scalar('category'),
                                 'modules':_plain(next(iter(variant.blocks('modules')),Node([]))),
                                 'upgrades':_plain(next(iter(variant.blocks('upgrades')),Node([])))}
                            digest=hashlib.sha256(json.dumps(row,sort_keys=True).encode()).hexdigest()
                            designs.append({'id':digest,**row})
                else:
                    with Image.open(io.BytesIO(raw)) as picture:
                        if picture.format not in ('PNG','JPEG') or picture.width*picture.height>40_000_000:raise ValueError('Invalid reference image')
                        picture.verify()
                    digest=hashlib.sha256(raw).hexdigest();blobs[digest]=raw
                    images.append({'id':digest,'name':path.stem,'section':path.parent.name,
                                   **image_metadata(path),
                                   'mime':'image/png' if suffix=='.png' else 'image/jpeg'})
        designs=list({r['id']:r for r in designs}.values())
        images=list({(r['id'],r['name'],r['category'],r['section'],tuple(r['tags'])):r for r in images}.values())
        if not designs and not images:raise ValueError('No equipment designs or reference images in this archive')
        self.root.mkdir(parents=True,exist_ok=True)
        for digest,raw in blobs.items():(self.root/digest).write_bytes(raw)
        data={'schema_version':1,'designs':designs,'images':images}
        temporary=self.root/'manifest.tmp';temporary.write_text(json.dumps(data,ensure_ascii=False,indent=2),encoding='utf8')
        temporary.replace(self.root/'manifest.json')
        return data

    def image(self,identifier):
        if not isinstance(identifier,str) or not re.fullmatch('[0-9a-f]{64}',identifier):raise ValueError('Invalid image reference')
        item=next((r for r in self.manifest()['images'] if r['id']==identifier),None)
        if item is None:raise ValueError('Reference image not found')
        return (self.root/identifier).read_bytes(),item['mime']


def design_matches(query,designs,upgrades=None):
    needle=name_key(query)
    exact=[r for r in designs if name_key(r['name'])==needle]
    rows=exact or [r for r in designs if name_key(base_name(r['name']))==needle]
    if not rows:
        # An omitted tier number is recoverable only with the complete explicit
        # design qualifier. Multiple compatible library designs stay ambiguous.
        qualified=re.fullmatch(r'(.+?)\s*(\([^()]+\))',query.strip())
        if qualified and not re.search(r'\d$',name_key(qualified[1])):
            for row in designs:
                candidate=re.fullmatch(r'(.+?)\s*(\([^()]+\))',row['name'].strip())
                if (candidate and name_key(candidate[2])==name_key(qualified[2])
                        and re.sub(r'\d+$','',name_key(candidate[1]))==name_key(qualified[1])):rows.append(row)
    if upgrades:
        rows=[r for r in rows if all(r.get('upgrades',{}).get(k)==str(v) for k,v in upgrades.items())]
    return rows


def bindings(guide,catalog):
    """Only declared names select library variants; no fuzzy model inference."""
    library=getattr(catalog,'reference_designs',{})
    designs=library.get('designs',[]);images=library.get('images',[])
    result={}
    if not designs:return result
    text='\n'.join(s for rows in guide['tracks'].values() for row in rows
                   for s in row['action'].get('params',{}).get('instructions',[]))
    # A design command can name several models separated by commas. Upgrade
    # tuples select engine/armor levels; other qualifiers are never guessed.
    for line in text.splitlines():
        hit=re.search(r'\bdesign(?:\s+and\s+produce)?\s+(.+)',line,re.I)
        if not hit:continue
        for part in hit[1].split(','):
            match=re.fullmatch(r'\s*([^():]+?)\s*(?:\(\s*(\d+)\s*/\s*(\d+)\s*\))?\s*',part)
            if not match:continue
            name=match[1].strip();upgrades=None
            if match[2] is not None:upgrades={'tank_nsb_engine_upgrade':match[2],'tank_nsb_armor_upgrade':match[3]}
            rows=design_matches(name,designs,upgrades)
            # Expand glossary-backed family names, e.g. Flame-I1 -> FL-I1.
            if not rows:
                from .research_upgrades import guide_language
                for short,label in guide_language()['abbreviations'].get('designs',{}).items():
                    long=label.removesuffix(' tank')
                    if name.lower().startswith(long.lower()):rows+=design_matches(short+name[len(long):],designs,upgrades)
            compatible=[r for r in rows if catalog.matches(r['equipment_id'],('equipment',),explicit=True)]
            if len(compatible)!=1:continue
            row=compatible[0];reference={'id':row['id'],'name':row['name'],'modules':row['modules'],'upgrades':row['upgrades']}
            # Reference images are associated only by complete normalized name.
            keys={name_key(base_name(row['name'])),name_key(row['name'])}
            qualifier=re.fullmatch(r'(.+?)\s*\(\d+/\d+/([A-Za-z]+)\)',row['name'])
            if qualifier:keys.add(name_key(qualifier[1]+qualifier[2]))
            candidates=[r for r in images if name_key(r['name']) in keys]
            if len(candidates)==1:reference['image_id']=candidates[0]['id']
            item={'equipment_ids':[row['equipment_id']],
                'variant_names':[name,row['name']],'category':'naval' if row['category']=='naval' else 'military',
                'reference':reference}
            values=result.setdefault(name_key(name),[])
            if item not in values:values.append(item)
    return result


def selector(query,catalog,declared=None):
    rows=(declared or {}).get(name_key(query),[])
    if rows:return rows
    library=getattr(catalog,'reference_designs',{})
    matches=design_matches(query,library.get('designs',[]))
    matches=[r for r in matches if catalog.matches(r['equipment_id'],('equipment',),explicit=True)]
    return [{'equipment_ids':[row['equipment_id']],'variant_names':[query,row['name']],
             'category':'naval' if row['category']=='naval' else 'military',
             'reference':{'id':row['id'],'name':row['name'],'modules':row['modules'],'upgrades':row['upgrades']}} for row in matches]


def attach_images(guide,catalog):
    """Literal reference labels can illustrate a method without completing it."""
    images=getattr(catalog,'reference_designs',{}).get('images',[]);changed=[]
    for rows in guide['tracks'].values():
        for row in rows:
            params=row['action'].get('params',{})
            text='\n'.join(params.get('instructions',[]))
            found=[{'id':image['id'],'name':image['name']} for image in images
                   if re.search(r'(?<![\w+])'+re.escape(image['name'])+r'(?![\w+])',text)]
            if found and params.get('reference_images')!=found[:8]:
                params['reference_images']=found[:8];changed.append({'key':row['key'],'actions':0,'step_keys':[row['key']],'unverifiable_timing':False})
    return changed
