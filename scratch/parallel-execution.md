# Main implementation handoff

LOO-298 · Jack Heart · Updated 2026-09-29 after the metadata checkpoint.

## Finish and ownership

Jack now requires complete implementation → concept review with him → resolved
findings → saved delivery workflow, required gates/CI repair and verified merge of
PR #1296. This supersedes the former final stop at code-complete review; it does
not bypass review. Rebase/publish coherent checkpoints regularly through `lf`.
No branch promotion or installed-Home migration before landing. No new native
retry-policy decision follows from delivery authorization.

Main owns executable edits, builds/tests, Git and this handoff. Supervisor owns
[control](parallel-work.md); follow its live contribution/release records. The
2026-09-29 curation explicitly authorizes main to compact evidence.md too. Keep
one executable writer. The active Session-view proposal under
`.lf/tmp/active-session-proposal/` is not released: do not integrate unfinished
files or the older test-only active-observation characterization patch.

Read these complete, unchanged acceptance owners before choosing another cut:
[accepted model](data-model-one-table-per.md), [remaining work](remaining-work.md),
[import preservation](import-preservation.md), [Chapters](chapters.md),
[native tradeoff](native-turn-retry-tradeoff.md), [questions](questions.md).
Older drafts are evidence, not approval. Their full acceptance matrix remains
binding even when a slice below passes. [Evidence](evidence.md) indexes original
red results, replacements and limits.

## Current boundary and publication

Same captured `feature` invocation `1f9ba70e-f0e9-41d4-9722-948d7bc4ce8c`,
implement iteration 10. Do not restart it. Current worker
`run_65fc64d8389d49ea9fdbeb47efdc9c40` continued after the prior worker's compaction
capacity failure. GitHub publication of `e6aa8d4c775c5c02bccd63ca80d6b616a3b60077`
is verified; PR #1296 remains open. The checkpoint rebased without conflicts onto
`a3820bf7e493b7d533a45677b7945020adf80721` (v0.12.25 release). No behavioral rerun
was required solely for that conflict-free rebase. The checks below describe
pre-rebase implementation bytes, not a final integrated gate.

Use the captured installed control pair through:
`uv run python .lf/tmp/cut-i/control-checkpoint.py ...`.
Bare `lf` can select a different store. This wrapper is for control/checkpoint/
publication, never for pointing a branch executable at installed data. Read
Task status and Session inventory through it; do not infer shared state from
worktrees/processes. Use ordinary source inspection for code.

## Next work

1. Finish the public history/discovery consumers and their deletion paths. The
   metadata cut removes captured/history decoding from passive Session inventory;
   FlowSession inventory, final history/active wire, Desktop paging and measured
   startup/query/payload partition remain open. Current Desktop inventory remains
   complete: passing individual pages into reconciliation would drop retained
   panes. Change paging and reconciliation together.
2. Integrate the Session-based active-view proposal only after Supervisor releases
   its terminal handback. Stable AgentSession plus exact driver Exec replaces
   obsolete per-input ActiveRun. Retain native client→Session links, exact PID/start
   evidence, uncertain drivers, sequential Flow steps in one Exec, shared engines,
   last-good/watch behavior and database-outside-Home support. Add no driver-input
   owner just to preserve retired wire. Main owns all behavioral proof.
3. Resolve Codex valid decision retry through Jack's existing decision conversation.
   The stale child is correctly rejected but the legitimate retry still receives
   old tool environment after a failed native turn. Keep independent work moving;
   do not silently change the decision interface or add provider machinery.
4. Complete preserved import/attribution and populated final-schema obligations,
   all-provider common connect/restart/recovery, public Chapter/default-Flow
   preservation, incident dispositions and integrated verification from the full
   matrix. Table deletion, local fixture passes and publication are not completion.
5. Bring the whole implementation and usage/ownership/deletion review to Jack.
   Resolve findings before gates and landing through the saved Flow. Configured
   provider/Desktop, backed-up real-Home conversion and release activation keep
   separate evidence/procedure requirements; local fixtures cannot discharge them.

## Current implementation and its limits

- AgentSession owns conversation admission/publication, immutable inputs and
  subordinate provider outcome/usage history. Ordinary and managed launches,
  reviews and Asks create no runtime Run rows. The Run table/CRUD were removed
  after retaining original SQL and unresolved inputs. Remaining Run names/wire,
  native evidence files and consumers still need final disposition.
- Exec is the actual `lf` process with causal parentage and observed terminal
  outcome. Async/blocking command ancestry is preserved. Command success, native
  turn completion and Flow settlement remain different facts. Observational Execs
  do not start Tasks. No synthetic process is imported without process evidence.
- FlowSession owns captured progression, claim/version and exact successful native
  history references. Mechanical boundaries retain independent start/outcome
  events, including success before a later operation fails in the same Exec.
  Task/taskless share the driver; numeric graph identity agrees across Rust/Swift.
- Bind is write-once null→Task, same-target idempotent, permits done Tasks and sets
  Started once. Prior usage retains attribution; prospective allocation is the
  recorded conservative assumption, not a new Jack decision. Unknown mid-turn
  allocation stays unknown. Neither driver replacement nor chapter movement
  rewrites historical owners.
- OpenCode native request/history integration passes scripted public automatic
  retry and real OpenCode/scripted-model final-output proofs. The latter's injected
  failure retries inside OpenCode, not Loopflow. Codex ordinary automatic retry
  passes zero-Run history/usage and exact consumption; valid decision retry does
  not. Configured accounts/all-provider recovery remain separate.
- Desktop reveals headless conversations, previews/confirms permanent binding and
  retains a Session's own workspace, surfaces, draft and companion. `runs --resume`
  is deleted; connect/open share the selected conversation path. Whole common-
  connect acceptance and configured Desktop acceptance remain open.
- Chapters use Linear Project statuses, preserve transferred Task identity/Started/
  worktree/PR/capture, and use Project Flow defaults. Scripted public Linux rotation
  and second-Home sync pass. No live Linear rotation or distributed no-work lock
  is claimed; unknown evidence never authorizes backlog cancellation.

## Proof discipline

Read logs before launching duplicate builds. Source checks use
`uv run python .lf/tmp/cut-i/run.py NAME COMMAND...`: scrub inherited LF_/LOOPFLOW_
authority, unrelated private Home/database, nice +10, four build jobs, saved full
log and bounded process cleanup. Run resource preflight and serialize builds.
Never touch real/shared provider engines to diagnose fixtures. The metadata
preflight had 52.3 GiB free: below cleanup target, above the emergency reserve.

One isolated materialized no-fail-fast matrix already ran (1,925 pass / 33 fail /
15 skip). Its later focused repairs do not create one green matrix. Preserve all
failure dispositions; run focused changed behavior now and final affected/gate
proof at its proper boundary. Do not repeat full matrices per small repair.
All-target Clippy and formatting precede Rust commits. Rebase without conflicts
needs no new behavioral proof. Leaked handles never count as clean settlement.

## Passive Session metadata and CI continuation (2026-09-29)

The retained metadata implementation now lists Sessions through a bounded SQL
projection before local client observation. Captured Flow graphs, request bodies,
transcripts and history payloads remain exact-detail/action evidence. Maintained
expression indexes project Flow names and recorded independent membership; a
pending-Session/state index removes the correlated review scan without changing
historical review selection. Unknown observation/occurrence travels through the
Rust/Swift wire. Prepared Asks stay independent; preparation retains inherited
attribution when a consumed input is replaced and launched again.

Review removed the unused public Flow-summary entry point supplied by the proposal;
Session inventory uses its shared internal projection directly. Metadata is not a
new durable owner. Compared with d4a66125c, production Rust/Swift changes are
+394/-10 (net +384), excluding test modules, SQL, docs/generated files and scratch;
`.lf/tmp/cut-i/metadata-source.json` records method and hashes. This is a reader
improvement, not a net reduction. Flow inventory, Desktop pagination and complete
CLI startup/query/payload measurements remain open.

The prior worker's five metadata unit checks and 68 Swift tests completed. Its
public/density logs remained red; the final attribution edit postdated those
binaries. The rebuilt Ask case passes in metadata-resume-ask.log. The subsequent
metadata-final-rust.log records 26 selected passes, including public Ask/taskless
review, metadata, DTO and seven native-client cases, with one Nextest leaked-handle
report on native-client publication. The separate final-projection run passes both
the reduced metadata consumer and publication case with no leak report. This does
not explain the earlier intermittent handle observation; no live owned test process
was found afterward. Both logs are retained.

NativeClient now selects a private Home/database before capture and restores its
ambient authority on drop. The test runner supplies an unrelated empty ambient
Home, covering the hosted missing-development-database case without changing
production ownership resolution. The mounted Monitor proof now waits for the
required terminal/window attachment with a bounded deadline after splitting;
its old fixed pause did not establish mount completion. All surface, draft,
companion, focus and history assertions remain. metadata-monitor-mount.log passes
eight tests across both Monitor suites and both chrome sizes. Local passes do not
establish a hosted timing diagnosis or configured Desktop acceptance.

Bundled SQLite 3.53.2 measured the 20,000-Session/5,000-Flow review fixture at
0.38ms first page, 1.41ms offset 4500 and 4.09ms empty offset 5100; the absent-title
query took 2.06 ms. Removing only the pending-review index measured 0.57 / 267.82 / 2181.93 ms
and 2.19 ms with identical returned IDs. EXPLAIN uses the covering pending-review
and membership indexes and no JSON function opcode for summary selection. These
are warm synthetic SQL measurements, not cold CLI or installed latency.

All-target Clippy passes (16.96s), formatting, migration history, architecture and
whitespace checks pass; website docs were regenerated. Public Session suites ran all 34 cases without fail-fast:33 passed and the
checkout-attribution fixture failed. Its helper preferred a two-value historical
manifest source over the unbound Session's recorded four-value source. The fixture
now uses unchanged admission ancestry/source before bind, while preserving earlier
manifest attribution after assignment. Four focused checkout/declared/Ask/bind/
continuation cases pass in metadata-public-attribution.log; production attribution
was unchanged for that correction. No full-suite rerun was needed. Full scope, native decision-policy question,
configured-provider proofs and populated final import obligations remain unchanged.
No landing, promotion, installed-Home access or Flow navigation is selected.

## Preserved chronology

The full pre-curation handoff is [committed at e6aa8d4c7](https://github.com/loopflowstudio/loopflow/blob/e6aa8d4c775c5c02bccd63ca80d6b616a3b60077/scratch/parallel-execution.md). Its exact private copy is `.lf/tmp/scratch-curation-20260929-metadata/parallel-execution.md`
(133,112 bytes; SHA-256
`026bdf2d4a95689c8ec622a09c06e2b8b12a4b4f4a81b944d2c263b7c46b9549`).
`manifest.json` beside it records the commit, section line index and exact proof
references. No observation, red result or acceptance document was deleted. This
curation executes no new behavioral proof and makes no new delivery judgment.
