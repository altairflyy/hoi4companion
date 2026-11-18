'use strict';
const fs=require('node:fs'),path=require('node:path'),{spawnSync}=require('node:child_process');
const {ROOT,fastProfile,BACKEND_TEST_CONFIG}=require('./native-tooling.cjs');
const usage='Usage: node scripts/dev-native.cjs <check|build|test> <crate> [cargo arguments...]\nExample: node scripts/dev-native.cjs test backend --lib some_test';
function plan(argv,baseEnv=process.env){
 const [action,crate,...extra]=argv;
 if(!['check','build','test'].includes(action)||!crate||!/^[a-z-]+$/.test(crate)||!fs.existsSync(path.join(ROOT,'native',crate,'Cargo.toml')))throw Error(usage);
 // Keep development artifacts isolated from release and CI, including when
 // caller-provided Cargo arguments or environment attempt to select a profile.
 if(extra.some(a=>/^(--(?:release|profile|target-dir|manifest-path|config)|-r)(?:=|$)/.test(a)))throw Error('Profile, manifest and target directory are managed by dev-native.');
 const local=path.join(ROOT,'.local/toolchains/cargo/bin/cargo.exe'),cargo=fs.existsSync(local)?local:'cargo';
 const env={...baseEnv};
 if(cargo===local){env.CARGO_HOME=path.join(ROOT,'.local/toolchains/cargo');env.RUSTUP_HOME=path.join(ROOT,'.local/toolchains/rustup');}
 const family=crate==='desktop'?'desktop':crate==='backend'||crate==='overlay'?'backend':'core';
 const backendTests=action==='test'&&crate==='backend';
 env.CARGO_TARGET_DIR=path.join(ROOT,'.local',backendTests?'rust-dev-backend-tests-target':`rust-dev-${family}-target`);
 fastProfile(env);
 return {cargo,env,args:[action,...(backendTests?BACKEND_TEST_CONFIG:[]),'--release','--locked','--manifest-path',path.join(ROOT,'native',crate,'Cargo.toml'),...extra]};
}
if(require.main===module){
 try{const p=plan(process.argv.slice(2));console.error(`Development-only cache: ${p.env.CARGO_TARGET_DIR}`);const result=spawnSync(p.cargo,p.args,{cwd:ROOT,env:p.env,stdio:'inherit',windowsHide:true});if(result.error)throw result.error;process.exitCode=result.status??1;}
 catch(error){console.error(error.message);process.exitCode=1;}
}
module.exports={plan};
