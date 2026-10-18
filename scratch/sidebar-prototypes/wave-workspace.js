// Fictional Work from LOO-490 study 03, adapted locally for the LOO-489 pane study.
// No API calls or execution. Membership and session samples are design evidence only.
const tasks = [
  {id:'LOO-489', title:'Compose retained Work in dashboards', state:'Needs you', sessions:['Dashboard design','Navigation exploration'], brief:'Keep a useful arrangement of Work. Visit a Task and return without losing the composition, panes or drafts.'},
  {id:'LOO-487', title:'Terminal feedback and color', state:'Working', sessions:['Contrast implementation','Native review'], brief:'Keep terminal input, status and selection clear across the native workspace.'},
  {id:'LOO-476', title:'Restore a retained workspace', state:'Paused', sessions:['Restoration investigation'], old:true, brief:'Restore useful workspace context without losing access to retained conversations.'},
  {id:'LOO-490', title:'Show a Wave’s unstarted Tasks in Desktop again', state:'Unstarted', brief:'Choose the next piece of work inside Desktop. Read its brief before deciding whether to start it.'},
  {id:'LOO-501', title:'Keep the last useful place when returning to a Wave', state:'Unstarted', brief:'Return to the work you were reading without losing your place or an unfinished draft.'},
  {id:'LOO-502', title:'Make planning failures visible where work is chosen', state:'Unstarted', brief:'Distinguish an unavailable plan from an empty plan and retain useful context.'},
  {id:'LOO-503', title:'Find Tasks by their title or identifier', state:'Unstarted', brief:'Find the next Task using the information already loaded.'},
  {id:'LOO-504', title:'Read the current chapter before its history', state:'Unstarted', brief:'Show the current plan first and keep historical chapters reachable.'},
  {id:'LOO-505', title:'Preserve drafts while switching between Work', state:'Unstarted', brief:'Leave unsent text exactly where it was when moving between a Wave and its Tasks.'},
];
const el = id => document.getElementById(id);
const escape = text => String(text).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const drafts = new Map();
let selected = null;
let viewedTask = null;
let scroll = 0;
function renderList() {
  const query = el('task-search').value.toLowerCase();
  const matches = tasks.filter(t => `${t.id} ${t.title}`.toLowerCase().includes(query));
  el('task-list').innerHTML = [
    ['With Sessions', 'All chapters', matches.filter(t => t.sessions)],
    ['Unstarted', 'Current chapter', matches.filter(t => !t.sessions)]
  ].filter(([, , rows]) => rows.length).map(([title, scope, rows]) => `<div class="task-section-title">${title} · ${rows.length}<span>${scope}</span></div>${rows.map(task => `<button class="task-row" data-open="${task.id}"><span class="row-top"><span class="id">${task.id}</span><span class="state ${task.state==='Needs you'?'waiting':task.state.toLowerCase()}">${task.state}</span></span><strong>${escape(task.title)}</strong>${task.sessions?`<small>${task.sessions.length} conversation${task.sessions.length===1?'':'s'}${task.old?' · Earlier chapter':''}</small>`:''}</button>`).join('')}`).join('') || '<p class="empty">No matching Tasks.</p>';
}
function paintSelection() {
  document.querySelectorAll('.sidebar [data-open]').forEach(button => {
    const current = button.dataset.open === selected;
    button.classList.toggle('selected',current);
    if(current) button.setAttribute('aria-current','true'); else button.removeAttribute('aria-current');
  });
}
function openTask(id) {
  const task = tasks.find(t => t.id === id);
  if (!task) return;
  if (!el('task-browser').hidden) scroll = el('task-list').scrollTop;
  selected = id;
  viewedTask = id;
  el('task-browser').hidden = true;
  el('task-detail').hidden = false;
  el('task-detail').innerHTML = `<button class="back" data-back>← Tasks</button><div class="detail-id">${task.id} · ${task.state}</div><h1>${escape(task.title)}</h1><p>${escape(task.brief)}</p>${task.sessions?`<h2>Conversations</h2>${task.sessions.map((session,index) => `<button class="conversation-row" data-conversation="${index}">${escape(session)}<small>${index===0?task.state:'Retained conversation'}</small></button>`).join('')}`:'<h2>Not started</h2><p>This Task has no conversation yet. Reading its brief does not start execution.</p>'}`;
  paintSelection();
}
function showTasks() {
  el('task-detail').hidden=true;
  el('task-browser').hidden=false;
  el('task-list').scrollTop=scroll;
}
function openConversation(index) {
  const task = tasks.find(t=>t.id===viewedTask);
  const key = `${task.id}:${index}`;
  el('task-detail').innerHTML = `<button class="back" data-open="${task.id}">← ${task.id}</button><div class="detail-id">${task.id} · Conversation</div><h1>${escape(task.sessions[index])}</h1><p>${escape(task.brief)}</p><textarea class="detail-draft" data-draft="${key}" aria-label="${escape(task.sessions[index])} draft" placeholder="Continue this conversation…">${escape(drafts.get(key)||'')}</textarea><p>The Wave session stays beside this conversation.</p>`;
}
function setRatio(value) {
  const ratio=Math.max(40,Math.min(78,value));
  el('split').style.setProperty('--session-width',`${ratio}%`);
  el('divider').setAttribute('aria-valuenow',String(Math.round(ratio)));
}
document.addEventListener('click', event => {
  const button=event.target.closest('button');
  if(!button) return;
  if(button.dataset.open) openTask(button.dataset.open);
  if(button.hasAttribute('data-back')) showTasks();
  if(button.hasAttribute('data-conversation')) openConversation(Number(button.dataset.conversation));
});
document.addEventListener('input', event => {
  if(event.target.dataset.draft) drafts.set(event.target.dataset.draft,event.target.value);
});
el('task-search').addEventListener('input',renderList);
el('wave-home').addEventListener('click',()=>{selected=null;showTasks();paintSelection();});
el('focus-session').addEventListener('click',()=>el('wave-draft').focus());
el('wave-draft').addEventListener('focus',()=>{selected=null;paintSelection();});
el('reset-layout').addEventListener('click',()=>setRatio(61.8));
el('divider').addEventListener('keydown',event=>{
  if(!['ArrowLeft','ArrowRight','Home'].includes(event.key)) return;
  event.preventDefault();
  setRatio(event.key==='Home'?61.8:Number(el('divider').getAttribute('aria-valuenow'))+(event.key==='ArrowLeft'?-2:2));
});
el('divider').addEventListener('pointerdown',event=>{
  el('divider').setPointerCapture(event.pointerId);
});
el('divider').addEventListener('pointermove',event=>{
  if(!el('divider').hasPointerCapture(event.pointerId)) return;
  const bounds=el('split').getBoundingClientRect();
  setRatio((event.clientX-bounds.left)/bounds.width*100);
});
el('divider').addEventListener('pointerup',event=>{
  if(el('divider').hasPointerCapture(event.pointerId)) el('divider').releasePointerCapture(event.pointerId);
});
renderList();
