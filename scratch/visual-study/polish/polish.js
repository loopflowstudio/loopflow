// Three visual polish directions over the native build's current structure.
// Sample data only: no provider, PM or native terminal is connected.

// ---------- sample data ----------

const FLOW = [
  { s: 'kickoff' },
  { s: 'review-design', human: true },
  { s: 'implement' },
  { s: 'compress' },
  { s: 'review-slice' },
  { s: 'concept-review' },
  { s: 'loop-decide', id: 'decide' },
  { s: 'demo', human: true },
  { s: 'loop-decide', id: 'decide_delivery' },
  { s: 'compress' },
  { s: 'update-wave' },
  { s: 'gate' },
  { s: 'pr land -c', op: true },
];
// Authored order: loop 1 closes at decide, loop 2 at decide_delivery. Both return to implement.
const LOOPS = [
  { n: 1, from: 6, to: 2 },
  { n: 2, from: 8, to: 2 },
];
// Row breaks per direction. Every loop's endpoints stay on one row, so the
// whole Flow (including the queue → land tail) is visible without scrolling.
const THREE = [[0, 1], [2, 3, 4, 5, 6, 7, 8], [9, 10, 11, 12]];
const TWO = [[0, 1, 2, 3, 4, 5, 6, 7, 8], [9, 10, 11, 12]];
// D takes B's center structure, so center decisions read through this.
const center = () => (S.v === 'd' ? 'b' : S.v);
function flowRows() {
  if (center() === 'b' && innerWidth >= 1240) return { rows: TWO, labels: [null, '↳ delivery'] };
  if (center() === 'b') return { rows: THREE, labels: ['once', 'loops', 'then'] };
  if (S.v === 'c') return { rows: THREE, labels: ['Once', 'Loops', 'Then'] };
  return { rows: THREE, labels: [null, null, null] };
}

const WAVES = [
  {
    id: 'product', name: 'product', paused: true,
    objective: 'Loopflow keeps Cube, Etude, Kata, and Hootro moving on explicitly selected work.',
    krs: [
      { t: 'Ten configured planning-to-Session trials on human-selected external work', held: false },
      { t: 'Twenty repository opens paint the scoped Session list within the published p95 budget', held: false },
      { t: 'A parallel concept anywhere is a failure event', held: true },
    ],
    plan: ['LOO-291', 'LOO-293', 'LOO-296', 'LOO-298', 'LOO-284'],
    notices: [{
      short: 'LOO-277’s owning Project is missing from the current plan.',
      full: 'LOO-277: Task’s owning Project is absent from the current PM snapshot.',
      cmd: 'lf work abandon task task_40fbeeaadfbca5367aa7391432ae84ff --reason "Project is absent from the current PM snapshot"',
    }],
    metric: { name: 'Task loops earn trust', value: '100%', target: '≥ 100%', window: '7d', met: true,
      body: 'Fraction of Tasks settled in the trailing seven days that either landed every PR through auto-merge or stopped with a non-resumable failure receipt.' },
  },
  {
    id: 'infrastructure', name: 'infrastructure', paused: false,
    objective: 'Execution and recovery stay boring: fresh Runs, exact decisions, nothing lost on restart.',
    krs: [{ t: 'Every restarted Task resumes at its saved boundary without replaying work', held: false }],
    plan: ['LOO-285', 'LOO-295', 'LOO-292'], notices: [], metric: null,
  },
  {
    id: 'intelligence', name: 'intelligence', paused: false,
    objective: 'Raw context and traces are complete enough to judge any outcome later.',
    krs: [], plan: [], notices: [], metric: null,
  },
];

const TASKS = {
  'LOO-291': {
    wave: 'product', title: 'A calmer desktop workspace', started: true,
    flow: 'feature', state: 'running', current: 3, done: [0, 1, 2], returns: [2, 1],
    status: 'compress · Claude · 2m 14s', sessions: ['s1', 's2'],
    desc: `<p>Move between work and its conversations without losing your place.</p>
<h4>Outcome</h4><ul><li>One repo → Wave → Task → Session hierarchy</li><li>Retained drafts, viewport and companion terminals</li><li>Flow state that reads at a glance</li></ul>
<h4>Constraints</h4><p>Shared planning, identity and legal actions stay in <code>lf</code>; the desktop presents them.</p>`,
    comments: [
      ['Sep 23', 'Jack', 'Accepted A frame and Flow C. Keep native panes.'],
      ['Sep 24', 'Loopflow', 'Capture/input baseline recorded: 462/462 observations.'],
      ['Sep 25', 'Jack', 'Feature has two loops. Iteration is a tuple.'],
    ],
  },
  'LOO-293': {
    wave: 'product', title: 'Understand running work', started: true,
    flow: 'feature', state: 'stopped', current: 4, done: [0, 1, 2, 3], returns: [1, 0],
    status: 'Task worker exited after compress', sessions: [],
    desc: '<p>See which Runs are working on a Task right now, and what each is doing.</p>',
    comments: [['Sep 23', 'Loopflow', 'Monitor pane landed in the shared multiplexer.']],
  },
  'LOO-296': { wave: 'product', title: 'Explain Session membership at a glance', started: false, flow: 'feature', state: 'none', sessions: [], desc: '<p>Clicking a membership chip reveals its exact Flow node.</p>', comments: [] },
  'LOO-298': { wave: 'product', title: 'Read the Linear comment thread natively', started: false, flow: 'build', state: 'none', sessions: [], desc: '<p>Collapsed, counted, and truthful when the read fails.</p>', comments: [] },
  'LOO-284': { wave: 'product', title: 'Shared Session actions and labels', started: true, completed: true, flow: 'feature', state: 'finished', sessions: [], desc: '<p>Rust projects legal actions; the Mac renders them.</p>', comments: [] },
  'LOO-285': {
    wave: 'infrastructure', title: 'Retry and publish the chapter safely', started: true,
    flow: 'feature', state: 'blocked', current: 11, done: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10], returns: [3, 1],
    status: 'Required check tests-result is failing on the release target.', sessions: [],
    desc: '<p>A lost reply during chapter publication retries without duplicating Projects.</p>',
    comments: [['Sep 24', 'Loopflow', 'gate failed: tests-result red on 7a1c2e.']],
  },
  'LOO-295': { wave: 'infrastructure', title: 'Reliable local development', started: false, flow: 'feature', state: 'none', sessions: [], desc: '<p>A development build never opens a store another build migrated ahead of it.</p>', comments: [] },
  'LOO-292': { wave: 'infrastructure', title: 'Keep Task observation read-only', started: true, completed: true, flow: 'feature', state: 'finished', sessions: [], desc: '<p>Reading a Task never records that it started.</p>', comments: [] },
};

const SESSIONS = {
  s1: {
    task: 'LOO-291', name: 'Release outcomes', provider: 'Claude', state: 'Your turn',
    member: { node: 3, label: 'feature · compress · iteration (2, 1)' },
    summary: 'Proposed moving the warning into a compact note with Details.',
    lines: [
      ['you', 'Can we make the Wave warning less shouty without hiding it?'],
      ['agent', 'Yes — a one-line note with the Task ID, a Details disclosure for the full reason and recovery command, and Dismiss for this visit only.'],
      ['agent', 'I kept the command copyable inside Details. Want me to apply the same pattern to the chapter-unavailable state?'],
    ],
  },
  s2: {
    task: 'LOO-291', name: 'Verification cases', provider: 'Codex', state: 'Working',
    member: null, summary: 'Running the mounted retention proof with three owned PTYs.',
    lines: [
      ['you', 'Run the retention proof again after the tint change.'],
      ['agent', 'Running swift test --filter namedSessionDrillDownRetainsTerminal …'],
    ],
  },
};

// ---------- state ----------

const q = new URLSearchParams(location.search);
const S = {
  v: ['a', 'b', 'c', 'd'].includes(q.get('v')) ? q.get('v') : 'd',
  surface: ['wave', 'task', 'session'].includes(q.get('surface')) ? q.get('surface') : 'wave',
  wave: 'product',
  task: 'LOO-291',
  session: 's1',
  node: null,
  highlight: null,
  descOpen: false,
  commentsOpen: false,
  noticeOpen: false,
  noticeHidden: {},
  restart: false,
  renaming: false,
  drafts: { s1: 'Yes, and keep the retry button next to', s2: '' },
  collapsed: {},
  search: '',
};

// ---------- helpers ----------

const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const linear = id => `https://linear.app/loopflow/issue/${id}`;
const ICON = {
  chat: '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 3.5h10a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H7l-3 2.5v-2.5H3a1 1 0 0 1-1-1v-6a1 1 0 0 1 1-1z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>',
  chev: '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  view: '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2.5 4h11M4.5 8h7M6.5 12h3" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>',
  search: '<svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.5" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M10.5 10.5L14 14" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>',
  pencil: '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M10.5 3l2.5 2.5L6 12.5 3 13l.5-3z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>',
  note: '<svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="6" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M8 4.8v3.8M8 10.8v.4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>',
};

function stateOf(t, i) {
  if (t.state === 'none' || t.state === 'finished') return 'future';
  if (t.done.includes(i)) return 'done';
  if (i === t.current) return t.state; // running | stopped | blocked
  if (FLOW[i].human) return 'human';
  return 'future';
}

function statusLine(t) {
  switch (t.state) {
    case 'running': return { cls: 'running', head: 'Running', body: t.status };
    case 'stopped': return { cls: 'stopped', head: `Stopped at ${FLOW[t.current].s}`, body: t.status };
    case 'blocked': return { cls: 'blocked', head: `Blocked at ${FLOW[t.current].s}`, body: t.status };
    case 'finished': return { cls: 'done', head: 'Finished', body: 'The pinned definition is not retained after settlement; preview shows today’s Flow.' };
    default: return { cls: 'idle', head: 'Not started', body: 'No runs yet.' };
  }
}

const tuple = t => (t.returns ? `(${t.returns.join(', ')})` : null);
const returnsText = n => `${n} ${n === 1 ? 'return' : 'returns'}`;

// ---------- sidebar ----------

function sidebar() {
  const term = S.search.trim().toLowerCase();
  const waves = WAVES.map(w => {
    const tasks = Object.entries(TASKS).filter(([id, t]) => t.wave === w.id && t.started && !t.completed)
      .filter(([id, t]) => !term || (id + t.title).toLowerCase().includes(term));
    if (term && !tasks.length && !w.name.includes(term)) return '';
    const open = !S.collapsed[w.id] || term;
    const selW = S.surface === 'wave' && S.wave === w.id;
    return `<li class="nav-wave">
      <div class="nav-row nav-wave-row${selW ? ' is-selected' : ''}">
        <button class="disclose" data-act="fold" data-id="${w.id}" aria-expanded="${!!open}" aria-label="${open ? 'Collapse' : 'Expand'} ${w.name}">${ICON.chev}</button>
        <button class="nav-wave-name" data-act="wave" data-id="${w.id}" aria-current="${selW ? 'page' : 'false'}">${esc(w.name)}</button>
        ${w.paused ? '<span class="nav-meta">paused</span>' : ''}
      </div>
      ${open ? `<ul class="nav-tasks">${tasks.length ? tasks.map(([id, t]) => navTask(id, t)).join('') : '<li class="nav-empty">No started Tasks</li>'}</ul>` : ''}
    </li>`;
  }).join('');
  return `<aside class="side" aria-label="Work">
    <div class="side-head">
      <button class="repo" aria-haspopup="menu" title="Switch repository">loopflow ${ICON.chev}</button>
      <button class="icon-btn" aria-label="Presentation: compact" title="Presentation">${ICON.view}</button>
    </div>
    <ul class="nav" role="tree">${waves}</ul>
    <label class="side-search">${ICON.search}<input id="search" type="search" placeholder="Find work or Sessions" value="${esc(S.search)}" aria-label="Find work or Sessions"></label>
  </aside>`;
}

function navTask(id, t) {
  const inTask = (S.surface === 'task' && S.task === id) || (S.surface === 'session' && SESSIONS[S.session].task === id);
  const dot = { running: 'running', stopped: 'stopped', blocked: 'blocked' }[t.state] || 'idle';
  const n = t.sessions.length;
  return `<li><div class="nav-row nav-task${inTask ? ' is-selected' : ''}">
    <span class="dot dot-${dot}" aria-hidden="true"></span>
    <button class="nav-task-name" data-act="task" data-id="${id}" title="${esc(t.title)} · ${id}" aria-current="${inTask ? 'page' : 'false'}">${esc(t.title)}</button>
    ${n ? `<button class="nav-count" data-act="task-overview" data-id="${id}" aria-label="${n} open Sessions: ${t.sessions.map(s => SESSIONS[s].name).join(', ')}" title="${t.sessions.map(s => SESSIONS[s].name).join(' · ')}">${ICON.chat}<span>${n}</span></button>` : '<span class="nav-count-empty"></span>'}
  </div></li>`;
}

// ---------- wave ----------

function heading(text, extra = '') {
  return `<div class="sec-head"><h3>${text}</h3>${extra}</div>`;
}

function waveView() {
  const w = WAVES.find(x => x.id === S.wave);
  const notice = w.notices.length && !S.noticeHidden[w.id] ? noticeView(w) : '';
  const restore = w.notices.length && S.noticeHidden[w.id]
    ? `<button class="link quiet" data-act="notice-restore">Show ${w.notices.length} notice</button>` : '';
  const krs = w.krs.length
    ? `<ul class="krs">${w.krs.map(k => `<li class="${k.held ? 'held' : ''}"><span class="kr-mark" aria-label="${k.held ? 'Holds' : 'Not yet'}"></span><span>${esc(k.t)}</span></li>`).join('')}</ul>`
    : '<p class="empty">No current chapter plan.</p>';
  const plan = w.plan.length
    ? `<ol class="plan">${w.plan.map((id, i) => planRow(id, i)).join('')}</ol>`
    : '<p class="empty">No Tasks in this chapter.</p>';
  return `<div class="page wave-page">
    <nav class="crumbs" aria-label="Location"><span>loopflow</span><span class="sep">/</span><span aria-current="page">${w.name}</span></nav>
    <header class="wave-head">
      <h1 class="wave-title">${esc(w.name)}</h1>
      ${w.paused ? '<span class="chip chip-neutral">paused</span>' : ''}
      <div class="head-actions">${w.paused ? '<button class="btn">Resume</button>' : ''}</div>
    </header>
    <p class="objective">${esc(w.objective)}</p>
    ${notice}${restore ? `<div class="restore">${restore}</div>` : ''}
    <section>${heading('Current KRs', '<a class="link quiet" href="#" data-act="noop">Chapter history →</a>')}${krs}</section>
    <section>${heading(`Tasks <span class="count">${w.plan.length}</span>`)}${plan}</section>
    ${w.metric ? `<section>${heading('Metrics')}${metricView(w.metric)}</section>` : ''}
  </div>`;
}

function planRow(id, i) {
  const t = TASKS[id];
  const st = t.completed ? 'done' : t.started ? ({ running: 'running', stopped: 'stopped', blocked: 'blocked' }[t.state] || 'idle') : 'upcoming';
  const label = { done: 'Completed', running: 'Running', stopped: 'Stopped', blocked: 'Blocked', idle: 'Started', upcoming: 'Upcoming' }[st];
  return `<li class="plan-row st-${st}">
    <span class="plan-rank">${i + 1}</span>
    <span class="plan-mark" aria-hidden="true"></span>
    <button class="plan-title" data-act="task" data-id="${id}">${esc(t.title)}</button>
    <span class="chip chip-${st}">${label}</span>
    <a class="plan-id" href="${linear(id)}" target="_blank" rel="noreferrer">${id}</a>
  </li>`;
}

function noticeView(w) {
  const n = w.notices[0];
  const details = S.noticeOpen
    ? `<div class="notice-details" id="notice-details"><p>${esc(n.full)}</p><p class="notice-cmd-label">Recover with</p><code class="notice-cmd">${esc(n.cmd)}</code></div>` : '';
  return `<div class="notice" role="status">
    <div class="notice-line">${ICON.note}<span class="notice-text">${esc(n.short)}</span>
      <button class="link" data-act="notice" aria-expanded="${S.noticeOpen}" aria-controls="notice-details">${S.noticeOpen ? 'Hide details' : 'Details'}</button>
      <button class="link quiet" data-act="notice-hide" aria-label="Dismiss notice for this visit">Dismiss</button>
    </div>${details}
  </div>`;
}

function metricView(m) {
  if (center() === 'b') {
    return `<table class="metric-table"><thead><tr><th>Measure</th><th>Value</th><th>Target</th><th>Window</th><th>State</th></tr></thead>
      <tbody><tr><td title="${esc(m.body)}">${esc(m.name)}</td><td class="num">${m.value}</td><td class="num">${m.target}</td><td>${m.window}</td><td><span class="chip chip-done">Met</span></td></tr></tbody></table>`;
  }
  return `<div class="metric"><div class="metric-top"><span class="metric-name">${esc(m.name)}</span><span class="chip chip-done">Met</span></div>
    <p class="metric-body">${esc(m.body)}</p>
    <div class="metric-fig"><span class="metric-value">${m.value}</span><span class="metric-target">target ${m.target}</span><span class="metric-window">${m.window} window</span></div></div>`;
}

// ---------- task ----------

function taskView() {
  const id = S.task, t = TASKS[id];
  const st = statusLine(t);
  const sessions = t.sessions.map(sid => SESSIONS[sid]);
  return `<div class="page task-page">
    <nav class="crumbs" aria-label="Location"><button class="link" data-act="wave" data-id="${t.wave}">${t.wave}</button><span class="sep">/</span><a class="crumb-id" href="${linear(id)}" target="_blank" rel="noreferrer">${id}</a></nav>
    <header class="task-head"><h1 class="task-title">${esc(t.title)}</h1><button class="btn" data-act="new-session">${ICON.chat} New session</button></header>
    ${S.newSession ? `<p class="inline-note">Prototype: would open an independent conversation in ${id}'s worktree. The Flow is unchanged. <button class="link quiet" data-act="new-session-x">Dismiss</button></p>` : ''}
    <section class="flow-sec">${flowView(t)}</section>
    <div class="status st-${st.cls}"><span class="status-dot" aria-hidden="true"></span><span class="status-head">${st.head}</span><span class="status-body">${esc(st.body)}</span></div>
    ${sessions.length ? `<section>${heading(`Sessions <span class="count">${sessions.length}</span>`)}<ul class="sessions">${t.sessions.map(sessionRow).join('')}</ul></section>` : ''}
    <section>${heading('Description', '<button class="link quiet" data-act="noop">Edit description</button>')}
      <div class="desc${S.descOpen ? ' open' : ''}">${t.desc}</div>
      ${t.desc.length > 200 ? `<button class="link quiet more" data-act="desc" aria-expanded="${S.descOpen}">${S.descOpen ? 'Show less' : 'Show more'}</button>` : ''}
    </section>
    <section class="comments">
      <button class="disclosure-head" data-act="comments" aria-expanded="${S.commentsOpen}">${ICON.chev}<span>Comments</span><span class="count">${t.comments.length}</span></button>
      ${S.commentsOpen ? (t.comments.length ? `<ul class="comment-list">${t.comments.map(c => `<li><div class="c-meta"><b>${esc(c[1])}</b><span>${c[0]}</span></div><p>${esc(c[2])}</p></li>`).join('')}</ul>` : '<p class="empty">No comments.</p>') : ''}
    </section>
  </div>`;
}

function sessionRow(sid) {
  const s = SESSIONS[sid];
  const member = s.member ? `<span class="member">${esc(s.member.label)}</span>` : '<span class="member member-ind">Independent</span>';
  const cls = s.state === 'Your turn' ? 'human' : 'running';
  return `<li><button class="session-row" data-act="session" data-id="${sid}">
    <span class="s-icon">${ICON.chat}</span>
    <span class="s-main"><span class="s-name">${esc(s.name)}</span><span class="s-sub">${esc(s.provider)} · ${member}</span><span class="s-sum">${esc(s.summary)}</span></span>
    <span class="chip chip-${cls}">${esc(s.state)}</span>
  </button></li>`;
}

function flowView(t) {
  const pinned = ['running', 'stopped', 'blocked'].includes(t.state);
  const tup = pinned ? tuple(t) : null;
  const action = t.state === 'none' ? '<button class="btn btn-primary" data-act="noop">Start</button>'
    : t.state === 'stopped' ? '<button class="btn btn-primary" data-act="noop">Resume</button>'
    : t.state === 'blocked' ? '<button class="btn" data-act="noop">Get help</button>' : '';
  const layout = flowRows();
  const rows = layout.rows.map((row, ri) => {
    const label = layout.labels[ri];
    return `<div class="flow-row${label ? ' labelled' : ''}${row.includes(2) ? ' has-loop' : ''}" data-row="${ri}">
      ${label ? `<span class="row-label">${label}</span>` : ''}
      <div class="nodes">${row.map(i => nodeView(t, i)).join('')}</div></div>`;
  }).join('');
  const detail = S.node !== null ? nodeDetail(t, S.node) : '';
  const restart = S.restart && pinned ? `<div class="confirm" role="dialog" aria-label="Stop and restart">
      <p><b>Stop ${t.flow} and restart?</b> Loopflow commits and pushes the worktree, stops the worker, and starts the replacement Flow from its first step. The Task, worktree and PR history stay.</p>
      <div class="confirm-actions"><button class="btn" data-act="restart-x">Cancel</button><button class="btn btn-danger" data-act="restart-x">Stop &amp; restart</button></div></div>` : '';
  return `<div class="flow-bar">
      <span class="flow-name-wrap"><button class="flow-name" data-act="noop" aria-label="Flow ${t.flow}. Choose another Flow">${t.flow} ${ICON.chev}</button>
      ${tup ? `<span class="iter" title="Returns per loop, in authored order">Iteration ${tup}</span>` : pinned ? '' : '<span class="iter iter-preview">Preview</span>'}
      ${pinned ? '<button class="flow-restart" data-act="restart">Stop &amp; restart…</button>' : ''}</span>
      <span class="flow-actions">${action}</span>
    </div>${restart}
    <div class="flow" data-flow>${rows}<svg class="flow-svg" aria-hidden="true"></svg></div>
    ${detail}`;
}

function nodeView(t, i) {
  const n = FLOW[i];
  const st = stateOf(t, i);
  const done = st === 'done' && n.human ? ' human-done' : '';
  const sel = S.node === i ? ' is-selected' : '';
  const hl = S.highlight === i ? ' is-highlight' : '';
  const label = { done: 'completed', running: 'running', stopped: 'stopped here', blocked: 'blocked', human: 'waits for you', future: '' }[st];
  return `<button class="node st-${st}${n.human ? ' is-human' : ''}${n.op ? ' is-op' : ''}${done}${sel}${hl}" data-act="node" data-i="${i}" aria-pressed="${S.node === i}" aria-label="${n.s}${label ? ', ' + label : ''}">
    ${center() === 'b' ? `<span class="node-num">${i + 1}</span>` : ''}<span class="node-name">${esc(n.s)}</span></button>`;
}

function nodeDetail(t, i) {
  const n = FLOW[i];
  const loops = LOOPS.filter(l => i >= l.to && i <= l.from).map(l => `loop ${l.n}`);
  const ret = LOOPS.find(l => l.from === i);
  const kind = n.op ? 'operation' : n.human ? 'human step' : 'skill';
  const bits = [kind, n.id ? `id ${n.id}` : null, loops.length ? `inside ${loops.join(' and ')}` : 'runs once',
    ret ? `returns to implement · taken ${t.returns ? t.returns[ret.n - 1] : 0}×` : null].filter(Boolean);
  return `<div class="node-detail" role="region" aria-label="${n.s} detail"><b>${esc(n.s)}</b> <span>${bits.join(' · ')}</span>
    <button class="link quiet" data-act="node-x" aria-label="Close detail">Close</button></div>`;
}

// ---------- session ----------

function sessionView() {
  const s = SESSIONS[S.session], t = TASKS[s.task];
  const others = t.sessions.filter(x => x !== S.session);
  const name = S.renaming
    ? `<input class="rename" id="rename" value="${esc(s.name)}" aria-label="Session name">`
    : `<span class="crumb-session" aria-current="page">${esc(s.name)}</span><button class="icon-btn" data-act="rename" aria-label="Rename Session">${ICON.pencil}</button>`;
  const member = s.member
    ? `<button class="member-chip" data-act="member" title="Show this step in the Task's Flow">${esc(s.member.label)}</button>`
    : '<span class="member-chip ind">Independent</span>';
  return `<div class="page session-page">
    <nav class="crumbs session-crumbs" aria-label="Location">
      <button class="link" data-act="wave" data-id="${t.wave}">${t.wave}</button><span class="sep">/</span>
      <button class="link crumb-task" data-act="task-overview" data-id="${s.task}" title="${esc(t.title)}">${esc(t.title)}</button>
      <a class="crumb-id" href="${linear(s.task)}" target="_blank" rel="noreferrer">${s.task}</a><span class="sep">/</span>
      ${name}
      ${others.length ? `<span class="switch">${others.map(o => `<button class="link quiet" data-act="session" data-id="${o}">${esc(SESSIONS[o].name)}</button>`).join('')}</span>` : ''}
    </nav>
    <div class="session-meta">${member}<span class="s-provider">${s.provider}</span><span class="chip chip-${s.state === 'Your turn' ? 'human' : 'running'}">${s.state}</span>
      <span class="spacer"></span><button class="btn" data-act="noop">Complete</button></div>
    <div class="panes">
      <section class="pane pane-main is-focused" aria-label="${esc(s.name)} conversation">
        <div class="pane-head"><span>${esc(s.name)}</span><span class="pane-sub">${s.provider.toLowerCase()} · loopflow.main-view-task</span></div>
        <div class="term">${s.lines.map(([who, txt]) => `<p class="t-${who}"><span class="t-who">${who === 'you' ? '›' : '●'}</span>${esc(txt)}</p>`).join('')}
          <label class="t-input"><span class="t-who">›</span><textarea id="draft" rows="2" aria-label="Unfinished message">${esc(S.drafts[S.session] || '')}</textarea></label>
        </div>
      </section>
      <section class="pane pane-side" aria-label="Companion shell">
        <div class="pane-head"><span>zsh</span><span class="pane-sub">companion</span></div>
        <div class="term shell"><p>$ swift test --filter namedSessionDrillDown</p><p class="ok">✔ Test run with 1 test passed after 4.212 s</p><p>$ <span class="caret"></span></p></div>
      </section>
    </div>
  </div>`;
}

// ---------- render ----------

function render() {
  document.body.className = `v-${S.v}`;
  document.querySelectorAll('[data-v]').forEach(b => b.setAttribute('aria-pressed', b.dataset.v === S.v));
  document.querySelectorAll('[data-surface]').forEach(b => b.setAttribute('aria-pressed', b.dataset.surface === S.surface));
  const main = S.surface === 'wave' ? waveView() : S.surface === 'task' ? taskView() : sessionView();
  const app = document.getElementById('app');
  const scroll = app.querySelector('.main')?.scrollTop || 0;
  const focusedId = document.activeElement?.id;
  app.innerHTML = `${sidebar()}<main class="main">${main}</main>`;
  app.querySelector('.main').scrollTop = scroll;
  if (focusedId) { const el = document.getElementById(focusedId); if (el) { el.focus(); if (el.setSelectionRange && el.type !== 'textarea') el.setSelectionRange(el.value.length, el.value.length); } }
  drawFlow();
  const u = new URL(location); u.searchParams.set('v', S.v); u.searchParams.set('surface', S.surface); history.replaceState(null, '', u);
}

// Regions and return arrows are measured from the rendered nodes, so every
// direction's row layout draws correctly at any width.
function drawFlow() {
  const flow = document.querySelector('[data-flow]');
  if (!flow) return;
  const svg = flow.querySelector('.flow-svg');
  const t = TASKS[S.task];
  const box = flow.getBoundingClientRect();
  svg.setAttribute('width', box.width); svg.setAttribute('height', box.height);
  const r = i => { const b = flow.querySelector(`.node[data-i="${i}"]`).getBoundingClientRect(); return { l: b.left - box.left, r: b.right - box.left, t: b.top - box.top, b: b.bottom - box.top, cx: (b.left + b.right) / 2 - box.left }; };
  const v = center();
  const pad = { a: [6, 11], b: [4, 8], c: [10, 16] }[v];
  const lane = { a: [13, 27], b: [11, 22], c: [18, 38] }[v];
  const labelTop = { a: 14, b: 12, c: 20 }[v];
  const color = ['var(--loop1)', 'var(--loop2)'];
  let defs = '<defs>', regions = '', arrows = '', labels = '';
  LOOPS.slice().reverse().forEach(loop => {
    const k = loop.n - 1, a = r(loop.to), z = r(loop.from);
    const p = pad[k], top = a.t - p - labelTop - (k === 1 ? labelTop - 2 : 0), bottom = a.b + lane[k] + p + 4;
    regions += `<rect x="${a.l - p}" y="${top}" width="${z.r - a.l + 2 * p}" height="${bottom - top}" rx="${S.v === 'b' ? 6 : 12}" class="region region-${loop.n}"/>`;
    const pinned = t.returns && ['running', 'stopped', 'blocked'].includes(t.state);
    const text = pinned ? `Loop ${loop.n} · ${returnsText(t.returns[k])}` : `Loop ${loop.n}`;
    const lx = k === 0 ? a.l - p + 10 : z.r + p - 10;
    labels += `<text x="${lx}" y="${top + labelTop - 3}" class="loop-label loop-label-${loop.n}" text-anchor="${k === 0 ? 'start' : 'end'}">${text}</text>`;
  });
  LOOPS.forEach(loop => {
    const k = loop.n - 1, a = r(loop.to), z = r(loop.from);
    const toX = a.cx + (k === 0 ? 9 : -9);
    const y = a.b + lane[k];
    defs += `<marker id="head${loop.n}" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="${v === 'c' ? 9 : 7}" markerHeight="${v === 'c' ? 9 : 7}" orient="auto-start-reverse">${k === 0 || v !== 'c' ? '<path d="M0 0L10 5L0 10z" fill="' + color[k] + '"/>' : '<path d="M1 1L9 5L1 9" fill="none" stroke="' + color[k] + '" stroke-width="2"/>'}</marker>`;
    arrows += `<path d="M${z.cx} ${z.b} V${y} H${toX} V${a.b + 3}" class="ret ret-${loop.n}" marker-end="url(#head${loop.n})"/>`;
  });
  svg.innerHTML = `${defs}</defs>${regions}${arrows}${labels}`;
}

// ---------- events ----------

document.addEventListener('click', e => {
  const v = e.target.closest('[data-v]');
  if (v) { S.v = v.dataset.v; S.node = null; render(); return; }
  const sf = e.target.closest('[data-surface]');
  if (sf) {
    S.surface = sf.dataset.surface;
    if (S.surface === 'task') S.task = 'LOO-291';
    if (S.surface === 'session') S.session = 's1';
    S.node = null; S.highlight = null; render(); return;
  }
  const el = e.target.closest('[data-act]');
  if (!el) return;
  const id = el.dataset.id;
  switch (el.dataset.act) {
    case 'wave': S.surface = 'wave'; S.wave = id; break;
    case 'task': {
      const t = TASKS[id];
      if (t.sessions.length === 1) { S.surface = 'session'; S.session = t.sessions[0]; }
      else { S.surface = 'task'; S.task = id; }
      S.node = null; S.highlight = null; S.restart = false; S.newSession = false; break;
    }
    case 'task-overview': S.surface = 'task'; S.task = id; S.node = null; S.highlight = null; break;
    case 'session': S.surface = 'session'; S.session = id; S.renaming = false; break;
    case 'fold': S.collapsed[id] = !S.collapsed[id]; break;
    case 'notice': S.noticeOpen = !S.noticeOpen; break;
    case 'notice-hide': S.noticeHidden[S.wave] = true; S.noticeOpen = false; break;
    case 'notice-restore': S.noticeHidden[S.wave] = false; break;
    case 'desc': S.descOpen = !S.descOpen; break;
    case 'comments': S.commentsOpen = !S.commentsOpen; break;
    case 'node': { const i = +el.dataset.i; S.node = S.node === i ? null : i; S.highlight = null; break; }
    case 'node-x': S.node = null; break;
    case 'restart': S.restart = true; break;
    case 'restart-x': S.restart = false; break;
    case 'new-session': S.newSession = true; break;
    case 'new-session-x': S.newSession = false; break;
    case 'rename': S.renaming = true; render(); document.getElementById('rename')?.select(); return;
    case 'member': {
      const s = SESSIONS[S.session];
      S.surface = 'task'; S.task = s.task; S.highlight = s.member.node; S.node = s.member.node; break;
    }
    case 'noop': e.preventDefault(); return;
  }
  e.preventDefault();
  render();
});

document.addEventListener('input', e => {
  if (e.target.id === 'search') { S.search = e.target.value; render(); }
  if (e.target.id === 'draft') S.drafts[S.session] = e.target.value;
});

document.addEventListener('keydown', e => {
  if (e.target.id === 'rename') {
    if (e.key === 'Enter') { const v = e.target.value.trim(); if (v) SESSIONS[S.session].name = v; S.renaming = false; render(); }
    if (e.key === 'Escape') { S.renaming = false; render(); }
    return;
  }
  if (e.key === 'Escape') {
    if (S.restart) S.restart = false; else if (S.node !== null) S.node = null; else return;
    render();
  }
});

let lastRows = null;
window.addEventListener('resize', () => {
  const k = JSON.stringify(flowRows().rows);
  if (lastRows && k !== lastRows && S.surface === 'task') render(); else drawFlow();
  lastRows = k;
});
document.fonts?.ready.then(drawFlow);
render();
