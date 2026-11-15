const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const extractor = path.resolve(__dirname, '../scripts/benchmark-guide-sources.ps1');
const ps = (args) => {
  const result = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', ...args], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout;
};

test('extracts source coordinates and isolates duplicate and unsafe XML errors', { skip: process.platform !== 'win32' }, (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'hoi4-source-test-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const xml = `<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>ACTIONS</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>RESEARCH PRIORITY*</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>FOCUS PRIORITY</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t xml:space="preserve">Build </w:t><w:t>MIL</w:t><w:tab/><w:t>2</w:t><w:br/><w:t>Then wait</w:t></w:r></w:p><w:p/><w:p><w:r><w:t>Good Job!</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>`;
  fs.writeFileSync(path.join(dir, 'valid.xml'), xml);
  fs.writeFileSync(path.join(dir, 'unsafe.xml'), xml.replace('?><', '?><!DOCTYPE w:document [<!ENTITY x "x">]><'));
  const setup = `param($dir)
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
function AddEntry($zip,$name,$bytes) { $e=$zip.CreateEntry($name); $s=$e.Open(); try {$s.Write($bytes,0,$bytes.Length)} finally {$s.Dispose()} }
foreach($name in @('valid','unsafe')) { $p=Join-Path $dir ($name+'.docx'); $z=[IO.Compression.ZipFile]::Open($p,[IO.Compression.ZipArchiveMode]::Create); try { AddEntry $z 'word/document.xml' ([IO.File]::ReadAllBytes((Join-Path $dir ($name+'.xml')))) } finally {$z.Dispose()} }
$z=[IO.Compression.ZipFile]::Open((Join-Path $dir 'source.zip'),[IO.Compression.ZipArchiveMode]::Create)
try { AddEntry $z 'valid.docx' ([IO.File]::ReadAllBytes((Join-Path $dir 'valid.docx'))); AddEntry $z 'unsafe.docx' ([IO.File]::ReadAllBytes((Join-Path $dir 'unsafe.docx'))); AddEntry $z 'duplicate.docx' ([byte[]](1)); AddEntry $z 'duplicate.docx' ([byte[]](2)) } finally {$z.Dispose()}`;
  const setupPath = path.join(dir, 'setup.ps1');
  fs.writeFileSync(setupPath, setup);
  ps(['-File', setupPath, dir]);
  const archive = path.join(dir, 'source.zip');
  const requests = ['valid', 'duplicate', 'unsafe', 'missing'].map((name) => ({ guide_id: name, archive, entry: `${name}.docx` }));
  const requestPath = path.join(dir, 'requests.json');
  fs.writeFileSync(requestPath, JSON.stringify(requests));
  const results = JSON.parse(ps(['-File', extractor, '-Requests', requestPath]));
  assert.equal(results.length, 4);
  assert.equal(results[0].priority_table_count, 1);
  assert.match(results[0].document_sha256, /^[a-f0-9]{64}$/);
  assert.match(results[0].document_xml_sha256, /^[a-f0-9]{64}$/);
  assert.deepEqual(results[0].paragraphs, [
    { table_index: 1, row_index: 2, column_index: 1, paragraph_index: 1, text: 'Build MIL\t2\nThen wait' },
    { table_index: 1, row_index: 2, column_index: 1, paragraph_index: 3, text: 'Good Job!' },
  ]);
  for (const result of results.slice(1)) assert.ok(result.error, result.guide_id);
});
