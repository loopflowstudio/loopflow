// A read-only presentation of captured shared reads; no planning or Session writes.
if ((qsP.get("population") || "current") === "current") {
  const plan = window.CURRENT_PLAN;
  REPOS.splice(0, REPOS.length, ...plan.repos);
  for (const data of [TASKS, SESSIONS, RUNS]) for (const key of Object.keys(data)) delete data[key];
  Object.assign(TASKS, plan.tasks);
  S.currentData = plan;
  S.v = "a";
  S.ws = {};
  S.drafts = {};
  S.scopeMem = {};
  S.openWaves = {};
  S.q = "";
  S.zoom = 0;
  for (const repo of REPOS) {
    for (const wave of repo.waves) {
      S.openWaves[wave.id] = true;
      for (const id of wave.tasks) {
        S.ws[id] = {session:null, companion:false, monitor:false, focus:"session"};
        RUNS[id] = {complete:false, runs:[], gap:"Activity is not connected in this planning preview."};
      }
    }
    const first = repo.waves.flatMap(w => w.tasks)[0] || null;
    S.scopeMem[repo.id] = {sel:{kind:"wave", id:repo.waves[0].id}, lastTask:first, q:""};
  }
  S.scopeMem.all = null;
  S.scope = REPOS.some(r => r.id === qsP.get("repo")) ? qsP.get("repo") : "loopflow";
  const selected = qsP.get("task") || (S.scope === "loopflow" ? "LOO-291" : null);
  const saved = S.scopeMem[S.scope];
  S.sel = TASKS[selected]?.repo === S.scope ? {kind:"task", id:selected} : {...saved.sel};
  S.lastTask = S.sel.kind === "task" ? selected : saved.lastTask;
}
