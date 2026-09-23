# Frozen starting ledger

Captured from cache-only PM and local live reads at 2026-09-23T00:09:59.229630+00:00.

Exact Project definitions, KR claims (ordered; provider has no KR ids), and full open Task directives are in [frozen-ledger.json](frozen-ledger.json). Raw PM snapshots include completed Tasks for reconciliation. Raw status captures preserve historical Work independently of PM.

## engbot — `4dbc0638-a441-489a-b095-f9e407ea01bf`

PM synced_at: `None`. Live status: `ready`; historical active Tasks: 0; current open PM Tasks: 0.

Error: wave/engbot has no local PM snapshot. Run `lf pm sync --wave engbot`.

## infrastructure — `6155f18a-1b7f-418c-9af3-8d6fa5ce4989`

PM synced_at: `1790116742`. Live status: `ready`; historical active Tasks: 82; current open PM Tasks: 1.

### Stability & Security — `03c59a52-dbd4-4e11-a6ac-f6d6359c8e08`

Loopflow control and delivery paths preserve state, authority, isolation, and recoverability across processes, machines, credentials, CI, and releases. Failures stop safely, surface actionably, and require no hidden manual repair.

1. Seven consecutive days of real Task, Project, and Wave runs produce zero strands after owner, process, or provider failure; zero lost or duplicated durable commands; and zero terminal-truth regressions.
2. Across 30 days, one writer owns each assigned worktree and zero implementation writes occur outside it.
3. Across 30 days, credentials and forwarded authority preflight or fail closed: zero runs are blocked by expiry and zero child receives secrets it does not need.
4. Four consecutive weekly releases and fourteen nightly verifications complete with zero manual repair; every red produces actionable work within one day.
5. Across 30 days, host, daemon, migration, and registry drift surfaces before a human and recovery preserves state and history.

- LOO-279 (`15172e70-1ec8-4cb0-a15c-f84d49361381`): Make Linear OAuth recovery survive unattended Project runs

### Technical Architecture — `62b73cde-5057-4959-8ad8-fca96b9e80b5`

Loopflow architecture is legible from the top down: key data structures, authorities, process boundaries, and public APIs explain the system; implementation follows that map; obsolete concepts do not linger as alternate designs.

1. A top-down architecture map names every durable control owner, truth source, process boundary, and public API; four consecutive weekly drift checks find zero unnamed owner, mirror, or shim.
2. For 30 consecutive days, every landed PR maps cleanly to documented architecture concepts or updates the map in the same PR.
3. Stale pre-Run, pre-Home, and pre-native-PM language reaches zero outside migrations and historical release material and stays zero for 30 days.


### Performance & Efficiency — `3778bc54-6fcb-4f38-b262-8eb68bcaa602`

Ordinary Loopflow work is fast and economical for humans, agents, and machines. Setup, builds, verification, navigation, worktrees, landing, and resource use stay within explicit budgets and require no avoidable manual work.

1. Task launch-to-first-progress, pre-land verification, and land-to-merge each have named p50 and p95 budgets and stay within them for 30 days.
2. Thirty consecutive landings report every local pre-land verification stage against its budget.
3. Seven consecutive days of normal development require zero avoidable setup or repair steps and zero manual Git surgery.
4. Build artifacts, disk use, CPU, and agent spend remain within named budgets for 30 days without disrupting work.


## intelligence — `cb366067-4821-4071-9393-82ff4b6d61d2`

PM synced_at: `1790116739`. Live status: `ready`; historical active Tasks: 2; current open PM Tasks: 0.

### Trace — `2f8390f7-6614-426c-84a8-fa5291358691`

Every run explains and replays itself, locally. Any question about what
happened, what it cost, and why is answerable on your own machine for the
life of the system — and a run can be replayed, not just read. Personal
deployments only: no remote telemetry server, ever (a bound, not a gap).

1. A month of runs is 100% reconstructable — prompt, context, flow/skill shape, spawned work, cost, time, result — verified by 20/20 random spot-audits against the long-lived ledger.
2. The trace readers survive the ledger they actually read: a month in which every `lf runs`, `lf trace`, and `lf usage` invocation on a real, migrated ledger succeeds, and none fails on a schema the migrations never produce.
3. Token and cost evidence has one home and no unexplained silence: for a month, every agent-bearing run lands in the ledger carrying provider, tokens, and cost; `lf doctor` reports no gap-day and no silence longer than a working day; and no run's spend is attributed to a nested `lf` sharing its `run_id`.
4. Any run from the last month replays against its recorded context, for debugging and for evals: 10/10 sampled runs replay unattended.
5. The stats surface answers the standing questions — what runs hot, what it costs, how it trends — in one query, on a month of real history.


### Context — `0f37b71f-3c7c-43f4-b809-ca2c346ca5fa`

Everything the model sees is one information system. The authored surface and the dynamic surface—including Discord inputs, Wave memory, Work observations, and per-pass context—stay deliberate, minimal, bounded, and evidence-justified.

1. For 30 consecutive days, every landed prompt, skill, flow, context-assembly, or chat-routing change cites the failing or costly run that motivated it and a follow-up run showing the intended behavior with no first-pass gate regression; zero uncited changes land.
2. For 20/20 sampled launches, the exact ordered provider context is reconstructable and every authored or dynamic component has a source, stable identity, content hash, token count, rendered order, and stated role; every launch for 30 consecutive days stays within its published total and per-layer budgets as Wave history and scratch grow.
3. Task intent survives handoffs: in 10/10 sampled multi-pass or restarted Tasks, a fresh agent receives the current human direction, full design, relevant Work observations, and current slice from durable worktree and launch context alone; no locally passing slice violates a recorded invariant or forbidden outcome.
4. Across at least three same-repository, same-flow, same-provider:model cohorts spanning at least two repositories and containing at least five landed runs in each adjacent 30-day window, median provider input tokens per run are lower in the later window and first-pass gate pass rate is no lower.
5. Zero-config excellence repeats: 3/3 fresh repositories reach a landed PR without context configuration, and no context knob is added or required for 30 consecutive days.
6. For seven consecutive days, every applicable shared Discord message is either folded into the owning Wave context within one cadence or remains visibly pending; zero audited launches receive an irrelevant shared message, and no Wave action requires another Wave raw transcript.


## list — `9861d5b6-e53d-493d-a59b-cc8960ff88ed`

PM synced_at: `1790116750`. Live status: `ready`; historical active Tasks: 0; current open PM Tasks: 0.

### Task-first control plane — `b211197b-4c71-4ea9-bef1-37f826ba5b5b`

Task execution has one explicit control authority and one explicit release-transition model, so every Task can be attached, recovered, signaled, and promoted safely when OS process identity, provider Invocation state, and telemetry disagree or are incomplete.

1. For 30 consecutive days of local Task operation, every liveness, signal, recovery, and promotion decision is attributable to one documented authority and matches OS/provider disagreement probes; no Task is stranded by contradictory control records.
2. Across 30 consecutive days and at least three promotions with live or recoverable Tasks, each Task has exactly one authoritative process tree before and after promotion, with no duplicate progress, lost handoff, or ambiguous release ownership.
3. Forced missing or stale ProcessOwner, provider Invocation, trace, and Exec evidence produces the designed attach, restart, recovery, or fail-closed outcome in tests and dogfood, and never signals a process whose exact ownership cannot be proven.


## product — `5081a624-c095-4218-9a54-6d1a6711f282`

PM synced_at: `1790116739`. Live status: `ready`; historical active Tasks: 37; current open PM Tasks: 11.

### Mac Surface UX — `57fee17f-7231-46f3-afd8-031d866a1779`

The Mac app is Loopflow's fast control room: all repositories, Waves, Projects, Tasks, Runs, PRs, activity, health, and legal controls remain legible without making Swift a second runtime. Company Discord is the canonical conversation; the app links to it and shows Work consequences. The Project prioritizes outcome repair when either Sessions readiness signal misses its published p95, reports an incorrect count, or claims readiness before interaction is live; Met readings stay quiet while the 14-day evidence windows accrue.

1. For one full week of real use, a cold launch exposes every repository and Wave, their purpose, Project/KR proof, open Tasks, attention, active agents, output-token rate, and Discord delivery state without opening a terminal.
2. Across the shared fixture suite and 20 sampled live Wave, Project, Task, and Run rows, every visible state agrees with the shared API on liveness, next owner, legal action, and reason.
3. Twenty cold launches and navigation trials stay inside published paint and interaction budgets on the long-lived registry; missing or stale evidence never renders as a healthy empty state.
4. Ten of ten control-room paths start or recover Work, open the exact PR or trace, edit planning through the PM API, and open the owning Discord channel without Swift inventing lifecycle.
5. For one month, the Mac app contains no second company transcript or Swift-owned Work concept; Discord and the shared lf DTOs remain the conversation and control authorities.
6. For 14 days of real use, opening Loopflow to a repository paints its scoped Sessions list with the true session count within the published p95 budget across at least 20 opens, with no empty-then-populate flash and no manual repair inside the window.
7. For 14 days of real use, selecting a session opens a live interactive embedded terminal within the published p95 budget across at least 20 opens (serial-open position included), or shows an honest opening state that never claims false readiness.

- LOO-283 (`9ed54c17-2b89-4bf9-b20d-0a95a1a65d44`): Evaluate shared Warp and Loopflow viewing without terminal regressions
- LOO-282 (`f7d1f6d9-0ed9-4ace-8d49-ed967dc875c9`): Show the recorded location of an active Session client
- LOO-284 (`656ca5c8-fe92-41c1-848a-b69b8b10f633`): Project Session actions and Work labels from the shared API
- LOO-280 (`40872684-3ee0-4507-b038-f1d379f49df6`): Build one Ghostty-enabled Mac terminal product
- LOO-281 (`01a33274-2895-4b44-b599-7e298f9b7656`): Prove shell blocks through the real shell, geometry, and long-output paths
- LOO-274 (`56cfdb24-cc73-4153-9ea8-1f2dab2af71a`): Restore Wave fleet and roadmap loading in the Mac app
- LOO-251 (`3aef0d2f-630c-49ba-9535-01a7591b7d18`): Finish the native Sessions multiplexer and promoted Ask handoff
- LOO-148 (`daeb8f50-8a6a-4168-9c65-adbee99e2d96`): Surface abandoned-Task recovery on the shared Now/Roadmap DTO

### Loopflow API — `d19956b2-9955-437d-aea6-d91766231c77`

The loopflow API is the product contract for goal-authored computation. Waves,
projects, tasks, agents, runs, chat, delegation, termination, recovery, and
evidence all share one coherent model across CLI, Mac, iOS, prompts, and worker
processes.

1. The thesis holds under load: >= 5 waves run consistently for 1 week straight across loopflow and Cadenza, at least 2 per codebase driven from GOAL.md with zero repo-authored skills added for the work.
2. Task loops earn trust by streak: over one week of real work, every dispatched task loop either lands its PR unattended or stops with an actionable non-convergence record — zero silent stalls, zero human rescues inside the window.
3. The spawn chain survives a week: wave loops spawn project loops spawn task loops with caps enforced throughout, and no run escapes its budget.
4. One model everywhere, continuously: for a month of landings, CLI, Mac, iOS, and agent prompts expose the same wave/project/task model — a parallel concept appearing anywhere is a failure event.
5. A new wave can be created, steered, delegated, inspected, and closed through the shared API contract, repeated cold 3/3 times.

- LOO-278 (`eef88fa6-ddcf-4570-a779-8110c7a161c6`): Add chapter reviews and interactive plan resets
- LOO-257 (`2ada2b81-ffe1-40cb-9544-8c7bcb613c66`): Base new Task worktrees on current canonical main
- LOO-185 (`2c3d04d1-91b9-45ab-9ee5-2488f97d8aad`): Provision Loopflow's company Discord channels

### Auditability — `95159066-9098-4d0b-8903-01459dc7ec14`

Every product surface shows enough truth to trust the system. Curation helps the user read; it never replaces the raw record — and that includes the system's own planning record: every claim points back to its receipt.

1. One week of real operation answers every "what is this wave doing?" from the product surfaces — each drop to raw transcripts or files is logged as a failure of this bet.
2. Drill-down holds end to end, every time: wave state -> run detail -> attachable live session, N/N attempts across a week.
3. Every visible state carries its reason (failed, waiting, running, idle, blocked, done) for the full lifetime of a wave, not just at steady state.
4. Curation always points back: for a month, every summary, retained fact, and planning claim drills to the raw record or audit evidence that justifies it — `pm doctor`-class checks find zero orphaned claims.

