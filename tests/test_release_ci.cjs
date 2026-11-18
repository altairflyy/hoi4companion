'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const {ROOT,fastProfile,BACKEND_CRATES,BACKEND_TEST_CONFIG}=require('../scripts/native-tooling.cjs');
function commands(options,env={}){
 const calls=[],process={env,exitCode:0};
 vm.runInNewContext(fs.readFileSync(path.join(ROOT,'scripts/test-native.cjs'),'utf8'),{process,require(name){
  if(name==='node:child_process')return {spawnSync:(file,args,settings)=>{calls.push({file,args:Array.from(args),env:{...settings.env}});return {status:0}}};
  if(name==='./native-tooling.cjs')return {ROOT,options:()=>options,fastProfile,BACKEND_CRATES,BACKEND_TEST_CONFIG};
  return require(name);
 }});
 return calls;
}
test('CI executes every native suite with release semantics and isolated fast codegen',()=>{
 const ci=commands({_:[],ci:true}),normal=commands({_:[]}),lto=commands({_:[],lto:true});
 // Every crate under native/ with a Cargo.toml is tested.
 const crates=fs.readdirSync(path.join(ROOT,'native')).filter(d=>fs.existsSync(path.join(ROOT,'native',d,'Cargo.toml')));
 const tested=calls=>calls.map(c=>path.basename(path.dirname(c.args.find(a=>a.endsWith('Cargo.toml'))))).sort();
 assert.deepEqual(tested(ci),[...crates].sort());assert.deepEqual(tested(normal),[...crates].sort());assert.deepEqual(tested(lto),[...crates].sort());
 const b=ci.findIndex(c=>/[\\/]backend[\\/]Cargo\.toml$/.test(c.args.find(a=>a.endsWith('Cargo.toml'))));
 // Local backend tests add only the unoptimized-backend config; everything else matches CI.
 const withoutConfig=args=>args.filter((a,i)=>a!=='--config'&&args[i-1]!=='--config');
 assert.deepEqual(ci.map(c=>Array.from(c.args)),normal.map(c=>withoutConfig(Array.from(c.args))));
 const split=normal.map(c=>BACKEND_CRATES.includes(path.basename(path.dirname(c.args.find(a=>a.endsWith('Cargo.toml'))))));
 assert.deepEqual(split.filter(Boolean).length,BACKEND_CRATES.length);
 assert.deepEqual(normal.map(c=>c.args.includes('--config')),split);
 assert.ok(normal[b].args.includes('profile.release.package.hoi4companion-backend.opt-level=0'));
 assert.ok(!ci[b].args.includes('--config')&&!lto[b].args.includes('--config'),'CI and --lto test the optimized backend');
 assert.deepEqual(ci.map(c=>Array.from(c.args)),lto.map(c=>Array.from(c.args)));
 for(let i=0;i<ci.length;i++){
  assert.ok(ci[i].args.includes('--release'));assert.ok(ci[i].args.includes('--locked'));
  assert.equal(ci[i].env.CARGO_PROFILE_RELEASE_LTO,'false');
  assert.equal(ci[i].env.CARGO_PROFILE_RELEASE_CODEGEN_UNITS,'16');
  assert.notEqual(ci[i].env.CARGO_TARGET_DIR,normal[i].env.CARGO_TARGET_DIR);
  assert.match(ci[i].env.CARGO_TARGET_DIR,/rust-ci-(?:backend|core|desktop)-tests-target$/);
  // Local default: fast non-LTO profile in the dev cache shared with dev-native.
  assert.equal(normal[i].env.CARGO_PROFILE_RELEASE_LTO,'false');
  assert.equal(normal[i].env.CARGO_PROFILE_RELEASE_INCREMENTAL,'true');
  assert.match(normal[i].env.CARGO_TARGET_DIR,/rust-dev-(?:backend|core|desktop|backend-tests)-target$/);
  // --lto: production profile and caches, untouched.
  assert.equal(lto[i].env.CARGO_PROFILE_RELEASE_LTO,undefined);
  assert.doesNotMatch(lto[i].env.CARGO_TARGET_DIR,/rust-(?:ci|dev)-/);
 }
 assert.notEqual(ci[0].env.CARGO_TARGET_DIR,ci[b].env.CARGO_TARGET_DIR,'standalone cdylib outputs must not overwrite backend dependencies');
 assert.notEqual(normal[0].env.CARGO_TARGET_DIR,normal[b].env.CARGO_TARGET_DIR,'standalone cdylib outputs must not overwrite backend dependencies');
 assert.throws(()=>commands({_:[],ci:true,dev:true}),/release test semantics/);
 assert.throws(()=>commands({_:[],lto:true,dev:true}),/--lto cannot be combined/);
});
test('release workflow retains verification gates and exact compiled-resource cache identity',()=>{
 const workflow=fs.readFileSync(path.join(ROOT,'.github/workflows/windows-release.yml'),'utf8');
 assert.match(workflow,/needs: \[build, native-tests\]/);
 assert.match(workflow,/branches: \[main\]/,'default branch seeds caches readable by new tags');
 assert.match(workflow,/node scripts\/test-native\.cjs --ci/);
 assert.match(workflow,/node scripts\/test-web\.cjs/);
 assert.match(workflow,/node scripts\/smoke-native-windows\.cjs/);
 const exact=workflow.split('      - name: Cache exact native components')[1].split('      - name: Cache downloaded dependencies')[0];
 assert.doesNotMatch(exact,/restore-keys:/,'partial native artifacts must never bypass compilation');
 for(const input of ['native/**','src/hoi4companion/*.json','src/hoi4companion/*.md','scripts/build-*.ps1'])assert.ok(exact.includes(input),input);
 assert.match(workflow,/steps\.toolchain\.outputs\.key/);
 assert.match(workflow,/compression-level: 0/);
 for(const profile of ['native/backend/Cargo.toml','native/observer/Cargo.toml','native/desktop/Cargo.toml']){
  const text=fs.readFileSync(path.join(ROOT,profile),'utf8');
  assert.match(text,/lto = true/);assert.match(text,/codegen-units = 1/);
 }
});
