"""CWTools rule descriptions and the legacy, explicitly partial demo analyzer.

Jomini parses both scripts and lexically quoted CWT atoms. Unsupported schema
constructs and unknown scopes remain unknown, including across mod boundaries.
Installed-game analysis delegates to the full upstream engine through Rust.
The demo analyzer is never an installation fallback.
"""
from collections import Counter, defaultdict
from functools import lru_cache
import hashlib
import json
from pathlib import Path
import re

from .clausewitz import Node, parse
from .jomini_parser import parse_file
from .installation import collect_files
from .tooltips import _REFERENCE, _ENGINE_PARAMETERS

RULE_ROOT = Path(__file__).with_name('rules') / 'cwtools'
DIRECTORIES = ('common/national_focus','common/technologies','common/ideas',
    'common/characters','common/decisions','events','history/states',
    'common/special_projects','common/buildings','common/country_tags',
    'common/scripted_triggers','common/scripted_effects','common/country_tag_aliases')
DIRECTORY_TYPES = dict(zip(DIRECTORIES,(
    ('focus','shared_focus','joint_focus'),('technology',),('idea',),('character','idea'),
    ('decision',),('event','country_event','news_event','state_event','unit_leader_event'),
    ('state',),('special_project',),('building',),('country_tags',),('scripted_trigger',),('scripted_effect',),('country_tags',))))
NAMESPACES = {'focus','shared_focus','joint_focus','technology','idea','character',
    'decision','event','country_event','news_event','state_event','unit_leader_event',
    'state','special_project','building','country_tags','scripted_trigger','scripted_effect'}
_ATOMS = re.compile(r'"(?:\\.|[^"\\])*"|#[^\r\n]*|[{}]|[<>!]?=|<>|[^\s{}=]+')
_ALIAS = re.compile(r'^alias\[(trigger|effect):([^\]]+)\]$')
_DYNAMIC = re.compile(r'[@$\[\]?:]')


def _cwt(source):
    # CWT angle references (<focus>) are schema atoms, not script operators.
    # Only quote lexemes; Jomini still owns all container parsing and ordering.
    return parse(' '.join(token if token.startswith(('"','#')) or token in
        ('{','}','=','<','>','>=','<=','!=','<>','==') else json.dumps(token)
        for token in _ATOMS.findall(source) if not token.startswith('#')))


def _schema(value):
    return [[key,_schema(item)] for key,item in value.entries] if isinstance(value,Node) else value


class RuleBook:
    def __init__(self, root):
        self.provenance=json.loads((root/'provenance.json').read_text(encoding='utf-8'))
        for name,digest in self.provenance['files'].items():
            if hashlib.sha256((root/name).read_bytes()).hexdigest()!=digest:
                raise ValueError('Regole CWTools modificate o incomplete: '+name)
        documentation=json.loads((root/'script_documentation.json').read_text(encoding='utf-8-sig'))
        self.rules={'trigger':defaultdict(list),'effect':defaultdict(list)}
        self.links={};self.enums={};self.load_errors=[]
        for file in sorted(root.rglob('*.cwt')):
            try: node=_cwt(file.read_text(encoding='utf-8-sig'))
            except ValueError as exc:
                self.load_errors.append({'file':str(file.relative_to(root)),'error':str(exc)});continue
            for key,value in node.entries:
                match=_ALIAS.fullmatch(key)
                if match:
                    kind,name=match.groups()
                    if '<' in name:continue  # parameterized aliases need the full CWT engine
                    doc=documentation.get(kind+'s',{}).get(name,{})
                    self.rules[kind][name.lower()].append({'schema':_schema(value),
                        'scopes':[s.lower() for s in doc.get('supported_scope',[])],
                        'description':doc.get('description','')[:2000],
                        'file':str(file.relative_to(root)).replace('\\','/')})
                elif key=='links' and isinstance(value,Node):
                    for name,link in value.entries:
                        if not isinstance(link,Node):continue
                        scopes=link.first('input_scopes')
                        self.links[name.lower()]={'output':link.scalar('output_scope') or None,
                            'inputs':[v.lower() for k,v in scopes.entries if k=='' and isinstance(v,str)]
                                if isinstance(scopes,Node) else [scopes.lower()] if isinstance(scopes,str) else []}
                elif key=='enums' and isinstance(value,Node):
                    for name,items in value.entries:
                        if name.startswith('enum[') and isinstance(items,Node):
                            self.enums[name[5:-1]]={v for k,v in items.entries if k=='' and isinstance(v,str)}
        # Documentation-only rules still provide scope/description; shape unknown.
        for kind in self.rules:
            for name,doc in documentation.get(kind+'s',{}).items():
                if name.lower() not in self.rules[kind]:
                    self.rules[kind][name.lower()].append({'schema':None,
                        'scopes':[s.lower() for s in doc.get('supported_scope',[])],
                        'description':doc.get('description','')[:2000],'file':'script_documentation.json'})

    @classmethod
    @lru_cache(maxsize=1)
    def load(cls):return cls(RULE_ROOT)

    def describe(self,kind,name):
        variants=self.rules.get(kind,{}).get(name.lower(),[])
        return {'schema_version':1,'kind':kind,'name':name,'variants':variants,
            'scopes':sorted({s for row in variants for s in row['scopes']}),
            'revision':self.provenance['revision'],'source':'cwtools_rules','runtime_evaluated':False}


def _entities(path,root):
    """Profiles identify installed definitions; no nation or guide assumptions."""
    if path.startswith('common/national_focus/'):
        for kind,value in root.entries:
            if kind=='focus_tree' and isinstance(value,Node):
                for key,body in value.entries:
                    if key in ('focus','shared_focus','joint_focus') and isinstance(body,Node):
                        yield key,body.scalar('id'),body,'country'
            elif kind in ('shared_focus','joint_focus') and isinstance(value,Node):
                yield kind,value.scalar('id'),value,'country'
    elif path.startswith('events/'):
        for kind,body in root.entries:
            if kind.endswith('_event') and isinstance(body,Node):
                yield kind,body.scalar('id'),body,{'state_event':'state','unit_leader_event':'unit_leader'}.get(kind,'country')
    elif path.startswith('history/states/'):
        for body in root.blocks('state'):yield 'state',body.scalar('id'),body,'state'
    else:
        profiles={'common/technologies/':('technologies','technology','country'),
            'common/characters/':('characters','character','character'),
            'common/buildings/':('buildings','building',None),
            'common/special_projects/':('special_projects','special_project','special_project')}
        for prefix,(group,kind,scope) in profiles.items():
            if path.startswith(prefix):
                for container in root.blocks(group):
                    for key,body in container.entries:
                        if isinstance(body,Node):yield kind,key,body,scope
                return
        if path.startswith(('common/ideas/','common/decisions/')):
            kind='idea' if path.startswith('common/ideas/') else 'decision'
            groups=[v for k,v in root.entries if isinstance(v,Node)]
            if kind=='idea':groups=[v for g in groups for k,v in g.entries if isinstance(v,Node)]
            for group in groups:
                for key,body in group.entries:
                    if isinstance(body,Node) and key not in ('allowed','visible','available','default','ai_will_do'):
                        yield kind,key,body,'country'
        elif path.startswith(('common/country_tags/','common/country_tag_aliases/')):
            for key,value in root.entries:
                if re.fullmatch('[A-Z0-9]{3}',key):yield 'country_tags',key,Node(),None
        elif path.startswith(('common/scripted_triggers/','common/scripted_effects/')):
            kind='scripted_trigger' if path.startswith('common/scripted_triggers/') else 'scripted_effect'
            for key,body in root.entries:
                if isinstance(body,Node):yield kind,key,body,None


class _Analyzer:
    def __init__(self,book,index,labels,reference,incomplete,limit):
        self.book,self.index,self.labels,self.reference=book,index,labels,reference
        self.incomplete,self.limit=incomplete,limit
        self.diagnostics=[];self.counts=Counter();self.coverage=Counter()
        self.unhandled=Counter();self.loc_keys=set();self._loc_log=[]

    def emit(self,code,path,value,message,category,severity='warning',rule=None):
        self.counts['total']+=1;self.counts[severity]+=1;self.counts[category]+=1
        if len(self.diagnostics)<self.limit:
            self.diagnostics.append({'code':code,'file':self.file,'path':path,'value':str(value)[:300],
                'message':message,'category':category,'severity':severity,'source':'static_analysis',
                'rule':rule,'runtime_evaluated':False})

    def unknown(self,key):
        self.coverage['unsupported']+=1;self.unhandled[key]+=1

    def reference_check(self,kind,value,path):
        if _DYNAMIC.search(value):self.unknown('dynamic_reference');return None
        if kind.startswith('event.') and kind[6:] in ('country_event','news_event','state_event','unit_leader_event'):
            kind=kind[6:]
        kinds={'focus':('focus','shared_focus','joint_focus'),'event':('event','country_event','news_event','state_event','unit_leader_event')}.get(kind,(kind,))
        if kind in NAMESPACES and any(value in self.index[k] for k in kinds):return None
        if kind not in NAMESPACES or any(k in self.incomplete for k in kinds):self.unknown('namespace:'+kind);return None
        return None if any(value in self.index[k] for k in kinds) else ('reference_missing',f'ID {kind} «{value}» non trovato nel catalogo installato','references')

    def scalar_check(self,value,schema,path):
        if not isinstance(schema,str):return None
        known=(schema=='bool' or schema in ('scalar','string') or schema.startswith((
            '<','scope[','event_target[','enum[','int','float','variable_field','value_field','localisation')))
        if not known:self.unknown('datatype:'+schema);return None
        if not isinstance(value,str):return ('parameter_type','Era atteso un valore scalare','parameters')
        if _DYNAMIC.search(value):self.unknown('dynamic_value');return None
        if '|' in schema:
            return self.alternatives(value,schema.split('|'),path)
        if schema.startswith('<') and schema.endswith('>'):
            return self.reference_check(schema[1:-1],value,path)
        if schema.startswith(('scope[','event_target[')):
            target=schema[schema.index('[')+1:-1]
            if '.' in value:
                # A chained scope/variable expression is not a literal tag or
                # state ID. Resolving its runtime target needs more evidence.
                self.unknown('scope_expression');return None
            if (value.split('.')[0].upper() in ('ROOT','FROM','THIS','PREV','OWNER','CONTROLLER','CAPITAL')
                    or value.split('.')[0].lower() in self.book.links):
                self.unknown('scope_value');return None
            return self.reference_check('country_tags' if target=='country' else target,value,path)
        if schema.startswith('enum['):
            name=schema[5:-1]
            if name=='country_tags':return self.reference_check(name,value,path)
            if name in self.book.enums:
                return None if value in self.book.enums[name] else ('parameter_type','Valore non incluso in '+name,'parameters')
            self.unknown('enum:'+name);return None
        if schema=='bool':
            return None if value in ('yes','no') else ('parameter_type','Era atteso yes oppure no','parameters')
        if schema.startswith(('int','float','variable_field','value_field')):
            if 'variable' in schema or 'value_field' in schema:
                try:float(value)
                except ValueError:self.unknown('variable');return None
            try:
                import math
                number=float(value)
                if not math.isfinite(number) or schema.startswith('int') and not number.is_integer():raise ValueError()
                bounds=re.search(r'\[([^\]]+)\.\.([^\]]+)\]',schema)
                if bounds and not float(bounds[1])<=number<=float(bounds[2]):
                    return ('parameter_range','Valore fuori dall’intervallo CWT '+schema+'; compatibilità con il gioco da verificare','parameters')
            except ValueError:return ('parameter_type','Numero non valido per '+schema,'parameters')
            return None
        if schema.startswith('localisation'):
            self.localisation(value,path,allow_inline=True);return None
        if schema in ('scalar','string'):return None
        if re.fullmatch(r'[A-Za-z0-9_.-]+',schema) and schema not in ('replace_me','date_field','percentage_field'):
            # Unrecognized CWT datatype is not a literal enum to guess at.
            self.unknown('datatype:'+schema);return None
        self.unknown('datatype:'+schema);return None

    def alternatives(self,value,schemas,path):
        if len(schemas)==1:return self.shape(value,schemas[0],path)
        # Try overloads transactionally: a rejected form must not leak warnings
        # or turn a well-defined alternative into an unknown result.
        base=(self.counts.copy(),self.coverage.copy(),self.unhandled.copy(),len(self.diagnostics),len(self._loc_log))
        outcomes=[]
        for schema in schemas:
            failure=self.shape(value,schema,path)
            outcomes.append((failure,self.counts.copy(),self.coverage.copy(),self.unhandled.copy(),
                self.diagnostics[base[3]:],self._loc_log[base[4]:],
                int(isinstance(value,Node)!=isinstance(schema,list))))
            del self.diagnostics[base[3]:]
            for key in self._loc_log[base[4]:]:self.loc_keys.remove(key)
            del self._loc_log[base[4]:]
            self.counts,self.coverage,self.unhandled=(part.copy() for part in base[:3])
        if not outcomes:return None
        chosen=min(outcomes,key=lambda row:(row[0] is not None,row[6],row[2]['unsupported']))
        failure,self.counts,self.coverage,self.unhandled,diagnostics,keys,_=chosen
        self.diagnostics.extend(diagnostics);self.loc_keys.update(keys);self._loc_log.extend(keys)
        return failure

    def shape(self,value,schema,path):
        if schema is None:self.unknown('schema_missing');return None
        if isinstance(schema,str):return self.scalar_check(value,schema,path)
        if not isinstance(value,Node):return ('parameter_type','Era atteso un blocco di parametri','parameters')
        fields=defaultdict(list)
        for key,item in schema:fields[key].append(item)
        for raw,item in value.entries:
            key=raw.split(' ')[0]
            if key in fields:
                failure=self.alternatives(item,fields[key],path+'/'+key)
                if failure:return (failure[0],key+': '+failure[1],failure[2])
            elif any(key.startswith(('alias_name[','<','subtype[')) or key in ('scalar','scope_field') for key in fields):
                self.unknown('open_schema')
            else:
                # CWT cardinalities/subtypes are not compiled: unknown field is
                # advisory, never proof that a mod/game construct is invalid.
                self.unknown('parameter:'+key)
        return None

    def localisation(self,key,path,allow_inline=False):
        if not key or _DYNAMIC.search(key) or any(c.isspace() for c in key):return
        if key not in self.loc_keys:self.loc_keys.add(key);self._loc_log.append(key)
        if key not in self.labels and key not in self.reference:
            if allow_inline:self.unknown('localisation_or_inline');return
            self.emit('localisation_missing',path,key,'Chiave non trovata nei testi installati','localisation')
        elif key not in self.labels:
            self.emit('localisation_fallback',path,key,'Testo disponibile solo nella lingua di riferimento','localisation')

    def block(self,node,kind,scope,path,root_scope=None,previous=None,depth=0):
        if depth>60:self.unknown('depth_limit');return
        for position,(raw,value) in enumerate(node.entries):
            key=raw.split(' ')[0];lower=key.lower();at=f'{path}/{key}[{position}]'
            if lower in ('and','or','not','hidden_trigger','hidden_effect','else','else_if','if','limit') and isinstance(value,Node):
                self.block(value,'trigger' if lower in ('limit','hidden_trigger') else kind,scope,at,root_scope,previous,depth+1);continue
            if isinstance(value,Node) and (key.upper() in ('ROOT','THIS','PREV','FROM') or lower in self.book.links or
                    key in self.index['country_tags'] or key in self.index['state']):
                new_scope=scope
                if key.upper() in ('ROOT','THIS','PREV','FROM'):
                    new_scope={'ROOT':root_scope,'THIS':scope,'PREV':previous,'FROM':None}[key.upper()]
                elif key in self.index['country_tags']:new_scope='country'
                elif key in self.index['state']:new_scope='state'
                else:
                    link=self.book.links[lower]
                    if scope and link['inputs'] and scope not in link['inputs'] and 'any' not in link['inputs']:
                        self.emit('scope_mismatch',at,key,'Transizione non ammessa dal contesto '+scope,'scopes',rule='links.cwt')
                    new_scope=link['output']
                self.block(value,kind,new_scope,at,root_scope,scope,depth+1);continue
            variants=self.book.rules[kind].get(lower,[])
            if lower in self.index['scripted_'+kind]:
                self.unknown('scripted_call:'+lower);continue
            if not variants:self.unknown(kind+':'+key);continue
            self.coverage['recognized']+=1
            scopes={s for row in variants for s in row['scopes']}
            if not scopes:self.unknown('scope_metadata_missing')
            if scope is None:self.coverage['unknown_scope']+=1
            elif scopes and 'any' not in scopes and scope not in scopes and all(row['scopes'] for row in variants):
                self.emit('scope_mismatch',at,key,'Contesto '+scope+'; atteso '+', '.join(sorted(scopes)),'scopes',rule=variants[0]['file'])
            failure=self.alternatives(value,[row['schema'] for row in variants],at)
            if failure:
                code,message,category=failure
                self.emit(code,at,value if isinstance(value,str) else key,message,category,rule=variants[0]['file'])
            if isinstance(value,Node):
                # Alias containers carry nested triggers/effects, not parameters.
                alias_entries=[(k,v) for row in variants if isinstance(row['schema'],list) for k,v in row['schema'] if k.startswith('alias_name[')]
                if alias_entries:
                    nested='trigger' if any('trigger' in k for k,v in alias_entries) else kind
                    self.block(value,nested,None,at,root_scope,scope,depth+1)
                elif any(row['schema']=='scope_field' for row in variants):
                    self.block(value,kind,None,at,root_scope,scope,depth+1)

    def entity(self,kind,identifier,node,scope):
        path=kind+':'+identifier
        if kind in ('focus','shared_focus','joint_focus','technology','idea','decision','special_project'):
            name=node.scalar('text') or identifier if kind in ('focus','shared_focus','joint_focus') else identifier
            self.localisation(name,path+'/name')
            if name+'_desc' in self.labels or name+'_desc' in self.reference:self.localisation(name+'_desc',path+'/description')
        for key,value in node.entries:
            if key in ('prerequisite','mutually_exclusive') and isinstance(value,Node):
                for target in value.all('focus'):
                    if not isinstance(target,str):continue
                    failure=self.reference_check('focus',target,path+'/'+key)
                    if failure:self.emit(failure[0],path+'/'+key,target,failure[1],failure[2])
            elif key in ('title','desc','name') and isinstance(value,str):self.localisation(value,path+'/'+key,allow_inline=True)
            elif isinstance(value,Node) and key in ('available','allowed','visible','bypass','allow_branch','trigger','cancel','allowed_to_remove','allowed_civil_war'):
                self.block(value,'trigger',scope,path+'/'+key,scope)
            elif isinstance(value,Node) and key in ('completion_reward','select_effect','immediate','complete_effect','remove_effect','cancel_effect','on_research_complete'):
                self.block(value,'effect',scope,path+'/'+key,scope)
            elif key=='option' and isinstance(value,Node):
                for field,item in value.entries:
                    if field=='name' and isinstance(item,str):self.localisation(item,path+'/option/name',allow_inline=True)
                    elif field in ('trigger','available','visible') and isinstance(item,Node):self.block(item,'trigger',scope,path+'/option/'+field,scope)
                    elif field not in ('name','ai_chance','highlight','default','original_tag'):
                        self.block(Node([(field,item)]),'effect',scope,path+'/option',scope)
        if kind.startswith('scripted_'):self.block(node,kind[9:],None,path)

    def localisation_graph(self):
        # Only referenced text and its reachable expansions, with explicit engine
        # placeholders excluded. Iterative DFS avoids Python recursion limits.
        for language,labels in (('display',self.labels),('reference',self.reference)):
            done=set();active=set()
            for start in sorted(self.loc_keys):
                stack=[(start,False)]
                while stack:
                    key,leaving=stack.pop()
                    if leaving:active.discard(key);done.add(key);continue
                    if key in done or key not in labels:continue
                    if key in active:
                        self.emit('localisation_cycle',language,key,'Riferimento ciclico nei testi','localisation');continue
                    active.add(key);stack.append((key,True))
                    for match in reversed(list(_REFERENCE.finditer(labels[key]))):
                        target=match[1]
                        if target in _ENGINE_PARAMETERS:continue
                        if target not in labels:
                            self.emit('localisation_reference_missing',language+'/'+key,target,'Riferimento testuale non risolto','localisation')
                        else:stack.append((target,False))


def analyze_documents(documents,labels=None,reference_labels=None,*,book=None,incomplete=(),limit=2000):
    if type(limit) is not int or not 1<=limit<=10000:raise ValueError('Limite diagnosi non valido')
    book=book or RuleBook.load();index=defaultdict(set);entities=[]
    for file,root in documents:
        path=file.replace('\\','/')
        for kind,identifier,node,scope in _entities(path,root):
            if identifier:index[kind].add(identifier);entities.append((file,kind,identifier,node,scope))
            if kind=='character':
                for advisor in node.blocks('advisor'):
                    if advisor.scalar('idea_token'):index['idea'].add(advisor.scalar('idea_token'))
    engine=_Analyzer(book,index,labels or {},reference_labels or {},set(incomplete),limit)
    for file,kind,identifier,node,scope in entities:
        engine.file=file;engine.entity(kind,identifier,node,scope)
    engine.file='localisation';engine.localisation_graph()
    return {'schema_version':1,'source':'static_analysis','status':'partial','runtime_evaluated':False,
        'rules':{'repository':book.provenance['repository'],'revision':book.provenance['revision'],
            'adapter':'cwtools-subset-v2','game_version_compatibility':'not_certified',
            'trigger_names':len(book.rules['trigger']),'effect_names':len(book.rules['effect']),
            'load_errors':book.load_errors},
        'coverage':{'recognized':engine.coverage['recognized'],'unsupported':engine.coverage['unsupported'],
            'unknown_scope':engine.coverage['unknown_scope'],'unhandled':dict(engine.unhandled.most_common(100)),
            'documents':len(documents),'entities':len(entities),'incomplete_namespaces':sorted(incomplete)},
        'summary':dict(engine.counts),'diagnostics':engine.diagnostics,
        'truncated':engine.counts['total']>len(engine.diagnostics)}


def analyze_installation(game,mods,labels,reference_labels):
    from .cwtools_engine import analyze_installation as engine_analysis
    return engine_analysis(game,mods)
