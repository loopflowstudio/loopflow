# Research: Exec admission

## System understanding

2026-09-29 · Bounded source audit for LOO-298 / Jack Heart. HEAD `4682d662ad6db34ea5167666b869c6ecb4cee828`.
Read AGENTS, README, Infrastructure memory, accepted model, remaining-work §3 and retained counterevidence. No branch binary, provider, build, test, benchmark, installation or external write ran. Main's concurrent executable changes are outside this contribution.
Exact 39-file working-source manifest: `.lf/tmp/exec-admission-review/source-hashes.json`, SHA-256 `626288aa5d779a9c67fac8713ddfefd234dd84d29f85ca0be74fcba8e675daf4`. Paths below are relative to `rust/loopflow/src/` unless prefixed otherwise; line anchors describe these inspected bytes. Closing comparison is in the sibling `drift.json`.

### Architecture and data flow

`bin/lf.rs:1191` runs machine entry dispatch → tracing → argument normalization/Clap → installation authorization → branch-data isolation → signal handler → special early dispatch → cwd → `journal::with_runtime` → command dispatch.
`journal/mod.rs:223` owns ordinary start/finish; nested wrappers reuse its process-local `RunContext`. `ensure_run_context:488` resolves optional repo/Wave, caller/parent and trace, mints Exec identity, exports it, and registers interrupt cleanup plus exact PID/start evidence. Optional file journaling is separate from the SQL write, but context preparation still performs Git, store and filesystem work.
`ledger_insert:376` calls `SqliteStore::insert_run_event` (`store/sqlite.rs:2053`): one transaction inserts Exec if absent, appends the trace event, and fills the first terminal outcome. Command `Ok/Err` maps to exit 0/1; the interrupt hook maps to interrupted/130 with unknown signal. The local finished flag prevents competing terminal emissions. A later terminal write can insert the row using the process-local recorded start even if the first SQL write failed.
`open_ledger:446` uses ordinary `SqliteStore::new`, which validates installation authority and can initialize/migrate private development data (`store/sqlite.rs:442`). It is not a universal no-effects observer. Ledger failures warn once and do not fail a mechanical command; file failures also leave SQL eligible.

### Entry-path coverage: source-proven reachability, not new execution results

| Actual lf path | Start / outcome coverage | Boundary |
| --- | --- | --- |
| Ordinary parsed commands, including auth, catalog, Session operations, Task inspection, agent/Flow launch | Best-effort start and 0/1 completion | `bin/lf.rs:1305`; command resolution failures inside dispatch are recorded. Outer context precedes explicit Wave resolution. |
| Help/version and rejected Clap parsing | Neither | `Cli::parse_from` at 1213 prints/exits before observation. Parser unit tests call `try_parse_from`; they do not prove process recording. |
| Entry-gate failure, startup authorization/isolation failure, unavailable cwd | Neither | Fail before `with_runtime`; successful Unix entry dispatch uses `exec` (`machine_install.rs:820`), the same OS process, so gate plus destination must not become two Execs. |
| `install`, schedule, preflight/local-preflight, promote, rollback, recovery/advance | Neither through this entry | Early return at 1253. `install.rs:706` explicitly promises read-only preflight without journal writes. Candidate preflight itself launches another actual lf (`:1344`). |
| `screenshot` and `__screenshot-supervisor` | Neither | Early returns at 1238. Public capture spawns a real lf supervisor (`screenshot.rs:22`); browser processes are not Execs. Supervisor owns pipe-loss cleanup independently of Home/store. |
| `task __worker`, `__telemetry-scorecard`, provider-session observer, account-lease probe | Ordinary wrapper covers them | Worker dispatch at 1568; scorecard at 1606; observer at 1456; probe at 1346. Hidden command status is not a recording exception. |
| SSH remote command failure | Start; no normal terminal write | `lf/commands/ssh.rs:311` calls `process::exit(code)` after dropping its broker, bypassing wrapper and main cleanup wait. Transport errors return normally as errors. |
| `release check` with no commits | Start; no normal terminal write | `lf/commands/ops/mod.rs:1324` calls `process::exit(1)`; release operation unit tests do not exercise this CLI exit. |
| Interrupt after admitted context | Best-effort interrupted/130 | `journal/mod.rs:623`; `engine/agent.rs:81` holds cleanup mutex through exit. `bin/lf.rs:1187` waits for cleanup before ordinary return. SIGKILL, abort or death before admission supplies no completion. |

### Child attribution and work ownership

Direct children inherit `LF_TRACE_ID` / `LF_PROCESS_ID`; a fresh trace drops a stale parent, and an absent recorded parent is not invented (`journal/mod.rs:535–565,680`). Nested Rust calls do not create child Execs. Task workers explicitly forward these variables through tmux (`ops/run.rs:324`, `engine/process.rs:299`); the worker's own claim/destination checks occur after its command admission.
Agent-issued children carry `AgentCaller { session_id, provider_generation, origin_exec_id, flow_turn }`. Admission consumes/removes `LF_AGENT_CALLER` before direct children can inherit the agent edge. `store/sqlite/execs.rs:273` resolves a matching provider generation/origin to today's driver; absent driver or replacement retains the recorded historical origin if resolvable. This parent is written once; a later handoff cannot rewrite it.
`execs.via_agent` records the incoming edge, independently of whether the command launches an agent. No caller gives false on fresh admission; imported unknown remains nullable. Stale-generation commands can retain causal history without acquiring work/control: `agent_work_in:9` rejects replaced providers; Flow navigation separately checks selected native start, generation, caller token, version and absence of completion (`store/sqlite/flows.rs:535`).
The Codex harness creates each Flow-turn token (`harness/codex.rs:1098`), but retained failed-thread retry evidence shows tools keep the old environment. Current driver lookup cannot repair original-turn transport and must not accept today's token on behalf of an old child.
Exec stores argv/repo/cwd/caller, not typed Task/Wave command context. The journal separately records an ambient Wave label. Current Session ancestry is consulted when inheriting work into a new Session (`store/sqlite/sessions.rs:1006`); it is not a historical Task field on Exec. Task/history search must distinguish command context from actual performed work across several Sessions/Flow boundaries.
Started belongs to actual reservation/bind and mechanical start: Session triggers at `agent_session_admission…sql:96`, Flow operation history transaction at `store/sqlite/flows.rs:982`. Exec insertion updates neither Task nor Session. The existing inspection test checks zero old Run rows, not preservation of a preexisting unstarted Task; strengthen that specific boundary in downstream proof.

### Strict admission versus best-effort observation

Ordinary agent capture publishes immutable payload then requires a Session row (`run_record.rs:1680,1963`); reserved Flow input requires fenced publication (`:1590`). `engine/agent.rs:1146` and `lf/commands/run.rs:878` claim the conversation before launch. Driver/provider references are foreign keys to Exec. Failure of these required writes propagates; diagnostic observation failure does not grant provider admission.
A separate source gap exists: malformed `LF_AGENT_CALLER` returns before context creation (`journal/mod.rs:508`), `emit` swallows the error, and dispatch continues. `claim_conversation_driver` returns success when `current_exec_id` is absent (`run_record.rs:1720`), intended for library callers. Thus source permits a real CLI to reach that library exemption; whether a particular launch reaches provider effects needs the focused public probe below. Do not call the unavailable-store test proof of this case.

### Evidence and counterexamples

Inspected tests, not executed: `tests/exec_ownership_tests.rs:17,55,111` assert ordinary completion, obstructed file journal with SQL retention/actual cwd, and command failure. `:263` jointly checks OS exit130, SQL outcome and disappearance of the owned scorecard, with Linux fault injection. Journal tests at 1479/1511/1562/1749 cover parent retention/missingness and nested wrappers; the first simulates contexts within one test process.
`tests/exec_ownership_tests.rs:358,495` use the real Codex harness under opt-in setup to assert driver handoff, provider replacement, direct child count and passive old-client display; they also seed Session/driver facts. They are not evidence that every internal command path is covered. `tests/session_cutover_tests.rs:1119` obstructs the store and asserts no scripted provider starts. `tests/global_commands.rs:78,114,150` retains read-only candidate preflight, Git-free machine commands and failed repository-command admission; `tests/auth_tests.rs:629` proves no account/Session creation despite a new Exec/store.
Screenshot tests retain real CLI plus fake-browser success and parent-death cleanup, but inspect no Exec rows. SSH's `classify_exit` test (`ssh.rs:1084`) checks exit classification only. These do not close the newly identified early-exit coverage.
Historical evidence retained in `scratch/evidence.md:908`: parsed-command probe saw no help/version/argument-error Execs; 9–11ms early paths versus ~752ms empty inventory. The WAL example at 894 lost a start before later focused concurrency passes. Neither supplies a universal durability/performance guarantee.
Retained `scratch/parallel-execution.md:640,678,894` records Git discovery blocking cached auth, subdirectory journals landing in the wrong place, orphan scorecard cleanup, and OS exit1 versus SQL interrupted130 under controlled Linux ordering. They prohibit moving full repository/store initialization ahead of every command or bypassing the existing cleanup lock. These are recorded results, not rerun here.

## Tensions

- One process lifecycle versus late store selection: moving the existing whole `with_runtime` above parsing would introduce Git, Wave lookup, branch snapshot/migration and file writes into help, bootstrap and screenshot cleanup.
- Complete durable history versus unavailable/forbidden stores: first install has no settled store; preflight must not mutate its inspected target. No source-supported writer can guarantee an Exec there while retaining those contracts. Report missing persistence; do not create a second database/registry or pretend a receipt was observed.
- Causal observation versus authority: weak observation must not block mechanical commands; missing/malformed required Exec/Session authority must not become the library no-Exec exemption in an actual agent CLI.

## Observations

### Complexity and quality

Lifecycle ownership is concentrated in journal code, yet exits are distributed among Clap, startup, two command handlers and the interrupt hook. `with_runtime` currently bundles identity, file placement, attribution, store opening and completion; it is the concrete reduction point. Existing Session/Flow wrappers are harmlessly nested today, but obscure the true process boundary.
`engine/process.rs:390` clears many ambient variables for tmux but omits `LF_AGENT_CALLER`; the explicit forwarded set includes only trace/process provenance. A poisoned preexisting tmux-server environment is not covered by the inspected parent tests. This is a bounded transport check, not evidence of an observed misattribution or a reason for a general race framework.
### Potential

The existing Exec row/event transaction and process-local context already support one process performing many agent/mechanical boundaries. Exact command return can use the same writer; no new lifecycle table, generic attempt, provider change or process registry is needed.

## Recommendations

### Put observation at process entry/exit; attach its existing store only when safe

**Observation:** all uncovered returns and ordinary hard exits sit outside the existing finish path; the writer itself already has the right durable owner.
**Change:** separate minimal in-memory process identity/start/caller capture from optional enrichment and SQL attachment in the existing journal context. Route non-exiting `try_parse_from` results, startup failures and dispatch results through one finish path with actual chosen CLI exit code (including Clap 2 and SSH's code). Replace the two ordinary `process::exit` sites with a returned exit value/error carrying that code. Preserve interrupt cleanup ordering and one context across nested calls.
**Store boundary:** keep machine dispatch/authorization and branch isolation ahead of any ordinary write. Attach the observation to the already selected, permitted store; early-only commands may attempt an existing compatible observation sink without initializing, migrating or seeding data. Preflight's inspected store is never silently repurposed as writable telemetry. With no independently permitted sink, retain explicit unavailable coverage. Pin an attached sink for that process; installation changing selection must not split its start/end across stores. Unix gate exec remains one process, not a manufactured parent/child pair.
**Admission boundary:** a real CLI agent launch must require its current Exec row and Session/driver reservation before provider effects, regardless of best-effort earlier logging. Keep the library no-process case explicit. Do not fall back from invalid caller evidence to invented direct/root authority; mechanical commands can still report unavailable observation without acquiring agent work.
**Cost:** medium, approximately 5–7 production surfaces: CLI entry/exit, existing journal context/writer, store attachment/resolution, SSH/release returns and strict capture admission; existing install/screenshot dispatch supplies its constraints. Tests/docs add separate scope. No schema change is inherently needed for entry coverage; typed Task-context discovery remains separate unfinished work.
**Deletion opportunities:** remove duplicate per-dispatch lifetime wrapping once entry ownership is established (retain repo resolution), the two ordinary hard-exit branches and fixed 0/1-only terminal mapping. Keep exact process receipts, interrupt hooks and Session/Flow fences; those carry distinct required facts.
**Verdict:** coherent bounded repair, provided unavailable-store cases remain honestly best effort. Simply moving the present wrapper earlier is contradicted by retained incidents.

### Smallest distinguishing public CLI proofs (for main; none run here)

1. One private compatible Home, ordinary cwd/no Git: run `--help`, `--version`, a rejected known-command flag, successful auth inspection, missing Session rename and empty `release check` in its disposable repo. Match OS exit to exactly one completed Exec per actual process (0/2/1 as appropriate); zero agent work and an existing unstarted Task unchanged. Add fake SSH exit42 to expose the hard-exit gap. Query retained SQL without another lf process contaminating counts.
2. Repeat harmless inspection with an unwritable/unavailable ledger: behavior still succeeds and missing observation is explicit. Launch a scripted provider with the same obstruction and separately malformed caller JSON: require failure before its marker is written. This distinguishes strict agent admission from merely best-effort telemetry.
3. Extend existing screenshot fixture: exactly public+supervisor Execs, direct edge false, no browser Exec; preserve two-second owner-loss cleanup with broken storage and retain unknown outcome after SIGKILL. Use disposable OS installation fixture for first-install/incompatible preflight: target schema/history/data unchanged by observation; gate re-exec does not duplicate identity. No real installed Home.
4. Reuse actual caller-handoff proof, add a direct grandchild and stale-provider child plus poisoned tmux-server caller metadata; assert immutable accepted parents, incoming bits and no extra work/control. Repeat interrupt ordering only when the new finish path changes it. These are separate boundaries, not a reason to repeat the full matrix.

## Open questions

Universal durable recording before any permitted store exists cannot be established under read-only bootstrap and single-owner constraints; accepted docs already require explicit unavailable coverage. No new policy is selected here. Early-path latency, actual startup-failure outcomes, all-provider transport, tmux contamination, strict malformed-caller rejection and safe sink behavior remain unmeasured. Native valid-decision retry remains independently red; this audit neither changes that interface nor claims code-complete acceptance.
