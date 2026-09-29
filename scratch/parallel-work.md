# LOO-298 control index

2026-09-29 · Jack Heart’s newly set goal is to **finish LOO-298 and land PR1296
through the saved Loopflow delivery workflow**, Codex only. Complete implementation
must first reach concept review with Jack, then resolve findings, pass required
gates and repair CI before verified merge. Useful parallel contributions and
regular rebase/publication are authorized. This supersedes the prior final stop
at concept review without bypassing it. No branch promotion or installed-Home
migration before landing; branch-binary access to that Home remains forbidden.
The goal supplies no new Codex retry-policy decision.

## Read order and finish line

1. [Accepted model](data-model-one-table-per.md): Exec, AgentSession, FlowSession.
2. [Main handoff](parallel-execution.md): implementation and next proof.
3. [Full remaining scope](remaining-work.md): no slice substitutes for completion.
4. [Import preservation](import-preservation.md) and [Chapters](chapters.md).
5. [Evidence](evidence.md) and [open assumptions](questions.md).

The complete finish includes native continuity/authority, final history/wire and
reader removal, bounded discovery/paging, populated import and attribution,
Desktop preservation, Chapters/default Flow, incident dispositions and integrated
checks. Configured provider/Desktop and real-Home/release acceptance require their
own evidence/procedure. A published checkpoint or focused pass completes none of
those broader obligations. No review edge has been selected.

## Live ownership

Main Run `run_65fc64d8389d49ea9fdbeb47efdc9c40` owns executable edits,
builds/tests, Git, cleanup and its handoff. Same saved feature invocation
`1f9ba70e-f0e9-41d4-9722-948d7bc4ce8c`, implement iteration10. Shared Task
status reports running after the continuation recovery below. Recheck liveness.

Prior Run `run_344d4bfe1bf040b99a43e65434efbf97` failed during Codex remote
compaction at17:13:58UTC (model capacity); claim released. First continuation
`run_7f332bcc507c4bd2aaf27560f7ab8c84` failed before execution: task prompt
1,099,291 characters exceeded1,048,576. Supervisor archived three integrated
proposal originals with SHA manifest under `.lf/tmp/scratch-curation-20260929-continuation/`
and replaced their automatic-context copies with pointers, removing74,096bytes.
No evidence or scope was discarded. Same-boundary `lf task run` then launched
current Run; no restart or Flow navigation. Continuation direction
`ce409314-a34f-46e1-89b9-ab3d0e1881e6` was verified before launch.

Local HEAD d4a66125c commits import repair (seven focused cases). The final
committed cut is +34/−79, net−45production; the earlier −50 description referred
to intermediate edits. Whole local committed delta +17,874/−30,666=net−12,792
againstbase6e718; working changes excluded. Counts receipt:
`.lf/tmp/execution-model/status-counts-d4a66125c.json`.
Dirty metadata integration and NativeClient fixture repair remain main's work.
68 Swift tests/9suites completed successfully in metadata-swift.log. Later
metadata-final-rust.log passes26selected cases (one leaky; cleanup unresolved),
including Ask provenance; metadata-monitor-mount.log passes8tests/3suites with
bounded actual terminal-window attachment and original retention assertions.
Clippy completed successfully. BundledSQLite density passed: empty deep page
2181.925ms→4.087ms, identical returned IDs. SQL timings are not CLI/Desktop
latency proof. Earlier red logs remain retained. Supervisor log/source hashes:
`.lf/tmp/cut-i/metadata-supervisor-review.json`.

Supervisor owns this index and isolated reviews, with no competing executable
writer or build. Control Session `run_9c16dbbe2b04440db8469e9b4964912c` retains
its human title loopflow.

**Active private contributor:** tool14717, Run
`run_5a749c04bcb64b7195089b13a274857d`, Exec
`ceda711b-6a08-4188-9aac-bb79db8fbaae`, PID97123. Assignment acknowledged;
writes ONLY `.lf/tmp/active-session-proposal/`. Complete proposed Session-based
active-view/DTO conversion and interval-file deletion, no source/build/provider/
Home/PM/Git/process mutations or publication hold. Main owns integration after
reviewed terminal handback. Coordination `0bc2fe41-de49-4f11-9e83-f8264ef7b6c0`
was read back.

Prior tool72315/Run b527d5e completed and exited0. Its handback/SQL-only proof
under `.lf/tmp/active-observation-proposal/` establishes missing driver→input
correlation for the old per-input ActiveRun projection. Supervisor's
`supervisor-session-projection.json` shows stable Session/driver identity and
historical client-input→Session links survive replacement. Final Session identity
can remove that dependency; do not add a new driver-input owner merely to preserve
obsolete wire. This is a proposed reduction, not active-reader acceptance. No
production deletion delivered and its test-only characterization patch unapplied.

## Publication and checks

Published **636abc941081d2e70b5068dc97ab517cceec13a1** includes the alternate
runs --resume deletion and command/Desktop fixture repair. GitHub PR #1296 and
Task publication agree. Base and GitHub main were both
`6e7189926ede81b3edd05c8d81955c6cbe67a4e2` at this check; recheck before delivery.
Hosted CI 36601948200 is complete and red. Rust: 840 passed, one failed,
15 skipped, 1,141 unrun; session_stop_waits_for_native_client_publication opens
an absent ambient development DB during its final resume. Swift: 286 tests, one
issue at TaskMonitorTests.swift:144, terminal window identity. Both earlier chrome
failures are gone; separate UI job passes. Architecture, migrations, lint,
installation, Python, website and smoke pass. Scratch remains retained.
Supervisor direction `5c7cf8af-7482-4437-b629-e10fdd2390b2` was read back;
main owns both repairs. Complete bounded logs/direction live under
`.lf/tmp/cut-i/ci-636abc941-*`. **No full-green CI claim.**

The terminal predecessor CI 36600147964 at 59ed94ea5 failed:
- Rust 109515111201: 660 passed, 2 failed, 15 skipped, 1,321 unrun; 37.122 s.
  Generic launch and replay still used fake OpenCode one-shot executables after
  the adapter moved to serve HTTP/SSE. Main audited adjacent fixtures; generic
  launch/research use the existing Claude fixture protocol, replay retains the
  account-free native OpenCode fixture. No old production adapter restored.
- Swift 109515111628: 286 tests, five issues, 148.733 s. SessionChrome expected
  the former Task-only worktree toolbar at both widths. Monitor started Chapter
  transfer before its first asynchronous activity frame. Assertions now cover
  Session-owned and Task-only chrome; Monitor waits for attributed initial data
  and keeps retained history/terminal/draft/companion assertions.
- Other jobs passed except scratch-clear and aggregate. Full Rust/Swift logs
  are `.lf/tmp/cut-i/ci-59ed94ea5-*-clean.log`. Swift contains huge inspector
  values: use `ci-59ed94ea5-swift-summary.json`, never dump raw lines into prompts.

Reviewed local proof: `desktop-ci-proof-final.log` passes two mounted tests in
4.683 s, including both chrome sizes; `command-provider-replay-native.log`
passes replay in 1.105 s; `command-provider-native-shared.log` passes taskless
native decision and automatic retry/earlier-caller rejection in 7.536 s.
Earlier no-fail-fast command fixture runs were mixed; retain their logs. Main's
final static/repair receipt is in its handoff, not inferred from these passes.
Initial Desktop integration had five Rust checks, 67 Swift model/navigation
checks, mounted PTY retention and the Xcode fallback build passing. These are
fixture/source proofs, not installed acceptance.

## Released metadata proposal and concrete integration concern

Tool 27771 exited 0; Run `run_612a5d2f078842c8a2921ef240211391` is completed,
and Exec 82595041 is absent. [Handback](proposal-metadata-discovery.md), SHA
`1b44d1173d41f08632ba8c341591715aa23215eaa268393c5d112602ff68a4d8`, is released.
Private patch `.lf/tmp/metadata-discovery-proposal/metadata-discovery.patch`, SHA
`62431e5a89e3d5744e9feb586de3e9f0bd7415b6dafaddf5b29b2793ff303ff5`.
Supervisor verified 16 original/proposed files and read-only applicability at
7dcdee5ef. Release comment `cccf3c60-a479-4d69-9744-e50751590bc8` was read back.

The proposal removes capture/history decoding from passive inventory, uses
SQLite-maintained expression indexes for Flow name and known input membership,
and retains exact-action readers. Unknown state/occurrence moves with Rust/Swift
and fixtures. **Unapplied, uncompiled; +405/−6 production** by contributor method.
Six SQL-expression checks, syntax and hunk replay pass; no application proof.
Do not apply the superseded private Session-cache proposal alongside it.

Its pending-review predicate still scans Flow metadata before pagination.
Supervisor measured 20,003 Sessions / 5,001 Flows on Python SQLite 3.50.4:
first 100 rows 0.51–0.61 ms; offset 4,500 273.43–274.37 ms; offset 5,100 (empty)
2,221.51–2,260.62 ms. A private pending_session_id/state index yields 0.37–0.41,
1.27–1.33 and 3.09–3.43 ms respectively, with identical result-ID hashes.
`.lf/tmp/metadata-discovery-density-review/results.json` and `reproduce.py`
retain SQL, plans, scope and equivalent fixture. Three warm in-memory samples;
not bundled SQLite, CLI, cold-cache or installed evidence. No source edit.
Verified direction `4eb99cc9-e18f-4029-9e28-a707b32e2f5d`: use the simplest
justified existing-key lookup/index, preserving the row set; no new cache.

Final Flow inventory, Session/Exec history wire, Desktop paging, dense CLI
startup/query/payload partition and configured acceptance remain owed. Exec
list/show is already published with bounded SQL, strict DTOs and current-repo
name resolution. Its previous 100,001-Exec dense probe measured whole CLI
p50 314–339 ms/p95 320–341 ms; uncontrolled cache and no cost partition do not
establish the remaining targets. See `.lf/tmp/exec-discovery-measure/results.json`.

## Import preservation

Supervisor source review found old tui manifests without native identity and
earlier Flow-review manifests outside the active boundary falling into headless(),
which forces interactive=false and completed_at=None and omits resolution
history. Existing earlier-member fixtures were headless or still active review
records. This was source evidence, not an executed or installed counterexample.
Verified direction `733cc662-ee78-41d0-8e65-8eb95950b322` asks for two public CLI
red/green cases, original identity/membership, explicit closure without invented
native success/Exec, repeated import and removal of retired files. Source hash
and exact finding: `.lf/tmp/import-surface-review/`. Main owns the current repair.

Historical NULL-title/current-review rejection is a real SQL fixture result,
not an observed installed-Home defect. The released-writer audit through
v0.12.24 found no supported creator of that combination: Task insertion precedes
the title-omitting helper, and review writers require a String title. Retain
original red and corrected populated fixture separately; no runtime title
fallback or applied-draft rewrite. Private audit:
`.lf/tmp/historical-title-review/{title-writer-audit.json,classification.md}`.
A concrete imported counterexample reopens it.

Populated SQL/canonical proofs retain their limits in import-preservation.md.
Released positions and branch development Session members are distinct upgrade
frontiers. Unmapped evidence, filesystem/native/journal history, interrupted
conversion, SQL/terminal disagreement and public recovery still require proof.

## Native navigation remains unresolved

[Tradeoff](native-turn-retry-tradeoff.md), [options](research-native-retry-options.md)
and questions.md retain the decision already asked of Jack, unanswered: selected
successful-turn result versus in-turn CLI decision. Do not ask again merely
because context compacted. No alternative interface, provider fork, proxy or
retry-contract change is selected.

Actual Codex 0.157.1/scripted Responses/private Home reproduced a failed turn's
late child deciding after retry, wrongly advancing the Flow. Caller-token repair
rejects that child but also the legitimate retry because the failed loaded thread
retains the earlier tool environment. Unsubscribe/new client does not refresh it;
interrupt says no active turn. A successful-idle control refreshes correctly.
The latest fresh-engine probe also fails: -32600 active writer on thread/resume
while original engine and sibling remain. Supervisor read the result/log;
unsubscribe returned and both exact fixture engines exited 0 during cleanup.
It stops before sibling completion or valid handoff proof. Receipts:
`.lf/tmp/execution-model/native-rehost-failed/`. No production change follows.
Do not kill a shared engine or expand native machinery without a reviewed choice.
Independent implementation continues; this is not proof all recovery is impossible.

OpenCode's common native harness is published. Actual 1.18.33 with a private
scripted model completes the Flow and retains both final answers. Its injected
HTTP failure retries inside OpenCode, not Loopflow. Public simulated-server
checks cover same-input automatic retry, old-caller rejection, two consumed
successes, retained 40/10 usage and final text; tool-effect/disconnect retains
unknown completion. SQL recovery preserves original attribution and deduplicated
usage, but is not live driver reconnection. Configured accounts, interactive
connect and full managed recovery remain owed. Retained paths and failure/pass
chronology are in evidence.md and the archived control index.

## Control, context and retained receipts

Use only the pinned wrapper for current live control:
`uv run python .lf/tmp/cut-i/control-checkpoint.py …`.
It pins binary
`/Users/jack/.lf/bin/lf-f5ef8d640340e9f8b9e36d17d84de83e14e905305c43e959fd00c49a322a527f`
and Home `/Users/jack/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea`,
with executable/Home/database control and ordinary variables consistent. Bare lf
selects another Home. Refresh Session list each turn/after mutation, then shared
Task status/ps. No lf ask for Jack here, no unclaimed signaling, no restart on an
observation timeout. New contributions use --task LOO-298 and private artifacts.

The old worker failure classifier treated quoted `operation not permitted` text
as current capability denial. Source repair is published; pinned control predates
it. A later continuation hit Codex's 1,048,576-character input cap. Removing only
a duplicated inline patch allowed continuation; preserve artifacts and hashes,
never inline patches or huge fixture/event JSON. Current source proofs clear
ambient LF/LOOPFLOW authority and use disposable Homes. Promotion needs a
separate disposable OS account/container, not HOME alone.

Last measured published production delta, 636abc941 versus base6e718:
**+17,919 / −30,666 = net−12,747**. Same corrected production-prefix method,
no rename detection, tests/docs and working changes excluded. Receipt:
`.lf/tmp/execution-model/status-counts-636abc941.json`. OpenCode's cut was net+266;
Desktop controls net+221. Do not substitute whole-file test deletion for code.

Detailed prior index preserved before this curation:
`.lf/tmp/scratch-curation-20260929-control/parallel-work-93d8f1a19252.md` (SHA `93d8f1a192520564d35c0e31aca5324bca2147aaa2dc426984b89ece68f7ae73`).
Its committed version is `636abc941081d2e70b5068dc97ab517cceec13a1:scratch/parallel-work.md`.
Earlier scratch archive: 62 files / 676,767 bytes under
`.lf/tmp/scratch-consolidation-20260928/scratch/`, with SHA manifest and originals
retained in commit7e2101b41. This curation changes no main handoff, accepted scope,
Task state, review edge or evidence result. Useful conclusions still need durable
owners before delivery clears scratch.
