const {test} = require('node:test');
const assert = require('node:assert/strict');
const {DashboardStudy} = require('./dashboards.js');

test('repo composition and current Work content survive single-work visits and removal', () => {
  const study = new DashboardStudy();
  assert.deepEqual(study.visible(), ['repo']);
  study.add('product'); study.add('task-design'); study.add('task-design');
  const retained = study.work.get('task-design');
  retained.draft = 'Keep this thought'; retained.content = 'files'; retained.split = true;
  study.move('task-design', -1);
  study.single('task-design'); study.home();
  assert.deepEqual(study.visible(), ['repo','task-design','product']);
  assert.equal(study.selected, 'task-design');
  study.remove('task-design'); study.add('task-design');
  assert.equal(study.work.get('task-design'), retained);
  assert.equal(retained.draft, 'Keep this thought');
  assert.equal(retained.content, 'files'); assert.equal(retained.split, true);
});

test('Wave query includes paused and unstarted; errors retain membership and updates preserve drafts', () => {
  const study = new DashboardStudy();
  study.add('task-theme');
  study.wave('product');
  assert.deepEqual(study.visible(), ['product','task-design','task-theme','task-input']);
  study.remove('task-input'); study.move('task-theme',-1);
  assert.deepEqual(study.visible(), ['product','task-design','task-theme','task-input']);
  study.focus('task-theme'); study.work.get('task-theme').draft='still here';
  study.unavailable=true; study.update('task-theme','completed');
  assert.ok(study.visible().includes('task-theme'));
  study.unavailable=false; study.focus(study.selected);
  assert.ok(!study.visible().includes('task-theme'));
  assert.equal(study.selected,'product');
  study.home(); assert.ok(study.visible().includes('task-theme'));
  study.update('task-theme','unstarted');
  assert.equal(study.work.get('task-theme').draft,'still here');
});

test('embedded Wave is one workspace; destination history distinguishes its dashboard and conversation', () => {
  const study = new DashboardStudy();
  study.add('product'); study.add('task-design');
  assert.deepEqual(study.visible(), ['repo','product','task-design']);
  study.wave('product'); study.focus('task-design'); study.single('product');
  study.previous();
  assert.equal(study.destination.kind,'wave'); assert.equal(study.selected,'task-design');
  study.home(); assert.deepEqual(study.visible(), ['repo','product','task-design']);
});

test('repo session stays in its dashboard and conversation-only restores the Wave conversation', () => {
  const study = new DashboardStudy();
  study.remove('repo');
  assert.deepEqual(study.visible(), ['repo']); assert.equal(study.selected, 'repo');
  study.add('product');
  study.work.get('product').content = 'files';
  study.work.get('product').fileDraft = 'Retain these notes';
  study.conversation('product');
  assert.equal(study.work.get('product').content, 'conversation');
  assert.equal(study.work.get('product').fileDraft, 'Retain these notes');
  study.home(); assert.deepEqual(study.visible(), ['repo','product']);
});
