import {initialWork,presets,savedQueries,openTask,priority,resolveQuery,tileTree,treeMembers,projection,reconcile,validateTree,swapMembers} from './model.mjs';

const $=id=>document.getElementById(id);
const escape=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
let work=structuredClone(initialWork),catalog={...savedQueries},viewCache=new Map(),pinned=[],taskState=new Map(),activeTask=null,activeView;
let toastTimer;
const labels={available:'Needs you',running:'Working',paused:'Paused',idle:'Idle',backlog:'Not started',done:'Complete',blocked:'Blocked'};

function taskDetails(id){if(!taskState.has(id))taskState.set(id,{draft:'',shell:false,zoom:null,focus:'session'});return taskState.get(id);}
function notify(text){$('toast').textContent=text;$('toast').hidden=false;clearTimeout(toastTimer);toastTimer=setTimeout(()=>$('toast').hidden=true,3500);}
function useView(spec){
  activeTask=null;
  if(!viewCache.has(spec.query)){
    const members=resolveQuery(spec.query,work,catalog);
    viewCache.set(spec.query,{...spec,members,tree:tileTree(members.map(m=>m.id)),hidden:new Set(),touched:false});
  }
  activeView=viewCache.get(spec.query);activeView.name=spec.name;activeView.scope=spec.scope||'repo';
  reconcile(activeView,resolveQuery(activeView.query,work,catalog));
  syncEditor();render();
}
function syncEditor(){
  $('query').value=activeView.query;$('view-name').value=activeView.name;previewQuery();
}
function previewQuery(){
  try{const matches=resolveQuery($('query').value,work,catalog),unique=new Set(matches.map(m=>m.work.id));
    $('query-feedback').className='';$('query-feedback').textContent=`${unique.size} Work items${matches.length>unique.size?` · ${matches.length-unique.size} overlapping matches`:''}`;$('apply-query').disabled=false;
  }catch(error){$('query-feedback').className='error';$('query-feedback').textContent=error.message;$('apply-query').disabled=true;}
}
function selectionInfo(){
  const projected=projection(activeView.tree,activeView.members,activeView.hidden);
  const count=treeMembers(projected.tree).length;
  $('selection-reading').innerHTML=`<strong>${activeView.members.length} matches</strong> in this selection<br><strong>${count} visible</strong> · ${activeView.hidden.size} hidden${projected.suppressed.size?` · ${projected.suppressed.size} overlapping`:''}${projected.suppressed.size?'<br>First visible occurrence wins.':''}`;
  $('arrangement-note').textContent=activeView.touched?'Your arrangement is retained. New matches enter a strip on the right.':'Automatically tiled. Move a tile or resize a divider to keep your arrangement.';
  if(document.activeElement!==$('tree-input'))$('tree-input').value=JSON.stringify(activeView.tree,null,2);
  $('pin-view').textContent=pinned.some(p=>p.query===activeView.query)?'✓ View pinned':'＋ Pin this view';
  $('auto-layout').disabled=!!activeTask;
}
function renderSidebar(){
  $('sidebar').hidden=!!activeTask;
  const scope=activeView.scope;
  const tasks=work.filter(w=>openTask(w)&&(scope==='repo'||w.wave===scope)).sort((a,b)=>priority(a)-priority(b));
  const waves=work.filter(w=>w.kind==='wave'&&(scope==='repo'?!w.parent:w.parent===scope));
  $('sidebar').innerHTML=`<section><div class="nav-heading"><span class="eyebrow">OPEN TASKS</span><span class="eyebrow">${tasks.length}</span></div>${tasks.map(w=>`<button class="nav-row" data-task="${w.id}"><i class="dot ${w.status}"></i><span class="nav-label"><span>${escape(w.title)}</span><small>${w.id} · ${labels[w.status]}</small></span></button>`).join('')||'<p class="sidebar-foot">No open Tasks in this Wave.</p>'}</section>${waves.length?`<section><div class="nav-heading"><span class="eyebrow">WAVES</span><span class="eyebrow">${waves.length}</span></div>${waves.map(w=>`<button class="nav-row" data-wave="${w.id}"><span class="wave-symbol">≈</span><span class="nav-label"><span>${w.title}</span><small>${work.filter(t=>openTask(t)&&t.wave===w.id).length} open Tasks</small></span></button>`).join('')}</section>`:''}<section><div class="nav-heading"><span class="eyebrow">PINNED VIEWS</span></div><button class="nav-row ${activeView.query===presets.attention.query?'selected':''}" data-preset="attention"><span>◇</span><span class="nav-label"><span>Needs you</span><small>Across all Waves</small></span></button>${pinned.map((p,i)=>`<button class="nav-row ${p.query===activeView.query?'selected':''}" data-pin="${i}"><span>◇</span><span class="nav-label"><span>${escape(p.name)}</span></span></button>`).join('')}</section><div class="sidebar-foot">A view is a selection of Work.<br>Your conversations stay with their Tasks.</div>`;
}
function renderBreadcrumbs(){
  const crumbs=[`<button data-home>loopflow</button>`];
  if(activeView.name!=='Repository')crumbs.push(`<span class="crumb-sep">›</span><button data-parent>${escape(activeView.name)}</button>`);
  if(activeTask){if(activeView.name==='Repository')crumbs.push('<span class="crumb-sep">›</span><button data-parent>Open Work</button>');crumbs.push(`<span class="crumb-sep">›</span><button>${escape(activeTask.id)}</button>`);}
  $('breadcrumbs').innerHTML=crumbs.join('');
}
function tileContent(member){
  const w=member.work;
  const tools=`<div class="tile-tools"><button data-move="${member.id}" aria-label="Move ${escape(w.title)} earlier" title="Move earlier">←</button><button data-hide="${member.id}" aria-label="Hide ${escape(w.title)}" title="Hide; keep in selection">−</button></div>`;
  if(w.kind==='wave')return `<article class="tile wave-tile" data-member="${member.id}"><header class="tile-header"><span class="wave-symbol">≈</span><button class="tile-title" data-wave="${w.id}"><strong>${w.title}</strong><small>WAVE</small></button>${tools}</header><div class="wave-body"><h3>${w.title}</h3><p>${w.description}</p><button data-wave="${w.id}">Open Wave →</button></div><footer class="tile-foot"><span>${work.filter(t=>openTask(t)&&t.wave===w.id).length} open Tasks</span><span>Work reference</span></footer></article>`;
  const state=taskDetails(w.id),shellFace=state.zoom==='shell';
  return `<article class="tile" data-member="${member.id}"><header class="tile-header"><i class="dot ${w.status}"></i><button class="tile-title" data-task="${w.id}" title="Enter ${escape(w.title)}"><strong>${escape(w.title)}</strong><small>${w.id} · ${w.wave}</small></button>${tools}</header><div class="session-body">${shellFace?`<div class="session-label">Shell <span>Explicitly zoomed</span></div><div class="reply">❯ git status<br>On branch ${escape(w.branch)}<br><br>working tree clean<br><br>❯ <span class="muted">Same Task, another pane.</span></div>`:`<div class="session-label">${w.provider||'Task brief'} <span>${labels[w.status]}</span></div><p class="user-line">${escape(w.request)}</p><p class="reply">${escape(w.reply)}</p><p class="tool-line">${escape(w.tool)}</p>`}</div><footer class="tile-foot"><span>${labels[w.status]}</span><span class="face">${shellFace?'ZOOMED SHELL':'PRIMARY SESSION'}</span></footer></article>`;
}
function renderTree(tree,lookup){
  if(!tree)return '';
  if(tree.member)return tileContent(lookup.get(tree.member));
  const firstIds=treeMembers(tree.first).join(','),secondIds=treeMembers(tree.second).join(',');
  return `<div class="layout-node ${tree.axis}"><div class="layout-side" style="flex:${tree.ratio}">${renderTree(tree.first,lookup)}</div><div class="divider" role="separator" tabindex="0" aria-label="Resize split" aria-orientation="${tree.axis==='v'?'vertical':'horizontal'}" aria-valuemin="10" aria-valuemax="90" aria-valuenow="${Math.round(tree.ratio*100)}" data-first="${firstIds}" data-second="${secondIds}" data-axis="${tree.axis}"></div><div class="layout-side" style="flex:${1-tree.ratio}">${renderTree(tree.second,lookup)}</div></div>`;
}
function renderView(){
  const projected=projection(activeView.tree,activeView.members,activeView.hidden),lookup=new Map(activeView.members.map(m=>[m.id,m]));
  $('main').innerHTML=`<header class="view-heading"><div><h1>${escape(activeView.name)}</h1><p>${treeMembers(projected.tree).length} visible Work items${projected.suppressed.size?' · overlapping matches shown once':''}</p></div><span class="mode-label">${activeView.touched?'ARRANGED':'AUTO'}</span></header><div class="query-caption">${escape(activeView.query)}</div><div class="grid-area">${projected.tree?renderTree(projected.tree,lookup):`<div class="empty"><h2>${activeView.members.length?'Your Work is hidden.':'No Work matches yet.'}</h2><p>${activeView.members.length?'Restore a member below to bring it back.':'Change the query to choose another part of the repository.'}</p><button data-edit>Edit selection</button></div>`}</div>${activeView.hidden.size?`<div class="hidden-tray"><span>Hidden · still in this selection</span>${[...activeView.hidden].map(id=>`<button data-restore="${id}">＋ ${escape(lookup.get(id)?.work.title||id)}</button>`).join('')}</div>`:''}`;
  bindDividers();
  $('status').textContent=`${activeView.members.length} matches · ${treeMembers(projected.tree).length} visible${activeView.hidden.size?` · ${activeView.hidden.size} hidden`:''}${projected.suppressed.size?` · ${projected.suppressed.size} overlapping`:''} · ${activeView.touched?'Your arrangement':'Automatic layout'}`;
}
function renderTask(){
  const w=activeTask,state=taskDetails(w.id);
  $('main').innerHTML=`<div class="task-view"><header class="view-heading"><div><h1>${escape(w.title)}</h1><p>${w.id} · ${w.wave} · ${labels[w.status]}</p></div><div class="task-actions"><button data-parent>← Back to view</button><button id="add-shell" ${state.shell?'disabled':''}>＋ Shell</button></div></header><div class="task-panes">${state.zoom!=='shell'?`<article class="tile ${state.focus==='session'?'pane-focused':''}"><header class="tile-header"><i class="dot ${w.status}"></i><div class="tile-title"><strong>${w.provider||'Task brief'}</strong><small>Primary Session</small></div><span class="saved-tag">${w.id}</span></header><div class="session-body"><p class="session-label">loopflow.${escape(w.branch||'unstarted')} · sample conversation</p><p class="user-line">${escape(w.request)}</p><p class="reply">${escape(w.reply)}</p><p class="tool-line">${escape(w.tool)}</p></div><div class="composer"><span>›</span><textarea id="task-draft" aria-label="Draft for ${escape(w.title)}" placeholder="Leave a draft, then return to the view…">${escape(state.draft)}</textarea></div><footer class="tile-foot"><span>Draft retained in this study</span><span>Nothing is sent</span></footer></article>`:''}${state.shell?`<article class="tile ${state.focus==='shell'?'pane-focused':''}"><header class="tile-header"><button id="focus-shell" class="tile-title"><strong>Shell</strong><small>Task checkout</small></button><button id="zoom-shell" class="${state.zoom==='shell'?'zoom-active':''}" title="Use this pane as the Task tile face">${state.zoom==='shell'?'↙ Unzoom':'↗ Zoom'}</button></header><div class="shell-body" tabindex="0" id="shell-area"><p class="prompt">❯ git status</p><p>On branch ${escape(w.branch||'unstarted')}</p><p>nothing to commit, working tree clean</p><p class="prompt">❯</p><p class="muted">Focusing this shell leaves the tile on the primary Session. Zoom it to change the tile face.</p></div><footer class="tile-foot"><span>Sample output</span><span>${state.zoom?'Tile face: shell':'Tile face: primary'}</span></footer></article>`:''}</div></div>`;
  $('status').textContent='One Task layout · one retained draft · return to the same view';
  $('add-shell')?.addEventListener('click',()=>{state.shell=true;state.focus='shell';render();});
  $('focus-shell')?.addEventListener('click',()=>{state.focus='shell';render();});
  $('shell-area')?.addEventListener('focus',()=>{state.focus='shell';});
  $('zoom-shell')?.addEventListener('click',()=>{state.zoom=state.zoom?null:'shell';render();});
  $('task-draft')?.addEventListener('input',e=>state.draft=e.target.value);
}
function render(){renderBreadcrumbs();renderSidebar();activeTask?renderTask():renderView();selectionInfo();}
function enterTask(id){activeTask=work.find(w=>w.id===id);render();}
function enterWave(id){const wave=work.find(w=>w.id===id);useView({name:wave.title,query:`wave:${id} (kind:wave or state:open)`,scope:id});}
function resizeRetained(first,second,ratio){
  const a=first.split(',')[0],b=second.split(',')[0];
  function find(node){if(!node||node.member)return false;const left=treeMembers(node.first),right=treeMembers(node.second);if(left.includes(a)&&right.includes(b)){node.ratio=ratio;return true;}return find(node.first)||find(node.second);}
  if(find(activeView.tree)){
    activeView.touched=true;
    document.querySelector('.mode-label').textContent='ARRANGED';
    $('status').textContent=$('status').textContent.replace('Automatic layout','Your arrangement');
  }
}
function bindDividers(){
  document.querySelectorAll('.divider').forEach(divider=>{
    const update=ratio=>{ratio=Math.max(.1,Math.min(.9,ratio));divider.previousElementSibling.style.flex=ratio;divider.nextElementSibling.style.flex=1-ratio;divider.setAttribute('aria-valuenow',Math.round(ratio*100));resizeRetained(divider.dataset.first,divider.dataset.second,ratio);};
    divider.addEventListener('pointerdown',e=>{
      e.preventDefault();divider.setPointerCapture(e.pointerId);const rect=divider.parentElement.getBoundingClientRect();
      const move=event=>update(divider.dataset.axis==='v'?(event.clientX-rect.left)/rect.width:(event.clientY-rect.top)/rect.height);
      const end=()=>{divider.removeEventListener('pointermove',move);divider.removeEventListener('pointerup',end);render();};
      divider.addEventListener('pointermove',move);divider.addEventListener('pointerup',end);
    });
    divider.addEventListener('keydown',e=>{if(['ArrowLeft','ArrowUp','ArrowRight','ArrowDown'].includes(e.key)){e.preventDefault();update(Number(divider.getAttribute('aria-valuenow'))/100+(['ArrowLeft','ArrowUp'].includes(e.key)?-.05:.05));selectionInfo();}});
  });
}
function toggleEditor(open=!$('editor').hidden){$('editor').hidden=!open;$('edit-button').setAttribute('aria-expanded',String(open));if(open){syncEditor();$('query').focus();}}

document.addEventListener('click',e=>{
  const button=e.target.closest('button');if(!button)return;
  if(button.hasAttribute('data-home'))useView(presets.repo);
  else if(button.hasAttribute('data-parent')){activeTask=null;render();}
  else if(button.hasAttribute('data-edit'))toggleEditor(true);
  else if(button.dataset.task)enterTask(button.dataset.task);
  else if(button.dataset.wave)enterWave(button.dataset.wave);
  else if(button.dataset.preset)useView(presets[button.dataset.preset]);
  else if(button.dataset.pin!==undefined)useView(pinned[Number(button.dataset.pin)]);
  else if(button.dataset.hide){activeView.hidden.add(button.dataset.hide);render();notify('Hidden. Still a member of this selection.');}
  else if(button.dataset.restore){activeView.hidden.delete(button.dataset.restore);render();}
  else if(button.dataset.move){const visible=treeMembers(projection(activeView.tree,activeView.members,activeView.hidden).tree);const at=visible.indexOf(button.dataset.move);if(at>0){activeView.tree=swapMembers(activeView.tree,visible[at],visible[at-1]);activeView.touched=true;render();}else notify('Already the first visible tile.');}
});
$('query').addEventListener('input',previewQuery);
$('apply-query').addEventListener('click',()=>{
  try{resolveQuery($('query').value,work,catalog);useView({name:$('view-name').value.trim()||'Untitled view',query:$('query').value.trim(),scope:'repo'});notify('Selection updated. Retained Task layouts stay with their Work.');}catch(error){notify(error.message);}
});
$('edit-button').addEventListener('click',()=>toggleEditor($('editor').hidden));
$('close-editor').addEventListener('click',()=>toggleEditor(false));
$('auto-layout').addEventListener('click',()=>{activeView.touched=false;reconcile(activeView,resolveQuery(activeView.query,work,catalog));render();notify('Automatic tiling restored. Hidden members stay hidden.');});
$('apply-tree').addEventListener('click',()=>{try{const tree=JSON.parse($('tree-input').value);validateTree(tree,activeView.members.map(m=>m.id));activeView.tree=tree;activeView.touched=true;$('tree-error').textContent='';render();notify('Arrangement applied. Membership is unchanged.');}catch(error){$('tree-error').textContent=error.message;}});
$('pin-view').addEventListener('click',()=>{if(!pinned.some(p=>p.query===activeView.query)){pinned.push({name:activeView.name,query:activeView.query,scope:activeView.scope});renderSidebar();selectionInfo();notify('Pinned in this study.');}});
$('activity-button').addEventListener('click',()=>$('activity').hidden=!$('activity').hidden);
function changeSample(id,change){Object.assign(work.find(w=>w.id===id),change);for(const view of viewCache.values())reconcile(view,resolveQuery(view.query,work,catalog));$('activity').hidden=true;render();previewQuery();}
$('start-sample').addEventListener('click',()=>{changeSample('LF-108',{started:true,status:'running',provider:'Codex',branch:'empty-state',reply:'This sample Task just started.\n\nIt now matches state:open. Your existing arrangement stays intact.',tool:'Working · new match'});notify('Sample Task started. Open selections now include it.');});
$('finish-sample').addEventListener('click',()=>{changeSample('LF-103',{finished:true,status:'done',tool:'Completed'});notify('Sample Task completed. It leaves state:open selections.');});
$('reset').addEventListener('click',()=>{work=structuredClone(initialWork);catalog={...savedQueries};viewCache=new Map();pinned=[];taskState=new Map();activeTask=null;useView(presets.repo);toggleEditor(true);notify('Study reset.');});
document.addEventListener('keydown',e=>{if((e.metaKey||e.ctrlKey)&&e.key.toLowerCase()==='k'){e.preventDefault();toggleEditor(true);}if((e.metaKey||e.ctrlKey)&&e.key==='Enter'&&!$('editor').hidden){e.preventDefault();$('apply-query').click();}if(e.key==='Escape'){if(!$('activity').hidden)$('activity').hidden=true;else if(activeTask){activeTask=null;render();}else toggleEditor(false);}});
useView(presets.repo);
