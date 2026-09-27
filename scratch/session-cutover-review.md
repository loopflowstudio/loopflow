# Session cutover review

2026-09-26 · LOO-298 · Reviewed `ca1be1116d704a4d908d473c2434ef59fd23261f`
and the supplied working notes. Preservation checkpoint: `cd7215b9a`.

## Disposition

**The required Session cutover fails.** The attempted implementation withdrew
all executable edits. Rust and Swift are byte-identical to `eeb609881`; the
complete current-slice executable diff is empty. The Task attempt foundation
retains its preceding proof, but this attempt makes no executable progress
toward Jack Heart's `50cf5b12` requirement. No publication is warranted.

The [complete design](data-model-one-table-per.md), [amended approval](data-model-review-feedback.md),
[Session ownership](session-runs-and-current-run.md), and [dated decisions](questions.md)
govern. Session owns Runs and a current Run; Invocation Task is nullable;
bind fills missing ancestry once; `started_at` records first assignment time.
The original Task title and earlier Wave-memory model are superseded.

## Actual CLI counterexample

Built this checkout's CLI with `cargo build -p loopflow --bin lf` using the
existing isolated, four-worker, nice +10, 900-second runner. Build passed in
27.2 seconds. Resource preflight passed: 92.8 GiB free, this checkout 4.0/12 GiB.

The probe created a private Home under `.lf/tmp/loo298-cutover-review/`, cleared
all inherited `LF_*` and `LOOPFLOW_*` variables, and set explicit private Home,
database and executable paths. It seeded one synthetic interactive manifest
and provider-history reference, with no process receipt or real provider.

| CLI operation | Observed result |
| --- | --- |
| `session list --all --json` on the empty Home | Empty list; schema initialized successfully |
| Same command after seeding the files | One interactive Session with its Run ID as Session ID |
| `session rename <id> 'Cutover review' --json` | Returned the requested human title |
| `session list --all --json` again | Returned the new title |
| Direct read-only SQL inspection afterward | `sessions = 0`, `runs = 0` |
| Artifact inspection | `session-name.json` contains the new title; manifest bytes unchanged |

Every command exited zero. This proves that public inventory, lookup and rename
still operate without a Session or Run row. Successful commands establish the
old behavior, **not** acceptance of the new ownership contract. This is actual
CLI execution over synthetic local evidence, not configured-provider or
installed-app proof. No provider was launched or stopped; no real Home changed.

Reproduction: `uv run python .lf/tmp/loo298-cutover-review/probe.py`.
The script retains command stdout/stderr and a receipt with the private Home,
Run ID and binary digest. Current receipt: `receipt.json`; binary SHA-256:
`b60151c61439bd8b2391991549512624f39d67152a5212f47a2ba42492708d18`.
Build log: `.lf/tmp/loo298-attempts/review-current-cli-build.log`.

## Complete-design evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| 1. Ancestry and attempts | Shared nullable ancestry, retained attempts, stale-action rejection | Task constructor and attempt fences exist; all-kind and runtime child paths remain unfinished | Retained matrix/replacement proofs; current source comparison | Partial, unchanged |
| 2. One CLI reader and confirmed bind | List/lookup/rename/bind read and update SQL; all readers agree | Interactive list/rename require no rows; general bind absent | Actual CLI counterexample above; Session command dispatch | **Fail** |
| 3. Execution preservation | Common Task/taskless invocation driver; feedback and keyed answers retained | Task reviews use SQL; Ask/taskless Flow still own files and reset feedback on replacement | `human_session.rs:1199`, `flow_session.rs:277`; retained Task controller proof | Gap |
| 4. Repository Chapter | One repository clock, every Wave advanced together | Existing per-Wave `wave_chapters` remains | Chapter source and preceding 32/33 owner inventory | Gap |
| 5. Populated import | All conversation kinds, captures, unknown membership and repeated attempts preserved before activation | SQL drafts preserve selected inputs; all-kind filesystem importer absent | Migration/source inspection; preceding canonical SQL proof | Gap |
| 6. DTO and desktop | Typed current-Run parents, attempts, cached grouping and retained panes | Existing Work/path fields and roadmap-based grouping remain | `SessionRecord.swift:134`, `WorkspaceProjection.swift:43` | Gap |
| 7. Configured acceptance | Backed-up real Homes converted and read through matching CLI/app | Private synthetic Home only | Probe isolation and receipt | Gap |
| 8. Deletion and consistency | Replaced authority gone; implemented docs match | Four-source list, file dispatch, title sidecar/copy and derived ancestry remain | Reachability audit below; actual CLI | **Fail** |

No new human choice or external service dependency prevents the selected local
implementation. Resource availability is no longer a blocker. A full cutover
is a material change, beyond a bounded repair in this review.

## Reachable authority that still needs replacement

| Path | Observation and exact replacement dependency |
| --- | --- |
| `ops/human_session.rs:575` | `list` concatenates Task SQL, Ask files, taskless Flow files and interactive manifests. Convert their writers and import history before replacing it with the joined Session query. |
| `ops/human_session.rs:589` | `find_session` dispatches SQL, taskless ID parsing, boundary scans, manifests/prefixes and Ask records. One Session/Run lookup must retain explicit historical-attempt rejection. |
| `lf/commands/run.rs:762` | General capture takes the SQL reservation branch only for a Task review. Interactive and general Run reservation must enter the common constructor. |
| `ops/human_session.rs:2263,2285` | Ask file read/write owns requests, retry/completion and current Run. Move these facts to Session transactions, including completed keyed answers. |
| `ops/flow_run.rs:107,122,140` | `read`, `write`, `update` persist capture/cursor and boundary to `position.json`. Move taskless execution to Invocation with exact attempt and process fences. |
| `ops/human_session.rs:1208,1276`; `ops/flow_session.rs:301` | Ask and taskless replacement call `carry_session_name`; both reset saved feedback. Preserve Session fields while appending Runs, then delete copying/reset paths. |
| `run_record.rs:1056,1079`; `human_session.rs:1441,1453` | Title sidecar reader/writer and derived ancestry remain live. Convert all writers/import and coordinated Rust/Swift DTOs before deleting them. |
| `store/sqlite/sessions.rs:147` | Existing inventory only selects open Sessions referenced by current invocations. It cannot substitute for interactive, completed Ask or full historical inventory. |

This audit follows public command dispatch through the live owners; matches in
historical migration fixtures are not counted as runtime readers. Keep exact
native-process exclusion, immutable publication evidence, current pointers and
invocation/version/worker fences. They protect distinct facts. Deleting file
readers now, without their dependent conversion, would hide conversations.

## Retained proof and measurement

Compared all 2,035 entries in the retained `final-source-hashes.json`: only
`store/sqlite/durable.rs`, `store/sqlite/runs.rs`, and the design/questions notes
differ, exactly as in the preceding review. The two Rust changes are its shared
cursor-decoder repair and regression. `git diff eeb609881 -- rust swift` is empty.
The comparison and metrics are saved in
`.lf/tmp/loo298-cutover-review/source-comparison.json`.

Inspected retained logs: the historical replacement regression first fails with
`missing field index`; the final durable suite passes 20 tests; the Task
controller suite passes 29 before that decoder repair; canonical repetition of
the regression passes once. These remain 49 distinct source tests plus one
canonical repeat, with the limits in [the preceding review](data-model-slice-review.md).
No behavioral suite was repeated on unchanged executable bytes. Prior static
and migration results retain their original scope; this is not a new full gate.

The retained non-test-prefix measurement against `4cd64be3d` still reports
**+1,617 / −556**, 18 production Rust/Swift files, zero deleted files.
`human_session.rs` has **2,350 lines before its trailing test attribute**, 2,351
including it. This review and the withdrawn attempt add/remove **zero executable
lines**. The method excludes test files and terminal test modules, retains inline
test helpers, and does not replace Jack's whole-file count. Being below the line
ceiling does not establish deletion of authority.

Bounded documentation repair: corrected the ledger's claim that `ca1be1116d`
preserved the concept review. `021d24f7b` preserved it; `ca1be1116d` deleted it
and the slice review. Compression restored both, and this review checkpointed
the supplied notes before editing. The concept/compression notes remain intact.

## Required next implementation and proof

Finish Session list/lookup end to end in one implementation pass: generalize
the existing Session/Run transactions; convert interactive capture, Ask and
taskless Invocation writers; import their history; switch readers and actions;
delete the replaced authority and obsolete representation tests. Another
owner-only foundation or a list-only SQL switch does not meet Jack's direction.

Start with a populated isolated Home containing interactive, completed keyed
Ask, Task review and taskless pending review. Prove actual CLI inventory and
rename through SQL, keyed retry without a provider launch, source-independent
taskless recovery, two Run replacements retaining identity/title/feedback and
ordered history, stale-action rejection and feedback consumed once. Pair with
failed-then-successful headless attempts. Prove confirmed write-once bind races,
done/landed targets, nullable invocation equality, set-once Started and populated
idempotent/interrupted import on materialized schema before activation.

The complete design still requires runtime loop children, attempts/usage DTOs,
repository Chapters, desktop retention, real-Home acceptance and comparable
measurements. Retain the first-Home `run_events` race, `wave_chapters` inventory
gap, historical whitespace and supplied publication/stacking/cancellation reports
as unresolved. None was repaired by this review. This note chooses no Flow edge,
does not publish, and does not complete the Task.
