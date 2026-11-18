'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),path=require('node:path');
const {plan}=require('../scripts/dev-native.cjs');
test('fast native builds isolate optimized incremental outputs from release and CI',()=>{
 const p=plan(['build','backend','--bin','hoi4-backend'],{CARGO_TARGET_DIR:'elsewhere',CARGO_PROFILE_RELEASE_LTO:'true'});
 assert.match(p.env.CARGO_TARGET_DIR,/rust-dev-backend-target$/);assert.equal(p.env.CARGO_PROFILE_RELEASE_LTO,'false');assert.equal(p.env.CARGO_PROFILE_RELEASE_INCREMENTAL,'true');assert.ok(p.args.includes('--release'));assert.ok(p.args.includes('--locked'));
 assert.match(plan(['test','parser']).env.CARGO_TARGET_DIR,/rust-dev-core-target$/);
 assert.match(plan(['build','desktop']).env.CARGO_TARGET_DIR,/rust-dev-desktop-target$/);
 assert.deepEqual(p.args.slice(-2),['--bin','hoi4-backend']);
 // Only backend TESTS build the backend crate unoptimized, in their own cache;
 // the backend build installed by -Dev stays optimized.
 assert.ok(!p.args.includes('--config'));
 const t=plan(['test','backend','--lib']);
 assert.match(t.env.CARGO_TARGET_DIR,/rust-dev-backend-tests-target$/);
 assert.ok(t.args.includes('profile.release.package.hoi4companion-backend.opt-level=0'));
 assert.ok(plan(['test','guides']).args.includes('profile.release.package.hoi4companion-guides.opt-level=0'));
 assert.match(plan(['test','model']).env.CARGO_TARGET_DIR,/rust-dev-backend-tests-target$/);
 assert.ok(!plan(['test','overlay']).args.includes('--config'));
 for(const arg of ['--target-dir=elsewhere','--profile','--release','--config=foo'])assert.throws(()=>plan(['build','backend',arg]));
 assert.throws(()=>plan(['build','../backend']));
});
