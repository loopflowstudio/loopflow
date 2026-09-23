# Manual baseline — complete recorded KR evidence

Extracted from the original HTML without changing the review judgments. Exact current provider claims are frozen separately in the next chapter; typography and removed command names in this historical review remain as observed.

## Stability & Security

Operators can dispatch, resume, and release work without lost commands, expired authority, manual repair, or ambiguous terminal state.

Degraded state is often reported honestly, but credential expiry blocked planning and all 35 observed scheduled telemetry runs failed. The required strand and ownership cohorts were not retained.

Failure reporting improved; dependable unattended operation was neither achieved nor fully measured.

Review Run: `run_cbd9f4b09a3a42ffa6af298d2333a982`.

### KR 1 — unknown

Seven consecutive days of real Task, Project, and Wave runs produce zero strands after owner, process, or provider failure; zero lost or duplicated durable commands; and zero terminal-truth regressions.

Evidence: Current status preserves terminal PM truth and renders degraded historical Work. The observed recent Run history covers only part of 22 September, not seven days.

Gap: A seven-day cross-kind ledger for strands, command loss or duplication, and terminal-truth regressions.

### KR 2 — unknown

Across 30 days, one writer owns each assigned worktree and zero implementation writes occur outside it.

Evidence: On 22 September, LOO-279 and its uncommitted work were in the assigned worktree.

Gap: A 30-day writer-ownership and filesystem-write audit.

### KR 3 — fail

Across 30 days, credentials and forwarded authority preflight or fail closed: zero runs are blocked by expiry and zero child receives secrets it does not need.

Evidence: Linear token expiry blocked Project planning on 28 August and again across three Project runs on 22 September.

Gap: A complete authority-forwarding audit remains absent, but the zero-expiry-block condition is already disproved.

### KR 4 — fail

Four consecutive weekly releases and fourteen nightly verifications complete with zero manual repair; every red produces actionable work within one day.

Evidence: The latest fourteen verifications all failed. Across the larger observed window, all 35 telemetry targets failed; 32 of 33 release-run targets succeeded.

Gap: A four-publication release series and durable red-to-action latency mapping.

### KR 5 — unknown

Across 30 days, host, daemon, migration, and registry drift surfaces before a human and recovery preserves state and history.

Evidence: Telemetry surfaced stale-binary and schema drift on 22 September; a later doctor run showed aligned migrations and an intact store.

Gap: A 30-day detection-to-recovery ledger proving completeness and preservation for every incident.

## Technical Architecture

Maintainers can understand and change Loopflow from one architectural map without stale concepts or different local and hosted answers.

Nineteen consecutive first-parent landings mapped cleanly to the architecture model. The current tree still contains 43 locally detected stale vocabulary hits, and checker environments disagree.

Landed changes stayed legible, but the promise of one consistently enforced architecture remains incomplete.

Review Run: `run_4963e1d5bfd04a6e9a904a31d4e36dcc`.

### KR 1 — unknown

A top-down architecture map names every durable control owner, truth source, process boundary, and public API; four consecutive weekly drift checks find zero unnamed owner, mirror, or shim.

Evidence: A clean current archive mapped every discovered category. Four weekly snapshots replay cleanly.

Gap: The retained reports from the four actual scheduled hosted runs were unavailable; replay is not an execution receipt.

### KR 2 — holds

For 30 consecutive days, every landed PR maps cleanly to documented architecture concepts or updates the map in the same PR.

Evidence: All 19 first-parent landings from 23 August through 22 September passed the architecture checker contained in their own commit.

Gap: No material gap for the stated window.

### KR 3 — fail

Stale pre-Run, pre-Home, and pre-native-PM language reaches zero outside migrations and historical release material and stays zero for 30 days.

Evidence: The normal current checkout reports 43 stale-vocabulary hits under ignored .lf prompt and temporary history: 15 ‘session context’, 26 ‘lf radio’, and 2 ‘pm.linear_project’ references.

Gap: Unify local and hosted reviewed-source boundaries, then retain 30 clean days.

## Performance & Efficiency

Everyday commands start promptly and local development stays inside a predictable resource envelope.

The main build occupied 18.4 GiB against a 12 GiB budget. Authoritative latency, energy, thermal, and denominator scorecards were unavailable, while scheduled telemetry was failing.

The user experience is not demonstrably faster, and one explicit resource promise does not hold.

Review Run: `run_386f9b3906de470bb52845f981f9460d`.

### KR 1 — unknown

Task launch-to-first-progress, pre-land verification, and land-to-merge each have named p50 and p95 budgets and stay within them for 30 days.

Evidence: Named budgets exist, but the scorecard covers 14 days, owns no live metric, and failed on 22 September because the database lacked agent_turns.

Gap: A working authoritative 30-day scorecard.

### KR 2 — unknown

Thirty consecutive landings report every local pre-land verification stage against its budget.

Evidence: The trailing ledger had 16 records and 98 planned phases, but all were resource-blocked and executed zero phases; current merged landings do not join to exact-head gate records.

Gap: A durable landing-to-pre-land join covering 30 consecutive landings.

### KR 3 — unknown

Seven consecutive days of normal development require zero avoidable setup or repair steps and zero manual Git surgery.

Evidence: Manual Git repairs were recorded on 22 and 28 August. Recent operations cover only 20–22 September, not an exhaustive seven-day window.

Gap: A complete seven-day denominator for setup, repair, and Git surgery events.

### KR 4 — fail

Build artifacts, disk use, CPU, and agent spend remain within named budgets for 30 days without disrupting work.

Evidence: On 22 September the main build occupied 18.4 GiB against its 12.0 GiB limit. A current breach disproves continuous compliance.

Gap: Thirty-day CPU and agent-spend coverage is also absent.

## Trace

When a run fails or surprises someone, they can inspect a complete record, understand its cost, and replay eligible work.

Settled records still omit context, launch contracts, token counts, or cost, and no unattended replay cohort was recorded.

Trace data exists, but users cannot yet rely on it as a complete debugging or replay surface.

Review Run: `run_666e4d99aa7247199cf99f58641882c0`.

### KR 1 — fail

A month of runs is 100% reconstructable — prompt, context, flow/skill shape, spawned work, cost, time, result — verified by 20/20 random spot-audits against the long-lived ledger.

Evidence: Among 82 settled records, 6 lacked context and 35 lacked a launch contract. Of 80 settled usage rows, 21 lacked tokens and 76 lacked cost.

Gap: A durable 20-run reconstructability audit after population coverage reaches 100%.

### KR 2 — fail

The trace readers survive the ledger they actually read: a month in which every lf runs, lf trace, and lf usage invocation on a real, migrated ledger succeeds, and none fails on a schema the migrations never produce.

Evidence: lf runs, lf usage, and doctor succeeded on 22 September. lf trace is no longer a command; its replacement is lf runs <run>.

Gap: Rewrite the stale contract and retain a month-long reader receipt series.

### KR 3 — fail

Token and cost evidence has one home and no unexplained silence: for a month, every agent-bearing run lands in the ledger carrying provider, tokens, and cost; lf doctor reports no gap-day and no silence longer than a working day; and no run's spend is attributed to a nested lf sharing its run_id.

Evidence: Twenty-one of 80 settled rows lacked tokens, 76 lacked cost, and doctor retained eight gap-days in the month.

Gap: Close measurement gaps and retain a full-month continuity record.

### KR 4 — fail

Any run from the last month replays against its recorded context, for debugging and for evals: 10/10 sampled runs replay unattended.

Evidence: Sixty-three records appear eligible for replay, but the durable event set contains zero replay operations.

Gap: Run and retain the required 10/10 unattended cohort.

### KR 5 — fail

The stats surface answers the standing questions — what runs hot, what it costs, how it trends — in one query, on a month of real history.

Evidence: lf usage returns raw per-Run counters but does not aggregate hotness, cost, or trends, and does not claim interval completeness.

Gap: One Project-owned cohort query covering hotness, cost, and trend behavior.

## Context

Agents receive the right bounded context, and users can inspect what shaped a run without paying for unexplained prompt growth.

A twenty-launch audit reconstructed ordered provider context every time. Context budgets, thirty-day compliance, and durable Run citations were not established.

Sampled launches became explainable; efficiency and sustained correctness remain unproved.

Review Run: `run_c5880b1d2f684e12b95d8e4638ec2106`.

### KR 1 — fail

For 30 consecutive days, every landed prompt, skill, flow, context-assembly, or chat-routing change cites the failing or costly run that motivated it and a follow-up run showing the intended behavior with no first-pass gate regression; zero uncited changes land.

Evidence: Three qualifying landings on 28 August, 1 September, and 22 September lacked motivating and follow-up Run identifiers.

Gap: A complete join from qualifying landings to motivating Run, follow-up Run, and first-pass gate result.

### KR 2 — fail

For 20/20 sampled launches, the exact ordered provider context is reconstructable and every authored or dynamic component has a source, stable identity, content hash, token count, rendered order, and stated role; every launch for 30 consecutive days stays within its published total and per-layer budgets as Wave history and scratch grow.

Evidence: The 20-launch reconstruction audit passed completely. The conjunct still fails because no total or per-layer budgets are published and no 30-day compliance window exists.

Gap: Publish the budgets and record daily compliance.

### KR 3 — unknown

Task intent survives handoffs: in 10/10 sampled multi-pass or restarted Tasks, a fresh agent receives the current human direction, full design, relevant Work observations, and current slice from durable worktree and launch context alone; no locally passing slice violates a recorded invariant or forbidden outcome.

Evidence: Durable Work binding, scratch capture, and restart behavior shipped on 28 August.

Gap: A named ten-Task handoff sample with fresh-agent and invariant results.

### KR 4 — unknown

Across at least three same-repository, same-flow, same-provider:model cohorts spanning at least two repositories and containing at least five landed runs in each adjacent 30-day window, median provider input tokens per run are lower in the later window and first-pass gate pass rate is no lower.

Evidence: Usage spans many repositories but only about one direct 30-day window and does not join Runs to landed delivery or first-pass gates.

Gap: Three qualifying adjacent-window cohorts with delivery and gate joins.

### KR 5 — unknown

Zero-config excellence repeats: 3/3 fresh repositories reach a landed PR without context configuration, and no context knob is added or required for 30 consecutive days.

Evidence: No audit names three fresh repositories, their landed PRs, and configuration state.

Gap: Three recorded zero-config repository journeys and a 30-day knob audit.

### KR 6 — unknown

For seven consecutive days, every applicable shared Discord message is either folded into the owning Wave context within one cadence or remains visibly pending; zero audited launches receive an irrelevant shared message, and no Wave action requires another Wave raw transcript.

Evidence: Unified Wave chat and delivery reconciliation shipped on 1 September.

Gap: A seven-day message disposition, cadence, launch-relevance, and cross-Wave transcript audit.

## Mac Surface UX

People can understand, steer, and recover goal-authored work from the Mac app without creating a second source of truth.

Mac remained on shared authority, but Project editing and recovery are absent, and an externally terminated provider can appear live.

The shared foundation held; the Mac surface is still an incomplete and sometimes misleading control surface.

Review Run: `run_6d8bb1c87bd843c2920fa38794e64518`.

### KR 1 — unknown

For one full week of real use, a cold launch exposes every repository and Wave, their purpose, Project/KR proof, open Tasks, attention, active agents, output-token rate, and Discord delivery state without opening a terminal.

Evidence: Current source covers the named surfaces, but no dated week-long cold-launch census exists.

Gap: A seven-day census against the long-lived registry.

### KR 2 — unknown

Across the shared fixture suite and 20 sampled live Wave, Project, Task, and Run rows, every visible state agrees with the shared API on liveness, next owner, legal action, and reason.

Evidence: Sixty focused Swift tests and six Rust DTO fixture tests passed on 22 September.

Gap: A recorded comparison of 20 live rows across all four kinds.

### KR 3 — unknown

Twenty cold launches and navigation trials stay inside published paint and interaction budgets on the long-lived registry; missing or stale evidence never renders as a healthy empty state.

Evidence: Tests prove failed refreshes retain last-good evidence and disclose the reason. No paint or interaction budgets are published.

Gap: Published budgets and twenty long-lived-registry trials.

### KR 4 — fail

Ten of ten control-room paths start or recover Work, open the exact PR or trace, edit planning through the PM API, and open the owning Discord channel without Swift inventing lifecycle.

Evidence: Start, resume, PR, evidence, Wave pause/resume, and Discord actions exist. Recovery and PM mutation do not.

Gap: Shared recovery and PM-edit actions, then ten configured-path trials.

### KR 5 — holds

For one month, the Mac app contains no second company transcript or Swift-owned Work concept; Discord and the shared lf DTOs remain the conversation and control authorities.

Evidence: The 22 August–22 September history retained backing-aware Discord, provider-native Sessions, and shared lf DTO authority; Swift added no independent transcript persistence.

Gap: No material gap for the stated month.

### KR 6 — unknown

For 14 days of real use, opening Loopflow to a repository paints its scoped Sessions list with the true session count within the published p95 budget across at least 20 opens, with no empty-then-populate flash and no manual repair inside the window.

Evidence: The app reads repo-scoped Sessions and emits one-off time_to_sessions logs.

Gap: A published threshold plus a retained 14-day, 20-open count and flash dataset.

### KR 7 — fail

For 14 days of real use, selecting a session opens a live interactive embedded terminal within the published p95 budget across at least 20 opens (serial-open position included), or shows an honest opening state that never claims false readiness.

Evidence: A 22 September review found an externally terminated provider could remain displayed as live. The repair was not on main/v0.12.18.

Gap: Land the repair, publish p95, and retain 20 real opens over 14 days.

## Loopflow API

The same work model functions across CLI, Mac, iOS, and agents and scales across Waves and codebases without babysitting.

Five Waves were registered locally, but only one was live; no Cadenza week, complete spawn-chain cohort, or iOS application path was available.

Core commands exist, but portability, scale, and unattended trust remain largely unproved.

Review Run: `run_d2006f6015cb48189b3cb93ab1f0c44e`.

### KR 1 — unknown

The thesis holds under load: >= 5 waves run consistently for 1 week straight across loopflow and Cadenza, at least 2 per codebase driven from GOAL.md with zero repo-authored skills added for the work.

Evidence: Five Waves were registered locally, all in Loopflow; only Infrastructure was live. No Cadenza or seven-day fleet history was available.

Gap: Seven continuous days across both codebases and Homes, including skill provenance.

### KR 2 — unknown

Task loops earn trust by streak: over one week of real work, every dispatched task loop either lands its PR unattended or stops with an actionable non-convergence record — zero silent stalls, zero human rescues inside the window.

Evidence: The task-loop-trust metric exists but is uninstrumented, freshness never.

Gap: Classify every settled Task over seven days by unattended landing, non-convergence, rescue, and repair.

### KR 3 — unknown

The spawn chain survives a week: wave loops spawn project loops spawn task loops with caps enforced throughout, and no run escapes its budget.

Evidence: Recent records do not expose a complete Wave → Project → Task lineage or declared-cap comparison.

Gap: A seven-day lineage and cap-violation ledger.

### KR 4 — fail

One model everywhere, continuously: for a month of landings, CLI, Mac, iOS, and agent prompts expose the same wave/project/task model — a parallel concept appearing anywhere is a failure event.

Evidence: CLI, Mac, and prompts share the model, but the repository explicitly has no iOS application target. Git and Task state also disagreed for LOO-278 on 22 September.

Gap: An actual iOS path and a month-long compatibility/reconciliation ledger.

### KR 5 — unknown

A new wave can be created, steered, delegated, inspected, and closed through the shared API contract, repeated cold 3/3 times.

Evidence: The commands exist; no dated configured-path record exercises the full lifecycle three times.

Gap: Three timestamped cold create → steer → delegate → inspect → close runs.

## Auditability

A user can see what every Wave is doing and why, then drill from the summary to the run, session, and raw evidence.

Status surfaces are more readable, but PM, Work, Run, Git, and presentation state visibly disagree, and Project or KR claims lack durable evidence links.

Visibility improved; the product still exposes contradictions that prevent the summary from being trusted.

Review Run: `run_02669cd8b91f4529b41cf7581e241d32`.

### KR 1 — unknown

One week of real operation answers every ‘what is this wave doing?’ from the product surfaces — each drop to raw transcripts or files is logged as a failure of this bet.

Evidence: Status exposes substantial Wave truth, but Auditability had an empty weekly activity window and no fallback ledger.

Gap: A seven-day question and raw-record-fallback denominator.

### KR 2 — unknown

Drill-down holds end to end, every time: wave state -> run detail -> attachable live session, N/N attempts across a week.

Evidence: A Wave Run resolved by durable ID, but its detail had no trace, event, or Session link; Session rows had work:null.

Gap: An N/N weekly drill-down and live-attachment attempt ledger.

### KR 3 — fail

Every visible state carries its reason (failed, waiting, running, idle, blocked, done) for the full lifetime of a wave, not just at steady state.

Evidence: On 22 September human status rendered several missing-worktree Tasks ready while roadmap JSON called them blocked. Other rows simultaneously carried done, blocked, waiting, and start recommendations.

Gap: One transition record and presentation contract that reconciles PM, Work, Run, PR, and UI state.

### KR 4 — fail

Curation always points back: for a month, every summary, retained fact, and planning claim drills to the raw record or audit evidence that justifies it — pm doctor-class checks find zero orphaned claims.

Evidence: Project and KR payloads expose no evidence references; no claim-citation implementation exists; pm doctor does not audit orphaned claims.

Gap: Receipt fields, claim provenance, orphan checks, and a one-month observation window.
