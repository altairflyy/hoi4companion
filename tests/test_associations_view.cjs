'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const tick=()=>new Promise(setImmediate);
function harness(){
 class Node{constructor(tag=''){this.tag=tag;this.children=[];this.value='';this.text='';this.hidden=false;}set textContent(v){this.text=String(v);this.children=[];}get textContent(){return this.text+this.children.map(n=>n.textContent||'').join('');}get options(){return this.children;}append(...v){this.children.push(...v);}replaceChildren(...v){this.text='';this.children=v;}}
 const html=fs.readFileSync('src/hoi4companion/web/associations.html','utf8'),nodes=new Map([...html.matchAll(/id="([^"]+)"/g)].map(m=>[m[1],new Node()])),get=id=>{assert.ok(nodes.has(id),id);return nodes.get(id);};
 const source={schema_version:1,id:'example',title:'Example',applies_to:{country:'TST',tree_id:'tree'},tracks:{research:[{key:'r',name:'Old technology',action:{kind:'research',entity:{kind:'technology',id:'old'}}}]}};
 let data={guide:structuredClone(source),changes:[],revision:0},reject=false;const requests=[];
 const catalog=[{kind:'technology',id:'old',name:'Old technology'},{kind:'technology',id:'new',name:'New technology',localized_name:'Nuova tecnologia'},{kind:'focus',id:'different-kind',name:'New technology',scope:'tree'}];
 const fetch=async(path,options)=>{const body=options?.body?JSON.parse(options.body):null;requests.push({path,body});
  if(body){if(reject)return {ok:false,json:async()=>({error:'Percorso incompatibile'})};data={guide:structuredClone(source),changes:body.entity?[{key:'r',original:source.tracks.research[0].action.entity,entity:body.entity}]:[],revision:data.revision+1};data.guide.tracks.research[0].action.entity=body.entity||source.tracks.research[0].action.entity;}
  return {ok:true,json:async()=>path==='/api/guides'?{guides:[source]}:path==='/api/import/catalog'?{entities:catalog}:data};};
 runLocalized(fs.readFileSync('src/hoi4companion/web/associations.js','utf8'),{document:{getElementById:get,createElement:tag=>new Node(tag)},fetch,location:{search:'?guide=example&step=r'},URLSearchParams});
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

function runLocalized(code,context,options){const i18n=require('../src/hoi4companion/web/i18n.js');i18n.setLanguage('italian');context.HoiI18n=i18n;return vm.runInNewContext(code,context,options);}
