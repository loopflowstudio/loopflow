// Sample content for the density demo. No live planning or provider connections.
if (qsP.get("population") === "large") {
  const repo = REPOS.find(r => r.id === "loopflow");
  repo.waves.push(
    {id:"delivery", name:"Delivery", color:"#977344", objective:"Ship small changes with a clear path from review to release.", tasks:[]},
    {id:"integrations", name:"Integrations", color:"#72638B", objective:"Keep work connected to the tools people already use.", tasks:[]}
  );
  const titles = {
    product:["Keep the sidebar in place", "Restore a workspace after restart", "Make keyboard navigation predictable", "Show what needs my attention", "Remember each repository’s layout", "Make Task details easier to scan", "Improve empty workspace guidance", "Keep search fast while typing", "Resize the sidebar", "Choose a default conversation", "Make dark mode feel native", "Explain unavailable activity"],
    infra:["Speed up incremental builds", "Recover an interrupted install", "Keep local data through upgrades", "Make test failures actionable", "Reduce idle memory use", "Restore credentials after wake", "Make offline startup reliable", "Fix paths containing spaces", "Cache dependency downloads", "Simplify fresh checkout setup", "Measure cold launch time", "Clean up abandoned build outputs"],
    intel:["Summarize a long running attempt", "Explain why a Run is waiting", "Keep review feedback in context", "Resume from the latest evidence", "Recognize repeated failed attempts", "Improve the next-step recommendation", "Show useful progress summaries", "Preserve decisions across handoffs", "Compare two implementation approaches", "Make uncertainty explicit", "Reduce repeated repository reading", "Link conclusions to their evidence"],
    delivery:["Clarify the release checklist", "Keep release notes readable", "Recover a failed upload", "Show which checks are blocking", "Make preview builds easy to open", "Preserve review comments on rebase", "Track the current candidate", "Reduce time waiting on CI", "Explain a failed signing step", "Compare the candidate with main", "Make rollback steps discoverable", "Archive completed release evidence"],
    integrations:["Reconnect a disconnected account", "Keep issue links in Task details", "Choose where a conversation opens", "Show a useful authentication error", "Refresh planning after an edit", "Preserve links after a Task moves", "Handle a renamed repository", "Make account switching clear", "Import an existing issue", "Preview a planning update", "Respect the configured editor", "Explain a provider rate limit"]
  };
  titles.product.push(
    "Show the last useful update", "Keep long Task titles readable",
    "Restore focus after closing details", "Make Wave plans easier to browse",
    "Show completed work in chapter history", "Explain why a Task is paused",
    "Pin useful context beside a conversation", "Open a second conversation without losing the first",
    "Make split boundaries easier to grab", "Keep shell titles meaningful",
    "Find a Task by its issue number", "Return to the last scrolled position",
    "Keep a draft through repository switching", "Show failed reads beside their source",
    "Make empty Session counts quiet", "Distinguish waiting from running",
    "Improve the new conversation entry point", "Keep Task actions within reach",
    "Make account names easier to recognize", "Show a clear path back to the Wave",
    "Preserve a layout when a pane closes", "Support keyboard-only pane switching",
    "Make the selected Task easier to spot", "Improve contrast in inactive panes",
    "Explain a missing local checkout", "Show which conversation needs a reply",
    "Keep multiple windows independent", "Make search results easier to scan",
    "Group planning changes by chapter", "Show the original directive beside edits",
    "Make canceling an edit predictable", "Keep error recovery near the failed action",
    "Expose shortcuts without visual clutter", "Make zoom transitions feel continuous",
    "Keep the sidebar useful on a laptop", "Review the workspace with a first-time user"
  );
  let sequence = 310;
  for (const wave of repo.waves) {
    S.openWaves[wave.id] = true;
    titles[wave.id].slice(0, wave.id === "product" ? 48 : ({infra:4, intel:4, delivery:3, integrations:3}[wave.id])).forEach((title, index) => {
      const id = `sample-${sequence}`;
      const key = `LOO-${sequence++}`;
      const started = index < (wave.id === "product" ? 4 : titles[wave.id].length);
      const count = started ? [0, 1, 2, 1, 0, 3, 1, 0][index] : 0;
      const sessions = [];
      for (let n = 0; n < count; n++) {
        const sid = `${id}-session-${n}`;
        const name = ["Implementation", "Design discussion", "Review"][n];
        sessions.push(sid);
        SESSIONS[sid] = {
          id:sid, title:name, task:id, agent:n === 1 ? "Claude" : "Codex",
          where:index === 3 ? "elsewhere" : "here", run:`run_sample_${id}_${n}`,
          lines:[["you", `Let’s work on “${title}”.`], ["codex", "The current approach is ready to review. Your draft and companion panes stay with this Task."]]
        };
        S.drafts[sid] = n === 0 ? "One thing I want to check before we continue…" : "";
      }
      TASKS[id] = {
        id, key, title, repo:"loopflow", wave:wave.id, started,
        state:started ? (index % 3 === 1 ? "review" : "progress") : "upcoming",
        stateLabel:started ? (index % 3 === 1 ? "Needs your review" : "In progress") : "Upcoming",
        brief:`${title}, while keeping the current work easy to resume.`,
        outcome:["The common path is clear without extra setup", "Existing work survives navigation and recovery"],
        checkout:started ? `loopflow.${id}` : null, pr:null, sessions,
        directive:[`Improve this part of the ${wave.name.toLowerCase()} experience. Prove the user-visible result and preserve existing work.`],
        diag:`Sample planning only\nTask ${key}\n${started ? "Work has started" : "Work has not started"}`
      };
      wave.tasks.push(id);
      S.ws[id] = {session:sessions[0] || null, companion:started && index % 2 === 0, monitor:started && !count, focus:count ? "session" : "monitor"};
      S.reads[id] = "14:06:12";
      RUNS[id] = {complete:true, runs:started && !count ? [{name:"implement", agent:"Codex", started:"13:48", state:"working", exec:`exec_sample_${id}`}] : []};
    });
  }
  S.sel = {kind:"wave", id:"product"};
  const previewTask = qsP.get("task");
  if (TASKS[previewTask]) {
    S.sel = {kind:"task", id:previewTask};
    S.lastTask = previewTask;
  }
}
