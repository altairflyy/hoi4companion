'use strict';
const fs=require('node:fs'),path=require('node:path'),{spawnSync}=require('node:child_process');
const root=path.resolve(__dirname,'..'),tests=fs.readdirSync(path.join(root,'tests')).filter(n=>/^test_.*\.cjs$/.test(n)).sort().map(n=>path.join(root,'tests',n));
const result=spawnSync(process.execPath,['--test',...tests],{cwd:root,stdio:'inherit',windowsHide:true});if(result.error)throw result.error;process.exitCode=result.status??1;
