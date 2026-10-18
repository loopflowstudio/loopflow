const {test}=require('node:test');
const assert=require('node:assert/strict');
const Study=require('./model.js');

test('return trail restores the exact query and preserves Task draft and pane',()=>{
  const study=new Study('navigation','b');
  study.visit({kind:'query',id:'Available'});study.scrolls['query:Available']=73;
  const task=study.task('489'),pane=task.pane;
  study.visit({kind:'task',id:'489'});task.draft='Keep this thought';
  study.visit({kind:'task',id:'497'});study.back();
  assert.equal(study.task(study.route.id),task);assert.equal(task.pane,pane);assert.equal(task.draft,'Keep this thought');
  study.back();assert.deepEqual(study.route,{kind:'query',id:'Available'});assert.equal(study.scrolls[study.key()],73);
  study.ahead();assert.equal(study.route.id,'489');assert.equal(study.task('489'),task);
});
test('alongside keeps the originating Wave and closes back to it',()=>{
  const study=new Study('navigation','c'),task=study.task('489');
  study.visit({kind:'task',id:'489'},true);task.draft='Unsent';
  study.visit({kind:'task',id:'497'},true);
  assert.deepEqual(study.beside,{kind:'wave',id:'Product'});
  study.closeBeside();assert.deepEqual(study.route,{kind:'wave',id:'Product'});
  assert.equal(study.beside,null);assert.equal(study.task('489'),task);assert.equal(task.draft,'Unsent');
});
for(const variant of ['a','b','c'])test(`arrangement ${variant} preserves survivors, hidden membership and drafts through churn`,()=>{
  const study=new Study('arrangement',variant),task=study.task('487');task.draft='Retain on departure';
  study.move('497','489');const before=study.ordered().map(t=>t.id);
  study.arrive();study.flipAttention();
  assert.deepEqual(study.ordered().filter(t=>t.id!=='506').map(t=>t.id),before);
  assert.deepEqual(study.inbox,['506']);study.hide('489');
  assert(study.members().some(t=>t.id==='489'));assert(!study.ordered().some(t=>t.id==='489'));
  study.restore('489');assert.equal(study.ordered()[1].id,'489');
  study.depart();assert(!study.members().includes(task));study.depart();
  assert(study.members().includes(task));assert.equal(study.task('487'),task);assert.equal(task.draft,'Retain on departure');
});
test('attention sorts an untouched layout without changing membership',()=>{
  const study=new Study(),ids=study.members().map(t=>t.id);study.flipAttention();
  assert.deepEqual(study.members().map(t=>t.id),ids);assert.equal(study.ordered()[2].id,'453');
});
test('selected combination freezes attention size and order, appends arrivals and retains drafts on breadcrumb return',()=>{
  const study=new Study('combined','a');
  study.flipAttention();
  const ordered=study.ordered().map(t=>t.id);
  study.move('497','489');
  assert.deepEqual(study.ordered().map(t=>t.id),['497','489',...ordered.slice(2)]);
  assert(study.prominent('453'));
  study.flipAttention();
  assert(study.prominent('453'));
  const frozen=study.ordered().map(t=>t.id);
  study.arrive();
  assert.deepEqual(study.ordered().map(t=>t.id),[...frozen,'506']);
  assert(!study.prominent('506'));
  const task=study.task('489');task.draft='Selected A / B / A';
  study.visit({kind:'task',id:'489'});study.visit({kind:'wave',id:'Product'});
  assert.equal(study.task('489'),task);assert.equal(task.draft,'Selected A / B / A');
  assert.deepEqual(study.ordered().map(t=>t.id),[...frozen,'506']);
  study.resetArrangement();assert(!study.prominent('453'));
});
