'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs');
const refs=require('../src/hoi4companion/web/guide-references.js');
const row=(...columns)=>'[Table row]\n'+columns.map((c,i)=>`[Column ${i+1}]\n${c}`).join('\n')+'\n';
const guide={id:'test',sources:[{id:'doc',type:'file',title:'Country.docx',text:
 row('ACTIONS','RESEARCH PRIORITY','FOCUS PRIORITY')+row('ROM setup*','Tech','Focus')+
 row('WAR PLANS')+row('INITIAL')+row('- Change all to DEF.\n- Train.')+
 row('ROM')+row('- Defend the river.\n- Use the army.')+
 row('YUG (after CZE)')+row('- Convert the divisions.\n- Push the border.')+
 row('AXIS')+row('- Prepare the western front.')+
 row('PEACE CONFERENCE PRIORITY')+row('ROM','Annex the cores.')+
 row('RNG / VARIABLE OUTCOMES')+row('GER DOES NOT ATTACK','Justify instead.')} ]};
const links=(text,index=refs.build(guide),extras=[])=>refs.segments(text,index,extras).filter(p=>p.reference);
const tables={id:'tables',sources:[{id:'country',title:'Country.docx',text:
 row('ARMY SETUP')+row('NAVY')+row('COMMAND','NAME','FLEET / ROLE / ORDERS')+
 row('Admiral 1','','Two fleets for NED.')+
 row('ARMY')+row('HQ SETUP','')+row('MARSHAL 1','Mikhail T → Georgy Z')+
 row('ARMY','GENERAL','ROLE / ORDERS')+row('10 ALP\n11 ARM\n1 MOT','Konstantine R\nIvan K\nANY DEF','Invade NED.\nARM pushes; MOT follows.')+
 row('MARSHAL 2','ANY DEF; MARSHAL 3 (ANY DEF)')+row('ARMY','GENERAL','UNITS / ROLE / ORDERS')+
 row('M2: 3×24 INF\nM3: 18→24 NKVD→INF','Aleksandr V','Defend POL.\nDo not change these source values.')+
 row('WAR PLANS')+row('START & NED')+row('Prepare the landing.')+
 row('GER & AXIS')+row('Push Minsk → Wilno.')+row('FRA / UK / USA')+row('Invade through the indicated ports.')+
 row('PEACE CONFERENCE PRIORITY')+row('NED','Take navy, puppet, resource rights, then reparations.')+
 row('AXIS','Puppet all but CZE.')+row('RELEVANT EVENTS')+row('JUL 1937','JAP war on CHI')+
 row('SEP 1939','GER attack POL\nACHIEVEMENTS')+row('ACHIEVEMENT','GUIDE ADJUSTMENTS')+row('Unrelated achievement','Do not include in events.')} ]};
test('Word references point to their own subsection and preserve the original wording',()=>{
 const index=refs.build(guide),initial=links('ARMY: Initial Setup*, train',index)[0];
 assert.equal(initial.text,'Initial Setup*');assert.equal(initial.candidates.length,1);
 assert.equal(initial.candidates[0].title,'INITIAL');assert.match(initial.candidates[0].text,/Change all to DEF/);
 assert.doesNotMatch(initial.candidates[0].text,/Defend the river/);
 const rom=links('ROM setup*',index)[0];assert.equal(rom.candidates.length,1);
 assert.match(rom.candidates[0].text,/Defend the river/);assert.doesNotMatch(rom.candidates[0].text,/Annex the cores/);
 assert.equal(links('YUG setup*',index)[0].candidates[0].title,'YUG (after CZE)');
 assert.equal(links('See PEACE CONFERENCE PRIORITY',index)[0].candidates[0].title,'PEACE CONFERENCE PRIORITY');
 assert.match(links('If GER does not attack RNG*',index)[0].candidates[0].text,/Justify instead/);
 assert.equal(refs.segments('ROM setup* and Initial Setup*',index).map(p=>p.text).join(''),'ROM setup* and Initial Setup*');
});
test('unqualified country mentions are not turned into references; ambiguous references offer all candidates',()=>{
 assert.equal(links('REL ROM, NA ROM, attack ROM').length,0);
 const matches=links('See ROM')[0];assert.equal(matches.candidates.length,2);
 assert.ok(matches.candidates.every(c=>c.source_id==='doc'&&c.context));
 assert.equal(links('See ROM setup*')[0].candidates.length,1);
});
test('explicit missing references stay accessible without invented content or arbitrary fuzzy matching',()=>{
 const part=links('Refer to Missing Setup*')[0];assert.equal(part.candidates.length,0);
 assert.equal(part.text,'Missing Setup*');
 assert.equal(links('See AXIZ setup*')[0].candidates.length,0);
 assert.equal(links('Push GER* when ready')[0].candidates.length,0);
});
test('reference wrappers and generic build/invasion wording work across countries',()=>{
 const index=refs.build({id:'other',sources:[{id:'source',text:'# NAVY BUILDS:\nChoose a naval build.\n# UK INVASION\nPrepare the ships.'}]});
 assert.equal(links('Navy BUILD',index)[0].candidates[0].title,'NAVY BUILDS');
 assert.equal(links('Invade UK*',index)[0].candidates[0].title,'UK INVASION');
});
test('paragraph sections following a Word table do not get swallowed by the final cell',()=>{
 const index=refs.build({id:'mixed',sources:[{id:'mixed-doc',text:row('Term','Meaning')+
  'PART III — DOCTRINES & BUILDS\nNAVY BUILDS:\nNIGHT (NIGHT ATTACK BUILD):\nSelect the fleet build.\nPART IV — STRATEGY REFERENCE\nUK INVASION\nPrepare the navy.'}]});
 assert.match(links('Navy BUILD',index)[0].candidates[0].text,/Select the fleet build/);
 assert.equal(links('Invade UK*',index)[0].candidates[0].text,'Prepare the navy.');
});
test('Markdown headings, repeated titles and guide changes remain scoped to their source',()=>{
 const make=(id,body)=>({id,type:'file',title:id,text:'# War plans\n## Initial Setup\n'+body+'\n## Other\nOther content'});
 const first=refs.build({id:'a',sources:[make('one','First instructions'),make('two','Second instructions')]});
 assert.equal(links('See Initial Setup',first)[0].candidates.length,2);
 const second=refs.build({id:'b',sources:[make('three','Third instructions')]});
 assert.equal(links('See Initial Setup',second)[0].candidates[0].source_id,'three');
 assert.equal(links('See Initial Setup',second)[0].candidates[0].text,'Third instructions');
});
test('exact associated template references use the existing local image endpoint and never remote image IDs',()=>{
 const row={action:{params:{reference_images:[{name:'OFF',id:'a'.repeat(64)},{name:'BAD',id:'https://example.com/x'}],design_reference:{name:'T-I 1',image_id:'b'.repeat(64),modules:{gun:'cannon'}}}}};
 const extra=refs.images(row);assert.equal(extra.length,2);
 assert.equal(links('Change DEF to OFF',refs.build(guide),extra)[0].candidates[0].image_id,'a'.repeat(64));
 assert.equal(links('Design T-I 1',refs.build(guide),extra)[0].candidates[0].image_id,'b'.repeat(64));
});

function dom(){
 class Node{
  constructor(tag){this.tag=tag;this.children=[];this.style={};this.hidden=false;this.isConnected=true;this.text='';}
  set textContent(value){this.text=String(value);this.children=[];}get textContent(){return this.text+this.children.map(x=>x.textContent).join('');}
  append(...items){this.children.push(...items);}replaceChildren(...items){this.text='';this.children=items;}
  setAttribute(key,value){this[key]=value;}contains(node){return node===this||this.children.some(x=>x.contains?.(node));}
  getBoundingClientRect(){return {left:25,top:100,bottom:125,width:500,height:250};}
  focus(){document.activeElement=this;this.onfocus?.();}
 }
 const listeners={},document={body:new Node('body'),documentElement:{clientWidth:1280,clientHeight:900},
  createElement:tag=>new Node(tag),createTextNode:text=>({textContent:text}),
  addEventListener(type,fn){listeners[type]=fn;},getElementById(id){return walk(this.body).find(n=>n.id===id);}};
 const walk=node=>[node,...(node.children||[]).flatMap(walk)];
 return {document,listeners,walk,node:()=>new Node('p')};
}
test('hover previews, click pins, Escape closes without reopening and source HTML stays literal',async()=>{
 const {document,listeners,walk,node}=dom(),controller=refs.create({document});controller.setGuide(guide);
 const line=node();controller.append(line,'See Initial Setup');const button=walk(line).find(n=>n.tag==='button');
 button.onpointerenter();const panel=document.getElementById('guideReferencePreview');assert.equal(panel.hidden,false);assert.match(panel.textContent,/Change all to DEF/);
 button.onclick();assert.equal(document.activeElement,panel);button.onpointerleave();await new Promise(r=>setTimeout(r,240));assert.equal(panel.hidden,false);
 // The displayed source survives a routine replacement of its card by live UI.
 button.isConnected=false;line.replaceChildren();assert.match(panel.textContent,/Change all to DEF/);
 button.isConnected=true;listeners.keydown({key:'Escape',preventDefault(){}});assert.equal(panel.hidden,true);assert.equal(document.activeElement,button);
 controller.setGuide({id:'other',sources:[{id:'raw',text:'# Initial Setup\n<script>alert(1)</script>'}]});
 const other=node();controller.append(other,'See Initial Setup');walk(other).find(n=>n.tag==='button').onclick();
 assert.match(panel.textContent,/<script>alert\(1\)<\/script>/);assert.equal(walk(panel).some(n=>n.tag==='script'),false);
 controller.setGuide({id:'third'});assert.equal(panel.hidden,true);
});
test('ambiguous previews retain each source separately and missing references offer the source document',()=>{
 const {document,walk,node}=dom(),controller=refs.create({document});controller.setGuide(guide);
 const line=node();controller.append(line,'See ROM');walk(line).find(n=>n.tag==='button').onclick();
 const panel=document.getElementById('guideReferencePreview');assert.match(panel.textContent,/Several sections match/);assert.equal(walk(panel).filter(n=>n.tag==='details').length,2);
 controller.setGuide(guide);const missing=node();controller.append(missing,'Refer to Missing Setup*');walk(missing).find(n=>n.tag==='button').onclick();
 assert.match(panel.textContent,/could not be located/);assert.ok(walk(panel).some(n=>n.tag==='a'&&n.href==='#sourceDetails'));
});

test('Armies Setup resolves the army table rather than its navy sibling and keeps cells intact',()=>{
 const index=refs.build(tables),part=links('Armies: Rename, Setup* & Train',index)[0];
 assert.equal(part.candidates.length,1);const army=part.candidates[0];assert.equal(army.title,'ARMY');
 assert.match(army.text,/Mikhail T → Georgy Z/);assert.match(army.text,/MARSHAL 3/);
 assert.doesNotMatch(army.text,/Two fleets/);
 const units=army.blocks.find(b=>b.type==='row'&&b.cells[0].startsWith('10 ALP'));
 assert.deepEqual(units.cells,['10 ALP\n11 ARM\n1 MOT','Konstantine R\nIvan K\nANY DEF','Invade NED.\nARM pushes; MOT follows.']);
 const inherited=refs.segments('Setup*',index,[],'Armies: Rename, Setup & Train').find(p=>p.reference);
 assert.equal(inherited.candidates[0].id,army.id);
 assert.equal(links('Navy: Setup*',index)[0].candidates[0].title,'NAVY');
 assert.ok(links('Setup*',index)[0].candidates.length>1);
 const mixed=links('Armies: Rename, Setup* & Train\nNavy: Setup*',index);
 assert.deepEqual(mixed.map(p=>p.candidates[0].title),['ARMY','NAVY']);
 assert.equal(links('ARMY: Rename\n- Request forces and setup*\nNAVY: gather',index)[0].candidates[0].title,'ARMY');
 assert.equal(links('Assign scientist & setup done',index).length,0);
 assert.equal(links('army setup*',index)[0].candidates[0].title,'ARMY');
 const named=links('USSR Setup*',index)[0];assert.equal(named.text,'USSR Setup*');assert.equal(named.candidates.length,0);
 assert.equal(refs.segments('USSR   Setup*',index).map(p=>p.text).join(''),'USSR   Setup*');
 assert.equal(links('UK setup*',index)[0].candidates[0].title,'FRA / UK / USA');
});

test('quick references preserve complete war, peace and event sections without crossing boundaries',()=>{
 const index=refs.build(tables),quick=refs.quickReferences(index);
 assert.deepEqual(quick.map(q=>q.label),['Army setup','War plans','Peace conference','Relevant events']);
 assert.match(quick[1].candidates[0].text,/START & NED.*GER & AXIS.*FRA \/ UK \/ USA/s);
 assert.doesNotMatch(quick[1].candidates[0].text,/Take navy, puppet/);
 assert.match(quick[2].candidates[0].text,/Take navy, puppet/);assert.doesNotMatch(quick[2].candidates[0].text,/JUL 1937/);
 const events=quick[3].candidates[0];assert.deepEqual(events.blocks.filter(b=>b.type==='row').map(b=>b.cells),[['JUL 1937','JAP war on CHI'],['SEP 1939','GER attack POL']]);
 assert.doesNotMatch(events.text,/achievement/i);
 assert.equal(refs.quickReferences(refs.build({id:'empty'})).length,0);
});

test('table previews render semantic cells, retain source text and stay pinned through identical live projections',()=>{
 const {document,listeners,walk,node}=dom(),controller=refs.create({document});
 assert.equal(controller.setGuide(tables),true);
 const line=node();controller.append(line,'Armies: Rename, Setup* & Train');walk(line).find(n=>n.tag==='button').onclick();
 const panel=document.getElementById('guideReferencePreview');assert.ok(walk(panel).some(n=>n.tag==='table'));
 assert.ok(walk(panel).some(n=>n.tag==='th'&&n.textContent==='GENERAL'&&n.scope==='col'));
 assert.ok(walk(panel).some(n=>n.tag==='td'&&n.textContent==='Konstantine R\nIvan K\nANY DEF'));
 assert.equal(walk(panel).some(n=>n.tag==='pre'),false);assert.doesNotMatch(panel.textContent,/\[Table row\]|\[Column \d+\]/);
 assert.equal(controller.setGuide({...structuredClone(tables),runtime_definition_revision:'new-view'}),false);
 assert.equal(panel.hidden,false);assert.match(panel.textContent,/Mikhail T/);
 const shortcuts=controller.shortcuts();assert.equal(shortcuts.length,4);shortcuts[3].onclick();
 assert.match(panel.textContent,/JAP war on CHI/);assert.doesNotMatch(panel.textContent,/Take navy, puppet/);
 assert.equal(controller.setGuide({...tables,id:'another-campaign-guide'}),true);assert.equal(panel.hidden,true);
 controller.setGuide(tables);const quick=controller.shortcuts();quick[1].onpointerenter();
 const scroller=walk(panel).find(n=>n.className==='guide-reference-table-scroll');listeners.scroll({target:scroller});assert.equal(panel.hidden,false);
 listeners.scroll({target:document.body});assert.equal(panel.hidden,true);
});

test('reference cells never become HTML and source updates invalidate an open preview',()=>{
 const {document,walk,node}=dom(),controller=refs.create({document});
 const unsafe={id:'safe',sources:[{id:'s',text:row('RELEVANT EVENTS')+row('JUL 1937','<img src=x onerror=alert(1)>')}]};
 controller.setGuide(unsafe);controller.shortcuts()[0].onclick();const panel=document.getElementById('guideReferencePreview');
 assert.match(panel.textContent,/<img src=x onerror=alert\(1\)>/);assert.equal(walk(panel).some(n=>n.tag==='img'),false);
 const changed=structuredClone(unsafe);changed.sources[0].text=row('RELEVANT EVENTS')+row('1938','Changed source');
 assert.equal(controller.setGuide(changed),true);assert.equal(panel.hidden,true);
 const frozen=JSON.stringify(tables);refs.build(tables);assert.equal(JSON.stringify(tables),frozen);
});

test('quick source references have a persistent accessible toolbar outside the collapsed plan',()=>{
 const html=fs.readFileSync('src/hoi4companion/web/guide.html','utf8'),css=fs.readFileSync('src/hoi4companion/web/guide.css','utf8');
 assert.ok(html.indexOf('id="guideQuickReferences"')<html.indexOf('<details id="operationsPlan"'));
 assert.match(html,/<nav id="guideQuickReferences"[^>]*aria-label="Guide references"/);
 assert.match(css,/\.guide-reference-toolbar\{[^}]*position:sticky/);
 assert.match(css,/\.guide-reference-table\{[^}]*font:14px\/1\.6/);
});
