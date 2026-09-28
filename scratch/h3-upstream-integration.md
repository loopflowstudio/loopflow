# H3 integration onto main

2026-09-27 · LOO-298 · Bounded local integration requested by Jack Heart.
Separate review-slice follows; this note chooses no Flow edge.

## History and scope

Starting H3: `f252e1c06a10e9e9e50e27d362f688674e3eaf3a`.
Target main: `5bdcef6b65419d5db2583ee791cede2f1a95b3df` (v0.12.23).
`lf rebase --manual` and `lf rebase --continue` replayed the 69 authored
commits locally. Mechanical rebase head: `5f3b610ef3ac2f997f946f684cdee4df6b9fd5e4`.
No other writer, provider, publication, installation, or installed-Home source
command was used. The final local checkpoint adds the semantic repairs below.

Main brought Task deletion/command consolidation (#1302), withdrawn release-PR
retry and bounded Swift verification (#1305), and the v0.12.23 release (#1306).
The released SQL remains byte-identical to main; `task_agent` is now released,
leaving 13 branch drafts. No draft was rewritten or invented for integration.

## Reconciliation

- Retained H3's `FlowInvocation`, common executor, SQL current attempt, strict
  Run ancestry and claim-time Run reservation. Task restart carries main's
  expected-observation comparison through the invocation transaction. Stop
  still requires exact process evidence; claim release uses the common API.
  Main's removed Work/Task reopen APIs stay removed.
- Retained main's narrow PM writeback updates and native deletion bookkeeping.
  Chapter confirmation now queries the Task's selected `flow_invocations` row;
  an automatic merge had left a runtime query against `task_flow_positions`.
  Historical Started retirement evidence remains; current Started remains the
  first-assignment column. This is existing Chapter integration, not H7.
- Retained main's provider-client launch/stop lock, identity checks, exit
  confirmation and refusal on unreadable manifests. Interactive completion
  persists on `sessions`; no resolution sidecar or manifest inventory returns.
  Native launch admission reads the Run's typed Task. Ask inheritance checks
  deletion using that same recorded Task. Historical attribution stays intact.
- Converted upstream restart/checkout/deleted-Task tests to H3's reserved Run
  and invocation APIs. Three native-launch fixtures now select their private
  Home explicitly, since admission reads a row even for unbound Runs.
- Reapplied consolidated Task/Wave/repository command names, deletion semantics,
  branch identity and new SQL table owners in the overlapping docs. The existing
  docs still contain pre-H7 Chapter specification; accepted `chapters.md` governs
  that later implementation. No chapter schema or listener deletion was added.

Conflicts occurred in `docs/{architecture-reference,lf,waves}.md`,
`ops/{human_session,run,task}.rs`, `run_record.rs`, `lf/commands/util.rs`,
`store/children.rs`, and `store/sqlite/{durable,children,chapters}.rs`.
Ignored hunk snapshots and scripts are under `.lf/tmp/h3-rebase/`.

## Review and proof

The simulated source review found two non-textual seams: main's Chapter query
still named a removed table, and Task restart could otherwise lose its new
stop-to-restart fence during the FlowPosition removal. Both are repaired. The
main deletion regression now checks SQL attribution while rejecting new native
resume/Ask admission; no historical resolver was restored to make it pass.

Initial Clippy runs exposed removed-API calls in automatically merged tests and
the native admission path. Those failures are retained, not passing evidence.
Verification runs scrub inherited `LF_*` and `LOOPFLOW_*`, use private fixtures,
nice +10, four Cargo workers, and a 900-second phase limit. Resource preflight
passed with 71.4 GiB free against the 64 GiB floor.

- First focused library selection: **23 passed**, including five installed
  development frontier tests, populated H3 claim preservation, five worker-stop
  cases, native stop/identity tests, stale restart/review checks and deleted-Task
  admission with retained attribution.
- Consumer selection: **28 library + 3 Flow CLI + 2 Session CLI tests passed**.
  The library filters also selected related completion/boundary tests beyond
  the intended Chapter case; this remains a filtered run, not an affected-suite
  gate. Real CLI proofs cover main/parent upstream identity, historical Started
  retirement, H3 claim reservation/worker publication and managed resume policy,
  plus Session open/rename/replacement. Providers are stand-ins.
- Native launch selection: **4 passed**, covering intentional move, unrequested
  SIGTERM, OpenCode history publication and name forwarding on resume. Their
  launches use private Homes after row-based admission.
- Canonical repeat: **7 passed** with all 13 drafts materialized into the
  disposable 0.12.24 batch: five installed-development frontier cases, populated
  H3 claim/current-attempt preservation, and the reconciled Chapter consumer.
  Authoring migrations and package version remain unchanged.
- Formatting and lint: `cargo fmt --all --check` and
  `cargo clippy --all-targets -j 4 -- -D warnings` passed.
- Whitespace: `git diff --check` passed.
- Migration history checker: **54 released migrations unchanged**, 13 drafts.
- Portable architecture and README/index sync: **2 passed**. Regenerating the
  portable architecture produced no change.
- Architecture inventory: seven inventories pass; **34/35 SQLite owners**,
  missing `wave_chapters` as before. H7 owns that known gap. The first invocation
  also scanned an old ignored canonical source copy and reported its historical
  vocabulary; moving both disposable snapshots outside the source tree leaves
  only the known owner gap. No checker exemption was added.

Logs are `.lf/tmp/h3-codex-recovery/rebase-*.log`. The canonical source copy is
outside the checkout; `.lf/tmp/h3-rebase/canonical-root.txt` records its location.
It models a hypothetical 0.12.24 batch; it is not a released or installed build.

No configured provider, real Linear deletion, installed migration, app rendering,
full gate or hosted CI is proved. H4–H7, runtime children and Cut I remain outside
this Run. No new product decision was required.
