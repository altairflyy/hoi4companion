'use strict';
// Guide evaluations produced by the Rust backend for browser presentation tests.
// Inputs and their expected values live in fixtures/presentation-cases.json; the
// evaluations are written by native/backend/tests/presentation_fixtures.rs.
const cases=require('./fixtures/presentation-cases.json'),evaluations=require('./fixtures/presentation-evaluations.json');
const names=cases.map(c=>c.name);
if(names.length!==Object.keys(evaluations).length||names.some(name=>!Object.hasOwn(evaluations,name)))
 throw Error('presentation-evaluations.json does not match presentation-cases.json; run the Rust presentation_fixtures test');
module.exports=name=>{
 const c=cases.find(x=>x.name===name);if(!c)throw Error('Unknown presentation case: '+name);
 return structuredClone({guide:c.guide,state:c.state,progress:c.progress,evaluation:evaluations[name]});
};
// Looks up a recorded case by its exact inputs, for page tests that request evaluations as they run.
module.exports.forInputs=(guide,state,progress)=>{
 const key=JSON.stringify([guide,state,progress]),c=cases.find(x=>JSON.stringify([x.guide,x.state,x.progress])===key);
 if(!c)throw Error('No Rust evaluation is recorded for these inputs; add them to fixtures/presentation-cases.json');
 return structuredClone(evaluations[c.name]);
};
