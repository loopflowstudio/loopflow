// Shared prototype model: Tasks, scenarios, readers with latency/failure, renderers, conversation.
(function () {
const $ = (s, el = document) => el.querySelector(s);
const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const sleep = ms => new Promise(r => setTimeout(r, ms));

// ---------- scenarios ----------
const SCEN = Object.assign({ pr: 'open', brief: 'normal', machine: 'ok' },
  JSON.parse(sessionStorage.getItem('lf-proto-scen') || '{}'));
const scenListeners = [];
function setScen(k, v) { SCEN[k] = v; sessionStorage.setItem('lf-proto-scen', JSON.stringify(SCEN)); scenListeners.forEach(f => f(k)); }

// ---------- Tasks ----------
const KNOWN = {
  'LOO-489': 'Enable the multitask navigation experience',
  'LOO-497': 'Drive Desktop through separate selection and arrangement APIs',
  'LOO-416': 'Keep Task sessions seamless across switching and app restart',
  'LOO-402': 'Show which Task is waiting from the sidebar and jump to it',
  'LOO-515': 'Make Task PR, file-browser and brief views useful beside the conversation',
};
const LONG_TAIL = Array.from({ length: 14 }, (_, i) => `\n\n## Review notes, round ${i + 1}\n\n` +
  `Jack Heart walked the review loop on build ${i + 41}. The Brief stayed readable at 680 pt; the outline jumped correctly to **Acceptance**. ` +
  `Changes showed the PR card above the changed files and kept GitHub's reading separate from Loopflow's record.\n\n` +
  `* Long code identifiers like \`SessionWorkspace.files(for:)\` wrap without overflowing the pane.\n* Links such as [the Linear issue](https://linear.app/loopflow/issue/LOO-515) open externally.\n* Related Task LOO-${400 + i} stays a plain identifier when Loopflow does not know it.`).join('');

const CHANGES_515 = [
  { path: 'swift/LoopflowMac/Views/TaskFilesView.swift', st: 'M', a: 142, d: 96 },
  { path: 'swift/LoopflowMac/Views/TaskBriefView.swift', st: 'A', a: 164, d: 0 },
  { path: 'swift/LoopflowMac/Views/TaskPullRequestView.swift', st: 'M', a: 38, d: 121 },
  { path: 'swift/LoopflowMac/PullRequestStore.swift', st: 'M', a: 24, d: 88 },
  { path: 'swift/LoopflowMac/DesktopActions.swift', st: 'M', a: 12, d: 0 },
  { path: 'tests/fixtures/dto/task_files.json', st: 'A', a: 18, d: 0 },
  { path: 'swift/LoopflowTests/PullRequestPatchTests.swift', st: 'D', a: 0, d: 64 },
  { path: 'swift/README.md', st: 'M', a: 14, d: 9 },
];

const TASKS = {
  'LOO-515': {
    id: 'LOO-515', title: KNOWN['LOO-515'], slot: 1,
    state: 'Idle', stateDot: 'waiting', reason: 'Waiting at explore · design review in this conversation',
    step: 'explore', machine: 'this Mac', machineRemote: false,
    checkout: '~/src/loopflow.loo-515-make-task-pr', branch: 'jack/loo-515-make-task-pr-…', due: null,
    brief: () => SCEN.brief === 'empty' ? '' : SCEN.brief === 'long' ? BRIEF_515 + LONG_TAIL : SCEN.brief === 'changed'
      ? BRIEF_515.replace('Acceptance:', 'Acceptance (revised Oct 28 by Jack Heart — PR folds into Changes):') : BRIEF_515,
    briefChanged: () => SCEN.brief === 'changed',
    changes: () => CHANGES_515, base: 'acd37dd',
    pr: () => PR_SCEN[SCEN.pr],
  },
  'LOO-489': {
    id: 'LOO-489', title: KNOWN['LOO-489'], slot: 2,
    state: 'Active', stateDot: 'working', reason: 'Graph ship running · step gate',
    step: 'gate', machine: 'mini-heart', machineRemote: true,
    checkout: 'mini-heart:~/src/loopflow.loo-489-multitask', branch: 'jack/loo-489-…', due: 'Nov 3',
    brief: () => 'Jack Heart wants to keep several Tasks in motion from one Desktop window.\n\nAcceptance:\n\n* Cmd-number slots switch Tasks and preserve each Task\'s panes.\n* The selected Task is always identifiable.\n\nBuilds on LOO-497.',
    briefChanged: () => false,
    changes: () => [], base: '9967df7',
    pr: () => ({ kind: 'none' }),
  },
};

const CHECKS = [
  { name: 'rust-lint', s: 'success', t: '4m 12s' }, { name: 'rust-test', s: 'success', t: '11m 40s' },
  { name: 'swift-test', s: 'working', t: 'running 6m' }, { name: 'python', s: 'success', t: '1m 03s' },
];
const PR_SCEN = {
  none: { kind: 'none' },
  publishing: { kind: 'publishing', requested: '40 s ago' },
  open: { kind: 'open', num: 1764, title: 'Make Task details useful beside the conversation', head: 'b41e9c2', localHead: 'b41e9c2',
    review: 'Review required · Jack Heart', read: 'read 12 s ago', fresh: true, checks: CHECKS, merge: 'Auto-Ship off' },
  stale: { kind: 'open', num: 1764, title: 'Make Task details useful beside the conversation', head: 'b41e9c2', localHead: 'e07a311',
    review: 'Review required · Jack Heart', read: 'read 9 min ago', fresh: false, ahead: 2, checks: CHECKS, merge: 'Auto-Ship off' },
  unavailable: { kind: 'open', num: 1764, title: 'Make Task details useful beside the conversation', head: 'b41e9c2', localHead: 'b41e9c2',
    review: 'Review required', read: 'last read 14 min ago', fresh: false, error: 'GitHub unavailable: gh: could not resolve api.github.com', checks: CHECKS, merge: 'Auto-Ship off' },
  merged: { kind: 'merged', num: 1764, title: 'Make Task details useful beside the conversation', sha: '7c2d0e4', read: 'read 30 s ago', fresh: true,
    checks: CHECKS.map(c => ({ ...c, s: 'success', t: c.t.replace('running ', '') })), review: 'Approved · Jack Heart' },
  abandoned: { kind: 'abandoned', num: 1764, title: 'Make Task details useful beside the conversation', read: 'read 2 min ago', fresh: true },
};

// ---------- markdown ----------
function inline(s) {
  let h = esc(s);
  h = h.replace(/`([^`]+)`/g, '<code>$1</code>');
  h = h.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
  h = h.replace(/\[([^\]]+)\]\(&lt;?([^)&]+?)&gt;?\)/g, (m, t, u) => {
    const id = (t.match(/LOO-\d+/) || [])[0];
    return id && KNOWN[id] ? `<button class="taskref" data-task="${id}" title="${esc(KNOWN[id])}">${esc(t)}</button>` : `<a href="${u}" target="_blank" rel="noopener">${t} ↗</a>`;
  });
  h = h.replace(/(^|[\s(])(LOO-\d+)(?![^<]*<\/(button|a)>)/g, (m, p, id) => KNOWN[id] ? `${p}<button class="taskref" data-task="${id}" title="${esc(KNOWN[id])}">${id}</button>` : m);
  return h;
}
function md(text) {
  const out = []; let list = null, n = 0;
  const flush = () => { if (list) { out.push('<ul>' + list.join('') + '</ul>'); list = null; } };
  for (const raw of text.split('\n')) {
    const line = raw.trimEnd();
    let m;
    if ((m = line.match(/^(#{1,3}) (.*)/))) { flush(); out.push(`<h${m[1].length + 1} data-anchor="b${n++}">${inline(m[2])}</h${m[1].length + 1}>`); }
    else if ((m = line.match(/^[*-] (.*)/))) { (list = list || []).push(`<li>${inline(m[1])}</li>`); }
    else if (!line) { flush(); }
    else if (/^Acceptance/.test(line)) { flush(); out.push(`<h3 data-anchor="b${n++}">${inline(line.replace(/:$/, ''))}</h3>`); }
    else { flush(); out.push(`<p>${inline(line)}</p>`); }
  }
  flush();
  return out.join('');
}
function outline(text) {
  const items = [];
  for (const line of text.split('\n')) {
    let m;
    if ((m = line.match(/^(#{1,3}) (.*)/))) items.push(m[2]);
    else if (/^Acceptance/.test(line)) items.push(line.replace(/:$/, '').replace(/\s*\(.*\)/, ''));
  }
  return items;
}
function related(text) { return [...new Set((text.match(/LOO-\d+/g) || []))].filter(id => KNOWN[id] && !text.startsWith(id)).filter(id => id !== 'LOO-515'); }
function outcome(text) { return (text.split('\n').find(l => l.trim()) || '').trim(); }
function acceptance(text) {
  const lines = text.split('\n'); const i = lines.findIndex(l => /^Acceptance/.test(l)); if (i < 0) return [];
  const items = []; for (const l of lines.slice(i + 1)) { if (/^[*-] /.test(l)) items.push(l.slice(2)); else if (items.length && l.trim() && !/^[*-] /.test(l)) break; }
  return items;
}

// ---------- checkout tree ----------
const TREE = {};
for (const p of REPO_PATHS) { const parts = p.split('/'); let node = TREE; parts.forEach((part, i) => { node[part] = node[part] || (i === parts.length - 1 ? null : {}); if (node[part]) node = node[part]; }); }
TREE.tests.fixtures.large = Object.fromEntries(Array.from({ length: 1840 }, (_, i) => [`snapshot-${String(i).padStart(4, '0')}.json`, null]));
const PAGE = 500;
async function listDir(task, dir, cursor = 0) {
  await sleep(dir === 'tests/fixtures/large' ? 650 : 260 + Math.random() * 200);
  if (task.machineRemote && SCEN.machine === 'offline') throw new Error(`mini-heart is unreachable · ssh: connect timed out`);
  if (dir === 'vendor') throw new Error('Permission denied (os error 13)');
  let node = TREE; if (dir) for (const part of dir.split('/')) node = node && node[part];
  if (!node) throw new Error('No such directory');
  const entries = Object.keys(node).sort((a, b) => (node[a] === null) - (node[b] === null) || a.localeCompare(b))
    .map(name => ({ path: dir ? `${dir}/${name}` : name, name, kind: node[name] === null ? 'file' : 'directory' }));
  return { entries: entries.slice(cursor, cursor + PAGE), next: cursor + PAGE < entries.length ? cursor + PAGE : null, total: entries.length };
}
async function readFile(task, path) {
  await sleep(180);
  if (task.machineRemote && SCEN.machine === 'offline') throw new Error('mini-heart is unreachable · ssh: connect timed out');
  const ch = task.changes().find(c => c.path === path);
  if (ch && ch.st === 'D') throw new Error('File no longer exists in the checkout · deleted in this Task');
  if (FILE_CONTENTS[path]) return FILE_CONTENTS[path];
  const ext = path.split('.').pop();
  const c = { rs: '//', swift: '//', py: '#', md: '', json: '' }[ext] ?? '//';
  if (ext === 'json') return `{\n  "path": "${path}",\n  "kind": "fixture",\n  "entries": []\n}`;
  return `${c ? c + ' ' : ''}${path}\n${c ? c + ' ' : ''}(prototype: contents not embedded — the real view reads this through lf task files)\n`;
}
function diffFor(path) {
  const ch = TASKS['LOO-515'].changes().find(c => c.path === path); if (!ch) return null;
  const src = (FILE_CONTENTS[path] || '').split('\n');
  if (path.endsWith('TaskFilesView.swift')) return [
    ['hunk', '@@ -26,22 +26,31 @@ struct TaskFilesView: View {'], ...src.slice(25, 28).map(l => ['', l]),
    ['del', '            HStack(spacing: 4) {'], ['del', '                ScrollView(.horizontal) {'], ['del', '                    HStack(spacing: 12) {'],
    ['del', '                        if task != nil { Button("Brief") { store.page = .task; store.noteInteraction() } }'],
    ['del', '                        Button("Changes") {'], ['del', '                            store.page = .files; store.mode = .diff; store.navigator = .pinned; store.noteInteraction()'],
    ['add', '            TaskDetailsHeader(task: task, page: $store.page)'], ['add', '            if store.page == .files {'],
    ['add', '                TaskDocumentRow(store: store)'], ['add', '                TaskLocationLine(location: store.location)'], ['add', '            }'],
    ['add', '            switch store.page {'], ['add', '            case .brief: TaskBriefView(task: task, scroll: $store.briefScroll)'],
    ['add', '            case .changes: TaskChangesView(store: store, publication: task?.pr)'], ['add', '            case .files: filesPage'], ['add', '            }'],
    ['hunk', '@@ -158,14 +167,22 @@'], ['del', '                    sectionHeader("Changed files", expanded: $store.showsChanges).padding(.top, 12)'],
    ['add', '                    TaskTreeView(tree: store.tree, changes: store.changes, selection: $store.selection)'],
    ['add', '                        .onKeyPress(.rightArrow) { store.tree.expand(store.cursor); return .handled }'],
  ];
  if (ch.st === 'A') return [['hunk', `@@ -0,0 +1,${ch.a} @@`], ...Array.from({ length: 22 }, (_, i) => ['add', i === 0 ? '#if os(macOS)' : i === 1 ? 'import SwiftUI' : i === 3 ? `/// ${path.split('/').pop()} — new in LOO-515` : i === 4 ? 'struct TaskBriefView: View {' : `    ${['let task: RoadmapTask', '@Binding var scroll: ScrollPosition', 'var body: some View {', '    ScrollView {', '        TaskStatusBlock(task: task)', '        RelatedTaskChips(ids: task.relatedIdentifiers)', '        BriefOutline(markdown: task.description)', '        MarkdownBlocks(task.description).frame(maxWidth: 680)', '    }', '}'][i % 10]}`])];
  if (ch.st === 'D') return [['hunk', `@@ -1,${ch.d} +0,0 @@`], ...['import Testing', '@testable import LoopflowMac', '', '@Test func publishedDiffDisclosesMissingAndPartialPatchesAndRenames() async throws {', '    let store = PullRequestStore(runner: .fixture("pr-files-partial"))', '    await store.loadFiles()', '    #expect(store.files.count == 3)'].map(l => ['del', l])];
  const at = Math.min(40, Math.max(0, src.length - 20));
  return [['hunk', `@@ -${at + 1},12 +${at + 1},${12 + Math.min(ch.a, 6) - Math.min(ch.d, 4)} @@`], ...src.slice(at, at + 4).map(l => ['', l]),
    ...src.slice(at + 4, at + 4 + Math.min(ch.d, 4)).map(l => ['del', l]),
    ...Array.from({ length: Math.min(ch.a, 6) }, (_, i) => ['add', (src[at + 4 + i] || '').replace(/pullRequest/g, 'changes').replace(/^(\s*)/, '$1') + (i === 0 ? '  // LOO-515' : '')]),
    ...src.slice(at + 8, at + 12).map(l => ['', l])];
}
function renderCode(text, hl) {
  return '<div class="code">' + text.split('\n').map((l, i) => `<span class="row${hl === i + 1 ? ' hl' : ''}"><span class="ln">${i + 1}</span>${esc(l)}</span>`).join('') + '</div>';
}
function renderDiff(rows) {
  let o = 0, n = 0;
  return '<div class="code">' + rows.map(([k, l]) => {
    if (k === 'hunk') { const m = l.match(/-(\d+).*\+(\d+)/); o = +m[1]; n = +m[2]; return `<span class="row hunk"><span class="ln"></span>${esc(l)}</span>`; }
    const ln = k === 'add' ? n++ : k === 'del' ? o++ : (o++, n++);
    return `<span class="row ${k}"><span class="ln">${ln}</span>${k === 'add' ? '+' : k === 'del' ? '−' : ' '}${esc(l)}</span>`;
  }).join('') + '</div>';
}

// ---------- PR card ----------
function prHeadline(pr) {
  return { none: ['hollow', 'No pull request yet'], publishing: ['working', 'Publishing…'],
    open: [pr.error || !pr.fresh ? 'neutral' : 'working', `Open #${pr.num}`], merged: ['success', `Merged #${pr.num}`], abandoned: ['neutral', `Abandoned #${pr.num}`] }[pr.kind];
}
function checksSummary(pr) {
  if (!pr.checks) return '';
  const c = { success: 0, working: 0, failure: 0 }; pr.checks.forEach(x => c[x.s]++);
  return [c.success && `${c.success} passed`, c.working && `${c.working} running`, c.failure && `${c.failure} failed`].filter(Boolean).join(' · ');
}
function renderPRCard(task, opts = {}) {
  const pr = task.pr();
  if (pr.kind === 'none') return `<div class="pr-card"><div class="pr-top"><span class="dot hollow" style="margin-top:6px"></span><div><div class="pr-title">No pull request yet</div>
    <div class="pr-sub">${task.changes().length ? 'The changes below are local to this checkout.' : 'Nothing changed in this checkout yet.'} Publishing happens when the Task's workflow reaches delivery.</div></div></div></div>`;
  if (pr.kind === 'publishing') return `<div class="pr-card"><div class="pr-top"><span class="dot working" style="margin-top:6px"></span><div><div class="pr-title">Publishing…</div>
    <div class="pr-sub">Publication requested ${pr.requested}. No GitHub pull request exists yet.</div></div></div></div>`;
  const [dot, head] = prHeadline(pr);
  const dim = pr.error || !pr.fresh;
  const checks = pr.checks ? `<div class="checks">${pr.checks.map(c => `<div class="check"><span class="dot ${dim ? 'neutral' : c.s}"></span>${c.name}<span class="dim" style="margin-left:auto">${dim ? '—' : c.t}</span></div>`).join('')}</div>` : '';
  const taskLine = pr.kind === 'merged' ? `<div class="warnline" style="color:var(--text2);margin-top:6px"><span class="dot hollow"></span>Task still open · completion is a separate step</div>` : '';
  return `<div class="pr-card">
    <div class="pr-top"><span class="dot ${dot}" style="margin-top:6px"></span><div style="min-width:0;flex:1">
      <div class="pr-title">${head} · ${esc(pr.title)}</div>
      <div class="pr-sub">${pr.kind === 'merged' ? `Merged as <span class="mono">${pr.sha}</span>` : pr.kind === 'abandoned' ? 'Closed without merging' : `${esc(pr.review)} · ${pr.merge}`}</div>${taskLine}
    </div></div>
    ${pr.kind === 'abandoned' ? '' : `<div class="pr-gh ${dim ? 'stale' : ''}">
      ${pr.error ? `<div class="errline">⚠ ${esc(pr.error)}</div>` : ''}
      ${pr.ahead ? `<div class="warnline">Local checkout is ${pr.ahead} commits ahead of the PR · <span class="mono">${pr.localHead}</span> vs <span class="mono">${pr.head}</span></div>` : ''}
      <div style="display:flex;gap:8px;align-items:center;margin-top:${pr.error || pr.ahead ? 6 : 0}px"><span>${dim ? 'Checks as of last reading' : 'Checks'} · ${checksSummary(pr)}</span>
      <span class="dim" style="margin-left:auto">GitHub, ${pr.read}${!pr.fresh && !pr.error ? ' · stale' : ''}</span><button class="icon-btn" data-act="pr-refresh" title="Read GitHub again">↻</button></div>
      ${opts.compactChecks ? '' : checks}</div>`}
    <div class="pr-links"><a href="https://github.com/jack-heart/loopflow/pull/${pr.num}" target="_blank">Open on GitHub ↗</a><a href="https://github.com/jack-heart/loopflow/pull/${pr.num}/files" target="_blank">Files changed on GitHub ↗</a>${pr.kind === 'open' ? '<a href="#" data-act="pr-comments">Review comments (2) ↗</a>' : ''}</div>
  </div>`;
}
function renderChangeRows(task, selected) {
  const ch = task.changes();
  if (!ch.length) return `<div class="empty">No changes in this checkout yet.<br>Base <span class="mono">${task.base}</span></div>`;
  return '<div class="changes-list">' + ch.map(c => `<div class="row${selected === c.path ? ' sel' : ''}" tabindex="0" data-change="${c.path}" title="${c.path}">
    <span class="st ${c.st}">${c.st}</span><span class="path">${esc(c.path)}</span><span class="num">${c.a ? `<span class="a">+${c.a}</span> ` : ''}${c.d ? `<span class="d">−${c.d}</span>` : ''}</span></div>`).join('') +
    `<div class="dim" style="padding:8px;font-size:12px">Compared with base <span class="mono">${task.base}</span> · <a href="#">Saved versions ↗</a></div></div>`;
}

// ---------- conversation ----------
const LOGS = {
  'LOO-515': [['you', 'hows it going'], ['tool', '⏺ Bash(lf task status LOO-515 --json)'],
    ['', 'Kickoff finished. LOO-515 is available for your design review: it\'s at the explore stage, nothing is running.\n\nThe draft proposes one header with Brief · Changes · Files, folds the PR into Changes and adds a real checkout tree.'],
    ['you', 'show me 3 different html prototypes'], ['tool', '⏺ Write(scratch/prototypes/…)'], ['', 'Three prototypes are open in Chrome. Each uses the real LOO-515 brief and this checkout\'s tree.']],
  'LOO-489': [['tool', '⏺ lf -b task run LOO-489 ship'], ['', 'gate is running on mini-heart. swift-test is the slow suite; about 8 more minutes.']],
};
function mountConvo(el, getTask, getContext) {
  el.innerHTML = `<div class="convo-head"><span class="dot"></span><span class="ct"></span><span class="spacer"></span><span class="dim">claude</span></div>
    <div class="convo-log" tabindex="0"></div><label class="convo-input"><span class="dim">&gt;</span><input placeholder="Ask about this Task…" aria-label="Message the Task conversation"><span class="ctx"></span></label>`;
  const log = $('.convo-log', el), input = $('input', el), ctx = $('.ctx', el);
  const scrolls = {};
  function render() {
    const t = getTask();
    $('.ct', el).textContent = `${t.id} conversation`; $('.convo-head .dot', el).className = 'dot ' + t.stateDot;
    log.innerHTML = LOGS[t.id].map(([k, s]) => `<p class="${k}">${esc(s)}</p>`).join('');
    log.scrollTop = scrolls[t.id] ?? log.scrollHeight;
    refreshCtx();
  }
  function refreshCtx() { const c = getContext(); ctx.textContent = c ? `viewing ${c}` : ''; }
  log.addEventListener('scroll', () => { scrolls[getTask().id] = log.scrollTop; });
  input.addEventListener('keydown', async e => {
    if (e.key !== 'Enter' || !input.value.trim()) return;
    const t = getTask(), q = input.value.trim(), c = getContext(); input.value = '';
    LOGS[t.id].push(['you', q]); render();
    await sleep(500);
    LOGS[t.id].push(['', c ? `Looking at ${c}: ${answer(q, c)}` : answer(q, c)]); render(); log.scrollTop = log.scrollHeight;
  });
  return { render, refreshCtx, focus: () => input.focus() };
}
function answer(q, c) {
  if (/500|page/i.test(q)) return 'PAGE_SIZE is 500 so one directory read stays under ~40 KB of JSON over ssh; `Load more` fetches the next cursor.';
  if (/why|what/i.test(q)) return 'this file is part of the Changes page. The PR card reads Loopflow\'s publication first and GitHub second, so a failed gh read never hides that a PR exists.';
  return 'noted. I\'ll keep that in the brief for the implementation pass.';
}

// ---------- chrome ----------
function mountChrome(current, getTaskId, setTask) {
  const bar = $('.titlebar');
  bar.innerHTML = `<div class="lights"><i></i><i></i><i></i></div>
    ${Object.values(TASKS).map(t => `<button class="slot" data-slot="${t.id}"><span class="dot ${t.stateDot}"></span>${t.id}<kbd>⌘${t.slot}</kbd></button>`).join('')}
    <span class="spacer"></span>
    <nav class="proto-nav"><a href="index.html">All</a><a href="a-pages.html" class="${current === 'a' ? 'on' : ''}">A · Pages</a><a href="b-dossier.html" class="${current === 'b' ? 'on' : ''}">B · Dossier</a><a href="c-navigator.html" class="${current === 'c' ? 'on' : ''}">C · Navigator</a></nav>`;
  const sync = () => bar.querySelectorAll('.slot').forEach(b => b.classList.toggle('on', b.dataset.slot === getTaskId()));
  bar.addEventListener('click', e => { const b = e.target.closest('.slot'); if (b) { setTask(b.dataset.slot); sync(); } });
  document.addEventListener('keydown', e => { if (e.metaKey && !e.altKey && (e.key === '1' || e.key === '2')) { e.preventDefault(); setTask(e.key === '1' ? 'LOO-515' : 'LOO-489'); sync(); } });
  sync();
  return sync;
}
function mountScenario(extraKeys) {
  const d = document.createElement('details'); d.className = 'scenario'; d.open = sessionStorage.getItem('lf-proto-panel') === 'open';
  d.innerHTML = `<summary>Prototype states</summary>
    <label>PR <select data-k="pr">${Object.keys(PR_SCEN).map(k => `<option ${SCEN.pr === k ? 'selected' : ''}>${k}</option>`).join('')}</select></label>
    <label>Brief <select data-k="brief">${['normal', 'long', 'empty', 'changed'].map(k => `<option ${SCEN.brief === k ? 'selected' : ''}>${k}</option>`).join('')}</select></label>
    <label>mini-heart <select data-k="machine">${['ok', 'offline'].map(k => `<option ${SCEN.machine === k ? 'selected' : ''}>${k}</option>`).join('')}</select></label>
    <div class="keys">${extraKeys}<br><kbd>⌘1</kbd> <kbd>⌘2</kbd> switch Task · LOO-489 has no PR and lives on mini-heart. <span class="mono">vendor/</span> fails to read; <span class="mono">tests/fixtures/large</span> pages.</div>`;
  d.addEventListener('change', e => setScen(e.target.dataset.k, e.target.value));
  d.addEventListener('toggle', () => sessionStorage.setItem('lf-proto-panel', d.open ? 'open' : 'closed'));
  document.body.appendChild(d);
}
function toast(msg) { let t = $('.toast'); if (!t) { t = document.createElement('div'); t.className = 'toast'; document.body.appendChild(t); } t.textContent = msg; t.classList.add('show'); clearTimeout(t._h); t._h = setTimeout(() => t.classList.remove('show'), 1600); }

// ---------- tree component ----------
// state: { expanded:Set, pages:{dir:{entries,next,error,loading}}, selection, cursor }
function treeState() { return { expanded: new Set(), pages: {}, selection: null, cursor: null, scroll: 0 }; }
async function loadDir(task, st, dir, more, rerender) {
  const p = st.pages[dir] = st.pages[dir] || { entries: [], next: 0 };
  p.loading = true; p.error = null; rerender();
  try { const r = await listDir(task, dir, more ? p.next : 0); p.entries = more ? p.entries.concat(r.entries) : r.entries; p.next = r.next; p.total = r.total; }
  catch (e) { p.error = e.message; }
  p.loading = false; rerender();
}
function visibleRows(st, dir = '', depth = 0, out = []) {
  const p = st.pages[dir];
  if (!p || (p.loading && !p.entries.length)) { out.push({ kind: 'loading', dir, depth }); return out; }
  if (p.error) { out.push({ kind: 'error', dir, depth, msg: p.error }); return out; }
  for (const e of p.entries) { out.push({ kind: e.kind, path: e.path, name: e.name, depth }); if (e.kind === 'directory' && st.expanded.has(e.path)) visibleRows(st, e.path, depth + 1, out); }
  if (p.next != null) out.push({ kind: 'more', dir, depth, shown: p.entries.length, total: p.total });
  return out;
}
function mountTree(el, getTask, getSt, onOpen, opts = {}) {
  el.classList.add('tree'); el.tabIndex = 0; el.setAttribute('role', 'tree');
  const rerender = () => render();
  function render() {
    const task = getTask(), st = getSt();
    if (!st.pages['']) { loadDir(task, st, '', false, rerender); }
    const changed = Object.fromEntries(task.changes().map(c => [c.path, c.st]));
    const rows = visibleRows(st);
    el._rows = rows;
    el.innerHTML = rows.map((r, i) => {
      const pad = `style="padding-left:${6 + r.depth * 14}px"`;
      if (r.kind === 'loading') return `<div class="note" ${pad}><div class="skeleton" style="width:${60 + (i % 3) * 30}px"></div></div>`;
      if (r.kind === 'error') return `<div class="note err" ${pad}>⚠ ${esc(r.msg)} <button class="btn" data-retry="${r.dir}">Retry</button></div>`;
      if (r.kind === 'more') return `<div class="note" ${pad}>${r.shown} of ${r.total} <button class="btn" data-more="${r.dir}">Load ${Math.min(PAGE, r.total - r.shown)} more</button></div>`;
      const isDir = r.kind === 'directory', open = st.expanded.has(r.path);
      const badge = changed[r.path] || (isDir && Object.keys(changed).some(p => p.startsWith(r.path + '/')) ? '•' : '');
      return `<div class="node${st.selection === r.path ? ' sel' : ''}${st.cursor === r.path ? ' cursor' : ''}" ${pad} data-path="${r.path}" data-kind="${r.kind}" role="treeitem" aria-expanded="${isDir ? open : ''}">
        <span class="caret${open ? ' open' : ''}">${isDir ? '›' : ''}</span><span class="name">${esc(r.name)}</span>${badge ? `<span class="badge ${badge === '•' ? 'M' : badge}">${badge}</span>` : ''}</div>`;
    }).join('');
    if (opts.afterRender) opts.afterRender();
  }
  function toggle(path, force) {
    const st = getSt(); const open = force ?? !st.expanded.has(path);
    if (open) { st.expanded.add(path); if (!st.pages[path] || st.pages[path].error) loadDir(getTask(), st, path, false, rerender); } else st.expanded.delete(path);
    render();
  }
  el.addEventListener('click', e => {
    const st = getSt();
    const r = e.target.closest('[data-retry]'); if (r) { loadDir(getTask(), st, r.dataset.retry, false, rerender); return; }
    const m = e.target.closest('[data-more]'); if (m) { loadDir(getTask(), st, m.dataset.more, true, rerender); return; }
    const n = e.target.closest('.node'); if (!n) return;
    st.cursor = n.dataset.path;
    if (n.dataset.kind === 'directory') toggle(n.dataset.path); else { st.selection = n.dataset.path; render(); onOpen(n.dataset.path); }
  });
  el.addEventListener('keydown', e => {
    const st = getSt(); const nodes = (el._rows || []).filter(r => r.path); if (!nodes.length) return;
    let i = nodes.findIndex(r => r.path === st.cursor); if (i < 0) i = 0;
    const cur = nodes[i];
    if (e.key === 'ArrowDown') { st.cursor = nodes[Math.min(i + 1, nodes.length - 1)].path; }
    else if (e.key === 'ArrowUp') { st.cursor = nodes[Math.max(i - 1, 0)].path; }
    else if (e.key === 'ArrowRight' && cur.kind === 'directory') { toggle(cur.path, true); return e.preventDefault(); }
    else if (e.key === 'ArrowLeft') { if (cur.kind === 'directory' && st.expanded.has(cur.path)) { toggle(cur.path, false); return e.preventDefault(); } const parent = cur.path.split('/').slice(0, -1).join('/'); if (parent) st.cursor = parent; }
    else if (e.key === 'Enter') { if (cur.kind === 'directory') toggle(cur.path); else { st.selection = cur.path; onOpen(cur.path); } }
    else return;
    e.preventDefault(); render();
    const c = el.querySelector('.node.cursor'); if (c) c.scrollIntoView({ block: 'nearest' });
  });
  // reveal a path: expand ancestors, then select
  async function reveal(path) {
    const st = getSt(), task = getTask(); const parts = path.split('/');
    for (let i = 1; i < parts.length; i++) { const d = parts.slice(0, i).join('/'); st.expanded.add(d); if (!st.pages[d]) await loadDir(task, st, d, false, rerender); }
    st.selection = st.cursor = path; render();
    const c = el.querySelector('.node.sel'); if (c) c.scrollIntoView({ block: 'center' });
  }
  return { render, reveal };
}

window.LF = { $, esc, sleep, SCEN, setScen, onScen: f => scenListeners.push(f), TASKS, KNOWN, PR_SCEN, md, outline, related, outcome, acceptance,
  listDir, readFile, diffFor, renderCode, renderDiff, renderPRCard, renderChangeRows, prHeadline, checksSummary, mountConvo, mountChrome, mountScenario, mountTree, treeState, toast };
})();
