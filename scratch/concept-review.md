# One Session interaction across all conversation kinds

2026-09-26 · LOO-298 · Autonomous concept review of `eeb6098815088fcd853572f3467534d7aa89cc62`

## Judgment

The accepted concepts are sufficient. Jack should open, name and complete a
conversation without knowing whether a Task, a taskless Flow or Ask created it.
Session owns the conversation; Run owns each execution attempt. The current
implementation gives only Task reviews that ownership and recovery contract.
The next implementation must complete Session list/lookup and its dependent
writers/import, deleting the replaced paths in the same pass. Another foundation
without that public cutover does not satisfy Jack's latest direction.

This is review evidence, not a navigation decision. The
[slice review](data-model-slice-review.md) remains authoritative for executed
behavior and unresolved acceptance. The [amended approval](data-model-review-feedback.md),
[complete design](data-model-one-table-per.md) and [dated corrections](questions.md)
supersede the original Task title and copied Session-as-Run-child proposals.

## Usage first

Draft of the accepted end-state experience, **not commands verified on this
branch**:

```sh
lf --interactive : "Review the parser"
lf session rename <session-id> "Parser review"
lf session bind <session-id> --task INF-123 --json
lf session list --task INF-123 --json
lf runs --task INF-123 --json
lf session open <session-id>
```

Bind displays the exact permanent target and confirms once. JSON output is not
confirmation. The Session retains its ID, title, terminal and draft; its current
Run receives the previously missing Task and Wave. A landed or done Task remains
eligible without reopening it. Earlier Runs retain their own attribution.

For a review, Ready saves feedback and Complete returns it once to the waiting
Flow or Ask caller. Reopening retains that feedback and the Session's name.
Resume uses provider-native history when available. If replacement is justified
by exact execution evidence, it appends a Run to the same Session. An unresolved
launch stays visibly unresolved; absent native history alone does not authorize
another provider launch. A superseded Run cannot act on the replacement.

[docs/lf.md:584](../docs/lf.md#sessions) already explains the normal interaction
clearly; leave it unchanged during this review. At implementation, replace its
preceding prepared-Run paragraph (lines 570–576): listing must read imported or
reserved records, not tell the user to Open an old boundary to create its missing
record. An unpublished preparation retries the reserved Run; a published failed
attempt can become a new Run under the same Session only under the recovery
contract above. Reconcile that paragraph after all kinds use the same path.

Affected documentation owners remain `docs/lf.md` for usage,
`docs/architecture-reference.md` for invariants and
`docs/architecture/data.md` for implemented storage. Current target docs are
specification, not release evidence. No skill instruction needs a new command or
kind-specific workaround; existing Ready/Complete guidance remains valid.

## Model in one screen

| User action or fact | Type and owner | State-changing API responsibility |
| --- | --- | --- |
| Name, reopen, Ready, Complete | Session, stable ID, `sessions` | Rename/feedback/completion act on one conversation; replacement compares its expected current Run |
| Inspect one attempt, bind missing ancestry | Run, `RunId`, `runs` | Reserve, publish, settle and bind share typed parents; terminal and process evidence remain attached to the exact Run |
| Repeat a step after interruption | Position = invocation, node, iteration tuple; ordered Run attempts | Select one current attempt under the invocation fence; only successful current evidence advances |
| Execute a Flow with or without a Task | Invocation, captured graph/cursor/return counts, `flow_invocations` | Common execution persistence; optional Task, optional runtime parent; no template reload on resume |
| Choose work and read progress | Task through Project to Wave | Task implies Wave; first Run assignment sets `started_at` once; retirement retains its separate evidence checks |

Session kind still matters: interactive completion closes a conversation, a Flow
review returns feedback to execution, and Ask returns it to a caller. Kind does
not require a second identity scheme or persistence adapter. “Standalone Flow”
is the extra storage distinction to eliminate. A Taskless invocation remains a
real invocation, with the same recovery obligations and no synthetic Task.

Keep both current pointers: Session selects its conversational Run even after
closure; Invocation selects the attempt allowed to act at its current cursor.
They agree while that review is pending. Neither grants process ownership or
proves successful execution. No extra position object or generic lifecycle
framework is needed to explain these separate responsibilities.

## Findings from reachable source

| Finding | Source at reviewed HEAD | Consequence |
| --- | --- | --- |
| Public inventory still branches on storage | `ops/human_session.rs:575–637`: list concatenates four sources; lookup tries SQL, standalone tokens, boundary files and manifests | Replacing only the first list call would hide existing conversations. Convert/import all kinds before removing the union and dispatch |
| SQL Session APIs remain review-specific | `store/sqlite/sessions.rs:63–119,147–160`: reservation loads Task position/worktree; open inventory requires a current pending invocation. `session.rs:10–18` has no kind or Ask request/result | Generalize the existing owner for independent conversations and Ask; a renamed review query cannot serve all Sessions |
| Recovery still changes saved conversation state by kind | `ops/human_session.rs:1199–1213` clears Ask Run binding and feedback then copies title; `ops/flow_session.rs:277–306` does the same for taskless reviews. SQL replacement at `store/sqlite/sessions.rs:78–110` preserves Session fields | Move replacement through the same Session transaction. Preserve completed keyed Ask answers, title provenance and feedback; delete copying and reset paths |
| Taskless execution has a separate durable lifecycle | `ops/flow_run.rs:24–46,107–145`: Boundary carries feedback; FlowRun has capture/cursor and file read/update | Move capture and progress to Invocation, conversation facts to Session. Retain source-independent recovery and exact effect exclusion while removing `position.json` authority |
| Lookup already protects historical attempts, but only the SQL route | `ops/human_session.rs:589–652`; `store/sqlite/sessions.rs:134–145` | Preserve exact/prefix Run-to-Session ownership and stale-action rejection in the common lookup. A historical Run selector must not silently select the current actor |
| Wire ancestry still requires reconstruction | `ops/human_session.rs:1441–1470`; `swift/Loopflow/Models/SessionRecord.swift:134–166`; `swift/LoopflowMac/WorkspaceProjection.swift:43–75,107–115` | Change Rust, Swift and fixtures together. Read typed current-Run parents; a Task absent from today's roadmap must remain bound and reachable |
| Attempt history is stored without a public consumer | `store/sessions.rs:7–20`; callers of `position_runs` are store tests | Node/Session detail must expose attempts and selected Run; usage/duration aggregate all attempts while elapsed status uses the current one |

These are source observations, not newly executed reproductions. In particular,
the taskless recovery source checks prepared/native/client evidence but does not
apply Task review Open's terminal-outcome requirement (`human_session.rs:1241–1259`).
The common recovery proof must cover that difference before the old path goes.

Current → required experience: a Task review retains its feedback, an Ask or
taskless review can reset it on replacement → every Session retains its identity,
name and feedback; only its execution attempt changes. Following that interaction
through storage removes name sidecars, `carry_session_name`, four-source
enumeration, kind-specific ID decoding and derived Session Work/path helpers.
Native process exclusion remains necessary; a SQL current pointer cannot fence
an already-running external effect by itself.

## Challenge and proof boundary

The smallest counterexample to a list-only change is a Home containing one
interactive Session, a completed keyed Ask, and a taskless pending review with
its source removed. The existing SQL open-review query cannot represent that
inventory or completed Ask retry. Seed and import those alongside a Task review;
then exercise the actual isolated CLI over the SQL-only path.

The next implementation proof must cover:

1. Capture a taskless Flow, remove its template, recover the review and replace
   its Run twice with justified execution evidence. Keep Session ID, title,
   feedback and ordered history; reject old-attempt actions and consume feedback
   once. Pair with headless failed-then-successful attempts at the same position.
2. Retry a completed keyed Ask without creating another conversation or provider;
   list/lookup/rename interactive and both Flow review origins through one owner.
3. Confirm an interactive Session's bind to a done Task. Race replacement and
   competing targets against confirmation: preserve the losing Task timestamp,
   history, feedback and artifacts. Cover Wave and nullable invocation conflicts.
4. Preserve populated historical files/SQL and identity mappings on import,
   including unknown membership and repeated attempts. Prove idempotence,
   interruption recovery and canonical migration materialization before activation.
5. Search all reachable writers/readers/actions again and delete the replaced
   paths with their obsolete representation tests. Report comparable line counts;
   moving code between files does not demonstrate ownership removal.

The complete design's eight obligations remain. Runtime loop-child invocations,
repository-wide Chapters, full Run/usage/Started conversion, DTO and retained-pane
proof, backed-up real-Home cutover, comparable measurements and final deletion
research still govern acceptance. Keep the initialization race (`run_events`
missing during concurrent first-Home reads), `wave_chapters` inventory gap,
historical whitespace, and supplied publication-recording, existing-Task stacking
and cancellation/refused-start reports open. Cancel must use the existing client's
canceled-type state, not report completion. This review repairs none of them.

## Decisions and evidence

No new human choice is required for this cutover. Preserve the current-Run-only
bind assumption and replacement inheritance; do not bulk-rewrite earlier Runs.
Binding a taskless invocation's individual Run remains constrained by nullable
Task equality. Lifting that restriction or binding an entire conversation's
history would require an explicit ownership decision; neither is selected here.

Read the retained controller, durable and canonical decoder logs: 29 controller
tests passed before the decoder repair, 20 durable tests passed after it, and
the historical replacement regression passed on materialized schema. These are
the slice review's 49 distinct source tests plus one canonical repeat, with
simulated providers, not fresh test execution or installed acceptance.

Compared the 2,035 entries in `.lf/tmp/loo298-attempts/final-source-hashes.json`:
only `store/sqlite/durable.rs`, `store/sqlite/runs.rs` and the two design/question
notes differ. The two Rust differences are the reviewed decoder repair and its
regression. Earlier passing claims keep the scope recorded by the slice review;
the proposed all-kind changes will require new behavior/import/DTO proof, not
reuse of Task-only results as evidence for the new callers.

Re-ran the retained non-test-prefix measurement: against `4cd64be3d`, **+1,617 /
−556**, 18 production Rust/Swift files, zero deleted files. `human_session.rs`
has **2,350 lines before the test attribute**, 2,351 including it. This review
adds/removes zero executable lines. The method excludes test files and trailing
test modules but retains inline test helpers; it is not Jack's whole-file count.

Only this review document changes. No product tests were repeated, no API or
migration changed, and no execution, publication or Task disposition was selected.
