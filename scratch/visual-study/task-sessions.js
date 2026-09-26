/* Task 3 study: open Sessions on a Task. Sample data only; nothing is sent. */
(function(){
  let renaming = null;
  const statusClass = st => st === "Working" ? "working" : (st === "Your turn" ? "turn" : "other");
  const excerptOf = s => s.excerpt || (s.lines && s.lines.length ? s.lines[s.lines.length-1][1] : "");
  const openIds = task => (task.sessions || []).filter(id => SESSIONS[id]);
  const relationship = s => s.flow
    ? `<span class="ts-origin ts-flow-origin" title="Session in the active ${esc(s.flow.name)} Flow, iteration ${s.flow.iteration}">${esc(s.flow.name)} · <code>${esc(s.flow.step)}</code></span>`
    : `<span class="ts-origin">Independent</span>`;
  const statusTag = s => s.status
    ? `<span class="ts-status ts-${statusClass(s.status)}"><span class="ts-dot" aria-hidden="true"></span>${esc(s.status)}</span>` : "";

  function currentId(task){
    const ids = openIds(task); const w = S.ws[task.id];
    if (w && ids.includes(w.session)) return w.session;
    return ids[0] || null;
  }

  function overview(task){
    const ids = openIds(task);
    if (!ids.length) return "";
    const rows = ids.map(id => { const s = SESSIONS[id];
      return `<li><button class="ts-row" data-act="selsession" data-id="${esc(id)}" aria-label="Open ${esc(s.title)}${s.status?" — "+esc(s.status):""}">
        <span class="ts-row-top"><span class="ts-title">${esc(s.title)}</span><span class="ts-agent">${esc(s.agent)}</span>${relationship(s)}${statusTag(s)}</span>
        <span class="ts-excerpt">${esc(excerptOf(s))}</span></button></li>`; }).join("");
    return `<section class="ts-overview" aria-label="Open Sessions">
      <h2 class="ts-heading">Sessions <span class="ts-count">${ids.length}</span></h2>
      <ul class="ts-list">${rows}</ul></section>`;
  }

  window.renderSessionBreadcrumb = function(task){
    const ids = openIds(task), sid = currentId(task);
    if (!sid) return "";
    if (renaming === sid) return `<span class="sep">/</span><form class="ts-rename" data-rename-session="${esc(sid)}"><input aria-label="Session name" data-fk="rename-session" value="${esc(SESSIONS[sid].title)}" required maxlength="100"><button type="submit">Save</button><button type="button" data-rename-cancel>Cancel</button></form>`;
    const name = ids.length === 1
      ? `<span class="ts-session-name" aria-current="page">${esc(SESSIONS[sid].title)}</span>`
      : `<select class="ts-session-name" aria-label="Session" data-ts-task="${esc(task.id)}" data-fk="session-name">${ids.map(id=>`<option value="${esc(id)}" ${id===sid?'selected':''}>${esc(SESSIONS[id].title)}</option>`).join('')}</select>`;
    return `<span class="sep" aria-hidden="true">/</span><span class="ts-named-session">${name}<button class="ts-rename-button" data-rename-session="${esc(sid)}" aria-label="Rename Session" title="Rename Session">✎</button></span>`;
  };

  window.renderOpenSessions = function(task, depth){
    if (!depth) return overview(task);
    const sid = currentId(task);
    if (!sid) return "";
    return `<div class="ts-workspace">
      <div class="ts-session-context">${relationship(SESSIONS[sid])}${SESSIONS[sid].flow ? `<span class="ts-iteration">Iteration ${SESSIONS[sid].flow.iteration}</span>` : ""}${statusTag(SESSIONS[sid])}</div>
      <div class="ts-surface">${sessionPane(sid, task.id, true)}</div></div>`;
  };

  document.addEventListener("click", e => {
    const button = e.target.closest?.('button[data-rename-session],button[data-rename-cancel]');
    if (!button) return;
    renaming = button.dataset.renameSession || null;
    render();
    document.querySelector('.ts-rename input')?.select();
  });
  document.addEventListener("submit", e => {
    const form = e.target.closest?.('form[data-rename-session]');
    if (!form) return;
    e.preventDefault();
    const input = form.querySelector('input'), name = input.value.trim();
    if (!name) { input.setCustomValidity('Enter a Session name.'); input.reportValidity(); return; }
    const session = SESSIONS[form.dataset.renameSession];
    session.title = name; session.nameSource = 'human';
    renaming = null; render();
  });
  document.addEventListener("input", e => {
    if(e.target.matches('.ts-rename input')) e.target.setCustomValidity('');
  });
  document.addEventListener("keydown", e => {
    if(e.key === 'Escape' && e.target.matches('.ts-rename input')) {
      e.preventDefault(); e.stopImmediatePropagation(); renaming = null; render();
    }
  }, true);
  document.addEventListener("change", e => {
    const select = e.target.closest?.("select[data-ts-task]");
    if (!select) return;
    const w = S.ws[select.dataset.tsTask];
    w.session = select.value; w.focus = "session";
    render();
  });
})();
