'use strict';
// Data derived from third-party guides (benchmark corpora, reviews, probe cases)
// is never committed. It lives only on the developer's machine, in
// `.local/guide-corpus` or in the folder named by HOI4_GUIDE_CORPUS. Tests and
// audits that need it are skipped when it is absent.
const fs = require('node:fs'), path = require('node:path');
const ROOT = path.join(__dirname, '..');
const dir = path.resolve(process.env.HOI4_GUIDE_CORPUS || path.join(ROOT, '.local', 'guide-corpus'));
function file(name) { const p = path.join(dir, name); return fs.existsSync(p) ? p : null; }
/** For node:test `skip`: false when every file exists, otherwise the reason. */
function skip(...names) {
  const missing = names.filter(name => !file(name));
  return missing.length ? 'local guide corpus not available: ' + missing.join(', ') : false;
}
module.exports = { dir, file, skip };
