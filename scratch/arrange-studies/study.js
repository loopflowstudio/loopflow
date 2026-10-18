'use strict';
const params=new URLSearchParams(location.search);
const set=['combined','navigation','geometry','arrangement'].includes(params.get('set'))?params.get('set'):'combined';
const variant=set!=='combined'&&['a','b','c'].includes(params.get('variant'))?params.get('variant'):'a';
const arranged=set==='arrangement'||set==='combined';
let model=new ArrangeStudy(set,variant);
const definitions={
  combined:{number:'SELECTED',title:'Your chosen direction, together',description:'01 A Breadcrumbs · 02 B Attention first · 03 A At the end',
    names:['Selected combination'],hypothesis:['Enter a Task. Return to the same arrangement.'],
    exercise:['Arrange the grid, change attention, add a match, then enter and return.'],
    tradeoff:['Breadcrumbs return through the hierarchy; ⌘[ follows visit history back to Available. Attention sizes an untouched grid. After arranging, sizes and order stay put; new matches append at the end.']},
  navigation:{number:'01',title:'How do you return to your place?',description:'One grid. Three ways to enter a Task and keep your bearings.',
    names:['Breadcrumbs','Return trail','Open alongside'],
    hypothesis:['Hierarchy is enough.','The route back remembers where you came from.','Keep the originating view in sight.'],
    exercise:['Try Available → a Task → its Wave crumb.','Try Available → a Task → ⌘K → another Task → Return.','Try Open alongside, then another Task in the grid, then Close.'],
    tradeoff:['The hierarchy stays quiet and predictable. Returning to a Wave does not recover a cross-Wave query; Back still follows visit history with ⌘[.','An explicit return control preserves query context and Task-to-Task visits, but adds another navigation concept beside the hierarchy.','The source stays visible, at the cost of terminal width. The Task occupies one live position; its source tile points to that panel.']},
  geometry:{number:'02',title:'How much of each Task should you see?',description:'Identical navigation and content. Only tile proportions change.',
    names:['Equal tiles','Attention first','Reading rows'],
    hypothesis:['Equal space makes every Task easy to find.','Give waiting Tasks more room to explain themselves.','Favor readable output over simultaneous coverage.'],
    exercise:['Scan the Wave, enter a Task, return by breadcrumb.','Compare what the waiting Tasks reveal before you enter.','Read the output, then scroll to find a distant Task.'],
    tradeoff:['Dense and stable, with comparable visual weight. Long output is clipped and usually requires entering a Task.','Waiting Tasks communicate more, while working Tasks remain compact. Changing attention can move more of an untouched grid.','Terminal lines are readable without entering. Fewer Tasks fit above the fold, so activity is easier to miss farther down.']},
  arrangement:{number:'03',title:'Where should a new matching Task arrive?',description:'Same membership and retained order. Only the arrival region changes.',
    names:['At the end','Inbox rail','Inbox above'],
    hypothesis:['New matches join at the trailing edge.','Give new matches a dedicated side rail.','Put new matches in a visible inbox above the layout.'],
    exercise:['Swap two Tasks, add a match, hide/restore one.','Swap two Tasks, add a match, then integrate it.','Swap two Tasks, add a match, then integrate it.'],
    tradeoff:['The original geometry stays stable and the layout stays simple. New work can arrive below the visible area.','Arrival is visible without displacing the pinned sequence vertically. Reserving rail width makes existing terminals narrower.','New work is hard to miss, but the inbox pushes the retained grid down. Integrating it is an explicit arrangement action.']}
};
const d=definitions[set],vi='abc'.indexOf(variant);
const $=id=>document.getElementById(id);
const esc=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const routeButton=(label,kind,id,extra='')=>`<button data-action="visit" data-kind="${kind}" data-id="${esc(id)}" ${extra}>${esc(label)}</button>`;
$('sets').innerHTML=Object.entries(definitions).map(([key,v])=>`<a class="${set===key?'selected':''}" href="?set=${key}&variant=a">${key==='combined'?'Selected':v.number+' '+(key==='navigation'?'Navigation':key==='geometry'?'Tile layout':'New arrivals')}</a>`).join('');
$('variants').hidden=set==='combined';
$('variants').innerHTML=d.names.map((name,i)=>`<a class="${vi===i?'selected':''}" href="?set=${set}&variant=${'abc'[i]}" ${vi===i?'aria-current="page"':''}><b>${'ABC'[i]}</b>${name}</a>`).join('');
$('question-number').textContent=set==='combined'?'ACCEPTED CHOICES · INTERACTION STUDY':`QUESTION ${d.number} / 03`;$('question').textContent=d.title;
$('description').textContent=d.description;$('hypothesis').textContent=d.hypothesis[vi];$('exercise').textContent=d.exercise[vi];$('tradeoff').textContent=d.tradeoff[vi];
document.querySelector('.study-footer>span').textContent=set==='combined'?'Selected choices combined. Sample Work; native integration remains.':'Shared Tasks, pane content and colors. Only the named decision changes.';
document.title=`Loopflow · ${d.names[vi]} · ${set}`;
function saveScroll(){document.querySelectorAll('[data-scroll]').forEach(el=>model.scrolls[el.dataset.scroll]=el.scrollTop);}
function restoreScroll(){document.querySelectorAll('[data-scroll]').forEach(el=>el.scrollTop=model.scrolls[el.dataset.scroll]||0);}
function sidebar(){
  const route=model.route;
  $('sidebar').hidden=route.kind==='task';if(route.kind==='task')return;
  const tasks=model.members(route.kind==='repo'?{kind:'repo',id:'loopflow'}:route).sort((a,b)=>(a.status==='Waiting'?0:1)-(b.status==='Waiting'?0:1)||model.tasks.indexOf(a)-model.tasks.indexOf(b));
  $('sidebar').innerHTML=`<div class="side-heading"><span>${route.kind==='repo'?'Open Tasks':route.kind==='query'?'Matching Tasks':'Tasks in '+esc(route.id)}</span><span>${tasks.length}</span></div>`+
    tasks.map(t=>`<button class="side-row ${model.lastTask===t.id?'selected':''}" data-action="task" data-id="${t.id}"><i class="dot ${t.status.toLowerCase()}"></i><span class="row-copy"><strong>${esc(t.title)}</strong><small>LOO-${t.id} · ${model.hidden.has(t.id)?'Hidden tile':route.kind==='repo'||route.kind==='query'?t.wave:t.status}</small></span></button>`).join('')+
    (route.kind==='repo'?`<div class="side-heading pinned">Waves</div>${routeButton('Product','wave','Product','class="wave-row"')}${routeButton('Infrastructure','wave','Infrastructure','class="wave-row"')}`:'')+
    `<div class="pinned"><div class="side-heading">Pinned view</div>${routeButton('◉  Available','query','Available')}<small style="display:block;padding:8px 9px;color:var(--muted);font-size:9px">Across this repository</small></div>`;
}
function breadcrumbs(){
  const r=model.route;let html=routeButton('loopflow','repo','loopflow');
  const wave=r.kind==='task'?model.task(r.id).wave:r.kind==='wave'?r.id:null;
  if(wave)html+=`<span class="crumb-sep">›</span>${routeButton(wave,'wave',wave)}`;
  if(r.kind==='query')html+=`<span class="crumb-sep">›</span>${routeButton(r.id,'query',r.id)}`;
  if(r.kind==='task')html+=`<span class="crumb-sep">›</span><button data-action="palette" title="Find Work">LOO-${r.id} · ${esc(model.task(r.id).title)}</button>`;
  $('crumbs').innerHTML=html;
  const prev=model.history.at(-1);
  $('history-controls').innerHTML=set==='navigation'&&variant==='b'?`<button class="return" data-action="back" ${prev?'':'disabled'} aria-label="${prev?'Return to '+esc(model.label(prev.route)):'No previous view'}">← ${prev?'Return to '+esc(model.label(prev.route)):'Return'}</button>`:'';
}
function tile(t,{suppressed=false,isNew=false}={}){
  if(suppressed)return `<article class="tile suppressed" data-task="${t.id}"><strong>LOO-${t.id} · ${esc(t.title)}</strong><p>This Task is open alongside.<br>Its conversation has one place here.</p><button data-action="focus-draft" data-id="${t.id}">Go to conversation →</button></article>`;
  const arrange=arranged;
  return `<article class="tile ${model.lastTask===t.id?'last':''} ${model.prominent(t.id)?'prominent':''}" data-task="${t.id}" ${arrange?'draggable="true"':''}>
    <header><i class="dot ${t.status.toLowerCase()}"></i><button class="task-name" data-action="task" data-id="${t.id}"><small>LOO-${t.id}${isNew?' · NEW MATCH':''}</small><strong>${esc(t.title)}</strong></button>${arrange?`<button class="mini" data-action="earlier" data-id="${t.id}" aria-label="Move LOO-${t.id} earlier" title="Move earlier">←</button><button class="mini" data-action="hide" data-id="${t.id}" aria-label="Hide LOO-${t.id}" title="Hide (keep membership)">−</button>`:''}</header>
    <div class="tile-output" data-action="task" data-id="${t.id}" role="button" tabindex="0" aria-label="Enter LOO-${t.id}"><p class="provider">codex · primary session</p><p>${esc(t.output)}</p><p class="next ${t.status.toLowerCase()}">${t.status==='Waiting'?'◇':'›'} ${esc(t.next)}</p></div>
    <footer class="tile-footer"><span>${t.draft?'Draft retained':t.status==='Waiting'?'Waiting for you':'Working'} · ${esc(t.wave)}</span>${set==='navigation'&&variant==='c'?`<button data-action="alongside" data-id="${t.id}" aria-label="Open LOO-${t.id} alongside">Open alongside ↗</button>`:`<button data-action="task" data-id="${t.id}" aria-label="Open LOO-${t.id}">Enter ↗</button>`}</footer></article>`;
}
function gridScope(route,beside=false){
  const members=model.ordered(route),active=beside?model.route.id:null;
  const arrivals=members.filter(t=>model.inbox.includes(t.id));
  const separate=arranged&&model.touched&&variant!=='a';
  const regular=separate?members.filter(t=>!model.inbox.includes(t.id)):members;
  const hidden=model.members(route).filter(t=>model.hidden.has(t.id));
  const inbox=separate&&arrivals.length?`<section class="inbox"><header><span>New matches · ${arrivals.length}</span><button data-action="integrate">Integrate into layout</button></header><div class="grid">${arrivals.map(t=>tile(t,{isNew:true})).join('')}</div></section>`:'';
  return `<section class="scope ${beside?'beside-scope':''} ${set==='combined'?'geometry-b':set==='geometry'?'geometry-'+variant:''} ${arranged?'arrangement-'+variant:''}">
    ${beside?`<header class="panel-header"><span>${esc(model.label(route))}</span><button data-action="close-beside">Return here ↗</button></header>`:''}
    <header class="scope-heading"><h2>${esc(model.label(route))}</h2><p>${route.kind==='query'?'Waiting across Waves':'Open Tasks'}</p><span class="scope-count">${members.length} visible</span></header>
    ${arranged?`<div class="lab"><span>Simulation</span><button data-action="swap">Swap first two</button><button data-action="arrival">New match</button><button data-action="attention">Change attention</button><button data-action="departure">${model.task('487').open?'Complete':'Reopen'} LOO-487</button></div><div class="arrangement-info"><span>${model.touched?'⌑ Arrangement retained':'▦ Auto · attention first'}</span><span>Drag a tile or use its ← control.</span>${model.touched?'<button data-action="auto">Reset to auto</button>':''}</div>`:''}
    <div class="grid-scroll" data-scroll="${esc(model.key(route))}">
      ${variant==='c'&&arranged?inbox:''}<div class="board"><div class="grid">${regular.length?regular.map(t=>tile(t,{suppressed:t.id===active,isNew:arranged&&model.inbox.includes(t.id)})).join(''):'<p class="empty">No visible Tasks in this view.</p>'}</div>${variant==='b'&&arranged?inbox:''}</div>
      ${hidden.length?`<div class="hidden-members"><span>${hidden.length} hidden · still members</span>${hidden.map(t=>`<button data-action="restore" data-id="${t.id}">Restore LOO-${t.id}</button>`).join('')}</div>`:''}
    </div></section>`;
}
function taskView(t,beside=false){
  return `<section class="task-workspace ${beside?'beside-task':''}">
    <header class="task-heading"><small>LOO-${t.id}</small><h2>${esc(t.title)}</h2><div class="task-actions">${beside?'<button data-action="close-beside">Close panel ×</button>':''}</div></header>
    <div class="panes"><section class="session" data-pane="${t.pane}"><header class="pane-heading"><span>⌘ &nbsp; Codex · primary Session</span><span class="${t.status.toLowerCase()}">${t.status}</span></header>
    <div class="session-scroll" data-scroll="task:${t.id}"><p class="path">loopflow.loo-${t.id} &nbsp; / &nbsp; jack/loo-${t.id} &nbsp; · &nbsp; ${t.session}</p>
    <div class="message user"><span class="speaker">you</span><p>Keep the change focused. Preserve the existing work and show me what needs a decision.</p></div>
    <div class="message"><span class="speaker">codex</span><p>${esc(t.output)}</p><p>The retained workspace is still here: its conversation, Files draft and pane positions belong to this Task.</p><pre class="code">✓ Same Task identity\n✓ Same conversation and pane\n✓ Your draft stays with this workspace</pre><p>${esc(t.next)}.</p></div></div>
    <div class="composer"><span>›</span><textarea aria-label="Draft for LOO-${t.id}" data-draft="${t.id}" placeholder="Leave a draft, visit another Task, then return…">${esc(t.draft)}</textarea></div><div class="draft-foot"><span id="draft-status">${t.draft?'Draft retained':'Not sent · local to this study'}</span><span>⌘K to visit another Task</span></div></section>
    <section class="files"><header>Files · LOO-${t.id}</header><div>⌄ scratch/<br><span class="selected-file">&nbsp; design.md</span>&nbsp; decisions.md<br>⌄ swift/<br>&nbsp; LoopflowMac/</div><p>Each Task keeps its own pane tree. Entering a Task reveals its companion panes.</p></section></div></section>`;
}
function render(){
  breadcrumbs();sidebar();
  if(model.route.kind==='task')$('main').innerHTML=taskView(model.task(model.route.id),!!model.beside)+(model.beside?gridScope(model.beside,true):'');
  else $('main').innerHTML=gridScope(model.route.kind==='repo'?model.lastContainer:model.route);
  $('status').textContent=model.notice||`${model.label()} · ${model.route.kind==='task'?'Task-owned pane tree':model.route.kind==='repo'?'Last view retained · open Tasks, then Waves':'Primary Sessions · attention first'}`;
  restoreScroll();
}
function palette(){
  $('search').value='';paletteResults('');$('palette').showModal();$('search').focus();
}
function paletteResults(search){
  const q=search.toLowerCase(),routes=[{label:'Product',kind:'wave',id:'Product',detail:'Wave'},{label:'Infrastructure',kind:'wave',id:'Infrastructure',detail:'Wave'},{label:'Available',kind:'query',id:'Available',detail:'Pinned view · across Waves'},...model.tasks.map(t=>({label:t.title,kind:'task',id:t.id,detail:`LOO-${t.id} · ${t.wave}`}))];
  $('results').innerHTML=routes.filter(r=>(r.label+' '+r.detail).toLowerCase().includes(q)).map(r=>`<button data-action="visit" data-kind="${r.kind}" data-id="${r.id}"><span>${esc(r.label)}</span><small>${esc(r.detail)}</small></button>`).join('')||'<p>No matching Work.</p>';
}
function act(action,id,kind){
  saveScroll();model.notice='';
  if(action==='palette'){palette();return;}
  if(action==='focus-draft'){document.querySelector(`[data-draft="${id}"]`)?.focus();return;}
  if(action==='task'||action==='alongside')model.visit({kind:'task',id},action==='alongside'||!!model.beside);
  if(action==='visit'){
    if(kind==='repo'&&model.route.kind!=='task'&&model.route.kind!=='repo')model.lastContainer={...model.route};
    const saved={...model.lastContainer};model.visit({kind,id});if(kind==='repo')model.lastContainer=saved;
    $('palette').close();
  }
  if(action==='back')model.back();if(action==='forward')model.ahead();if(action==='close-beside')model.closeBeside();
  if(action==='arrival')model.arrive();if(action==='attention')model.flipAttention();if(action==='departure')model.depart();
  if(action==='hide')model.hide(id);if(action==='restore')model.restore(id);
  if(action==='swap'){const tasks=model.ordered();if(tasks.length>1)model.move(tasks[1].id,tasks[0].id);}
  if(action==='earlier'){const tasks=model.ordered(),index=tasks.findIndex(t=>t.id===id);if(index>0)model.move(id,tasks[index-1].id);}
  if(action==='integrate'){model.inbox=[];model.notice='New matches integrated at the end of the retained layout.';}
  if(action==='auto'){model.resetArrangement();model.notice='Auto layout restored. Attention determines order.';}
  render();
}
document.addEventListener('click',event=>{const close=event.target.closest('[data-close]');if(close){$(close.dataset.close).close();return;}const el=event.target.closest('[data-action]');if(el)act(el.dataset.action,el.dataset.id,el.dataset.kind);});
document.addEventListener('input',event=>{if(event.target.matches('[data-draft]')){model.task(event.target.dataset.draft).draft=event.target.value;$('draft-status').textContent='Draft retained · not sent';}if(event.target.id==='search')paletteResults(event.target.value);});
document.addEventListener('keydown',event=>{
  if((event.metaKey||event.ctrlKey)&&event.key.toLowerCase()==='k'){event.preventDefault();palette();}
  if((event.metaKey||event.ctrlKey)&&['[',']'].includes(event.key)){event.preventDefault();act(event.key==='['?'back':'forward');}
  if(event.key==='Enter'&&event.target.matches('[role="button"][data-action]')){event.preventDefault();act(event.target.dataset.action,event.target.dataset.id);}
});
let dragged=null;
document.addEventListener('dragstart',event=>{const tile=event.target.closest('[draggable]');if(!tile)return;dragged=tile.dataset.task;event.dataTransfer.setData('text/plain',dragged);tile.classList.add('dragging');});
document.addEventListener('dragover',event=>{if(dragged&&event.target.closest('[draggable]'))event.preventDefault();});
document.addEventListener('drop',event=>{const target=event.target.closest('[draggable]');if(!target||!dragged)return;event.preventDefault();saveScroll();model.move(dragged,target.dataset.task);dragged=null;render();});
document.addEventListener('dragend',()=>{dragged=null;document.querySelectorAll('.dragging').forEach(el=>el.classList.remove('dragging'));});
$('find').onclick=palette;$('open-repo').onclick=()=>$('repos').showModal();$('reset').onclick=()=>{model=new ArrangeStudy(set,variant);render();};
render();
