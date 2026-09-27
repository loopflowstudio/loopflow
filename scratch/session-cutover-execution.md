# Session cutover execution recovery

2026-09-26 · LOO-298 · Unblock Session

## Status and evidence

Recovery direction accepted on 2026-09-27. The participant's name is unresolved.
This note changes neither the approved model nor acceptance.
No Flow navigation, worker restart, publication or Home activation is selected.

The [cutover review](session-cutover-review.md) records a withdrawn implementation
and an actual CLI counterexample: an interactive Session lists and renames with
zero Session/Run rows and a title sidecar. This Session independently inspected
the retained probe and confirmed `git diff eeb609881 -- rust swift` is empty.
The existing unstaged [concept review](concept-review.md) remains untouched.
The [slice review](data-model-slice-review.md) retains the 49 Task tests and
canonical decoder repeat within their recorded scope; no tests were rerun here.

Observation: the retained probe asserts the old behavior, including zero rows.
It is useful diagnostic evidence, but cannot become green by implementing the
accepted contract. The four-origin acceptance fixture has not been supplied.
Source still shows four-way inventory and lookup, file-backed taskless progress,
and a Session domain type without Ask or interactive lifecycle fields.

Hypothesis: repeated whole-cutover instructions without executable intermediate
targets contribute to the stall. The record does not establish why the worker
withdrew its edits, whether a turn limit caused it, or whether delegation would
help. No model ambiguity or external blocker has been demonstrated.

## Accepted change to execution

Keep one implementation effort in this checkout. Use the four-origin proof as
its first executable deliverable and continue from its concrete failures through
the complete conversion. Internal stages are working progress, not separately
accepted owner-only slices. Preserve useful unfinished edits across continuation;
do not withdraw them solely because the entire cutover is not yet finished.
Defective or superseded edits may still be removed with the reason recorded.

1. Add the acceptance fixture beside `rust/loopflow/tests/session_cli_tests.rs`,
   using the actual compiled CLI and a private Home. Seed historical interactive,
   completed keyed Ask, Task review and pending taskless review evidence. Give
   each explicit expected IDs, titles, feedback, attribution, capture and outcome
   values. Keep the diagnostic probe unchanged. Clear inherited execution
   authority and pin all store/artifact/executable selections per TESTING.md.
   Establish a failing assertion for the new contract before production edits.
2. Implement the offline import and required Session/Run/Invocation transactions
   against that fixture. Use existing constructors and fences. Verify preserved
   values, unknown membership, repeated attempts, idempotence, conflicting-input
   rejection and recovery from interrupted import. Preserve the source files as
   evidence; ordinary reads must not import or fall back to them.
3. Convert interactive capture, Ask request/result/retry, and taskless execution
   writers. Add fresh conversations through the public CLI as well as importing
   old ones: a manually populated database cannot prove the new writers work.
   Preserve source-independent capture and exact process/publication authority.
4. Switch list, lookup and actions to the common owner. Move affected Rust/Swift
   DTOs and fixtures together. In the disposable fixture only, make retired input
   files unavailable after import while retaining immutable evidence and native
   history. List, lookup and rename must still work and recreate no retired
   files. A completed keyed Ask must return its answer without another launch;
   default inventory need not list closed conversations to prove that fact.
5. Finish recovery, bind and deletion proof before cutover review. Replace the
   taskless review Run twice after justified terminal evidence, retaining Session
   identity/title/feedback/history; reject stale actions and consume feedback
   once. Pair with failed-then-successful headless attempts. Prove confirmed
   current-Run bind to a done Task, competing bind/replacement rejection,
   nullable invocation equality and set-once Started. Delete the replaced
   inventory, file lookup/writers, ancestry helpers, sidecars and title copying.

Rehearse populated preservation against canonical materialized migrations.
Search reachable callers after deletion and report comparable non-test counts;
moving functions does not count as removing an owner. Then run affected checks
under TESTING.md. The four-origin fixture can be several focused tests sharing
the same seed; it need not be one oversized test or simulate real providers.

## Continuation and finish line

At a continuation boundary record only the current failing assertion/command,
actual result, edits retained, and next dependent change. Reuse unchanged proof
within its recorded limits. Do not send an unfinished foundation through another
compression/concept-review cycle as though it completed the selected cutover.
If a real blocker appears, report the failing command and exact missing decision
or prerequisite; unchanged dependency lists are not new blockers.

The first evidence that this approach helped is a runnable four-origin fixture
failing on required ownership/preservation, followed by a production change that
makes a named assertion pass while retaining the other origins. The cutover
finishes only when import, fresh writers, public reads/actions and deletion pass
together. A test-only deliverable or intermediate table extension is not success.

The [complete design](data-model-one-table-per.md),
[amended approval](data-model-review-feedback.md), and
[dated decisions](questions.md) still govern. Full Task completion additionally
requires the retained runtime-loop, Chapter, usage/desktop, configured-Home and
measurement obligations. No fixture substitutes for installed acceptance.

## Participant feedback and next action

On 2026-09-27 the participant asked for the concrete question. The proposal was
restated: build the failing four-origin CLI test first, keep implementing across
continuations without discarding useful unfinished code, and review when the
whole cutover passes. The participant replied, verbatim, “whatever. sure.”
This accepts the execution sequence; it does not relax preservation, approve an
intermediate publication, or establish the hypothesized cause of the stall.
The waiting caller should reassess using this note; this Session chooses no edge.

Next action: write and execute the new-contract four-origin fixture,
then retain it and its implementation progress until the complete cutover passes.
