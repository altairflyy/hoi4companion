'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const tick=()=>new Promise(setImmediate);
function harness(search='?guide=example&step=r'){
 class Node{constructor(tag=''){this.tag=tag;this.children=[];this.value='';this.text='';this.hidden=false;}set textContent(v){this.text=String(v);this.children=[];}get textContent(){return this.text+this.children.map(n=>n.textContent||'').join('');}get options(){return this.children;}append(...v){this.children.push(...v);}replaceChildren(...v){this.text='';this.children=v;}}
 const html=fs.readFileSync('src/hoi4companion/web/associations.html','utf8'),nodes=new Map([...html.matchAll(/id="([^"]+)"/g)].map(m=>[m[1],new Node()])),get=id=>{assert.ok(nodes.has(id),id);return nodes.get(id);};
 const source={schema_version:1,id:'example',title:'Example',applies_to:{country:'TST',tree_id:'tree'},tracks:{research:[{key:'r',name:'Old technology',action:{kind:'research',entity:{kind:'technology',id:'old'}}}]}};
 let data={guide:structuredClone(source),changes:[],revision:0},reject=false;const requests=[];
 const catalog=[{kind:'country',id:'YUG',name:'Yugoslavia',localized_name:'Jugoslavia'},{kind:'technology',id:'old',name:'Old technology'},{kind:'technology',id:'new',name:'New technology',localized_name:'Nuova tecnologia'},{kind:'focus',id:'different-kind',name:'New technology',scope:'tree'}];
 let lines={schema_version:1,guide_id:'example',steps:[{key:'raw',track:'operations',text:'Def against YUG',category:'checkpoints',verification:'uninterpreted',interpretation:null}],stale:['gone'],categories:['construction','army','checkpoints']};
 const fetch=async(path,options)=>{const body=options?.body?JSON.parse(options.body):null;requests.push({path,body});
  if(path.startsWith('/api/guide/interpretations')){if(body){const t=body.interpretation?.target;lines=structuredClone(lines);lines.steps[0].interpretation=body.interpretation&&{category:body.interpretation.category,target:t?{...t,name:'Jugoslavia'}:null};lines.steps[0].verification=body.interpretation?'manual':'uninterpreted';}return {ok:true,json:async()=>lines};}
  if(body){if(reject)return {ok:false,json:async()=>({error:'Percorso incompatibile'})};data={guide:structuredClone(source),changes:body.entity?[{key:'r',original:source.tracks.research[0].action.entity,entity:body.entity}]:[],revision:data.revision+1};data.guide.tracks.research[0].action.entity=body.entity||source.tracks.research[0].action.entity;}
  return {ok:true,json:async()=>path==='/api/guides'?{guides:[source]}:path==='/api/import/catalog'?{entities:catalog}:data};};
 runLocalized(fs.readFileSync('src/hoi4companion/web/associations.js','utf8'),{document:{getElementById:get,createElement:tag=>new Node(tag)},fetch,location:{search},URLSearchParams});
 return {get,requests,setReject:()=>reject=true};
}
test('association editor searches compatible types and saves/restores only explicit corrections',async()=>{
 const h=harness();await tick();assert.equal(h.get('associationStep').value,'r');assert.match(h.get('currentAssociation').textContent,/old/);
 assert.equal(h.get('saveAssociation').disabled,true);assert.match(h.get('returnGuide').href,/guide=example/);
 h.get('associationSearch').value='Nuova';h.get('associationSearch').oninput();assert.equal(h.get('associationMatches').children.length,1);
 h.get('associationMatches').value=h.get('associationMatches').children[0].value;h.get('associationMatches').onchange();assert.equal(h.get('saveAssociation').disabled,false);
 await h.get('saveAssociation').onclick();assert.deepEqual(h.requests.find(r=>r.body).body,{guide_id:'example',key:'r',expected:{kind:'technology',id:'old'},entity:{kind:'technology',id:'new'}});
 assert.equal(h.get('restoreAssociation').disabled,false);assert.match(h.get('associationNotice').textContent,/Correzione salvata/);
 await h.get('restoreAssociation').onclick();assert.equal(h.requests.filter(r=>r.body).at(-1).body.entity,null);assert.equal(h.get('restoreAssociation').disabled,true);
});
test('failed correction is shown without claiming success or losing current identity',async()=>{
 const h=harness();await tick();h.get('associationSearch').value='Nuova';h.get('associationSearch').oninput();h.get('associationMatches').value=h.get('associationMatches').children[0].value;h.get('associationMatches').onchange();h.setReject();
 await h.get('saveAssociation').onclick();assert.equal(h.get('associationError').hidden,false);assert.match(h.get('associationError').textContent,/incompatibile/);assert.doesNotMatch(h.get('associationNotice').textContent,/salvata/);
});
test('a step opened from its card returns to that card and says nothing changes until saved (UX R12)',async()=>{
 const h=harness('?guide=example&step=r&from=card');await tick();assert.equal(h.get('associationStep').value,'r');
 assert.equal(h.get('returnGuide').href,'/walkthrough?guide=example#operation-r');assert.match(h.get('associationNotice').textContent,/Aperta dalla card del passo/);
 assert.equal(h.requests.some(r=>r.body),false);
});
test('a requested step without a correctable binding is named, never swapped for another step',async()=>{
 const h=harness('?guide=example&step=raw');await tick();assert.equal(h.get('associationStep').value,'');
 assert.match(h.get('currentAssociation').textContent,/non ha un focus, una tecnologia o un progetto/);assert.equal(h.get('saveAssociation').disabled,true);
 assert.equal(h.get('associationMatches').children.length,0);
});

test('a line followed by hand gets the player reading: category and an optional catalog entity, removable (UX R13)',async()=>{
 const h=harness('?guide=example&step=raw&from=card');await tick();
 assert.equal(h.get('associationStep').value,'',"the binding editor is not swapped to another step");
 assert.match(h.get('currentAssociation').textContent,/Cosa significa una riga/);
 assert.equal(h.get('interpretStep').value,'raw');assert.match(h.get('interpretLine').textContent,/Def against YUG/);
 assert.match(h.get('interpretNotice').textContent,/1 letture precedenti non corrispondono/);
 assert.equal(h.get('removeInterpretation').disabled,true);
 h.get('interpretCategory').value='army';h.get('interpretCategory').onchange();
 h.get('interpretTargetSearch').value='jugo';h.get('interpretTargetSearch').oninput();
 const options=h.get('interpretTargets').children;assert.equal(options.length,2,'no entity, then the one match');assert.equal(options[0].value,'');
 h.get('interpretTargets').value=options[1].value;h.get('interpretTargets').onchange();assert.equal(h.get('saveInterpretation').disabled,false);
 await h.get('saveInterpretation').onclick();
 assert.deepEqual(h.requests.find(r=>r.body).body,{guide_id:'example',key:'raw',expected:'Def against YUG',interpretation:{category:'army',target:{kind:'country',id:'YUG'}}},'the name is never sent: Rust takes it from the catalog');
 assert.match(h.get('interpretNotice').textContent,/mai quando conta come fatta/);
 assert.equal(h.get('interpretTargets').value,JSON.stringify({kind:'country',id:'YUG'}));assert.equal(h.get('saveInterpretation').disabled,true,'nothing new to save');
 await h.get('removeInterpretation').onclick();assert.equal(h.requests.filter(r=>r.body).at(-1).body.interpretation,null);
 assert.equal(h.get('removeInterpretation').disabled,true);assert.equal(h.requests.filter(r=>r.body&&r.path==='/api/guide/associations').length,0,'no binding is changed');
});

function runLocalized(code,context,options){const i18n=require('../src/hoi4companion/web/i18n.js');i18n.setLanguage('italian');context.HoiI18n=i18n;return vm.runInNewContext(code,context,options);}
