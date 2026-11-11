'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const {load} = require('../src/hoi4companion/web/guide-intent.js');
const model = JSON.parse(fs.readFileSync('src/hoi4companion/web/guide-intent-model.json', 'utf8'));

test('quantized guide intent model loads with seven classes and finite scores', () => {
  const classifier = load(model);
  assert.equal(classifier.labels.length, 7);
  for (const instruction of ['Build MILs', 'Inf->25', 'Send spies', 'Research radio', '', 'On picking focus']) {
    const result = classifier.predict(instruction);
    assert.ok(classifier.labels.includes(result.label));
    assert.ok(Number.isFinite(result.margin));
    assert.ok(Number.isInteger(result.features));
    assert.equal(typeof result.requires_review, 'boolean');
  }
});
test('unsafe, malformed or incompatible matrices fail closed', () => {
  for (const patch of [
    {schema_version: 2}, {domain: 'FOCUS'}, {idf_scale: 0},
    {weights_i8: ''}, {weight_scales: [1]}, {labels: []}
  ]) assert.throws(() => load({...model, ...patch}));
});
test('an empty or unseen source is only an advisory review, not an executable action', () => {
  const classifier = load(model);
  const result = classifier.predict('');
  assert.equal(result.requires_review, true);
  assert.equal(result.features, 0);
  assert.ok(!('action' in result));
  assert.ok(!('entity' in result));
});
test('classification never mutates the original source or word step schema', () => {
  const classifier = load(model);
  const step = {key:'word-0001',type:'reminder',title:'Build MILs',instructions:['Build MILs'],evidence:[{quote:'Build MILs'}]};
  const before = JSON.stringify(step);
  const result = classifier.predict(step.instructions[0]);
  assert.ok(result.label);
  assert.equal(JSON.stringify(step), before);
});
