'use strict';
const fs=require('node:fs'),path=require('node:path'),{spawnSync}=require('node:child_process');
const {ROOT,options,fastProfile,BACKEND_CRATES,BACKEND_TEST_CONFIG}=require('./native-tooling.cjs'),args=options(),local=path.join(ROOT,'.local/toolchains/cargo/bin/cargo.exe');
if(args.ci&&args.dev)throw Error('CI validation requires release test semantics');
if(args.lto&&(args.ci||args.dev))throw Error('--lto cannot be combined with --ci or --dev');
const cargo=fs.existsSync(local)?local:'cargo',env={...process.env};
if(cargo===local){env.CARGO_HOME=path.join(ROOT,'.local/toolchains/cargo');env.RUSTUP_HOME=path.join(ROOT,'.local/toolchains/rustup');}
// CI tests keep release semantics but avoid expensive whole-program LTO for
// every test executable. Never reuse these outputs for production packaging.
if(args.ci){env.CARGO_PROFILE_RELEASE_LTO='false';env.CARGO_PROFILE_RELEASE_CODEGEN_UNITS='16';}
// Local runs default to the same optimized, incremental, non-LTO profile and
// cache as dev-native and the -Dev builds, so a one-file change does not relink
// every test executable with whole-program LTO. --lto restores the production
// profile (and its cache) for an explicit pre-release check.
const fast=!args.ci&&!args.dev&&!args.lto;
if(fast)fastProfile(env);
const crates=args._.length?args._:['parser','observer','guide-core','catalog-core','tools','storage','model','guides','backend','overlay','desktop'];
for(const crate of crates){
 if(!/^[a-z-]+$/.test(crate)||!fs.existsSync(path.join(ROOT,'native',crate,'Cargo.toml')))throw Error('Unknown crate: '+crate);
 // Root cdylib+rlib outputs have un-hashed names. Standalone core tests use
 // a second shared cache so they cannot overwrite backend dependency artifacts.
 // Crates split out of the backend build with its dependency features, so they
 // share its cache family: building them in the core cache would overwrite the
 // un-hashed cdylib outputs of catalog-core and friends with another variant.
 const family=crate==='desktop'?'desktop':BACKEND_CRATES.includes(crate)||crate==='overlay'?'backend':'core';
 env.CARGO_TARGET_DIR=path.join(ROOT,'.local',args.ci?`rust-ci-${family}-tests-target`:fast?(BACKEND_CRATES.includes(crate)?'rust-dev-backend-tests-target':`rust-dev-${family}-target`):family==='desktop'?'rust-desktop-target':family==='backend'?'rust-backend-target':'rust-core-tests-target');
 const command=['test','--locked','--manifest-path',path.join(ROOT,'native',crate,'Cargo.toml')];if(!args.dev)command.push('--release');if(fast&&BACKEND_CRATES.includes(crate))command.splice(1,0,...BACKEND_TEST_CONFIG);
 const result=spawnSync(cargo,command,{cwd:ROOT,env,stdio:'inherit',windowsHide:true});if(result.error)throw result.error;if(result.status){process.exitCode=result.status;break;}
}
