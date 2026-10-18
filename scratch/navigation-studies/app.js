const {repositories,allTasks,create,visit,travel,pin}=NavStudy;
const repository=repositories.find(r=>r.id===new URLSearchParams(location.search).get('repo'))||repositories[0];
const {tasks,waves}=repository;
const matches=(query,items=tasks)=>NavStudy.matches(query,items);
const storageKey=`lf-navigation-study:${repository.id}`;
if(repository.id==='loopflow'&&!localStorage.getItem(storageKey)&&localStorage.getItem('lf-navigation-study'))localStorage.setItem(storageKey,localStorage.getItem('lf-navigation-study'));
window.name=`navigation-study-${repository.id}`;
const $=s=>document.querySelector(s), esc=s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const interpretations={a:{name:'Favorites first',title:'Your place, wherever you work.',number:'01 / PERSONAL NAVIGATION',description:'A stable sidebar holds favorites and saved views. A short working set changes underneath; the rest stays within reach in ⌘K.',tradeoff:'Strongest continuity. Wave hierarchy is one step away.'},b:{name:'Working set',title:'Work within reach.',number:'02 / WORKING SET',description:'The working set fills the sidebar. One row switches to Waves or Views; repository identity stays in the titlebar.',tradeoff:'Stable Task positions, inline attention, no favorites.'},c:{name:'Outline first',title:'See where everything belongs.',number:'03 / VISIBLE HIERARCHY',description:'An expandable Wave outline sits below pinned shortcuts. Keep several branches open; selecting a Task leaves its neighborhood in place.',tradeoff:'Strongest orientation. More hierarchy costs space and needs tending.'},d:{name:'Minimal',title:'Tasks, then Waves.',number:'04 / MINIMAL NAVIGATION',description:'Working Tasks occupy the sidebar first, with Waves directly underneath.',tradeoff:'One list, no favorites or navigation tabs.'},e:{name:'Repo menu',title:'Repo menu + hide.',number:'05 / SEPARATE CONTROLS',description:'Searchable repository menu; global work search stays separate. Collapse hides the sidebar entirely.',tradeoff:'Direct controls, two distinct entry points.'},f:{name:'One switcher',title:'One switcher + rail.',number:'06 / UNIFIED SWITCHER',description:'The repository header opens one search for repos, Waves and Tasks. Collapse keeps a narrow Task rail.',tradeoff:'One entry point; compressed Task identifiers need recognition.'},g:{name:'Sidebar scope',title:'Sidebar search + scope.',number:'07 / SCOPED SIDEBAR',description:'Find work and choose Working set, Available or Running in the sidebar. Collapse makes it a temporary drawer.',tradeoff:'Quick filtering uses more vertical space.'},h:{name:'Clear navigation',title:'One navigator.',number:'08 / CLEAR NAVIGATION',description:'The repository header opens the navigator. The sidebar toggle controls only visibility.',tradeoff:'Compare visual styles without changing the interaction.'}};
const minimalVariants=['d','e','f','g','h'];
const compactVariants=['b',...minimalVariants];
const explorationVariants=['e','f','g','h'];
const visualStyles={slack:'Slack style',notion:'Notion style',hybrid:'Hybrid',console:'Console hybrid'};
let visualStyle=new URLSearchParams(location.search).get('look');
if(!visualStyles[visualStyle])visualStyle='notion';
let appearance=new URLSearchParams(location.search).get('theme')==='dark'?'dark':'light';
let exploration=new URLSearchParams(location.search).get('study')==='switching';
let variant=new URLSearchParams(location.search).get('v')||'d';if(!interpretations[variant])variant='d';
exploration=exploration||explorationVariants.includes(variant);
let saved;try{saved=JSON.parse(localStorage.getItem(storageKey)||'null')}catch{}
let actionTaskId=null;
let state=create(saved||{},repository.id),contextTab='work',contextFilter='all',taskTab='conversation',editingView=null,highlight=0,paletteRows=[],paletteMode='all',inlineRows=[],inlineHighlight=0,inlineQuery='',sidebarOffsets={};
function persist(){localStorage.setItem(storageKey,JSON.stringify(state))}
function notify(text){$('#toast').textContent=text;$('#toast').classList.add('show');clearTimeout(notify.timer);notify.timer=setTimeout(()=>$('#toast').classList.remove('show'),2700)}
function currentTask(){return tasks.find(t=>t.id===state.route.id&&state.route.kind==='task')}
function labelWave(w){return w[0].toUpperCase()+w.slice(1)}
function routeName(){const r=state.route;return r.kind==='task'?tasks.find(t=>t.id===r.id)?.key||r.id:r.kind==='wave'?labelWave(r.id):r.kind==='view'?state.views.find(v=>v.id===r.id)?.name||'Filtered work':repository.name}
function navigate(route){if(route.repo&&route.kind==='home'){openRepository(route.repo);return}if(route.repo&&route.repo!==repository.id){openRepository(route.repo,route);return}visit(state,route);if(route.kind==='task'){state.expanded[tasks.find(t=>t.id===route.id).wave]=true}contextTab='work';taskTab='conversation';inlineQuery='';if(sidebarMode()==='drawer')setSidebarMode('hidden');persist();render();if(route.kind==='task')$('#draft')?.focus({preventScroll:true})}
function section(label,body,extra=''){return `<section class="section"><div class="section-label"><span>${label}</span>${extra}</div>${body}</section>`}
const TOOL_ICONS={
 shell:'<svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true"><rect x="1.5" y="2.5" width="13" height="11" rx="2" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="m4.5 6 2 2-2 2M8 10.5h3.5" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/></svg>',
 files:'<svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true"><path d="M2 4.5V12a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1H8L6.5 3.5H3a1 1 0 0 0-1 1Z" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/></svg>'};
const CHAT_ICON='<svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true"><path d="M2.5 3.5h11v7h-6l-3 2.5v-2.5h-2z" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/></svg>';
const SCOPE_CHATS={
 repo:{agent:'Claude',where:'main · ~/src/loopflow',prompt:'What needs me today?',work:[['Read','lf wave list --json'],['Read','lf task status --json','5 working · 2 Available']],
  reply:'Two Tasks are waiting on you: LOO-489 needs a sidebar decision and LOO-503 needs a retry boundary. LOO-497 is paused. Describe new work here and I’ll file it as a Task.'},
 product:{agent:'Claude',where:'Product Wave',prompt:'Where is Product this week?',work:[['Read','.lf/wave/product/MEMORY.md']],
  reply:'Navigation (LOO-489) is in design review, conversation opening (LOO-499) is waiting on you, and waveless Tasks (LOO-505) are in progress. Custom workspaces (LOO-497) are paused until navigation lands.'},
 infrastructure:{agent:'Codex',where:'Infrastructure Wave',prompt:'What’s blocking reliability work?',work:[['Read','.lf/wave/infrastructure/MEMORY.md']],
  reply:'Session retention (LOO-416) is verifying reattachment. The merge repair (LOO-503) needs your choice of retry boundary. Two reconnect and logging Tasks have not started.'}};
const WAVE_ICON='<svg class="wave-icon" width="14" height="14" viewBox="0 0 16 16" aria-hidden="true"><path d="M1.5 4.5c1.1-1 2.1-1 3.2 0s2.1 1 3.3 0 2.1-1 3.2 0 2.1 1 3.3 0M1.5 8c1.1-1 2.1-1 3.2 0s2.1 1 3.3 0 2.1-1 3.2 0 2.1 1 3.3 0M1.5 11.5c1.1-1 2.1-1 3.2 0s2.1 1 3.3 0 2.1-1 3.2 0 2.1 1 3.3 0" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/></svg>';
function row(label,kind,id,icon='',count='',extra=''){const selected=state.route.kind===kind&&state.route.id===id;return `<button class="side-row ${selected?'selected':''} ${extra}" data-go="${kind}" data-id="${esc(id)}" ${selected?'aria-current="page"':''}><span class="row-icon">${icon}</span><span class="label">${esc(label)}</span>${count!==''?`<span class="count">${count}</span>`:''}</button>`}
function taskRow(t,detail=false){return `<button class="side-row ${detail?'working-row':''} ${state.route.kind==='task'&&state.route.id===t.id?'selected':''}" data-go="task" data-id="${t.id}" title="${t.key} · ${esc(t.title)}" ${state.route.kind==='task'&&state.route.id===t.id?'aria-current="page"':''}><span class="dot ${t.state}"></span><span class="label">${esc(t.title)}${detail?`<small>${t.key} · ${esc(t.detail)}</small>`:''}</span></button>`}
function compactRow(t){if(variant==='h'&&visualStyle==='console')return consoleRow(t);return `<button class="compact-row ${state.route.kind==='task'&&state.route.id===t.id?'selected':''}" data-go="task" data-id="${t.id}" ${state.route.kind==='task'&&state.route.id===t.id?'aria-current="page"':''}><span class="compact-title">${t.key} ${esc(t.title)}</span><span class="compact-detail">${esc(t.detail)}</span><span class="compact-meta"><span class="dot ${t.state}"></span>${t.state==='available'?'Available':t.state==='running'?'Working':'Idle'}<span class="compact-owner">${labelWave(t.wave)} · ${t.agent}</span></span></button>`}
function scopedTasks(){if(waves.includes(state.scope))return tasks.filter(t=>t.wave===state.scope);const v=state.views.find(v=>v.id===state.scope);return v?matches(v.query):tasks}
function viewRows(){return state.views.map(v=>row(v.name,'view',v.id,v.id==='available'?'◉':v.id==='running'?'↗':'≡',matches(v.query).length)).join('')}
function sidebar(){const favoriteRows=state.favorites.map(id=>taskRow(tasks.find(t=>t.id===id))).join('');let content='';
if(minimalVariants.includes(variant)){
 if(explorationVariants.includes(variant))return explorationSidebar();
 content=`<div class="side-scroll">${section('Working tasks',state.workingSet.map(id=>compactRow(tasks.find(t=>t.id===id))).join(''))}${section('Waves',waves.map(w=>row(labelWave(w),'wave',w,'◇',tasks.filter(t=>t.wave===w).length)).join(''))}</div>`;
}else if(variant==='a'){
 const list=state.scope==='all'?state.recent.map(id=>tasks.find(t=>t.id===id)):scopedTasks();const name=state.scope==='all'?'Working set':waves.includes(state.scope)?labelWave(state.scope):state.views.find(v=>v.id===state.scope)?.name;
 content=`<div class="side-scroll">${section('Favorites',favoriteRows||'<p class="scope-notice">Star a Task to keep it here.</p>')}${section('Views',viewRows(),'<button data-search title="Add a saved view" aria-label="Add a saved view">+</button>')}${section(esc(name),list.map(t=>taskRow(t,true)).join(''),'<button data-go="home" data-id="all" title="All work">↗</button>')}${section('Waves',waves.map(w=>row(labelWave(w),'wave',w,'◇',tasks.filter(t=>t.wave===w).length)).join(''))}</div>`;
}else if(variant==='b'){
 content=`<div class="segmented"><button data-context="work" class="${contextTab==='work'?'active':''}">Working set</button><button data-context="waves" class="${contextTab==='waves'?'active':''}">Waves</button><button data-context="saved" class="${contextTab==='saved'?'active':''}">Views</button></div><div class="side-scroll">${contextTab==='work'?state.workingSet.map(id=>compactRow(tasks.find(t=>t.id===id))).join(''):contextTab==='waves'?waves.map(w=>row(labelWave(w),'wave',w,'◇',tasks.filter(t=>t.wave===w).length)).join(''):viewRows()+'<button class="side-row" data-search>+ Find or save a view</button>'}</div>`;
}else{
 content=`<div class="side-scroll">${section('Pinned',viewRows()+favoriteRows,'<button data-search aria-label="Add a saved view">+</button>')}${section('Waves',waves.map(w=>`<div class="tree-group"><button data-expand="${w}" aria-label="${state.expanded[w]?'Collapse':'Expand'} ${labelWave(w)}" aria-expanded="${state.expanded[w]}">${state.expanded[w]?'⌄':'›'}</button>${row(labelWave(w),'wave',w,'',tasks.filter(t=>t.wave===w).length)}</div>${state.expanded[w]?`<div class="nested">${tasks.filter(t=>t.wave===w).map(t=>taskRow(t)).join('')}</div>`:''}`).join(''))}${section('Recently visited',state.recent.slice(0,3).map(id=>taskRow(tasks.find(t=>t.id===id))).join(''))}</div>`;
}
if(compactVariants.includes(variant))return content;
return content+'<div class="side-foot"><span>⌕ &nbsp; Everything else in ⌘K</span><button data-go="home" data-id="all">All work ↗</button></div>'}
function crumbs(){const t=currentTask(),r=state.route;if(variant==='h'&&visualStyle==='console'){const home=r.kind==='home';return `<span class="repo-crumb ${home?'at-home':''}"><button class="repo-name" data-go="home" data-id="" title="${esc(repository.name)} conversation" ${home?'aria-current="page"':''}><svg width="17" height="17" viewBox="0 0 18 18" aria-hidden="true"><path d="M2 5h5l2 2h7v8H2zM2 5V3h5l2 2h7v2" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/></svg>${esc(repository.name)}</button><button class="repo-switch" data-repositories aria-label="Navigate work and repositories" title="Navigate (⌘K)"><svg class="repo-chevron" width="9" height="9" viewBox="0 0 10 10" aria-hidden="true"><path d="m2 3.5 3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg></button></span>${t||r.kind==='wave'?`<span class="sep">/</span><button class="location-name" data-go="wave" data-id="${t?.wave||r.id}">${labelWave(t?.wave||r.id)}</button>`:''}${t?`<span class="sep">/</span><span class="current-task"><span>${t.key}</span><span class="crumb-title" title="${esc(t.title)}">${esc(t.title)}</span></span>`:''}`}if(variant==='h')return `<button class="repo-name" data-repositories aria-label="Navigate work and repositories: ${esc(repository.name)}" title="Navigate (⌘K)"><svg width="17" height="17" viewBox="0 0 18 18" aria-hidden="true"><path d="M2 5h5l2 2h7v8H2zM2 5V3h5l2 2h7v2" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/></svg>${esc(repository.name)}${visualStyle==='console'?'<svg class="repo-chevron" width="9" height="9" viewBox="0 0 10 10" aria-hidden="true"><path d="m2 3.5 3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>':' <span class="repo-disclosure">⌄</span><kbd>⌘K</kbd>'}</button>${t||r.kind==='wave'?`<span class="sep">/</span><button class="location-name" data-go="wave" data-id="${t?.wave||r.id}">${labelWave(t?.wave||r.id)}</button>`:''}${t?`<span class="sep">/</span><span class="current-task"><span>${t.key}</span>${visualStyle==='console'?`<span class="crumb-title" title="${esc(t.title)}">${esc(t.title)}</span>`:''}</span>`:''}`;let out=`<button class="repo-name" data-repositories aria-label="Repository: ${esc(repository.name)}" title="${variant==='f'?'Find work and repositories (⌘K)':'Repository windows'}"><span class="repo-icon" style="background:${repository.color}">${repository.icon}</span>${esc(repository.name)} <span class="repo-disclosure">${variant==='f'?'⌕ ⌘K':'⌄'}</span></button>`;const wave=t?.wave||(r.kind==='wave'?r.id:null);if(wave)out+=`<span class="sep">/</span><button data-go="wave" data-id="${wave}">${labelWave(wave)}</button>`;if(t)out+=`<span class="sep">/</span><button class="current" data-search title="Jump to another Task">${t.key} ⌄</button>`;else if(r.kind==='view')out+=`<span class="sep">/</span><button class="current" data-search>${esc(routeName())} ⌄</button>`;return out}
function center(){const t=currentTask();if(!t&&variant==='h'&&visualStyle==='console')return state.route.kind==='wave'?waveWorkspace():state.route.kind==='home'?`<div id="console-workspace">${scopeConversation()}</div>`:consoleOverview();if(t&&variant==='h'&&visualStyle==='console')return consoleCenter(t);if(!t){let list=state.route.kind==='wave'?tasks.filter(t=>t.wave===state.route.id):state.route.kind==='view'?matches(state.views.find(v=>v.id===state.route.id)?.query||''):tasks;const title=state.route.kind==='home'?'Your work':routeName();const view=state.route.kind==='view'?state.views.find(v=>v.id===state.route.id):null;return `<div class="overview"><p class="eyebrow">${view?'SAVED VIEW':state.route.kind==='wave'?'WAVE':'REPOSITORY'}</p><h2>${esc(title)}</h2><p>${view?`${list.length} matching ${list.length===1?'Task':'Tasks'}. This filter changes the list; each Task keeps its own workspace.`:state.route.kind==='wave'?'The work in this Wave. Select a Task to continue its conversation.':'A quiet place to find the next thing. Available Tasks are waiting for a decision.'}</p>${view?`<div class="view-tools"><code>${esc(view.query)}</code><button data-edit-view="${view.id}">Edit filter</button><button data-unpin="${view.id}">Unpin view</button></div>`:''}<div class="work-table">${list.map(t=>`<button class="table-row" data-go="task" data-id="${t.id}"><span class="dot ${t.state}"></span><span class="title">${esc(t.title)}<small>${esc(t.detail)}</small></span><code>${t.key}</code><span>↗</span></button>`).join('')||'<p class="empty">No matching Tasks. Edit the filter to widen the view.</p>'}</div></div>`}
 const favorite=state.favorites.includes(t.id);return `<div class="task-heading"><div class="task-eyebrow"><span>${t.key}</span><span>·</span><span>${labelWave(t.wave)}</span></div><h2>${esc(t.title)}</h2><div class="task-meta"><span class="${t.state}"><span class="dot ${t.state}"></span> ${t.state==='available'?'Available · your decision':t.state==='running'?'Working':'Notes saved'}</span><span>${t.agent}</span>${!compactVariants.includes(variant)?`<button data-star="${t.id}">${favorite?'★ Favorited':'☆ Add to favorites'}</button>`:''}</div><div class="task-tabs">${['conversation','brief','changes'].map(tab=>`<button data-tab="${tab}" class="${taskTab===tab?'active':''}">${tab[0].toUpperCase()+tab.slice(1)}</button>`).join('')}</div></div>${taskTab==='conversation'?`<div class="conversation"><div class="message user"><div class="byline">JACK <span>· earlier</span></div><p>${esc(t.prompt)}</p></div><div class="message"><div class="byline"><span class="agent-icon">✳</span>${t.agent.toUpperCase()} <span>· ${t.time} ago</span></div><p>${esc(t.reply)}</p><div class="result-line">✓ &nbsp; Working notes retained</div>${t.state==='available'?'<div class="decision"><strong>A decision is waiting here.</strong><p>Read the options, leave your direction, and return to the rest of your work.</p></div>':'<p class="scope-notice">You can switch Tasks while this work continues.</p>'}</div>${state.sent?.[t.id]?`<div class="message user"><div class="byline">JACK · study response</div><p>${esc(state.sent[t.id])}</p></div>`:''}</div><div class="composer"><textarea id="draft" aria-label="Message draft for ${t.key}" placeholder="Leave a thought for this Task…">${esc(state.drafts[t.id]||'')}</textarea><div class="composer-footer"><span id="draft-state">${state.drafts[t.id]?'Draft restored':'Draft stays with '+t.key}</span><button data-send="${t.id}" title="Adds a sample message only">Send in study ↑</button></div></div>`:`<div class="note-box"><h3>${taskTab==='brief'?'The intended outcome':'Working changes'}</h3><p>${taskTab==='brief'?esc(t.prompt):'The working change stays attached to this Task when you navigate elsewhere.'}</p><p>${taskTab==='brief'?'Preserve Task identity, its conversation and any unsent draft. Review the result before delivery.':'This prototype explores navigation. It does not contain a real repository diff.'}</p></div>`}`}
function render(){document.title=state.route.kind==='home'?repository.name:`${repository.name} — ${routeName()}`;const i=interpretations[variant];document.body.dataset.variant=variant;document.body.dataset.look=visualStyle==='console'?'hybrid':visualStyle;document.body.dataset.console=String(variant==='h'&&visualStyle==='console');document.body.dataset.theme=appearance;$('#theme-choice').hidden=variant!=='h'||!['hybrid','console'].includes(visualStyle);$('#theme').value=appearance;document.body.dataset.exploration=String(explorationVariants.includes(variant));document.body.dataset.density=compactVariants.includes(variant)?'compact':'study';$('.study-label').href=`?v=d&repo=${repository.id}${exploration?'':'&study=switching'}`;$('.study-label').textContent=exploration?'← Earlier studies':'More navigation studies →';$('#row-density').hidden=!exploration||variant==='h';$('#row-density').textContent=state.compactRows?'Detailed rows':'Compact rows';document.body.dataset.rows=state.compactRows?'compact':'detailed';const waveHint=$('[data-query="wave:product"]');if(waveHint){waveHint.dataset.query=`wave:${waves[0]}`;waveHint.textContent=labelWave(waves[0])}$('#variants').innerHTML=variant==='h'?Object.keys(visualStyles).map(look=>`<button data-look="${look}" class="${visualStyle===look?'active':''}" aria-pressed="${visualStyle===look}">${visualStyles[look]}</button>`).join(''):Object.entries(interpretations).filter(([k])=>exploration?['d','e','f','g'].includes(k):['a','b','c','d'].includes(k)).map(([k,v])=>`<button data-variant="${k}" class="${k===variant?'active':''}" aria-pressed="${k===variant}">${k.toUpperCase()} · ${v.name}</button>`).join('');$('#number').textContent=i.number;$('#headline').textContent=i.title;$('#description').textContent=i.description;$('#tradeoff').textContent=i.tradeoff;$('#context-action').hidden=compactVariants.includes(variant);$('#search').hidden=['f','h'].includes(variant);$('#more').hidden=variant==='h'&&(visualStyle!=='console'||!currentTask());$('#title-tabs').innerHTML=variant==='h'&&visualStyle==='console'&&currentTask()?consoleTabs(currentTask()):'';$('#activity-example').hidden=!(variant==='h'&&visualStyle==='console');renderSidebar();$('#crumbs').innerHTML=crumbs();$('#main').innerHTML=center();$('#back').disabled=!state.history.length;$('#forward').disabled=!state.future.length;$('#context-action').textContent=currentTask()?(state.favorites.includes(currentTask().id)?'★ Saved':'☆ Favorite'):'Filter work';if(variant==='h'){$('.study-label').href=`?v=e&repo=${repository.id}&study=switching`;$('.study-label').textContent='← Navigation variants'}$('#status').textContent=`${routeName()} · ${tasks.filter(t=>t.state==='available').length} Available · one selected workspace`;$('#draft-proof').textContent=Object.values(state.drafts).filter(Boolean).length?`${Object.values(state.drafts).filter(Boolean).length} Task draft(s) retained across navigation.`:'Drafts stay with their Task.';}
function openPalette(query='',viewId=null,mode='all'){actionTaskId=currentTask()?.id||null;if(variant==='g'&&mode==='all'&&!viewId){focusSidebarSearch(query);return}paletteMode=mode;$('#palette').dataset.mode=mode;$('#palette').classList.toggle('repo-popover',variant==='e'&&mode==='repositories');$('#palette-label').textContent=variant==='h'?'Navigate':mode==='repositories'?'Repository windows':'Find work';$('#navigator-scopes').hidden=variant!=='h'||mode!=='all';if(variant==='h')$('#navigator-scopes').innerHTML='<span>Sidebar shows</span>'+[['working','Working tasks'],['available','Available'],['running','Running']].map(([id,name])=>`<button data-sidebar-filter="${id}" aria-pressed="${(state.sidebarFilter||'working')===id}">${name}</button>`).join('');$('#query').placeholder=mode==='repositories'?'Find a repository…':'Search Tasks, Waves, or repositories…';editingView=viewId;highlight=0;$('#query').value=query;$('#palette').showModal();searchResults();$('#query').focus()}
function searchResults(){if(variant==='h'&&visualStyle==='console'){consoleSearchResults();return}const q=$('#query').value.trim();const isFilter=q.includes(':');paletteRows=[];let items='';for(const repo of repositories.filter(r=>!q||r.name.includes(q.toLowerCase()))){paletteRows.push({kind:'home',id:'all',repo:repo.id});items+=`<button data-result="${paletteRows.length-1}"><span class="repo-icon" style="background:${repo.color}">${repo.icon}</span><span>${esc(repo.name)}<small>${repo.id===repository.id?'This repository':'Open repository window'}</small></span><span>↗</span></button>`}if(paletteMode!=='repositories'&&!isFilter){for(const w of waves.filter(w=>!q||w.includes(q.toLowerCase()))){paletteRows.push({kind:'wave',id:w});items+=`<button data-result="${paletteRows.length-1}"><span class="row-icon">◇</span><span>${labelWave(w)}<small>Wave · ${esc(repository.name)}</small></span><span>↵</span></button>`;}}
 for(const t of paletteMode==='repositories'?[]:matches(q,allTasks)){paletteRows.push({kind:'task',id:t.id,repo:t.repo});items+=`<button data-result="${paletteRows.length-1}"><span class="dot ${t.state}"></span><span>${esc(t.title)}<small>${t.key} · ${esc(t.repo)} · ${labelWave(t.wave)}</small></span><span>↵</span></button>`;}
 $('#results').innerHTML=items||'<p class="empty">No matching work. Try wave:product or state:available.</p>';highlight=Math.min(highlight,Math.max(0,paletteRows.length-1));$('#results').querySelector(`[data-result="${highlight}"]`)?.classList.add('highlighted');$('#save-query').innerHTML=minimalVariants.includes(variant)?'<span>Find Tasks and Waves across repositories.</span>':isFilter?`<span>${matches(q).length} matches in ${esc(repository.name)} · <code>${esc(q)}</code></span><button data-pin-query>${editingView?'Save filter':'Pin this view'}</button>`:'<span>Search the long tail. Use wave:, state:, task: or combine filters.</span>';}
function toggleFavorite(id){state.favorites=state.favorites.includes(id)?state.favorites.filter(x=>x!==id):[...state.favorites,id];persist();render();notify(state.favorites.includes(id)?'Task added to favorites':'Task removed from favorites')}
function close(id){$('#'+id).close()}
document.addEventListener('click',e=>{const b=e.target.closest('button');if(!b)return;const d=b.dataset;
 if(d.tool||d.toolTab||d.toolAdd||d.toolClose){toolClick(d);return}
 if(d.action){close('actions');runAction(d.action)}
 else if(d.look){visualStyle=d.look;history.replaceState(null,'',`?v=h&repo=${repository.id}&look=${visualStyle}&theme=${appearance}`);render()}
 else if('repositories'in d){if(variant==='e')openPalette('',null,'repositories');else if(['f','h'].includes(variant))openPalette();else if(variant==='g')focusSidebarSearch('repo:');else openRepositoryPicker()}
 else if(d.sectionToggle){state.sections={...state.sections,[d.sectionToggle]:!state.sections?.[d.sectionToggle]};persist();renderSidebar()}
 else if(d.sidebarFilter){state.sidebarFilter=d.sidebarFilter;inlineQuery='';persist();renderSidebar();if($('#palette').open)close('palette')}
 else if(d.inlineResult!==undefined){navigate(inlineRows[Number(d.inlineResult)])}
 else if('pinSidebar'in d){setSidebarMode('expanded');renderSidebar()}
 else if('railSearch'in d){openPalette()}
 else if('railWaves'in d){setSidebarMode('expanded');renderSidebar();$('#sidebar').querySelector('[data-section-toggle=waves]')?.focus()}
 else if(d.openRepo){close('actions');openRepository(d.openRepo)}
 else if(d.variant){variant=d.variant;history.replaceState(null,'',`?v=${variant}&repo=${repository.id}${exploration?'&study=switching':''}`);render()}
 else if(d.go){navigate({kind:d.go,id:d.id})}
 else if(d.expand){state.expanded[d.expand]=!state.expanded[d.expand];persist();render()}
 else if(d.context){contextTab=d.context;render()}
 else if(d.filter){contextFilter=d.filter;render()}
 else if(d.tab){if(variant==='h'&&visualStyle==='console'){taskTab=d.tab===taskTab?'conversation':d.tab;renderCompanion()}else{taskTab=d.tab;render()}}
 else if(d.star){toggleFavorite(d.star)}
 else if('search'in d){openPalette()}
 else if(d.close){close(d.close)}
 else if(d.query){$('#query').value=d.query;highlight=0;searchResults();$('#query').focus()}
 else if(d.result!==undefined){close('palette');navigate(paletteRows[Number(d.result)])}
 else if('pinQuery'in d){const q=$('#query').value.trim();const existing=state.views.find(v=>v.query===q);const name=existing?.name||q.replace(/wave:/g,'').replace(/state:/g,'').replace(/\b\w/g,c=>c.toUpperCase());const view=editingView?state.views.find(v=>v.id===editingView):pin(state,name,q);if(editingView)view.query=q;close('palette');navigate({kind:'view',id:view.id});if(variant==='b')contextTab='saved';persist();render();notify(`“${view.name}” pinned to navigation`)}
 else if(d.unpin){state.views=state.views.filter(v=>v.id!==d.unpin);navigate({kind:'home',id:'all'});persist();notify('View unpinned; Tasks are unchanged')}
 else if(d.editView){const v=state.views.find(v=>v.id===d.editView);openPalette(v.query,v.id)}
 else if('scopePicker'in d){$('#action-title').textContent='Choose sidebar scope';$('#action-options').innerHTML='<button data-pick-scope="all">All repository work</button>'+waves.map(w=>`<button data-pick-scope="${w}">◇ ${labelWave(w)}</button>`).join('')+state.views.map(v=>`<button data-pick-scope="${v.id}">≡ ${esc(v.name)}</button>`).join('');$('#actions').showModal()}
 else if(d.pickScope){close('actions');navigate({kind:d.pickScope==='all'?'home':waves.includes(d.pickScope)?'wave':'view',id:d.pickScope})}
 else if(d.send){const text=state.drafts[d.send]?.trim();if(!text){notify('Write a sample response first');return}state.sent={...state.sent,[d.send]:text};state.drafts[d.send]='';persist();render();notify('Sample response added. No message was sent to an agent.')}
});
$('#theme').onchange=e=>{appearance=e.target.value;history.replaceState(null,'',`?v=h&repo=${repository.id}&look=${visualStyle}&theme=${appearance}`);render()};
$('#back').onclick=()=>{travel(state);persist();render()};$('#forward').onclick=()=>{travel(state,false);persist();render()};$('#search').onclick=()=>openPalette();$('#query-example').onclick=()=>openPalette('wave:product state:available');$('#context-action').onclick=()=>currentTask()?toggleFavorite(currentTask().id):openPalette(waves.includes(state.scope)?`wave:${state.scope}`:'state:open');$('#more').onclick=()=>{if(variant==='h'&&visualStyle==='console'){openActions();return}$('#action-title').textContent=routeName();$('#action-options').innerHTML=currentTask()?`${!compactVariants.includes(variant)?`<button data-star="${currentTask().id}" data-action-close>☆ Toggle favorite</button>`:''}<button data-go="wave" data-id="${currentTask().wave}" data-action-close>◇ Open owning Wave</button><button data-search data-action-close>⌕ Jump to another Task</button>`:'<button data-search data-action-close>⌕ Find work or create a saved view</button>';$('#actions').showModal()};$('#actions').addEventListener('click',e=>{if(e.target.closest('[data-action-close]'))close('actions')});$('#query').addEventListener('input',()=>{highlight=0;searchResults()});document.addEventListener('input',e=>{if(e.target.id==='draft'){state.drafts[draftKey()]=e.target.value;persist();($('#draft-state')||{}).textContent='Draft saved in this study';$('#draft-proof').textContent='Your draft will be here when you return.'}});
$('#query').addEventListener('keydown',e=>{if(['ArrowDown','ArrowUp'].includes(e.key)){e.preventDefault();highlight=Math.max(0,Math.min(paletteRows.length-1,highlight+(e.key==='ArrowDown'?1:-1)));searchResults();$('#results').querySelector(`[data-result="${highlight}"]`)?.scrollIntoView({block:'nearest'})}if(e.key==='Enter'&&paletteRows[highlight]){e.preventDefault();close('palette');navigate(paletteRows[highlight])}});
document.addEventListener('keydown',e=>{if($('dialog[open]'))return;if(variant==='h'&&visualStyle==='console'&&currentTask()&&((e.ctrlKey&&e.key==='`')||(e.metaKey&&e.shiftKey&&e.key.toLowerCase()==='e'))){e.preventDefault();toolClick({tool:e.key==='`'?'shell':'files'});return}if(explorationVariants.includes(variant)&&(e.metaKey||e.ctrlKey)&&e.key==='\\'){e.preventDefault();toggleSidebar()}if(e.key==='Escape'&&sidebarMode()==='drawer'){setSidebarMode('hidden');renderSidebar()}if((e.metaKey||e.ctrlKey)&&e.key.toLowerCase()==='k'){e.preventDefault();if(!$('#palette').open)openPalette()}if((e.metaKey||e.ctrlKey)&&['[',']'].includes(e.key)){e.preventDefault();travel(state,e.key==='[');persist();render()}});
$('#sidebar-toggle').onclick=toggleSidebar;$('#sidebar-dismiss').onclick=()=>{setSidebarMode('hidden');renderSidebar()};$('#row-density').onclick=()=>{state.compactRows=!state.compactRows;persist();render()};
$('#reset').onclick=()=>{state=create({},repository.id);contextTab='work';contextFilter='all';persist();render();notify('Sample work and drafts reset')};render();

function openRepositoryPicker(){
 $('#action-title').textContent='Repository windows';
 $('#action-options').innerHTML=repositories.map(r=>`<button data-open-repo="${r.id}"><span class="repo-icon" style="background:${r.color}">${r.icon}</span> ${esc(r.name)}${r.id===repository.id?' · this window':' ↗'}</button>`).join('')+'<p class="scope-notice">fieldnotes is sample work. Each repository keeps its own selection and drafts.</p>';
 $('#actions').showModal();
}
function openRepository(id,route){
 if(id===repository.id){if(route)navigate(route);return}
 const target=window.open('',`navigation-study-${id}`,'popup,width=1100,height=800');
 if(!target){notify('Allow this local study to open its repository window.');return}
 if(target.location.href==='about:blank'){
  const url=new URL(location.href);url.searchParams.set('repo',id);url.searchParams.set('v',variant);
  if(route?.kind==='task')url.searchParams.set('task',route.id);else url.searchParams.delete('task');
  target.location.href=url.href;
 }else if(route){target.postMessage({type:'study-route',route},location.origin)}
 target.focus();
}
window.addEventListener('message',e=>{if(e.origin===location.origin&&e.data?.type==='study-route'&&e.data.route?.repo===repository.id)navigate(e.data.route)});
const icon=document.createElement('link');icon.rel='icon';icon.href=`data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" rx="14" fill="${repository.color}"/><text x="32" y="46" text-anchor="middle" font-family="system-ui" font-size="44" fill="white">${repository.icon}</text></svg>`)}`;document.head.append(icon);
const requestedTask=new URLSearchParams(location.search).get('task');if(tasks.some(t=>t.id===requestedTask))navigate({kind:'task',id:requestedTask});


// Sidebar navigation studies share the same retained workspace.
function sidebarMode(){return explorationVariants.includes(variant)?state.sidebarModes?.[variant]||'expanded':'expanded'}
function setSidebarMode(mode){state.sidebarModes={...state.sidebarModes,[variant]:mode};persist()}
function toggleSidebar(){const mode=sidebarMode();setSidebarMode(mode==='expanded'?(variant==='f'?'rail':'hidden'):variant==='g'?(mode==='drawer'?'hidden':'drawer'):'expanded');renderSidebar();if(sidebarMode()==='drawer')$('#sidebar-query')?.focus()}
function sidebarTasks(){const filter=state.sidebarFilter||'working';return ['g','h'].includes(variant)&&filter!=='working'?tasks.filter(t=>t.state===filter):NavStudy.workingIds(state).map(id=>tasks.find(t=>t.id===id))}
function collapsibleSection(id,label,body,count){return `<section class="section"><button class="section-disclosure" data-section-toggle="${id}" aria-expanded="${!state.sections?.[id]}"><span>${state.sections?.[id]?'›':'⌄'}</span>${label}<span class="count">${count}</span></button>${state.sections?.[id]?'':body}</section>`}
function explorationSidebar(){
 const mode=sidebarMode();
 if(variant==='h'){const list=sidebarTasks();const name=({working:'Working tasks',available:'Available',running:'Running'})[state.sidebarFilter||'working'];if(visualStyle==='console')return `<div class="side-scroll">${section(name,list.map(compactRow).join(''),`<span class="count">${list.length}</span>`)}${currentTask()&&!list.includes(currentTask())?'<div class="scope-notice">Open Task is outside this list. <button data-sidebar-filter="working">Show working tasks</button></div>':''}<nav class="console-waves" aria-label="Waves">${section('Waves',waves.map(w=>row(labelWave(w),'wave',w,WAVE_ICON,tasks.filter(t=>t.wave===w).length)).join(''))}</nav></div>`;return `<div class="side-scroll">${section(name,list.map(compactRow).join(''),`<span class="count">${list.length}</span>`)}${section('Waves',waves.map(w=>row(labelWave(w),'wave',w,WAVE_ICON,tasks.filter(t=>t.wave===w).length)).join(''))}</div>`}
 if(mode==='rail')return `<div class="task-rail"><button data-rail-search aria-label="Find work" title="Find work (⌘K)">⌕</button>${state.workingSet.map(id=>{const t=tasks.find(t=>t.id===id);return `<button class="rail-task ${currentTask()?.id===id?'selected':''}" data-go="task" data-id="${id}" aria-label="${t.key} ${esc(t.title)}" title="${t.key} · ${esc(t.title)}"><span class="dot ${t.state}"></span><span>${t.id}</span></button>`}).join('')}<button data-rail-waves aria-label="Show Waves" title="Show Waves">◇</button></div>`;
 const list=sidebarTasks();
 const controls=variant==='g'?`<div class="sidebar-find"><span>⌕</span><input id="sidebar-query" aria-label="Search sidebar work and repositories" placeholder="Find work or repos…" value="${esc(inlineQuery)}"><kbd>⌘K</kbd></div><div id="inline-results" ${inlineQuery?'':'hidden'}></div>`:'';
 const filter=variant==='g'?`<div class="sidebar-scopes" aria-label="Sidebar contents">${[['working','Working set'],['available','Available'],['running','Running']].map(([id,name])=>`<button data-sidebar-filter="${id}" aria-pressed="${(state.sidebarFilter||'working')===id}"><span>${id==='working'?'≡':id==='available'?'◉':'↗'}</span>${name}<span class="count">${id==='working'?state.workingSet.length:tasks.filter(t=>t.state===id).length}</span></button>`).join('')}</div>`:'';
 return `${mode==='drawer'?'<div class="drawer-tools"><span>Navigation</span><button data-pin-sidebar>Keep open ↗</button></div>':''}${controls}<div class="side-scroll" id="sidebar-list" ${inlineQuery?'hidden':''}>${filter}${collapsibleSection('working',variant==='g'&&(state.sidebarFilter||'working')!=='working'?'Tasks':'Working tasks',list.map(compactRow).join(''),list.length)}${collapsibleSection('waves','Waves',waves.map(w=>row(labelWave(w),'wave',w,'◇',tasks.filter(t=>t.wave===w).length)).join(''),waves.length)}</div>`;
}
function renderSidebar(){
 const old=$('#sidebar .side-scroll');const scroll=old?.getClientRects().length?old.scrollTop:sidebarOffsets[variant]||0;sidebarOffsets[variant]=scroll;
 const mode=sidebarMode();document.body.dataset.sidebar=mode;
 $('#sidebar').innerHTML=sidebar();$('#sidebar').hidden=mode==='hidden';$('#sidebar-resize').setAttribute('aria-valuenow',state.sidebarWidth||240);$('#sidebar').style.width=variant==='h'&&visualStyle==='console'?`${state.sidebarWidth||240}px`:'';$('#sidebar-resize').hidden=mode==='hidden'||variant!=='h'||visualStyle!=='console';
 $('#sidebar-toggle').hidden=!explorationVariants.includes(variant);
 $('#sidebar-toggle').setAttribute('aria-label',mode==='expanded'||mode==='drawer'?'Collapse sidebar':'Show sidebar');
 $('#sidebar-toggle').setAttribute('aria-expanded',String(mode==='expanded'||mode==='drawer'));
 $('#sidebar-toggle').title='Toggle sidebar (⌘\\)';$('#sidebar-dismiss').hidden=mode!=='drawer';
 const next=$('#sidebar .side-scroll');if(next)next.scrollTop=scroll;
 if(variant==='g'){updateInlineResults();const input=$('#sidebar-query');input?.addEventListener('input',()=>{inlineQuery=input.value;inlineHighlight=0;updateInlineResults()});input?.addEventListener('keydown',e=>{if(e.key==='Escape'){inlineQuery='';input.value='';updateInlineResults()}if(['ArrowDown','ArrowUp'].includes(e.key)){e.preventDefault();inlineHighlight=Math.max(0,Math.min(inlineRows.length-1,inlineHighlight+(e.key==='ArrowDown'?1:-1)));updateInlineResults()}if(e.key==='Enter'&&inlineRows[inlineHighlight]){e.preventDefault();navigate(inlineRows[inlineHighlight])}})}
}
function focusSidebarSearch(query=''){if(sidebarMode()==='hidden')setSidebarMode('drawer');inlineQuery=query;inlineHighlight=0;renderSidebar();$('#sidebar-query')?.focus()}
function updateInlineResults(){
 const target=$('#inline-results');if(!target)return;const q=inlineQuery.trim();target.hidden=!q;$('#sidebar-list').hidden=Boolean(q);inlineRows=[];if(!q){target.innerHTML='';return}
 let html='';const repoQuery=q.replace(/^repo:\s*/i,'');
 for(const r of repositories.filter(r=>r.name.includes(repoQuery.toLowerCase()))){inlineRows.push({kind:'home',id:'all',repo:r.id});html+=`<button data-inline-result="${inlineRows.length-1}"><strong>${esc(r.name)}</strong><small>${r.id===repository.id?'This repository':'Open repository window ↗'}</small></button>`}
 for(const t of matches(q,allTasks)){inlineRows.push({kind:'task',id:t.id,repo:t.repo});html+=`<button data-inline-result="${inlineRows.length-1}"><strong>${t.key} ${esc(t.title)}</strong><small>${esc(t.repo)} · ${esc(t.detail)}</small></button>`}
 for(const w of waves.filter(w=>w.includes(q.toLowerCase()))){inlineRows.push({kind:'wave',id:w});html+=`<button data-inline-result="${inlineRows.length-1}"><strong>${labelWave(w)}</strong><small>Wave · ${repository.name}</small></button>`}
 target.innerHTML=html||'<p class="scope-notice">No matching work.</p>';target.querySelector(`[data-inline-result="${inlineHighlight}"]`)?.classList.add('highlighted');
}

// One Session preview is always qualified by the turn it describes.
function consolePreview(t){
 const mode=t.id==='489'?$('#activity-example').value:'current',a=consoleActivity[t.key];
 return NavStudy.activityPreview({phase:mode==='new-turn'?'running':t.state,paused:!!t.paused,
  observed:mode!=='unavailable',turn:'current',summary:t.detail,
  line:{turn:mode==='new-turn'?'previous':'current',text:a.tail,fresh:mode==='current',age:t.state==='running'?'2s':t.time}});
}
function consoleRow(t){
 const a=consoleActivity[t.key],selected=currentTask()?.id===t.id,p=consolePreview(t),slot=NavStudy.workingIds(state).indexOf(t.id)+1;
 const outside=state.route.kind==='wave'&&t.wave!==state.route.id;
 return `<button class="compact-row console-row ${p.tone} ${selected?'selected':''} ${outside?'outside-wave':''}" data-go="task" data-id="${t.id}" ${selected?'aria-current="page"':''}>
 <span class="compact-title">${esc(t.title)}</span><span class="console-tail" title="${esc(p.source==='line'?'Latest Session line · '+p.age:p.source==='unavailable'?'Activity unavailable · Task summary':'Task summary')}
${esc(p.text)}">${esc(p.text)}</span>
 <span class="console-meta"><span>${t.key}</span><span class="console-state">${p.label}</span>${a.pr?`<span>${a.pr}</span>`:''}<span class="console-slot"><span class="console-age">${esc(p.age)}</span>${slot>0&&slot<10?`<kbd class="task-number">⌘${slot}</kbd>`:''}</span></span></button>`;
}
// Task tools follow the LOO-497 contract: the Session, one Shell tool and one Files
// tool per Task. Tabs live inside Shell and Files; showing a tool adds it to the
// arrangement and hiding it undoes that split. Brief and Changes are Files tabs.
function taskTools(t){
 state.tools=state.tools||{};
 return state.tools[t.id]=state.tools[t.id]||{shell:false,files:false,shellTabs:['zsh'],shellSel:0,fileTabs:['Changes','Brief','WorkNavigator.swift'],fileSel:0};
}
function consoleTabs(t){
 if(t.started===false)return '';const k=taskTools(t);
 return ['shell','files'].map(tool=>`<button class="tool-toggle" data-tool="${tool}" aria-pressed="${k[tool]}" title="${tool==='shell'?'Shell (⌃`)':'Files (⌘⇧E)'}">${TOOL_ICONS[tool]}<span>${tool==='shell'?'Shell':'Files'}</span></button>`).join('');
}
function toolHeader(t,tool,k,many){
 if(!many)return '';
 if(tool==='session')return `<header class="pane-head"><span class="pane-kind">${esc(t.agent)}</span><span class="pane-note">Session</span></header>`;
 const tabs=tool==='shell'?k.shellTabs:k.fileTabs,sel=tool==='shell'?k.shellSel:k.fileSel;
 return `<header class="pane-head"><span class="pane-kind">${TOOL_ICONS[tool]}</span><div class="pane-tabs" role="tablist">${tabs.map((name,i)=>`<button role="tab" data-tool-tab="${tool}:${i}" aria-selected="${i===sel}">${esc(name)}</button>`).join('')}${tool==='shell'?'<button class="pane-add" data-tool-add="shell" title="New shell tab (⌘T)" aria-label="New shell tab">+</button>':''}</div><button class="pane-close" data-tool-close="${tool}" title="Hide ${tool==='shell'?'Shell':'Files'}" aria-label="Hide ${tool==='shell'?'Shell':'Files'}">×</button></header>`;
}
function shellBody(t,k){
 const name=k.shellTabs[k.shellSel],a=consoleActivity[t.key];
 const lines=k.shellSel===0?[`<span class="dim">~/src/${esc(t.repo)}.${esc(a.branch)}</span> <span class="ok">%</span> cargo watch -x 'test -p loopflow'`,'<span class="dim">[Running cargo test -p loopflow]</span>','test work::selection::defaults_keep_started_idle_and_paused_work … <span class="ok">ok</span>','test work::selection::first_visible_occurrence_wins … <span class="ok">ok</span>','<span class="ok">test result: ok.</span> 37 passed; 0 failed','<span class="dim">[Finished running. Watching for changes]</span>']:[`<span class="dim">~/src/${esc(t.repo)}.${esc(a.branch)}</span> <span class="ok">%</span> <span class="caret"></span>`];
 return `<div class="tool-body shell-body" aria-label="Shell ${esc(name)}">${lines.join('<br>')}</div>`;
}
function filesBody(t,k){
 const name=k.fileTabs[k.fileSel];
 if(name==='Brief')return `<div class="tool-body doc-body">${t.id==='489'?`<h3>Outcome</h3><p>Switch between working Tasks without losing your input or place.</p><h3>Not in this Task</h3><p>Custom dashboards, multi-work grids and the public API belong to LOO-497.</p>`:`<h3>Outcome</h3><p>${esc(t.prompt)}</p>`}</div>`;
 if(name==='Changes')return `<div class="tool-body doc-body"><p class="scope-notice">Sample diff. Native Files shows the real diff in this tab.</p>${[['swift/LoopflowMac/Views/WorkNavigator.swift',212,180],['swift/LoopflowMac/WorkModel.swift',64,22],['scratch/navigation-studies/app.js',96,40]].map(([f,a,d])=>`<div class="diff-file"><span>${esc(f)}</span><span class="add">+${a}</span><span class="del">−${d}</span></div>`).join('')}</div>`;
 return `<div class="tool-body code-body">${['struct WorkNavigator: View {','    @Environment(WorkModel.self) private var work','','    var body: some View {','        List(selection: $work.selection) {','            Section("Working tasks") {','                ForEach(work.workingSet) { TaskRow(task: $0) }','            }','            Section("Waves") { … }','        }','    }','}'].map((l,i)=>`<span class="ln">${i+1}</span>${esc(l)}`).join('<br>')}</div>`;
}
function sessionPane(t){
 const a=consoleActivity[t.key];
 return `<div class="console-terminal" aria-label="Session terminal">
 <div class="console-location">${esc(a.branch)} · ~/src/${esc(t.repo)}.${esc(a.branch)}</div>
 <p class="console-prompt"><span>›</span> ${esc(t.prompt)}</p>
 <div class="console-operations">${a.work.map(([verb,file,result])=>`<div><span class="console-bullet">●</span> <strong>${verb}</strong> <span>${esc(file)}</span>${result?`<small>└ ${esc(result)}</small>`:''}</div>`).join('')}</div>
 <p>${esc(t.reply)}</p><div class="console-input"><span aria-hidden="true">›</span><textarea id="draft" aria-label="Message draft for ${t.key}" rows="1" spellcheck="false">${esc(state.drafts[t.id]||'')}</textarea></div>
 </div>`;
}
function toolWorkspace(t){
 const k=taskTools(t),many=k.shell||k.files;
 const pane=(tool,body)=>`<section class="tool-pane tool-${tool}">${toolHeader(t,tool,k,many)}${body}</section>`;
 let left=pane('session',sessionPane(t));
 if(k.shell)left=`<div class="split split-h">${left}<div class="split-divider" aria-hidden="true"></div>${pane('shell',shellBody(t,k))}</div>`;
 if(k.files)return `<div id="console-workspace" class="split split-v">${left}<div class="split-divider" aria-hidden="true"></div>${pane('files',filesBody(t,k))}</div>`;
 return `<div id="console-workspace" class="${k.shell?'split-host':''}">${left}</div>`;
}
function renderCompanion(){
 const t=currentTask();if(!t||t.started===false)return;
 const draft=$('#draft');if(draft)state.drafts[t.id]=draft.value;
 $('#title-tabs').innerHTML=consoleTabs(t);
 $('#main').innerHTML=center();persist();
}
function toolClick(d){
 const t=currentTask();if(!t)return;const k=taskTools(t);
 if(d.tool)k[d.tool]=!k[d.tool];
 else if(d.toolClose)k[d.toolClose]=false;
 else if(d.toolAdd){k.shellTabs.push(`zsh ${k.shellTabs.length+1}`);k.shellSel=k.shellTabs.length-1}
 else if(d.toolTab){const [tool,i]=d.toolTab.split(':');k[tool==='shell'?'shellSel':'fileSel']=Number(i)}
 renderCompanion();
}
// Repository and Wave conversations. The repository conversation is where new work
// starts, so it keeps a permanent row above the working set; a Wave page pairs its
// conversation with its Tasks. Neither scope owns shells or files (LOO-497).
function draftKey(){const r=state.route;return r.kind==='task'?r.id:r.kind==='wave'?`wave:${r.id}`:'repo'}
function scopeConversation(){
 const c=state.route.kind==='wave'?SCOPE_CHATS[state.route.id]||SCOPE_CHATS.product:SCOPE_CHATS.repo;
 return `<div class="console-terminal scope-chat" aria-label="${state.route.kind==='wave'?'Wave':'Repository'} conversation">
 <div class="console-location">${esc(c.agent)} · ${esc(c.where)}</div>
 <p class="console-prompt"><span>›</span> ${esc(c.prompt)}</p>
 <div class="console-operations">${c.work.map(([verb,what,result])=>`<div><span class="console-bullet">●</span> <strong>${verb}</strong> <span>${esc(what)}</span>${result?`<small>└ ${esc(result)}</small>`:''}</div>`).join('')}</div>
 <p>${esc(c.reply)}</p><div class="console-input"><span aria-hidden="true">›</span><textarea id="draft" aria-label="Message draft" rows="1" spellcheck="false">${esc(state.drafts[draftKey()]||'')}</textarea></div></div>`;
}
function waveWorkspace(){
 const count=tasks.filter(t=>t.wave===state.route.id).length;
 return `<div id="console-workspace" class="split split-v wave-split"><section class="tool-pane tool-session"><header class="pane-head"><span class="pane-kind">${WAVE_ICON}</span><span class="pane-note">${esc(labelWave(state.route.id))} conversation</span></header>${scopeConversation()}</section><div class="split-divider" aria-hidden="true"></div><section class="tool-pane tool-files wave-tasks"><header class="pane-head"><span class="pane-note">Tasks</span><span class="pane-count">${count}</span></header>${consoleOverview()}</section></div>`;
}
function consoleCenter(t){
 if(t.started===false)return `<div class="overview console-overview unstarted-brief"><div class="plan-status">Not started · ${esc(t.key)}</div><h2>${esc(t.title)}</h2><h3>Outcome</h3><p>${esc(t.prompt)}</p><h3>Before starting</h3><p>Review the brief and choose the next step. No conversation or checkout has been started.</p><button class="start-task" data-action="start">Start Task…</button></div>`;
  return toolWorkspace(t);
}
function consoleOverview(){
 const list=state.route.kind==='wave'?tasks.filter(t=>t.wave===state.route.id):tasks;
 const started=list.filter(t=>t.started),planned=list.filter(t=>!t.started);
 const goals={product:'Make creating and switching between work feel natural.',infrastructure:'Keep Sessions and their tools reliable through interruption and change.',editor:'Keep writing fast and dependable.',sharing:'Make work easy to share and take elsewhere.'};
 function group(title,items){return `<section class="wave-task-group"><h3>${title}<span>${items.length}</span></h3><div class="work-table">${items.map(t=>`<button class="table-row" data-go="task" data-id="${t.id}"><span class="dot ${t.started?(t.paused?'paused':t.state):'planned'}"></span><span class="title">${esc(t.title)}<small>${t.started?esc(consolePreview(t).label)+' · ':''}${esc(t.detail)}</small></span><code>${t.key}</code></button>`).join('')||'<p class="empty-group">No Tasks in this section.</p>'}</div></section>`}
 return `<div class="overview console-overview"><h2>${state.route.kind==='wave'?WAVE_ICON:''}${esc(routeName())}</h2><p>${esc(goals[state.route.id]||'All repository work.')}<br>${list.length} Tasks · ${planned.length} not started · ${started.length} started</p>${group('Started',started)}${group('Not started',planned)}</div>`;
}
function consoleActions(){
 const t=tasks.find(t=>t.id===actionTaskId)||currentTask();if(!t)return [];
 return [
 ...(t.started===false?[{id:'start',label:'Start Task…',key:''}]:[]),
 {id:'new-conversation',label:'New conversation',key:''},
 {id:'editor',label:'Open in editor',key:''},
 {id:'move',label:'Move to Wave…',key:''},
 {id:'complete',label:'Complete Task',key:''},
 {id:'copy-id',label:`Copy ${t.key}`,key:''},
 ...(consoleActivity[t.key].branch?[{id:'copy-branch',label:'Copy branch name',key:''}]:[]),
 ...(consoleActivity[t.key].pr?[{id:'pr',label:`Open PR ${consoleActivity[t.key].pr}`,key:''}]:[]),
 {id:'wave',label:`Open ${labelWave(t.wave)} Wave`,key:''}];
}
function openActions(t=currentTask()){
 if(!t)return;actionTaskId=t.id;
 $('#action-title').textContent=t.key+' · Task actions';
 $('#action-options').innerHTML=consoleActions().map(a=>`<button data-action="${a.id}">${esc(a.label)} <kbd>${a.key}</kbd></button>`).join('')+'<p class="action-study-note">Sample Task · external actions are previews</p>';
 $('#actions').showModal();
}
async function runAction(id){
 const t=id==='start'?currentTask():tasks.find(t=>t.id===actionTaskId)||currentTask();if(!t)return;
 if(id==='start')notify(`Preview: choose the next execution step for ${t.key}. No Task started.`);
 else if(id==='wave')navigate({kind:'wave',id:t.wave});
 else if(id==='pr')notify(`Preview: open the recorded PR for ${t.key}. No external page opened.`);
 else if(id==='new-conversation')notify(`Preview: start a new conversation in ${t.key}; keep the existing one. No agent started.`);
 else if(id==='editor')notify(`Preview: open ${t.key}’s checkout in the configured editor.`);
 else if(id==='complete')notify(`Preview: complete ${t.key} through its Task operation. No Task changed.`);
 else if(id==='move'){$('#action-title').textContent=`Move ${t.key} to Wave`;$('#action-options').innerHTML=waves.map(w=>`<button data-action="move:${w}" ${w===t.wave?'disabled':''}>${labelWave(w)}${w===t.wave?' · current':''}</button>`).join('')+'<p class="action-study-note">Sample Task · ownership is unchanged</p>';$('#actions').showModal()}
 else if(id.startsWith('move:'))notify(`Preview: move ${t.key} to ${labelWave(id.slice(5))}. No Task changed.`);
 else if(id.startsWith('copy-')){try{await navigator.clipboard.writeText(id==='copy-id'?t.key:consoleActivity[t.key].branch);notify('Copied')}catch{notify('Clipboard unavailable')}}
}
function consoleSearchResults(){
 const q=$('#query').value.trim().toLowerCase();paletteRows=[];let html='';
 function group(label,items){if(!items.length)return;html+=`<div class="result-group">${label}</div>`;for(const item of items){paletteRows.push(item.route);html+=`<button data-result="${paletteRows.length-1}"><span>${esc(item.label)}<small>${esc(item.detail)}</small></span><span>${esc(item.key||'↵')}</span></button>`}}
 {
 if(paletteMode==='all'){
 const ranked=NavStudy.navigatorTasks(state,repository.id,q);
 const item=t=>({label:t.title,detail:`${t.key} · ${t.repo} · ${labelWave(t.wave)}`,route:{kind:'task',id:t.id,repo:t.repo}});
 if(q)group('Tasks',ranked.map(item));
 else{group('Working tasks · recent first',ranked.filter(t=>t.repo===repository.id&&NavStudy.workingIds(state).includes(t.id)).map(item));group('Other tasks',ranked.filter(t=>t.repo!==repository.id||!NavStudy.workingIds(state).includes(t.id)).map(item));}
 }
 if(paletteMode!=='repositories')group('Waves',waves.filter(w=>!q||w.includes(q)).map(w=>({label:labelWave(w),detail:repository.name,route:{kind:'wave',id:w}})));
 
 group('Repositories',repositories.filter(r=>!q||r.name.includes(q)).map(r=>({label:r.name,detail:r.id===repository.id?'This repository':'Open repository window',route:{kind:'home',id:'all',repo:r.id}})));
 }
 $('#results').innerHTML=html||'<p class="empty">No results.</p>';highlight=Math.min(highlight,Math.max(0,paletteRows.length-1));$('#results').querySelector(`[data-result="${highlight}"]`)?.classList.add('highlighted');$('#save-query').innerHTML='<span>↑↓ choose · ↵ open · Esc return</span>';
}
$('#activity-example').onchange=()=>renderSidebar();
document.addEventListener('keydown',e=>{
 if(variant!=='h'||visualStyle!=='console')return;
 document.body.classList.toggle('show-task-keys',e.metaKey);
 if($('dialog[open]'))return;
 if(e.metaKey&&!e.altKey&&!e.ctrlKey&&e.key==='/'){e.preventDefault();openActions();return}
 let id;
 if(e.metaKey&&!e.altKey&&!e.ctrlKey&&!e.shiftKey&&/^[1-9]$/.test(e.key))id=NavStudy.workingIds(state)[Number(e.key)-1];
 if(e.ctrlKey&&!e.metaKey&&!e.altKey&&e.key==='Tab'){const at=NavStudy.workingIds(state).indexOf(currentTask()?.id);id=NavStudy.workingIds(state)[(at+(e.shiftKey?-1:1)+NavStudy.workingIds(state).length)%NavStudy.workingIds(state).length]}
 if(e.metaKey&&!e.altKey&&!e.ctrlKey&&!e.shiftKey&&/^[1-9]$/.test(e.key))e.preventDefault();
 if(id){e.preventDefault();navigate({kind:'task',id});return}
 if(e.target.closest('#sidebar')&&['ArrowUp','ArrowDown','Home','End'].includes(e.key)){e.preventDefault();const rows=[...$('#sidebar').querySelectorAll('[data-go="task"]')],at=rows.indexOf(e.target.closest('button'));const next=e.key==='Home'?0:e.key==='End'?rows.length-1:Math.max(0,Math.min(rows.length-1,at+(e.key==='ArrowDown'?1:-1)));rows[next]?.focus()}
 if(e.target.closest('#sidebar')&&e.key==='Escape')$('#draft')?.focus();
 if(e.target.closest('.console-row')&&e.shiftKey&&e.key==='F10'){e.preventDefault();openActions(tasks.find(t=>t.id===e.target.closest('button').dataset.id))}
});
document.addEventListener('keyup',e=>document.body.classList.toggle('show-task-keys',e.metaKey));
window.addEventListener('blur',()=>document.body.classList.remove('show-task-keys'));
$('#sidebar').addEventListener('contextmenu',e=>{const row=e.target.closest('.console-row');if(!row)return;e.preventDefault();openActions(tasks.find(t=>t.id===row.dataset.id))});
const divider=$('#sidebar-resize');
function resizeSidebar(width){state.sidebarWidth=Math.round(Math.max(200,Math.min(420,innerWidth-400,width)));$('#sidebar').style.width=`${state.sidebarWidth}px`;divider.setAttribute('aria-valuenow',state.sidebarWidth);persist()}
divider.addEventListener('pointerdown',e=>{divider.setPointerCapture(e.pointerId);const start=e.clientX,width=$('#sidebar').getBoundingClientRect().width;const move=ev=>resizeSidebar(width+ev.clientX-start);divider.addEventListener('pointermove',move);divider.addEventListener('pointerup',()=>divider.removeEventListener('pointermove',move),{once:true})});
divider.addEventListener('dblclick',()=>resizeSidebar(240));
divider.addEventListener('keydown',e=>{if(e.key==='ArrowLeft'||e.key==='ArrowRight'){e.preventDefault();resizeSidebar((state.sidebarWidth||240)+(e.key==='ArrowRight'?10:-10))}});
