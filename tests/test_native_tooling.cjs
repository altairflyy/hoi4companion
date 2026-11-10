'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {ROOT,temporary,archive}=require('../scripts/native-tooling.cjs'),{zip}=require('./native-fixtures.cjs');
test('source, tests, release workflow and package metadata no longer require Python',()=>{
 const scan=folder=>fs.readdirSync(folder,{withFileTypes:true}).flatMap(entry=>{const p=path.join(folder,entry.name);return entry.isDirectory()?scan(p):[p];});
 for(const base of ['src/hoi4companion','tests','scripts'])assert.deepEqual(scan(path.join(ROOT,base)).filter(p=>/\.(?:py|pyc|pyd)$/.test(p)),[]);
 for(const name of ['pyproject.toml','setup.py','MANIFEST.in','packaging/requirements-windows.lock'])assert.equal(fs.existsSync(path.join(ROOT,name)),false,name);
 assert.doesNotMatch(fs.readFileSync(path.join(ROOT,'.github/workflows/windows-release.yml'),'utf8'),/setup-python|pip install|unittest|\.venv/);
 assert.equal(require('../package.json').version,'0.2.0');
});
test('archive tooling round trips native files and rejects traversal before extraction',{skip:process.platform!=='win32'},()=>{
 const owned=temporary('hoi4-archive-contract-');
 try{
  const source=path.join(owned.root,'Application');fs.mkdirSync(source);fs.writeFileSync(path.join(source,'file.txt'),'native');
  const output=path.join(owned.root,'application.zip');archive('Create',source,output);archive('Extract',output,path.join(owned.root,'unpacked'));
  assert.equal(fs.readFileSync(path.join(owned.root,'unpacked/Application/file.txt'),'utf8'),'native');
  const bad=path.join(owned.root,'bad.zip');fs.writeFileSync(bad,zip({'../escape.txt':'forbidden'}));
  assert.throws(()=>archive('Extract',bad,path.join(owned.root,'unsafe')));assert.equal(fs.existsSync(path.join(owned.root,'escape.txt')),false);assert.equal(fs.existsSync(path.join(owned.root,'unsafe')),false);
 }finally{owned.close();}
});
