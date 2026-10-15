const mode=document.body.dataset.mode;
const waves=[
{id:'product',name:'Product',summary:'Make daily work feel effortless.',detail:'A calm home for conversations, decisions and delivery.',tasks:[
{id:'home',code:'LOO-462',title:'Make Loopflow a work-first home',detail:'Explore the sidebar’s next direction',status:'Waiting for you',tone:'waiting',stage:'Design',pr:'PR #1531'},
{id:'handoff',code:'LOO-487',title:'Keep context when work changes hands',detail:'Verify the retained workspace',status:'Working',tone:'working',stage:'Implement',machine:'Orion'},
{id:'capture',code:'LOO-491',title:'Capture an idea without leaving the conversation',detail:'A smaller path from thought to Task',status:'',tone:'',stage:'Ready'}]},
{id:'infrastructure',name:'Infrastructure',summary:'Keep execution dependable.',detail:'Predictable workers, explicit ownership and recoverable progress.',tasks:[
{id:'recover',code:'LOO-475',title:'Recover a worker after disconnection',detail:'Review reconnection behavior',status:'Waiting for you',tone:'waiting',stage:'Review',pr:'PR #1528'},
{id:'dispatch',code:'LOO-482',title:'Send work to the right Machine',detail:'Checking destination ownership',status:'Working',tone:'working',stage:'Verify',machine:'Lyra'}]},
{id:'intelligence',name:'Intelligence',summary:'Bring the right context forward.',detail:'Useful memory and clear evidence across long-running work.',tasks:[
{id:'memory',code:'LOO-479',title:'Make yesterday’s decisions useful today',detail:'Shape a durable memory model',status:'',tone:'',stage:'Design'},
{id:'evidence',code:'LOO-489',title:'Show where an answer came from',detail:'Trace evidence across conversations',status:'',tone:'',stage:'Ready'}]}
];
const tasks=waves.flatMap(w=>w.tasks.map(t=>({...t,wave:w.id})));
let selected=mode==='focus'?'repo':'home',scope='product',collapsed=new Set(['intelligence']),history=[],future=[],query='',drafts={};
const lastInWave={product:mode==='focus'?'product':'home'},viewStates={},waveScroll={};
const titles={outline:['01','Continuous outline','One connected spine. Burgundy Wave headings carry the hierarchy; generous rows carry the work.'],focus:['02','Focus sidebar','Move into a Wave instead of indenting further. Keep the path home visible, and give the current work room.'],atlas:['03','Wave / Task navigator','Give each level a place. A persistent Wave column keeps sibling work one click away.']};
const descriptions={outline:'Try selecting the repo, folding a Wave, then opening a Task.',focus:'Try entering a Wave, opening a Task, then selecting an ancestor.',atlas:'Open a Task in two Waves. Switch back: each Wave remembers where you left it.'};
const files={outline:'01-continuous-outline.html',focus:'02-focus-sidebar.html',atlas:'03-wave-task-navigator.html'};
const [number,name,description]=titles[mode];
if(mode==='focus')document.body.classList.add('dark');
function esc(s){return String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));}
function waveFor(id){return waves.find(w=>w.id===id)||waves.find(w=>w.tasks.some(t=>t.id===id));}
function current(){return selected==='repo'?{id:'repo',title:'Loopflow',kind:'Repository',summary:'Conversations become work. Work keeps its context.'}:waves.find(w=>w.id===selected)?{...waves.find(w=>w.id===selected),title:waveFor(selected).name,kind:'Wave',summary:waveFor(selected).detail}:{...tasks.find(t=>t.id===selected),kind:'Task'};}
function attrs(id){return `data-select="${id}" aria-current="${selected===id?'page':'false'}"`;}
function selectedClass(id){return selected===id?' selected':'';}
function status(t){return t.status?`<span class="status ${t.tone}">${t.status}</span>`:'';}
function matches(t,w){return !query||`${t.title} ${t.code} ${t.detail} ${w.name} ${t.status} ${t.stage}`.toLowerCase().includes(query.toLowerCase());}
function taskrow(t,w){return `<button class="taskrow${selectedClass(t.id)}" ${attrs(t.id)} data-preview="${t.id}"><span class="title">${esc(t.title)}</span><span class="detail">${esc(t.detail)}</span><span class="metarow"><span class="mono">${t.code}</span><span class="dotsep">·</span><span>${t.stage}</span>${status(t)}${t.machine?`<span>on ${t.machine}</span>`:''}${t.pr?`<span>${t.pr}</span>`:''}</span></button>`;}
function repo(){return `<button class="repo${selectedClass('repo')}" ${attrs('repo')}><span class="eyebrow">Repository</span><span class="selected-word">Selected</span><span class="title">Loopflow</span><span class="meta">3 Waves <span class="dotsep">/</span> 7 Tasks <span class="dotsep">/</span> 2 need you</span></button>`;}
function outline(){return repo()+`<div class="section-label"><span>Work in motion</span><span>7 Tasks</span></div>`+waves.map(w=>{
const children=w.tasks.filter(t=>matches(t,w));if(query&&!children.length)return '';
const folded=collapsed.has(w.id)&&!query;
return `<section class="wavegroup${waveFor(selected)?.id===w.id?' context':''}"><div class="waverow"><button class="waveopen${selectedClass(w.id)}" ${attrs(w.id)}><span class="name">${w.name}</span><span class="desc">${w.summary}</span></button><button class="fold" data-fold="${w.id}" aria-expanded="${!folded}" aria-controls="children-${w.id}" aria-label="${folded?'Show':'Hide'} ${w.name} Tasks">${w.tasks.length} Tasks<br>${folded?'Show +':'Hide −'}</button></div><div class="tasklist" id="children-${w.id}" ${folded?'hidden':''}>${children.map(t=>taskrow(t,w)).join('')}</div></section>`;
}).join('');}
function focus(){const w=selected==='repo'&&!query?null:waves.find(w=>w.id===scope);
if(!w&&!query)return repo()+`<div class="section-label"><span>Choose a Wave</span><span>Enter to focus</span></div>`+waves.map(w=>`<button class="portal" ${attrs(w.id)}><span class="portal-head"><span class="name">${w.name}</span><span class="enter">↗</span></span><span class="desc">${w.detail}</span><span class="portal-foot"><span>${w.tasks.length} Tasks</span><span>${w.tasks.filter(t=>t.tone==='waiting').length? '1 needs you':'Ready to explore'}</span></span></button>`).join('');
if(query)return `<div class="scope-trail"><button class="ancestor" ${attrs('repo')}>Loopflow <span>Repository</span></button></div><div class="section-label"><span>Across all Waves</span></div>`+waves.map(w=>{let found=w.tasks.filter(t=>matches(t,w));return found.length?`<button class="ancestor" ${attrs(w.id)}>${w.name} <span>Open Wave ↗</span></button>${found.map(t=>taskrow(t,w)).join('')}`:''}).join('');
return `<div class="scope-trail"><button class="ancestor${selectedClass('repo')}" ${attrs('repo')}>Loopflow <span>All Waves ↖</span></button></div><button class="scopehero${selectedClass(w.id)}" ${attrs(w.id)}><span class="eyebrow">Wave</span><h2>${w.name}</h2><p>${w.detail}</p><div class="scope-meta">${w.tasks.length} Tasks · Wave conversation ↗</div></button><div class="section-label"><span>In this Wave</span><span>${w.tasks.length} Tasks</span></div>${w.tasks.map(t=>taskrow(t,w)).join('')}`;
}
function atlas(){let w=waves.find(w=>w.id===scope);const found=query?waves.flatMap(w=>w.tasks.filter(t=>matches(t,w)).map(t=>({t,w}))):w.tasks.map(t=>({t,w}));
return repo()+`<div class="atlas-columns"><div class="wave-rail"><div class="eyebrow">Waves</div>${waves.map((wave,i)=>`<button class="wave-tab${scope===wave.id?' context':''}${selectedClass(wave.id)}" data-switch="${wave.id}" aria-label="${esc('Return to '+wave.name+(lastInWave[wave.id]?' — '+(tasks.find(t=>t.id===lastInWave[wave.id])?.title||'Wave conversation'):''))}"><span class="ordinal">0${i+1} <span class="shortcut">⌥ ${i+1}</span></span><span class="name">${wave.name}</span><small>${wave.tasks.length} Tasks${wave.tasks.some(t=>t.tone==='waiting')?' · 1 needs you':''}</small><span class="resume-label">${lastInWave[wave.id]?(tasks.find(t=>t.id===lastInWave[wave.id])?.code||'Wave conversation'):'Open Wave'}</span></button>`).join('')}</div><div class="task-plane">${query?'<div class="section-label"><span>Search · all Waves</span></div>':`<button class="plane-heading${selectedClass(w.id)}" ${attrs(w.id)}><span class="eyebrow">Current Wave</span><span class="name">${w.name}</span><span class="desc">${w.summary}</span><span class="visit">Open Wave conversation ↗</span></button>`}${found.length?found.map(({t,w})=>taskrow(t,w)).join(''):'<p class="no-results">No matching work.</p>'}</div></div>`;}
function renderSidebar(){const content=({outline,focus,atlas}[mode])();document.querySelector('.sidebar-body').innerHTML=content+(query&&!tasks.some(t=>matches(t,waveFor(t.id)))&&mode!=='atlas'?'<p class="no-results">No matching work.</p>':'');}
function renderWork(){const item=current(),w=waveFor(selected);
const crumbs=`<button data-select="repo">Loopflow</button>${w?`<span>/</span><button data-select="${w.id}">${w.name}</button>`:''}${item.kind==='Task'?`<span>/</span><span>${item.code}</span>`:''}`;
const question=item.kind==='Task'?(item.id==='home'?'The hierarchy should feel continuous. Give the work more room to breathe.':`What is the next useful step for “${item.title}”?`):item.kind==='Wave'?`Where should we focus in ${item.title}?`:'What deserves our attention today?';
const reply=item.id==='home'?'The sidebar can show more than where work lives. It can make the next decision visible.<p>We’re exploring three ways to connect the repository, its Waves and the Tasks within them. Each gives the parent a place of its own.</p>':item.kind==='Wave'?`${w.detail}<p>There are ${w.tasks.length} Tasks here. ${w.tasks.some(t=>t.tone==='waiting')?'One is ready for a decision; the others can keep moving.':'The next step is to choose which idea deserves a closer look.'}</p>`:item.kind==='Repository'?'Product has a design ready to explore. Infrastructure has a recovery change ready for review.<p>Choose a Wave to see its direction, or open a Task to pick up exactly where the conversation left off.</p>':`${item.detail}.<p>The conversation, files and draft belong to this Task. Switching to other work keeps them here for when you return.</p>`;
const insight=item.kind==='Task'?`${item.stage} · ${item.status||'No reported activity'}${item.machine?' · '+item.machine:''}`:item.kind==='Wave'?`${w.tasks.length} Tasks · ${w.summary}`:'3 Waves · 7 Tasks · 2 decisions waiting';
document.querySelector('.worktop').innerHTML=`<button class="back" data-back aria-label="Go Back" ${history.length?'':'disabled'}>←</button><button class="back" data-forward aria-label="Go Forward" ${future.length?'':'disabled'}>→</button><nav class="breadcrumbs" aria-label="Current location">${crumbs}</nav><span class="preview-label">Sample workspace</span>`;
document.querySelector('.workbody').innerHTML=`<div class="kindline"><span class="eyebrow">${item.kind}${item.code?' / '+item.code:''}</span>${item.kind==='Task'?status(item):''}</div><h2>${esc(item.title)}</h2><p class="subtitle">${esc(item.summary||item.detail)}</p><div class="work-tabs"><strong>Conversation</strong><span class="muted">Retained workspace</span></div><div class="conversation"><div class="message-label">You</div><div class="message user">${esc(question)}</div><div class="message-label">Loopflow</div><div class="message">${reply}</div><div class="insight"><div class="insight-label">${item.kind==='Task'?'Next step':'At a glance'}</div><p>${esc(insight)}</p></div></div>`;
const area=document.querySelector('textarea');area.value=drafts[selected]||'';area.placeholder=`Continue the ${item.kind.toLowerCase()} conversation…`;area.setAttribute('aria-label',`Draft for ${item.title}`);document.querySelector('#draft-owner').textContent=`${item.kind} draft · stays here when you navigate`;
}
function saveContext(){
const area=document.querySelector('textarea');
viewStates[selected]={scroll:document.querySelector('.workbody').scrollTop,start:area.selectionStart,end:area.selectionEnd,height:area.style.height};
const plane=document.querySelector('.task-plane');if(plane&&!query)waveScroll[scope]=plane.scrollTop;
const w=waveFor(selected);if(w)lastInWave[w.id]=selected;
}
function restoreContext(){
const state=viewStates[selected],area=document.querySelector('textarea');
document.querySelector('.workbody').scrollTop=state?.scroll||0;
area.style.height=state?.height||'';area.setSelectionRange(state?.start||0,state?.end||0);
const plane=document.querySelector('.task-plane');if(plane)plane.scrollTop=waveScroll[scope]||0;
}
function navigate(id,push=true){
if(id===selected)return;saveContext();
if(push){history.push({selected,scope});future=[];}
selected=id;scope=waveFor(id)?.id||scope;
const w=waveFor(id);if(w)lastInWave[w.id]=id;
query='';document.querySelector('.search').value='';render();
}
function traverse(back){
const from=back?history:future,to=back?future:history;if(!from.length)return;
saveContext();to.push({selected,scope});const state=from.pop();selected=state.selected;scope=state.scope;
const w=waveFor(selected);if(w)lastInWave[w.id]=selected;
query='';document.querySelector('.search').value='';render();
}
function render(){renderSidebar();renderWork();restoreContext();}
document.body.innerHTML=`<header class="labbar"><a href="index.html" class="labmark"><span class="mark">ƒ</span><span class="lab-title">Loopflow / Sidebar studies</span></a><nav aria-label="Prototype directions">${Object.entries(titles).map(([key,v])=>`<a class="${key===mode?'active':''}" href="${files[key]}" ${key===mode?'aria-current="page"':''}>${v[0]} ${key==='outline'?'Outline':key==='focus'?'Focus':'Columns'}</a>`).join('')}</nav><button class="theme" aria-label="Toggle light and dark appearance">${document.body.classList.contains('dark')?'Light':'Dark'} appearance</button></header><section class="intro"><h1><span class="number">${number}</span>${name}</h1><p>${description}</p></section><div class="app"><aside class="sidebar" aria-label="Work navigation"><div class="side-top"><span class="windowdots" aria-hidden="true"><i></i><i></i><i></i></span><span class="eyebrow">Work, with context</span></div><nav class="sidebar-body" aria-label="Repository, Waves and Tasks"></nav><div class="searchwrap"><input class="search" type="search" aria-label="Find Work" placeholder="Find a Task or Wave…"><div class="searchhint"><span>Search across all Waves</span><span>⌘ K</span></div></div></aside><main class="workspace"><div class="worktop"></div><div class="workbody"></div><div class="composer"><textarea aria-label="Conversation draft" placeholder="Continue the conversation…"></textarea><div class="composer-foot"><span id="draft-owner"></span><span>Local prototype · not sent</span></div></div></main></div><footer class="footer"><span><b>Try it:</b> ${descriptions[mode]}</span><span>Fictional work · <a href="index.html">Research & comparison</a></span></footer>`;
document.addEventListener('click',e=>{
const b=e.target.closest('button');if(!b)return;
if(b.hasAttribute('data-select'))navigate(b.dataset.select);
if(b.hasAttribute('data-switch'))navigate(lastInWave[b.dataset.switch]||b.dataset.switch);
if(b.hasAttribute('data-fold')){let id=b.dataset.fold;collapsed.has(id)?collapsed.delete(id):collapsed.add(id);renderSidebar();}
if(b.hasAttribute('data-back'))traverse(true);
if(b.hasAttribute('data-forward'))traverse(false);
if(b.classList.contains('theme')){document.body.classList.toggle('dark');b.textContent=`${document.body.classList.contains('dark')?'Light':'Dark'} appearance`;}
});
document.querySelector('.search').addEventListener('input',e=>{query=e.target.value;renderSidebar();});
document.querySelector('textarea').addEventListener('input',e=>{drafts[selected]=e.target.value;});
document.addEventListener('keydown',e=>{if((e.metaKey||e.ctrlKey)&&e.key==='k'){e.preventDefault();document.querySelector('.search').focus();}if(e.altKey&&e.key==='ArrowLeft'){e.preventDefault();traverse(true);}if(e.altKey&&e.key==='ArrowRight'){e.preventDefault();traverse(false);}if(mode==='atlas'&&e.altKey&&/^Digit[123]$/.test(e.code)){e.preventDefault();const w=waves[Number(e.code.slice(-1))-1];navigate(lastInWave[w.id]||w.id);}});
render();
