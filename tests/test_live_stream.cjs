const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const vm=require('node:vm');

function boot(){
  class Element{
    constructor(){this.children=[];this.handlers={};this.classList={toggle(){}};}
    append(...items){this.children.push(...items);}
    after(){}
    setAttribute(){}
    replaceChildren(){this.children=[];}
    addEventListener(type,callback){this.handlers[type]=callback;}
    emit(type){this.handlers[type]?.();}
  }
  const made=[],document=new Element();
  document.body=new Element();document.hidden=false;
  document.createElement=()=>{const el=new Element();made.push(el);return el;};
  document.createTextNode=text=>text;
  document.querySelector=()=>new Element();
  class EventSource extends Element{
    static instances=[];
    constructor(url){super();this.url=url;EventSource.instances.push(this);}
    close(){this.closed=true;}
    send(type,data){this.handlers[type]?.({data:JSON.stringify(data)});}
  }
  const window=new Element();
  const intervals=[];
  const context={document,window,EventSource,sessionStorage:{getItem(){return null;},setItem(){}},
    setInterval:fn=>intervals.push(fn),Date,CustomEvent:class{}};
  runLocalized(fs.readFileSync(require.resolve('../src/hoi4companion/web/live-state.js'),'utf8'),context);
  return {window,document,streams:EventSource.instances,toggle:made.find(x=>x.type==='checkbox')};
}
const state={usable:true,status:'observed',sample:{country:'ROM',completed_focuses:[],researched_technologies:[],research_slots:[],continuous_focus:'training'}};
test('shared checklist events reach subscribers even while gameplay is unchanged',()=>{
 const app=boot(),received=[];
 app.window.hoiLive.subscribe(value=>received.push(value.guide_revision));
 app.streams[0].send('state',{...state,guide_revision:0,guide_checks:{}});
 app.streams[0].send('state',{...state,guide_revision:1,guide_checks:{f12:true}});
 assert.deepEqual(received.slice(-2),[0,1]);
});

test('run identity changes reach subscribers with identical gameplay and checks',()=>{
 const app=boot(),received=[];
 app.window.hoiLive.subscribe(value=>received.push(value.guide_runs?.g?.run_id));
 app.streams[0].send('state',{...state,guide_runs:{g:{run_id:'one'}},guide_checks:{}});
 app.streams[0].send('state',{...state,guide_runs:{g:{run_id:'two'}},guide_checks:{}});
 assert.deepEqual(received.slice(-2),['one','two']);
});

test('manual starts reach subscribers without completing checks or changing gameplay',()=>{
 const app=boot(),received=[];
 app.window.hoiLive.subscribe(value=>received.push(value.guide_started?.g?.build));
 app.streams[0].send('state',{...state,guide_started:{g:{build:false}},guide_checks:{}});
 app.streams[0].send('state',{...state,guide_started:{g:{build:true}},guide_checks:{}});
 assert.deepEqual(received.slice(-2),[false,true]);
 assert.equal(Object.keys(app.window.hoiLive.value.guide_checks).length,0);
});

test('stream loss invalidates immediately and heartbeat never revives old samples',()=>{
  const app=boot(),stream=app.streams[0];
  assert.equal(stream.url,'/api/state/events');
  stream.send('state',state);
  assert.equal(app.window.hoiLive.sample().country,'ROM');
  stream.send('error',{});
  assert.equal(app.window.hoiLive.sample(),null);
  stream.send('heartbeat',{usable:true});
  assert.equal(app.window.hoiLive.sample(),null);
  stream.send('state',state);
  stream.send('heartbeat',{usable:false});
  assert.equal(app.window.hoiLive.sample(),null);
});

test('hidden/toggled views close stream and resume only with fresh state',()=>{
  const app=boot(),old=app.streams[0];
  old.send('state',state);
  app.document.hidden=true;app.document.emit('visibilitychange');
  assert.equal(old.closed,true);
  assert.equal(app.window.hoiLive.sample(),null);
  app.document.hidden=false;app.document.emit('visibilitychange');
  assert.equal(app.streams.length,2);
  assert.equal(app.window.hoiLive.sample(),null);
  old.send('state',state);
  assert.equal(app.window.hoiLive.sample(),null);
  app.streams[1].send('state',state);
  assert.equal(app.window.hoiLive.sample().country,'ROM');
  app.toggle.checked=false;app.toggle.emit('change');
  assert.equal(app.streams[1].closed,true);
  assert.equal(app.window.hoiLive.enabled,false);
  assert.equal(app.window.hoiLive.sample(),null);
});

function runLocalized(code,context,options){const i18n=require('../src/hoi4companion/web/i18n.js');i18n.setLanguage('italian');context.HoiI18n=i18n;return vm.runInNewContext(code,context,options);}
