import {resolveQuery,tileTree,projection,treeMembers,swapMembers} from './model.mjs';

// Console navigation is the captured LOO-489 study. Dashboards add an independent
// selection/arrangement owner while drafts and tools stay in its Task state.
const data=tasks.map(t=>({...t,id:t.key,kind:'task',status:t.started===false?'backlog':t.paused?'paused':t.state,finished:false}));
data.push(...waves.map(id=>({id,kind:'wave',title:labelWave(id),parent:null})));
const catalog={product:'wave:product state:open',available:'state:available'};
const dashboardKey=`lf-dashboards:${repository.id}`;
let dashboards;
try{dashboards=JSON.parse(localStorage.getItem(dashboardKey))}catch{}
dashboards ||= [
 {id:'review',name:'Review desk',query:'state:available',hidden:[],tree:null},
 {id:'product-watch',name:'Product watch',query:'query:product or query:available',hidden:[],tree:null}
];
let editing=null;
const save=()=>localStorage.setItem(dashboardKey,JSON.stringify(dashboards));
const dashboard=()=>dashboards.find(d=>d.id===state.route.id);
const members=d=>resolveQuery(d.query,data,catalog);
function initialize(d){if(!d.tree)d.tree=tileTree(members(d).map(m=>m.id));return d}
function currentMembers(d){const items=members(d);initialize(d);const p=projection(d.tree,items,new Set(d.hidden));return {items,...p}}

const oldSidebar=sidebar;
sidebar=()=>oldSidebar().replace('</nav></div>',`</nav><nav aria-label="Dashboards">${section('Dashboards',dashboards.map(d=>row(d.name,'dashboard',d.id,'▦')).join(''),'<button data-new-dashboard aria-label="New Dashboard" title="New Dashboard">+</button>')}</nav></div>`);
const oldName=routeName;
routeName=()=>state.route.kind==='dashboard'?dashboard()?.name||'Dashboard':oldName();
const oldCrumbs=crumbs;
crumbs=()=>oldCrumbs()+(state.route.kind==='dashboard'?`<span class="sep">/</span><span class="dashboard-crumb">▦ &nbsp;${esc(routeName())}</span>`:'');
const oldCenter=center;
center=()=>state.route.kind==='dashboard'?dashboardCenter():oldCenter();
const oldRender=render;
render=()=>{
 oldRender();
 $('.study-label').textContent='Dashboard study';$('.study-label').removeAttribute('href');
 $('#variants').innerHTML='<span class="study-context">489 Console · sample Work</span>';
 $('#activity-example').hidden=true;
 $('#title-tabs').insertAdjacentHTML('beforeend',state.route.kind==='dashboard'?'<button class="tool-toggle" data-edit-dashboard>Edit Dashboard</button>':'');
};
const oldSearch=consoleSearchResults;
consoleSearchResults=()=>{
 oldSearch();const q=$('#query').value.trim().toLowerCase();
 const list=dashboards.filter(d=>!q||d.name.toLowerCase().includes(q));
 if(list.length)$('#results').querySelector('.empty')?.remove();
 if(list.length)$('#results').insertAdjacentHTML('beforeend','<div class="result-group">Dashboards</div>'+list.map(d=>{const index=paletteRows.push({kind:'dashboard',id:d.id})-1;return `<button data-result="${index}"><span>${esc(d.name)}<small>${esc(repository.name)} · ${esc(d.query)}</small></span><span>▦</span></button>`}).join(''));
 $('#results').querySelector(`[data-result="${highlight}"]`)?.classList.add('highlighted');
 $('#palette-label').textContent='Find Work';
};

function tile(item){
 const w=item.work,t=tasks.find(t=>t.key===w.id);
 const body=t?(t.started===false?`<div class="dashboard-brief"><small>Not started · no Session</small><h3>${esc(t.title)}</h3><p>${esc(t.prompt)}</p><button data-go="task" data-id="${t.id}">Read brief →</button></div>`:sessionPane(t).replace('id="draft"',`data-dashboard-draft="${t.id}"`)):`<div class="dashboard-brief"><h3>${esc(w.title)}</h3><p>${tasks.filter(t=>t.wave===w.id).length} Tasks in this Wave</p><button data-go="wave" data-id="${w.id}">Open Wave →</button></div>`;
 return `<section class="dashboard-tile" data-member="${esc(item.id)}"><header class="dashboard-tile-head" draggable="true" data-drag="${esc(item.id)}"><span class="drag-handle" aria-hidden="true">⠿</span><button data-go="${t?'task':'wave'}" data-id="${t?t.id:w.id}" title="Open ${esc(t?.key||w.title)} workspace"><span class="dot ${t?.state||''}"></span><span>${esc(t?.key||'Wave')} · ${esc(w.title)}</span></button><button data-hide-member="${esc(item.id)}" aria-label="Hide ${esc(t?.key||w.title)}" title="Hide this occurrence">×</button></header>${body}</section>`;
}
function dashboardCenter(){
 const d=dashboard();if(!d)return '<p class="empty">Dashboard not found.</p>';
 const {items,tree,suppressed}=currentMembers(d),visible=new Set(treeMembers(tree)),lookup=new Map(items.map(m=>[m.id,m]));
 function draw(node,path=''){
  if(!node)return '';if(node.member)return visible.has(node.member)?tile(lookup.get(node.member)):'';
  const first=draw(node.first,path+'0'),second=draw(node.second,path+'1');
  if(!first||!second)return first||second;
  return `<div class="dashboard-split axis-${node.axis}" style="--ratio:${node.ratio}" data-split-path="${path}"><div class="dashboard-branch" style="flex:${node.ratio}">${first}</div><div class="dashboard-divider" role="separator" tabindex="0" aria-label="Resize Dashboard split" aria-orientation="${node.axis==='v'?'vertical':'horizontal'}" aria-valuenow="${Math.round(node.ratio*100)}" data-resize="${path}"></div><div class="dashboard-branch" style="flex:${1-node.ratio}">${second}</div></div>`;
 }
 return `<div class="dashboard-toolbar"><code title="${esc(d.query)}">${esc(d.query)}</code><span>${visible.size} visible${suppressed.size?` · ${suppressed.size} repeated match${suppressed.size>1?'es':''} shown once`:''}</span><button data-retile title="Reset this Dashboard’s arrangement">Retile</button></div><div class="dashboard-canvas">${draw(d.tree)||'<div class="dashboard-empty"><h2>No visible work</h2><p>Restore a hidden Task or edit this Dashboard’s query.</p><button data-edit-dashboard>Edit Dashboard</button></div>'}</div>${d.hidden.length?`<div class="dashboard-hidden"><span>Hidden</span>${d.hidden.map(id=>`<button data-restore-member="${esc(id)}">＋ ${esc(lookup.get(id)?.work.id||id)}</button>`).join('')}</div>`:''}`;
}

const dialog=document.createElement('dialog');dialog.id='dashboard-editor';dialog.setAttribute('aria-label','Edit Dashboard');
dialog.innerHTML=`<header><span id="dashboard-editor-title">Edit Dashboard</span><button data-close="dashboard-editor" aria-label="Close Dashboard editor">Esc</button></header><form id="dashboard-form"><label for="dashboard-name">Name</label><input id="dashboard-name" required autocomplete="off"><label for="dashboard-query">Query</label><textarea id="dashboard-query" rows="3" spellcheck="false" required></textarea><div class="dashboard-presets"><button type="button" data-dashboard-preset="state:available">Available</button><button type="button" data-dashboard-preset="wave:product state:open">Product</button><button type="button" data-dashboard-preset="query:product or query:available">Overlapping queries</button><button type="button" data-dashboard-preset="task:LOO-330 or task:LOO-489">Include an unstarted Task</button></div><p id="dashboard-feedback" role="status"></p><details><summary>Query vocabulary</summary><p><code>state:open</code> started, unfinished Tasks<br><code>wave:product</code> Work in a Wave<br><code>task:LOO-330</code> an explicit Task, including unstarted<br><code>query:product</code> a saved expression<br>Spaces mean AND; use <code>or</code> to combine. Repository is this window.</p></details><footer><button type="button" data-close="dashboard-editor">Cancel</button><button type="submit" id="save-dashboard">Save Dashboard</button></footer></form>`;
document.body.append(dialog);
function feedback(){try{const result=resolveQuery($('#dashboard-query').value,data,catalog);$('#dashboard-feedback').textContent=`${new Set(result.map(m=>m.work.id)).size} unique Work · ${result.length} matches. Repeated Work appears only once.`;$('#dashboard-feedback').classList.remove('error');$('#save-dashboard').disabled=false;}catch(e){$('#dashboard-feedback').textContent=e.message;$('#dashboard-feedback').classList.add('error');$('#save-dashboard').disabled=true}}
function editDashboard(d){editing=d?.id||null;$('#dashboard-editor-title').textContent=d?'Edit Dashboard':'New Dashboard';$('#dashboard-name').value=d?.name||'';$('#dashboard-query').value=d?.query||'state:available';feedback();dialog.showModal();$('#dashboard-name').focus()}
$('#dashboard-query').addEventListener('input',feedback);
$('#dashboard-form').onsubmit=e=>{e.preventDefault();let d=dashboards.find(d=>d.id===editing);const query=$('#dashboard-query').value.trim();try{resolveQuery(query,data,catalog)}catch{return}if(d){if(d.query!==query){d.arrangements||={};d.arrangements[d.query]={tree:d.tree,hidden:d.hidden};const retained=d.arrangements[query];d.tree=retained?.tree||null;d.hidden=retained?.hidden||[]}d.name=$('#dashboard-name').value.trim();d.query=query;}else{d={id:crypto.randomUUID(),name:$('#dashboard-name').value.trim(),query,hidden:[],tree:null};dashboards.push(d)}save();dialog.close();navigate({kind:'dashboard',id:d.id})};

document.addEventListener('click',e=>{
 const b=e.target.closest('button');if(!b)return;const x=b.dataset,d=dashboard();
 if('newDashboard'in x)editDashboard(null);
 else if('editDashboard'in x)editDashboard(d);
 else if(x.dashboardPreset){$('#dashboard-query').value=x.dashboardPreset;feedback()}
 else if(x.hideMember&&d){d.hidden.push(x.hideMember);save();render()}
 else if(x.restoreMember&&d){d.hidden=d.hidden.filter(id=>id!==x.restoreMember);save();render()}
 else if('retile'in x&&d){d.tree=tileTree(members(d).map(m=>m.id));save();render()}
});
document.addEventListener('input',e=>{if(e.target.dataset.dashboardDraft){state.drafts[e.target.dataset.dashboardDraft]=e.target.value;persist()}});
let dragging=null;
document.addEventListener('dragstart',e=>{const el=e.target.closest('[data-drag]');if(!el)return;dragging=el.dataset.drag;e.dataTransfer.setData('text/plain',dragging);e.dataTransfer.effectAllowed='move'});
document.addEventListener('dragover',e=>{if(dragging&&e.target.closest('[data-member]'))e.preventDefault()});
document.addEventListener('drop',e=>{const target=e.target.closest('[data-member]');if(!target||!dragging)return;e.preventDefault();const d=dashboard();d.tree=swapMembers(d.tree,dragging,target.dataset.member);dragging=null;save();render()});
document.addEventListener('dragend',()=>dragging=null);
function nodeAt(path){let node=dashboard().tree;for(const side of path)node=node[side==='0'?'first':'second'];return node}
function resize(el,ratio){const node=nodeAt(el.dataset.resize);node.ratio=Math.max(.15,Math.min(.85,ratio));const split=el.parentElement;split.children[0].style.flex=node.ratio;split.children[2].style.flex=1-node.ratio;el.setAttribute('aria-valuenow',Math.round(node.ratio*100));save()}
document.addEventListener('pointerdown',e=>{const el=e.target.closest('[data-resize]');if(!el)return;e.preventDefault();el.setPointerCapture(e.pointerId);const rect=el.parentElement.getBoundingClientRect(),node=nodeAt(el.dataset.resize);const move=ev=>resize(el,node.axis==='v'?(ev.clientX-rect.left)/rect.width:(ev.clientY-rect.top)/rect.height);el.addEventListener('pointermove',move);el.addEventListener('pointerup',()=>el.removeEventListener('pointermove',move),{once:true})});
document.addEventListener('keydown',e=>{const el=e.target.closest('[data-resize]');if(el&&['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key)){e.preventDefault();resize(el,nodeAt(el.dataset.resize).ratio+(['ArrowRight','ArrowDown'].includes(e.key)?.05:-.05))}});

// Editable document drafts and shell inputs belong to Task/tab identities.
const oldFiles=filesBody,oldShell=shellBody,oldToolHeader=toolHeader;
toolHeader=(t,tool,k,many)=>{const header=oldToolHeader(t,tool,k,many);return tool==='files'?header.replace('</div><button class="pane-close"','<button class="pane-add" data-add-file aria-label="Open file tab" title="Open file tab">+</button></div><button class="pane-close"'):header};
const filePicker=document.createElement('dialog');filePicker.id='file-picker';filePicker.setAttribute('aria-label','Open file');filePicker.innerHTML='<header><span>Open file</span><button data-close="file-picker">Esc</button></header><div class="file-picker-list">'+['WorkNavigator.swift','WorkModel.swift','design.md'].map(name=>`<button data-open-file="${name}">${name}</button>`).join('')+'</div>';document.body.append(filePicker);
document.addEventListener('click',e=>{const d=e.target.closest('button')?.dataset;if(!d)return;if('addFile'in d)filePicker.showModal();if(d.openFile){const k=taskTools(currentTask());if(!k.fileTabs.includes(d.openFile))k.fileTabs.push(d.openFile);k.fileSel=k.fileTabs.indexOf(d.openFile);filePicker.close();renderCompanion()}});
filesBody=(t,k)=>{const name=k.fileTabs[k.fileSel];if(['Brief','Changes'].includes(name))return oldFiles(t,k);state.fileDrafts||={};const key=`${t.id}:${name}`;return `<textarea class="document-editor" aria-label="${esc(name)} draft" data-file-draft="${esc(key)}" spellcheck="false">${esc(state.fileDrafts[key]??'struct WorkNavigator: View {\n    // One retained workspace per Task.\n}\n')}</textarea>`};
shellBody=(t,k)=>oldShell(t,k)+`<div class="shell-input"><span>%</span><input aria-label="Shell command draft" data-shell-draft="${t.id}:${k.shellSel}" value="${esc(state.shellDrafts?.[`${t.id}:${k.shellSel}`]||'')}" placeholder="Sample shell · commands do not execute"></div>`;
document.addEventListener('input',e=>{const d=e.target.dataset;if(d.fileDraft){state.fileDrafts||={};state.fileDrafts[d.fileDraft]=e.target.value;persist()}if(d.shellDraft){state.shellDrafts||={};state.shellDrafts[d.shellDraft]=e.target.value;persist()}});
$('#reset').onclick=()=>{localStorage.removeItem(storageKey);localStorage.removeItem(dashboardKey);location.reload()};
if(!saved&&!new URLSearchParams(location.search).has('task'))state.route={kind:'dashboard',id:'review'};
render();
