// Explicit Session study fixtures. Captured planning text is unchanged.
(function(){
  const params = new URLSearchParams(location.search);
  const scenario = params.get('sessions');
  if (!['one','multiple'].includes(scenario)) return;
  const t = TASKS[params.get('task') || 'LOO-285'];
  if (!t) return;
  const examples = [
    {id:'study-release-discussion',title:'Release outcomes',agent:'Claude',status:'Working',flow:{name:'Feature',step:'implement',iteration:3},excerpt:'Implementing the deferred-release outcome and its resume path.',lines:[['you','Implement the deferred-release outcome and cover the resume path.'],['claude','A deferred release still has work to finish. A failed release has reached an unsuccessful outcome. Keeping that distinction visible lets you resume deferred work without counting a second release opportunity.'],['claude','I’m updating the resume path so the original opportunity is retained, then checking the deferred and failed outcomes separately.']]},
    {id:'study-release-checks',title:'Verification cases',agent:'Codex',status:'Working',excerpt:'Checking retry and overlap cases against the release model.',lines:[['you','Look at the retry and overlap cases. Which ones need a regression test?'],['codex','I’m checking what happens when a release resumes after a delay, and when another scheduled opportunity arrives before it finishes.']]},
    {id:'study-release-notes',title:'Acceptance notes',agent:'Claude',status:'Your turn',excerpt:'Which outcome should we demonstrate first?',lines:[['you','Help me put together a short demonstration of the release outcomes.'],['claude','We can follow one opportunity from its scheduled time through verification to its final outcome, then show a deferred attempt resuming. Which outcome should we demonstrate first?']]},
  ];
  const chosen = scenario === 'one' ? examples.slice(0,1) : examples;
  for (const sample of chosen) SESSIONS[sample.id] = {...sample,task:t.id,where:'here',run:'sample-'+sample.id};
  t.sessions = chosen.map(s=>s.id);
  t.started = true;
  S.ws[t.id].session = chosen[0].id;
  S.zoom = scenario === 'one' ? 1 : 0;
  window.isSessionStudyTask = task => task?.id === t.id;
  window.renderSessionStudyPage = task => {
    if (S.zoom > 0) return window.renderOpenSessions(task,S.zoom);
    return `<section class="page current-task ha"><header class="task-heading"><h1>${esc(task.title)}</h1>${window.renderTaskSessionAction(task)}</header>${window.renderTaskFlow(task)}${window.renderOpenSessions(task,0)}<div class="label-sm" style="margin-top:26px">Description</div><article class="directive-text">${task.directiveHtml}</article>${window.renderTaskComments(task)}</section>`;
  };
  window.renderSessionStudyHeader = task => {
    const session = S.zoom>0;
    const parent = session
      ? `<span class="ts-task-ancestor"><button data-act="inspecttask" data-id="${task.id}" title="${esc(task.title)}">${esc(task.title)}</button>${issueLink(task)}</span>`
      : issueLink(task);
    return `<button class="btn ghost small" data-act="${session?'inspecttask':'selwave'}" data-id="${session?task.id:task.wave}" aria-label="${session?'Back to Task':'Back to Wave'}">${I.back}</button><nav class="task-crumb ts-hierarchy" aria-label="Location"><button class="hwave" data-act="selwave" data-id="${task.wave}">${esc(waveOf(task.wave).name)}</button><span class="sep">/</span>${parent}${session ? window.renderSessionBreadcrumb(task) : ""}</nav>`;
  };
})();
