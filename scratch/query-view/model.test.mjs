import test from 'node:test';
import assert from 'node:assert/strict';
import {initialWork, presets, resolveQuery, tileTree, treeMembers, projection, reconcile, validateTree, swapMembers} from './model.mjs';

test('defaults retain paused and idle Tasks, exclude backlog/completed, and include Waves', () => {
  const result = resolveQuery(presets.repo.query, initialWork);
  assert.equal(result.length, 9);
  assert.deepEqual(result.filter(m => m.work.kind === 'task').map(m => m.work.id),
    ['LF-101', 'LF-104', 'LF-102', 'LF-103', 'LF-105', 'LF-106']);
  assert.equal(resolveQuery('task:LF-108', initialWork)[0].work.started, false);
});

test('overlap retains membership and hiding the winner promotes the next occurrence', () => {
  const members = resolveQuery(presets.overlap.query, initialWork);
  const tree = tileTree(members.map(m => m.id));
  const original = structuredClone(tree);
  const duplicates = members.filter(m => m.work.id === 'LF-101');
  assert.equal(members.length, 5);
  assert.equal(treeMembers(projection(tree, members, new Set()).tree).length, 4);
  const hidden = new Set([duplicates[0].id]);
  const shown = projection(tree, members, hidden);
  assert.ok(treeMembers(shown.tree).includes(duplicates[1].id));
  assert.equal(shown.suppressed.size, 0);
  assert.deepEqual(tree, original);
});

test('restoring after an arrangement edit retains newer order and split ratio', () => {
  const members = resolveQuery(presets.product.query, initialWork);
  const ids = members.map(m => m.id);
  const tree = tileTree(ids);
  const hidden = new Set([ids[0]]);
  projection(tree, members, hidden);
  const edited = swapMembers(tree, ids[1], ids[2]);
  edited.ratio = .7;
  hidden.clear();
  const restored = projection(edited, members, hidden).tree;
  assert.equal(restored.ratio, .7);
  assert.deepEqual(treeMembers(restored), [ids[0], ids[2], ids[1]]);
  assert.throws(() => validateTree(tileTree(ids.slice(1)), ids), /every member/);
});

test('new matches append to touched layout without replacing its geometry', () => {
  const members = resolveQuery(presets.product.query, initialWork);
  const tree = tileTree(members.map(m => m.id));
  tree.ratio = .65;
  const view = {tree, members, hidden:new Set([members[0].id]), touched:true};
  const changed = structuredClone(initialWork);
  changed.find(w => w.id === 'LF-108').started = true;
  reconcile(view, resolveQuery(presets.product.query, changed));
  assert.deepEqual(view.tree.first, tree);
  assert.equal(view.members.length, 4);
  assert.ok(view.hidden.has(members[0].id));
});

test('Wave scope includes descendant Tasks and catches recursive saved queries', () => {
  const work = [...initialWork,
    {id:'child', kind:'wave', parent:'product'},
    {id:'LF-200', kind:'task', wave:'child', started:true, finished:false, status:'idle'}];
  const result = resolveQuery('wave:product (kind:wave or state:open)', work);
  assert.ok(result.some(m => m.work.id === 'child'));
  assert.ok(result.some(m => m.work.id === 'LF-200'));
  assert.throws(() => resolveQuery('query:a', work, {a:'query:b', b:'query:a'}), /themselves/);
  assert.throws(() => resolveQuery('wave:(', work), /Expected a filter/);
});
