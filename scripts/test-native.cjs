'use strict';
const fs=require('node:fs'),path=require('node:path'),{spawnSync}=require('node:child_process');
const {ROOT,options}=require('./native-tooling.cjs'),args=options(),local=path.join(ROOT,'.local/toolchains/cargo/bin/cargo.exe');
const cargo=fs.existsSync(local)?local:'cargo',env={...process.env};
if(cargo===local){env.CARGO_HOME=path.join(ROOT,'.local/toolchains/cargo');env.RUSTUP_HOME=path.join(ROOT,'.local/toolchains/rustup');}
// CI tests keep release semantics but avoid expensive whole-program LTO for
// every test executable. Never reuse these outputs for production packaging.
if(args.ci){env.CARGO_PROFILE_RELEASE_LTO='false';env.CARGO_PROFILE_RELEASE_CODEGEN_UNITS='16';}
const crates=args._.length?args._:['parser','observer','guide-core','catalog-core','tools','storage','backend','overlay'];
for(const crate of crates){
 if(!/^[a-z-]+$/.test(crate)||!fs.existsSync(path.join(ROOT,'native',crate,'Cargo.toml')))throw Error('Unknown crate: '+crate);
 // Root cdylib+rlib outputs have un-hashed names. Standalone core tests use
 // a second shared cache so they cannot overwrite backend dependency artifacts.
 const family=crate==='backend'||crate==='overlay'?'backend':'core';
 env.CARGO_TARGET_DIR=path.join(ROOT,'.local',args.ci?`rust-ci-${family}-tests-target`:family==='backend'?'rust-backend-target':'rust-core-tests-target');
 const command=['test','--locked','--manifest-path',path.join(ROOT,'native',crate,'Cargo.toml')];if(!args.dev)command.push('--release');
 const result=spawnSync(cargo,command,{cwd:ROOT,env,stdio:'inherit',windowsHide:true});if(result.error)throw result.error;if(result.status){process.exitCode=result.status;break;}
}
