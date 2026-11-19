'use strict';
// The shared agent handoff is read at the start of every agent session, so its size is a recurring token cost.
// CLAUDE.md caps it; this test enforces the cap so finished history moves to docs/handoff-archive-*.md.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const HANDOFF = path.join(__dirname, '..', 'docs', 'AGENT_HANDOFF.md');
const MAX_BYTES = 6 * 1024;
const MAX_LINE_CHARS = 300;
const SECTIONS = ['Next actions', 'In progress', 'Recently verified', 'Open questions / hypotheses', 'Working boundaries'];

test('agent handoff stays within its byte cap', () => {
  const bytes = fs.statSync(HANDOFF).size;
  assert.ok(bytes <= MAX_BYTES, `docs/AGENT_HANDOFF.md is ${bytes} bytes, cap ${MAX_BYTES}: move finished history to docs/handoff-archive-*.md`);
});

test('agent handoff lines stay short', () => {
  const long = fs.readFileSync(HANDOFF, 'utf8').split(/\r?\n/)
    .map((line, i) => ({ n: i + 1, len: [...line].length }))
    .filter(({ len }) => len > MAX_LINE_CHARS);
  assert.deepEqual(long, [], `lines over ${MAX_LINE_CHARS} characters: split or link to evidence`);
});

test('agent handoff keeps its section order', () => {
  const headings = fs.readFileSync(HANDOFF, 'utf8').split(/\r?\n/)
    .filter((line) => line.startsWith('## ')).map((line) => line.slice(3).trim());
  const found = SECTIONS.map((name) => headings.indexOf(name));
  assert.ok(found.every((i) => i >= 0), `missing sections: ${SECTIONS.filter((_, k) => found[k] < 0).join(', ')}`);
  assert.deepEqual([...found].sort((a, b) => a - b), found, 'sections out of order');
});
