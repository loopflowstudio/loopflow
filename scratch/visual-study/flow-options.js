// Flow preview study for an unstarted Task. Presentation only: Start never launches work.
// Active only with ?flowstudy=a|b|c. Parent calls window.renderTaskFlow(task).
(function(){
  const option = new URLSearchParams(location.search).get("flowstudy");
  const active = ["a","b","c"].includes(option);
  const initialExecution = new URLSearchParams(location.search).get("execution");
  let executionState = ["running","paused","blocked"].includes(initialExecution) ? initialExecution : null;
  let runningStep = "feature-implement";
  let pausedNext = "feature-compress";
  let lastStep = "feature-implement";
  const studyTask = new URLSearchParams(location.search).get("task") || "LOO-285";
  // Only this explicitly simulated Task is unstarted; captured text stays intact.
  if (active && typeof TASKS !== "undefined" && TASKS[studyTask]) TASKS[studyTask].started = Boolean(executionState);

  // Illustrative local definitions; not a claim about any Wave's configured default.
  const FLOWS = {
    build: {name:"Build", summary:"Plan, implement, review and demonstrate one slice.", steps:[
      {id:"kickoff", name:"Kickoff", purpose:"Plan the slice", detail:"Reads the Task, its Wave and recent evidence, then writes the slice plan into scratch."},
      {id:"implement", name:"Implement", group:"Code", purpose:"Build the slice", detail:"Builds the planned slice with one focused behavioral proof."},
      {id:"compress", name:"Compress", group:"Code", purpose:"Simplify", detail:"Looks for code that can be removed or simplified without changing behavior."},
      {id:"review", name:"Review slice", purpose:"Check against Done when", detail:"Checks the slice against the design's Done when and records findings."},
      {id:"demo", name:"Demo", purpose:"Show it working", detail:"Shows the result on the real configured path."},
    ]},
    "task-design": {name:"Task design", summary:"Shape the Task, then wait for your approval.", steps:[
      {id:"kickoff", name:"Kickoff", purpose:"Draft the design", detail:"Reads the Task and its Wave, then drafts a design in scratch."},
      {id:"review-design", name:"Review design", purpose:"Human approval", human:true, detail:"Review the design before implementation. This opening step runs once."},
    ]},
    code: {name:"Code", summary:"Implement and compress, without planning or review.", steps:[
      {id:"implement", name:"Implement", purpose:"Build the change", detail:"Builds the change with one focused behavioral proof."},
      {id:"compress", name:"Compress", purpose:"Simplify", detail:"Looks for code that can be removed or simplified without changing behavior."},
    ]},
  };
  // Based on the sibling definitions; human direction overrides their design/demo return edges.
  FLOWS.feature = {name:"Feature", summary:"Design, iterate, and review with you.", steps:[
    {id:"feature-kickoff",name:"Kickoff",group:"Task design",purpose:"Draft the design",detail:"Prepare the design before implementation."},
    {id:"feature-design-review",name:"Review design",group:"Task design",purpose:"Human review",human:true,detail:"Review the design, then advance to implementation. This opening step runs once."},
    {id:"feature-implement",name:"Implement",group:"Pursue",purpose:"Build the change",detail:"Implement the next slice using the design and any direction from the preceding pass."},
    {id:"feature-compress",name:"Compress",group:"Pursue",purpose:"Simplify",detail:"Simplify the implementation while preserving the intended behavior."},
    {id:"feature-review",name:"Review slice",group:"Pursue",purpose:"Check the result",detail:"Review behavior against the design and recorded evidence."},
    {id:"feature-concepts",name:"Concept review",group:"Pursue",purpose:"Review the model",detail:"Examine the product concepts and ownership; provide findings for the decision step."},
    {id:"feature-decide",name:"Loop decide",group:"Pursue",purpose:"Advance or iterate",repeat:"feature-implement",detail:"Advance reaches human Demo. Iterate returns to Implement with direction. If blocked, ask for help through an Unblock Session; its completion returns evidence for reassessment, not approval."},
    {id:"feature-demo",name:"Demo",group:"Pursue",purpose:"Human review",human:true,detail:"Review the result after the implementation loop advances. Demo runs once; advancing finishes this Flow without itself merging or shipping."},
  ]};
  const ORDER = option === "c" ? ["feature","build","task-design","code"] : ["build","task-design","code"];

  const store = {};
  function st(id){
    const k = option+":"+id;
    return store[k] || (store[k] = {flow:option==="c"?"feature":"build", step:null, picker:false, query:"", index:0, comments:false, confirm:false, restarting:false, restartChoice:null, restartNote:false, sessionNote:false});
  }
  const esc = s => String(s).replace(/[&<>"']/g, c => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c]));
  const human = `<svg class="fs-human" viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="5" r="2.6" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M3.2 13.6c.6-2.6 2.5-4 4.8-4s4.2 1.4 4.8 4" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>`;
  const chev = `<svg class="fs-chev" viewBox="0 0 16 16" aria-hidden="true"><path d="M6 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>`;

  // Attributes shared by every control: action, Task and a focus key the host render() restores.
  const a = (act, tid, extra="", key=act) => `data-flow-act="${act}" data-flow-task="${esc(tid)}" data-fk="flow-${option}-${esc(tid)}-${key}" ${extra}`;

  function start(t, s, cls="btn primary small"){
    return `<button type="button" class="${cls} fs-start" ${a("start", t.id)}>Start</button>`;
  }
  function newSession(t){
    return `<button type="button" class="btn small" title="Open a plain Session in this Task’s worktree" ${a("new-session",t.id)}>New session</button>`;
  }
  function sessionNote(t,s){
    if (!s.sessionNote) return "";
    return `<div class="fs-session-note" role="status"><span class="fs-proto">Prototype</span><p>A plain Session would open in ${esc(t.id)}’s worktree with this Task as context. ${executionState?"The Flow would keep its current position.":"The Flow would remain unstarted."}</p><button type="button" class="btn small ghost" ${a("session-dismiss",t.id)}>Dismiss</button></div>`;
  }
  function matches(s){
    const q = s.query.trim().toLowerCase();
    return ORDER.filter(k => (FLOWS[k].name+" "+k).toLowerCase().includes(q));
  }
  function flowName(t, s){
    const f = FLOWS[s.flow];
    if (executionState && !s.picker) return `<span class="fs-started-flow"><span class="fs-pinned-name">${esc(f.name)}</span><button type="button" class="fs-restart-trigger" ${a("restart",t.id)} aria-label="Stop and restart with another Flow">Stop &amp; restart…</button></span>`;
    if (!s.picker) return `<div class="fs-flow-name"><button type="button" class="fs-name-button" aria-label="Flow: ${esc(f.name)}" aria-haspopup="listbox" aria-expanded="false" ${a("picker",t.id)}>${esc(f.name)}<span aria-hidden="true">⌄</span></button></div>`;
    const results = matches(s), selected = results[s.index];
    return `<div class="fs-flow-name editing"><input class="fs-flow-search" type="text" role="combobox" aria-label="Find a flow" aria-expanded="true" aria-autocomplete="list" aria-controls="fs-flow-results" ${selected?`aria-activedescendant="fs-option-${selected}"`:""} autocomplete="off" placeholder="Find a flow…" value="${esc(s.query)}" data-flow-search="${esc(t.id)}" data-fk="flow-search-${esc(t.id)}">
      <div class="fs-results" id="fs-flow-results" role="listbox" aria-label="Flows">${results.map((k,i)=>`<button type="button" role="option" tabindex="-1" id="fs-option-${k}" aria-selected="${i===s.index}" class="fs-result${i===s.index?" highlighted":""}" ${a("flow",t.id,`data-flow-value="${k}"`,"flow-"+k)}><span class="fs-result-title">${esc(FLOWS[k].name)}${k===s.flow?'<span class="fs-current">Selected</span>':""}</span><span class="fs-result-description">${esc(FLOWS[k].summary)}</span></button>`).join("") || '<div class="fs-no-results">No matching flows</div>'}</div></div>`;
  }
  function confirm(t, s){
    if (!s.confirm) return "";
    return `<div class="fs-confirm" role="status"><span class="fs-proto">Prototype</span>
      <span>Nothing started. In the app, this would start <strong>${esc(FLOWS[s.flow].name)}</strong> for ${esc(t.id)} · ${esc(t.title)}.</span>
      <button type="button" class="btn ghost small" ${a("dismiss", t.id)}>Dismiss</button></div>`;
  }
  function restartPanel(t,s){
    if (!s.restartChoice) return "";
    const chosen = FLOWS[s.restartChoice];
    if (s.restartNote) return `<div class="fs-restart-panel" role="status"><span class="fs-proto">Prototype</span><p>Would stop ${esc(FLOWS[s.flow].name)} and restart with ${esc(chosen.name)} from its first step. No live work was changed.</p><button type="button" class="btn ghost small" ${a("restart-cancel",t.id)}>Dismiss</button></div>`;
    return `<section class="fs-restart-panel" aria-label="Restart with another Flow"><div class="fs-restart-title">Restart with ${esc(chosen.name)}?</div><p>Stop the current run and start this Flow from the beginning. Keep the Task’s existing work and run history.</p><div class="fs-restart-route">${chosen.steps.map(x=>esc(skillName(x))).join(' <span aria-hidden="true">→</span> ')}</div><div class="fs-restart-actions"><button type="button" class="btn small ghost" ${a("restart-cancel",t.id)}>Cancel</button><button type="button" class="btn small primary" ${a("restart-confirm",t.id)}>Stop &amp; restart</button></div></section>`;
  }
  function comments(t, s){
    return `<section class="fs-comments"><button type="button" class="fs-disclose" aria-expanded="${s.comments}" aria-controls="fs-c-${esc(t.id)}" ${a("comments", t.id)}>${chev}Comments <span class="fs-comment-count">0</span></button>
      ${s.comments?`<div class="fs-comments-body" id="fs-c-${esc(t.id)}"><p>No comments in this study.</p>
        <textarea class="fs-composer" disabled aria-label="Comment (prototype, disabled)" placeholder="Commenting is not part of this prototype"></textarea></div>`:""}</section>`;
  }
  function stepDetail(step){
    if (!step) return "";
    return `<p class="fs-detail">${step.human?`<span class="fs-hbadge">${human}Your approval</span> `:""}<strong>${esc(step.name)}.</strong> ${esc(step.detail)}</p>`;
  }

  // A — compact route
  function renderA(t, s){
    const f = FLOWS[s.flow], cur = f.steps.find(x=>x.id===s.step);
    const pill = x => `<button type="button" class="fs-pill${x.id===cur?.id?" on":""}${x.human?" human":""}" aria-label="${esc(skillName(x))}${x.human?' (human step)':''}" aria-pressed="${x.id===cur?.id}" ${a("step", t.id, `data-flow-value="${x.id}"`, "step-"+x.id)}>${x.human?human:""}${esc(x.name)}</button>`;
    const parts = []; let i = 0;
    while (i < f.steps.length){
      const x = f.steps[i];
      if (x.group){ const g = f.steps.filter(y=>y.group===x.group); parts.push(`<span class="fs-grp"><span class="fs-grp-label">${esc(x.group)}</span><span class="fs-grp-row">${g.map(pill).join('<span class="fs-link" aria-hidden="true"></span>')}</span></span>`); i += g.length; }
      else { parts.push(pill(x)); i++; }
    }
    return `<section class="flow-study fs-a" aria-label="Flow preview">
      <div class="fs-bar"><span class="label-sm">Flow</span>${flowName(t,s)}
        <span class="fs-spacer"></span>${start(t, s)}</div>
      <div class="fs-route" role="group" aria-label="${esc(f.name)} steps, in order">${parts.join('<span class="fs-link" aria-hidden="true"></span>')}</div>
      ${stepDetail(cur)}${confirm(t, s)}</section>`;
  }

  // B — step outline
  function renderB(t, s){
    const f = FLOWS[s.flow];
    let lastGroup = null;
    const items = f.steps.map((x, i) => {
      const open = x.id===s.step;
      const head = x.group && x.group!==lastGroup ? `<li class="fs-ogroup" aria-hidden="true">${esc(x.group)}</li>` : "";
      lastGroup = x.group || null;
      return `${head}<li class="fs-ostep${open?" open":""}${x.human?" human":""}${x.group?" nested":""}">
        <span class="fs-dot" aria-hidden="true">${x.human?human:i+1}</span>
        <button type="button" class="fs-orow" aria-expanded="${open}" ${a("step", t.id, `data-flow-value="${x.id}"`, "step-"+x.id)}>
          <span class="fs-oname">${esc(x.name)}${x.human?' <span class="fs-hbadge">Your approval</span>':""}</span><span class="fs-opurpose">${esc(x.purpose)}</span>${chev}</button>
        ${open?`<p class="fs-odetail">${esc(x.detail)}</p>`:""}</li>`;
    }).join("");
    return `<section class="flow-study fs-b" aria-label="Flow preview">
      <ol class="fs-outline" aria-label="${esc(f.name)} steps, in order">${items}</ol>
      <aside class="fs-launch"><span class="label-sm">Flow</span>${flowName(t,s)}<p class="fs-sum">${esc(f.summary)}</p>
        ${start(t, s, "btn primary fs-wide")}
        ${confirm(t, s)}</aside></section>`;
  }

  const skillName = x => x.name.toLowerCase().replaceAll(" ", "-");
  function executionStepClass(x, f){
    if (!executionState) return "";
    const current = executionState === "blocked" ? "feature-decide" : executionState === "paused" ? pausedNext : runningStep;
    const index = f.steps.findIndex(y=>y.id===x.id), currentIndex = f.steps.findIndex(y=>y.id===current);
    return index < currentIndex ? " fs-step-done" : index === currentIndex ? " fs-step-current "+executionState : "";
  }
  function executionStepBadge(x, f){
    const state = executionStepClass(x,f);
    if (state.includes("fs-step-current")) return `<span class="fs-current-badge">${executionState === "paused" ? "Next" : executionState === "blocked" ? "Blocked" : "Running"}</span>`;
    if (state.includes("fs-step-done")) return '<span class="fs-step-check" aria-label="Completed">✓</span>';
    return "";
  }
  function loopMap(t, s, f){
    const steps = f.steps, returns = steps.filter(x=>x.repeat);
    const pitch = 120, nodeWidth = 106, nodeHeight = 46;
    const width = steps.length*pitch-14, height = 92;
    const regions = returns.map((x,i)=>{
      const left = steps.findIndex(y=>y.id===x.repeat)*pitch-8;
      const right = steps.indexOf(x)*pitch+nodeWidth+8;
      const bottom = 80+i*26;
      const fill = "#527fa819";
      const stroke = "#527fa860";
      return `<rect class="fs-loop-region" x="${left}" y="${-34-i*3}" width="${right-left}" height="${bottom+34+i*3}" rx="18" fill="${fill}" stroke="${stroke}" stroke-width="1"/><text class="fs-loop-heading" x="${(left+right)/2}" y="-14" text-anchor="middle">${executionState?'Loop · Iteration 3':'Loop'}${executionState?'<title>2 completed iterations; currently on iteration 3</title>':''}</text>`;
    }).reverse().join("");
    const edges = returns.map((x,i)=>{
      const from = steps.indexOf(x)*pitch+nodeWidth/2;
      const to = steps.findIndex(y=>y.id===x.repeat)*pitch+nodeWidth/2+i*9;
      const y = 66+i*26, color = "#527fa8", id = "return-"+x.id;
      return `<defs><marker id="${id}" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto"><path d="M0 0 L7 3.5 L0 7" fill="none" stroke="${color}"/></marker></defs><path d="M${from} ${nodeHeight} V${y-7} Q${from} ${y} ${from-7} ${y} H${to+7} Q${to} ${y} ${to} ${y-7} V${nodeHeight+2}" fill="none" stroke="${color}" stroke-width="1.2" marker-end="url(#${id})"/>`;
    }).join("");
    return `<div class="fs-loop-map" role="group" aria-label="Feature Flow"><div class="fs-loop-drawing" style="width:${width}px;height:${height}px"><svg viewBox="0 0 ${width} ${height}" aria-hidden="true">${regions}${edges}</svg><div class="fs-loop-nodes">${steps.map((x,i)=>`${i?'<span class="fs-edge" aria-hidden="true"></span>':""}<button class="fs-node${s.step===x.id?" on":""}${x.human?" human":""}${executionStepClass(x,f)}" aria-label="${esc(skillName(x))}${x.human?' (human step)':''}" aria-pressed="${s.step===x.id}" ${a("step",t.id,`data-flow-value="${x.id}"`,"step-"+x.id)}>${executionStepBadge(x,f)}<span class="fs-nname">${esc(skillName(x))}</span></button>`).join("")}</div><span class="sr">${returns.map(x=>`${x.name}: Iterate returns to ${steps.find(y=>y.id===x.repeat).name}.`).join(" ")}</span></div></div>`;
  }

  // C — flow map
  function renderC(t, s){
    const f = FLOWS[s.flow], cur = f.steps.find(x=>x.id===s.step);
    const node = x => `<button type="button" class="fs-node${x.id===cur?.id?" on":""}${x.human?" human":""}" aria-label="${esc(skillName(x))}${x.human?' (human step)':''}" aria-pressed="${x.id===cur?.id}" ${a("step", t.id, `data-flow-value="${x.id}"`, "step-"+x.id)}>
      ${executionStepBadge(x,f)}<span class="fs-nname">${esc(skillName(x))}</span></button>`;
    const parts = []; let i = 0;
    while (i < f.steps.length){
      const x = f.steps[i];
      if (x.group){ const g = f.steps.filter(y=>y.group===x.group); parts.push(`<div class="fs-mgroup" role="group" aria-label="${esc(x.group)} flow"><span class="fs-mglabel">${esc(x.group)}</span><div class="fs-mgrow">${g.map(node).join('<span class="fs-edge" aria-hidden="true"></span>')}</div></div>`); i += g.length; }
      else { parts.push(node(x)); i++; }
    }
    return `<section class="flow-study fs-c" aria-label="Flow preview">
      <div class="fs-bar"><span class="label-sm">Flow</span>${flowName(t,s)}<span class="fs-sum">${esc(f.summary)}</span>
        <span class="fs-spacer"></span>${executionState?"":start(t, s)}</div>
      ${restartPanel(t,s)}
      ${s.flow==="feature"?loopMap(t,s,f):`<div class="fs-map" role="group" aria-label="${esc(f.name)} steps, in order"><div class="fs-maprow">${parts.join('<span class="fs-edge" aria-hidden="true"></span>')}</div></div>`}
      <div class="fs-inspect">${stepDetail(cur)}</div>${confirm(t, s)}${sessionNote(t,s)}</section>`;
  }

  window.renderTaskSessionAction = task => active && task?.id === studyTask ? newSession(task) : "";

  window.renderTaskFlow = function(task){
    if (!active || !task || task.id !== studyTask) return "";
    const s = st(task.id);
    const flow = option==="a" ? renderA(task, s) : option==="b" ? renderB(task, s) : renderC(task, s);
    if (executionState && window.renderTaskExecution) return flow + window.renderTaskExecution(task, executionState, {skill:skillName(FLOWS.feature.steps.find(x=>x.id===runningStep)),nextSkill:skillName(FLOWS.feature.steps.find(x=>x.id===pausedNext)),lastSkill:skillName(FLOWS.feature.steps.find(x=>x.id===lastStep))});
    return flow + '<div class="fs-run-state"><span class="fs-unstarted-dot" aria-hidden="true"></span><strong>Not started</strong><span class="fs-state-separator" aria-hidden="true">·</span><span>No runs yet</span></div>';
  };
  window.renderTaskComments = function(task){
    if (!active || !task || task.id !== studyTask) return "";
    return comments(task, st(task.id));
  };
  if (!active) return;

  const rerender = () => { if (typeof render==="function") render(); };
  window.addEventListener("task-execution-action", e => {
    if (!executionState) return;
    if (e.detail?.action === "pause") {
      lastStep = runningStep;
      const steps = FLOWS.feature.steps;
      pausedNext = steps[Math.min(steps.findIndex(x=>x.id===runningStep)+1,steps.length-1)].id;
      executionState = "paused";
    }
    else if (e.detail?.action === "resume") { executionState = "running"; runningStep = pausedNext; }
    else return;
    rerender();
    document.querySelector('.task-execution [data-execution-action="'+(executionState === "paused" ? "resume" : "pause")+'"]')?.focus();
    window.parent.postMessage({type:"task-execution-state",state:executionState},location.origin);
  });
  document.addEventListener("click", e => {
    const el = e.target.closest && e.target.closest("[data-flow-act]");
    if (!el) {
      if (!e.target.closest(".fs-flow-name")) {
        const open = Object.values(store).find(s => s.picker);
        if (open) { open.picker = false; rerender(); }
      }
      return;
    }
    const s = st(el.dataset.flowTask), v = el.dataset.flowValue;
    switch (el.dataset.flowAct){
      case "restart": s.restarting = true; s.restartChoice = null; s.restartNote = false; s.picker = true; s.query = ""; s.index = 0; break;
      case "restart-cancel": s.restarting = false; s.restartChoice = null; s.restartNote = false; s.picker = false; break;
      case "restart-confirm": s.restartNote = true; break;
      case "picker": s.restarting = false; s.picker = true; s.query = ""; s.index = 0; break;
      case "flow":
        if (s.restarting) { s.restartChoice = v; s.restartNote = false; }
        else if (v!==s.flow){ s.flow = v; s.step = null; s.confirm = false; }
        s.picker = false; break;
      case "step": s.step = (option==="b" && s.step===v) ? null : v; break;
      case "comments": s.comments = !s.comments; break;
      case "new-session": s.sessionNote = true; break;
      case "session-dismiss": s.sessionNote = false; break;
      case "start": s.confirm = true; s.picker = false; break;
      case "dismiss": s.confirm = false; break;
    }
    rerender();
    if (["picker","restart"].includes(el.dataset.flowAct)) document.querySelector("[data-flow-search]")?.focus();
    // After a flow choice the picker closes; keep keyboard focus on its opener.
    if (el.dataset.flowAct==="flow") document.querySelector(s.restarting?'[data-flow-act="restart-confirm"]':`[data-flow-act="picker"][data-flow-task="${CSS.escape(el.dataset.flowTask)}"]`)?.focus();
    if (el.dataset.flowAct==="restart-cancel") document.querySelector('[data-flow-act="restart"]')?.focus();
    if (el.dataset.flowAct==="restart-confirm") document.querySelector('[data-flow-act="restart-cancel"]')?.focus();
    if (el.dataset.flowAct==="new-session") document.querySelector('[data-flow-act="session-dismiss"]')?.focus();
    if (el.dataset.flowAct==="session-dismiss") document.querySelector('[data-flow-act="new-session"]')?.focus();
    if (el.dataset.flowAct==="start") document.querySelector(".fs-confirm button")?.focus();
    if (el.dataset.flowAct==="dismiss") document.querySelector('[data-flow-act="start"]')?.focus();
  });
  document.addEventListener("input", e => {
    if (!e.target.matches("[data-flow-search]")) return;
    const s = st(e.target.dataset.flowSearch);
    s.query = e.target.value; s.index = 0;
    rerender();
  });
  document.addEventListener("keydown", e => {
    if (e.target.matches("[data-flow-search]")) {
      const s = st(e.target.dataset.flowSearch), results = matches(s);
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault(); e.stopImmediatePropagation();
        if (results.length) s.index = (s.index + (e.key === "ArrowDown" ? 1 : -1) + results.length) % results.length;
        rerender(); return;
      }
      if (e.key === "Enter") {
        e.preventDefault(); e.stopImmediatePropagation();
        if (results[s.index]) document.querySelector(`[data-flow-act="flow"][data-flow-value="${results[s.index]}"]`)?.click();
        return;
      }
      if (e.key === "Tab") {
        s.picker = false; rerender();
        document.querySelector(s.restarting?'[data-flow-act="restart"]':'[data-flow-act="picker"]')?.focus();
        return;
      }
    }
    if (e.key!=="Escape") return;
    const open = Object.values(store).find(s => s.picker || s.confirm || s.restartChoice);
    if (!open) return;
    e.stopImmediatePropagation();
    const target = open.restarting ? "restart" : open.picker ? "picker" : "start";
    open.picker = false; open.confirm = false; open.restarting = false; open.restartChoice = null; open.restartNote = false;
    rerender();
    document.querySelector(`[data-flow-act="${target}"]`)?.focus();
  }, true);
})();
