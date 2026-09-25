// Task 2 study: execution without an open Session. Scenario fixture only.
// Parent calls window.renderTaskExecution(task, "running"|"paused"|"blocked").
// Pause/Resume dispatch `task-execution-action` on window; parent owns graph state.
(function(){
  const esc = s => String(s).replace(/[&<>"']/g, c => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c]));
  const local = {}; // per Task: { history, note }
  const st = id => local[id] || (local[id] = { history: false, note: false });

  const scenarios = {
    running: {
      label: "Running", tone: "run",
      where: '<code>implement</code> · iteration 3 · Claude',
      line: "2m 14s · No open Sessions.",
      action: { kind: "event", value: "pause", text: "Pause",
        help: "Pause at the next step boundary. The current step finishes first." },
    },
    paused: {
      label: "Paused", tone: "pause",
      where: 'iteration 3 · next <code>compress</code>',
      line: "<code>implement</code> finished. No active Runs and no open Sessions.",
      action: { kind: "event", value: "resume", text: "Resume at compress",
        help: "Resume this iteration at compress." },
    },
    blocked: {
      label: "Blocked", tone: "block",
      where: 'iteration 3 · <code>loop-decide</code>',
      line: "No active Runs and no open Sessions.",
      reason: "The release target is unavailable. Restore access or choose another target before continuing.",
      action: { kind: "local", value: "unblock", text: "Unblock…",
        help: "Would open a human conversation about this blocker." },
    },
  };

  // Scenario-owned sample records; not a complete invocation history.
  const history = {
    running: [["implement", "iteration 3", "running"], ["loop-decide", "iteration 2", "iterate"], ["concept-review", "iteration 2", "finished"]],
    paused:  [["implement", "iteration 3", "finished"], ["loop-decide", "iteration 2", "iterate"], ["concept-review", "iteration 2", "finished"]],
    blocked: [["loop-decide", "iteration 3", "blocked"], ["concept-review", "iteration 3", "finished"], ["review-slice", "iteration 3", "finished"]],
  };

  function actionButton(task, a){
    return `<button type="button" class="btn small${a.kind === "event" ? " primary" : ""}" data-execution-action="${a.value}" data-execution-task="${esc(task.id)}" title="${esc(a.help)}">${esc(a.text)}</button>`;
  }

  window.renderTaskExecution = function(task, state, context = {}){
    const base = scenarios[state];
    const sc = base && {...base, action:{...base.action}};
    if (sc && state === "running" && context.skill) {
      sc.where = `<code>${esc(context.skill)}</code> · iteration 3 · Claude`;
      if (context.skill !== "implement") sc.line = "Just resumed · No open Sessions.";
    }
    if (sc && state === "paused" && context.nextSkill) {
      sc.where = `iteration 3 · next <code>${esc(context.nextSkill)}</code>`;
      sc.line = `<code>${esc(context.lastSkill)}</code> finished. No active Runs and no open Sessions.`;
      sc.action.text = `Resume at ${context.nextSkill}`;
      sc.action.help = `Resume this iteration at ${context.nextSkill}.`;
    }
    if (!task || !sc) return "";
    if (task.sessions.length) sc.line = sc.line.replace(/No open Sessions\.|no open Sessions\./, `${task.sessions.length} open Session${task.sessions.length === 1 ? "" : "s"}.`);
    const s = st(task.id);
    const records = history[state].map(row=>[...row]);
    if (state === "running" && context.skill) records[0][0] = context.skill;
    if (state === "paused" && context.lastSkill) records[0][0] = context.lastSkill;
    const rows = records.map(([step, pass, outcome]) =>
      `<li><code>${esc(step)}</code><span>${esc(pass)}</span><span class="te-outcome te-${esc(outcome)}">${esc(outcome)}</span></li>`).join("");
    const note = state === "blocked" && s.note
      ? `<div class="te-note" role="status"><span class="te-proto">Prototype</span><span>This would open a human conversation about the blocker. No Session is open, and the Flow stays at <code>loop-decide</code>.</span><button type="button" class="btn small ghost" data-execution-action="dismiss" data-execution-task="${esc(task.id)}">Dismiss</button></div>`
      : "";
    return `<section class="task-execution te-${sc.tone}" aria-label="Execution">
      <div class="te-row">
        <span class="te-state"><span class="te-dot" aria-hidden="true"></span>${sc.label}</span>
        <span class="te-where">${sc.where}</span>
        <span class="te-spacer"></span>
        ${actionButton(task, sc.action)}
      </div>
      ${sc.reason ? `<p class="te-reason">${esc(sc.reason)}</p>` : ""}
      <p class="te-line">${sc.line}</p>
      ${note}
      <div class="te-history">
        <button type="button" class="te-disclose" aria-expanded="${s.history}" data-execution-action="history" data-execution-task="${esc(task.id)}">
          <svg class="te-chev" viewBox="0 0 12 12" aria-hidden="true"><path d="M4 2l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>Recent runs</button>
        ${s.history ? `<ul class="te-runs">${rows}</ul><p class="te-caveat">Sample records for this scenario, not the full history.</p>` : ""}
      </div>
    </section>`;
  };

  document.addEventListener("click", e => {
    const el = e.target.closest && e.target.closest("[data-execution-action]");
    if (!el) return;
    const action = el.dataset.executionAction, id = el.dataset.executionTask, s = st(id);
    if (action === "pause" || action === "resume") {
      window.dispatchEvent(new CustomEvent("task-execution-action", { detail: { action } }));
      return;
    }
    if (action === "history") s.history = !s.history;
    if (action === "unblock") s.note = true;
    if (action === "dismiss") s.note = false;
    if (typeof render === "function") render();
    const sel = action === "unblock" ? '.task-execution [data-execution-action="dismiss"]'
      : action === "dismiss" ? '.task-execution [data-execution-action="unblock"]'
      : '.task-execution [data-execution-action="history"]';
    document.querySelector(sel)?.focus();
  });
})();
