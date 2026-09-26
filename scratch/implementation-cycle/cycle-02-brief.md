# Cycle 2 — named native Sessions

Strict subset of scratch/main-view-task.md: Session drill-down, inline rename,
and exact shared Flow membership. Do not rework all Wave/Task/Flow rendering yet.
Use task3-study.html, task-sessions.js/css, task3-fixture.js and the accepted
reference manifest as visual direction, not a new native transcript or store.

Implement through current RegistryQuery/Podium/WorkspaceNavigation and retained
Session/native surface owners. Preserve the one repo → Wave → Task → Session
hierarchy, final named breadcrumb choice for multiple Sessions, parent navigation
and Task's linked Linear ID. No separate Session-with-context presentation and
no Task Description on the Session surface. Inline rename uses shared CLI
readback, protects manual titles, retains draft/error on rejection, and cannot
publish an old rename/read into a new target after navigation or polling.

Project exact Session membership in its active Flow/step/iteration through the
shared Rust/Swift contract; include historical and unavailable evidence. Do not
infer membership from Task/cwd/provider, or call an unavailable relationship
Independent. Use an existing execution/Run relationship; do not invent a second
registry/index/writer. If current authority cannot supply a fact, make that
specific limitation explicit and preserve the full design.

Carry cycle-01-review.md's gaps forward: actual Flow boundary naming and recovery
proof, exact agent command before provider history, and remote Home behavior.
The candidate remote Flow title currently derives a generated local display seed
without reading the remote authoritative name. Never claim that fallback is the
remote canonical human/generated title; resolve through the existing authority
or represent the missing evidence explicitly. Route name edits through the proper
Home where supported. Scope naming guidance to actual Sessions (headless skills
must not try renaming a nonexistent Session).

Focused proof: held rename/read response through navigation/refresh; canonical
name after success, retained error/input after rejection; exact one/multiple
Session breadcrumb; same real native surface/PTY, draft and companion across
rename and ancestor return. Shared Rust/Swift DTO fixture proof plus Flow CLI
behavior in isolated Home. No real user Session mutation or configured provider
launch solely for evidence. Use resource-checked native build/test workflow;
no broad gate or repeated suites without new changes.

No commits, publication, PM writes, install, new worktree, or Task lifecycle
transition. Preserve all unrelated dirty work. No subagents or next phase from
this contribution. Use rg; Python through uv. Parent runs compress then review.
Receipt: scratch/implementation-cycle/cycle-02-implement.md with paths, exact
commands, outcomes, counterexamples, remaining limitations and next slice.
