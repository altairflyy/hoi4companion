require('../src/hoi4companion/web/i18n.js').setLanguage('italian');
const {test}=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {filterDiagnostics,summaryText,mount}=require('../src/hoi4companion/web/analysis.js');

test('full engine does not render unavailable coverage metrics as zero',()=>{
  const text=summaryText({coverage:{metrics_available:false,documents:10,parser_errors:2}});
  assert.match(text,/sconosciuta/);
  assert.doesNotMatch(text,/0 controlli/);
  assert.match(summaryText({coverage:{metrics_available:false,documents:10,parser_errors:1},parser_error_details:[{classification:'rejected_by_both'}]}),/entrambi i parser/);
});

test('analysis filters the four categories without treating unknown coverage as success',()=>{
  const report={status:'partial',coverage:{unsupported:7,unknown_scope:2},summary:{references:1,scopes:2,parameters:3,localisation:4},
    diagnostics:[{category:'references',file:'mod.txt',message:'Missing ID',value:'F1'},{category:'scopes',file:'event.txt',message:'Wrong context'}]};
  assert.equal(filterDiagnostics(report,'references','f1').length,1);
  assert.equal(filterDiagnostics(report,'scopes','f1').length,0);
  assert.match(summaryText(report),/7/);
  assert.match(summaryText(report),/2/);
  assert.doesNotMatch(summaryText(report),/completat|tutto valido/i);
  assert.match(summaryText(report),/si sovrappongono/);
});

test('set aside source errors have a separate count and a persistent log',()=>{
  const text=summaryText({coverage:{metrics_available:false,documents:10,parser_errors:3,ignored_parser_errors:2},
    source_error_log:{status:'saved',path:'local/source-errors.json'},
    parser_error_details:[{classification:'rejected_by_both'},{classification:'rejected_by_both'}]});
  assert.match(text,/1 errore/);
  assert.match(text,/2 file.*accantonati/);
  assert.match(text,/local\/source-errors.json/);
});

test('recovered definitions are distinguished from excluded files and gameplay proof',()=>{
  const text=summaryText({coverage:{metrics_available:false,documents:10,parser_errors:0,ignored_parser_errors:0,recovered_sources:31}});
  assert.match(text,/31 file recuperati in memoria/);
  assert.match(text,/proprietà senza valore restano omesse/);
  assert.match(text,/non verificano la partita/);
});

test('diagnostics render as text and a failed refresh clears the old results',async()=>{
  class Element{
    constructor(){this.children=[];this.value='';this.textContent='';this.hidden=false;}
    appendChild(child){this.children.push(child);return child;}
    replaceChildren(){this.children=[];}
    addEventListener(){}
    set innerHTML(value){throw new Error('Unsafe HTML insertion');}
  }
  const elements=new Map();
  const document={getElementById(id){if(!elements.has(id))elements.set(id,new Element());return elements.get(id);},createElement(){return new Element();}};
  let failed=false;
  const report={rules:{revision:'abc123',load_errors:[]},coverage:{documents:1,entities:1,unhandled:{}},summary:{references:1},
    diagnostics:[{category:'references',message:'<img src=x onerror=alert(1)>',file:'x',path:'a',value:'ID'}]};
  const controller=mount(document,async()=>({ok:!failed,json:async()=>failed?{error:'Cannot read scripts'}:report}));
  await controller.load();
  assert.equal(document.getElementById('diagnostics').children[0].children[0].textContent,report.diagnostics[0].message);
  failed=true;await controller.load(true);
  assert.equal(document.getElementById('diagnostics').children.length,0);
  assert.equal(document.getElementById('error').hidden,false);
  assert.equal(document.getElementById('analyze').disabled,false);
});

test('all main views retain import and expose optional script diagnostics',()=>{
  for(const file of ['index','technologies','country','guide','importer','associations','analysis']){
    const html=fs.readFileSync(path.join(__dirname,'../src/hoi4companion/web',file+'.html'),'utf8');
    assert.match(html,/href="\/analysis"/);
    assert.match(html,/href="\/importer"/);
  }
});
