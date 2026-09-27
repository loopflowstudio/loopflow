# LOO-298 concept review

2026-09-26 · Reviewed `df1443c044aadaa80ddddd71c7ee8cda61db3afd`
against `4cd64be3d0432aa04dd9256293eef7088db97ccc`; entry tree clean.

**Judgment: keep the amended concepts; the branch is not ready to publish.**
Task reviews now have a Session owner that retains title, feedback and Run
history. This supports the approved experience. The remaining work is carrying
that ownership through lookup, preparation recovery, all conversation kinds and
consumers. No speculative redesign or additional product noun is selected.

The [amended design](data-model-one-table-per.md), [review feedback](data-model-review-feedback.md)
and [Session decision](session-runs-and-current-run.md) govern. Jack commissioned
one SQLite owner per main product object. The later participant (name unresolved)
approved taskless Invocations and Session owning Runs plus a current Run. The
older Task title and copied handoff do not override those corrections. All eight
Done When obligations remain; this review does not choose Flow navigation.

## Usage first

Proposed end-state walkthrough, not an executed demonstration:

```sh
lf --interactive : "Review the parser"
lf session list --json
lf session rename <session-id> "Parser review"
lf session bind <session-id> --task INF-123 --json
lf session list --task INF-123 --json
lf runs --task INF-123 --json
lf session open <session-id>
```

Jack starts outside a registered Task checkout. Rename gives the conversation a
human title. Bind assigns its current Run to INF-123, even after that Task's PR
lands. The same Session remains selected and its live terminal keeps its draft.
INF-123 is illustrative. Bind and the Task-filtered Session list remain target
behavior; the integrated rename command exists, with SQL ownership for Task
reviews and sidecar ownership for the other kinds.

Recovery uses the same Session and Open action. Resume native history when
recoverable; when exact execution evidence permits replacement, append a Run and
switch the current pointer without losing title, feedback or earlier attempts.
Stable Session identity does not recover a dead provider's unsaved native state.
Ready saves review feedback; Complete returns it once to the waiting boundary.
Provider exit and pane closure never complete a review. An unrelated broken
invocation must not prevent opening a valid conversation.

Keep the examples and present-tense specification in `docs/lf.md` (Sessions).
Before shipping, reconcile its preceding prepared-Run paragraph with the final
publication contract: it currently describes preparation and launch-time context
from the integrated file path, while Task reviews use SQL reservation. Jack
should not need two recovery instructions depending on storage kind.
The architecture reference owns transactions and validators; `docs/waves.md`
owns Chapter history. The builtin headless surface correctly distinguishes
Ready from completion (`engine/builtins/surfaces/headless.md:9`), and
`task/skill/review-design.md:112` already requires recording assumptions. Keep
those skills unchanged; storage instructions do not belong in every review.

Two usage clarifications remain draft review copy, not new product approval:

- Binding affects the current Run and attribution inherited by its replacement.
  Earlier Runs retain their Task and usage attribution; one Session's history
  may span Tasks.
- A taskless Run can still belong to an Invocation. Under nullable Task equality,
  binding that individual Run to a Task rejects atomically. Orphan does not mean
  independent of execution.

## Core model in one screen

| Action | Identity and owner | State-changing responsibility |
| --- | --- | --- |
| Choose a workflow | Flow template; Project's default | Explicit Task selection may override the default |
| Start, resume or restart execution | Invocation ID; captured graph, cursor, returns and claim | Resume retains capture; restart creates a new execution identity and retains history |
| Enter a nested loop body on a pass | Child Invocation ID | Runtime parentage only; template composition creates no invocation parent |
| Inspect an attempt | Run ID; nullable Invocation, Task, Wave and Session | Run owns outcome, usage and exact process evidence |
| Open, name or complete a conversation | Session ID; title, feedback, completion and current Run | Replacement appends a Run; current pointer must select a member |
| Assign the conversation to work | Current Run's Task/Wave | Validated bind, without changing invocation membership or reopening Work |
| Investigate a past plan | Repository Chapter; Project at (Wave, Chapter) | Synchronized rotation preserves execution identity and frozen evidence |

A node and launch-time iteration tuple locate an attempt; no additional
occurrence object is needed. Loop return, Run retry, Session reopen and
Invocation restart remain distinct transitions because they preserve different
facts. There is no reason to rename Flow / Invocation again.

## Findings and consequences

Source paths below are relative to `rust/loopflow/src/` unless stated otherwise.
These are inspected paths, not executed reproductions.

### 1. The Session owner now prevents feedback loss for Task reviews

`store/sqlite/sessions.rs:84` reserves or appends a Run inside an immediate
transaction, compares the expected position, and switches the current pointer
with the invocation version. Rename at `:203` respects human-over-generated
ordering; Ready at `:224` requires the current published Run and invalidates a
pending completion snapshot. Completion at `:341` compares Run and feedback
inside the Task settlement transaction. `save_review_in` at `:269` no longer
lets an execution checkpoint overwrite an existing Session's conversation facts.

Normal path: `human_session::rename` (`ops/human_session.rs:1404`) resolves the
review and updates its Session row. Recovery path: `open_boundary` (`:1089`)
tries native resume, retains the Task review's feedback, and delegates Run
reservation after checking launch/native-client evidence. The earlier concept
review's claim that Task reopen clears feedback is superseded. Ask reopen still
clears feedback (`:1121`) and copies a name (`:1125`); its conversion remains.

The desired simplification is now concrete: Jack keeps one named conversation
while attempts change underneath it. Preserve the existing fences and exact
process authority. A Session row or `published` flag supplies neither provider
liveness nor permission to signal a process.

The repeated-replacement fixture (`store/sqlite/durable.rs:1613`) checks three
history rows, stale writes, checkpoint preservation and one completion. It
seeds publication and calls store APIs. The integrated naming/membership fixture
(`controller/task/mod.rs:2335`) exercises rename and listing with a replacement.
Both are compiled but unexecuted. Neither establishes real Open/launch recovery
or stale-provider completion through CLI dispatch.

### 2. An old Run ID can still become a second conversation on lookup

Inventory and lookup disagree about ownership. `boundary_run_ids`
(`ops/human_session.rs:1219`) excludes all SQL Session Runs from independent
inventory. But `find_boundary_for_run` (`:1457`) considers only current Runs of
open reviews. `find_session` (`:585`) then falls through to a Run manifest.
`run_record.rs:1016` accepts a TUI manifest as interactive history, regardless of
its retained SQL Session membership.

Consequently, a superseded or completed review Run with an unresolved TUI
manifest can resolve as `SessionTarget::Interactive`. Rename then reaches the
sidecar writer (`human_session.rs:1440`) instead of the retained Session title.
Open/Complete likewise dispatch by that reclassified target. This does **not**
establish that an old Run can settle the current Flow boundary; it establishes
an alternate conversation identity and operation path. Native availability and
action checks still constrain what those operations can do.

Simpler interaction: every known Run resolves through its Session membership,
including historical attempts. Keep inspection/history distinct from authority:
an old Run reference must not silently become the current actor or gain a right
to complete its replacement. Closed Session history remains inspectable without
reopening it. Carry this through the common lookup and DTO/history API; do not
add another manifest predicate as a permanent ownership layer.

Smallest disproof/proof: create two published review attempts with TUI manifests,
then address the older Run through the actual lookup/rename path. It must resolve
the same Session or explicitly report historical status, never mint an independent
identity or write `session-name.json`. Repeat after completion. Retain the separate
superseded-actor Ready/Complete rejection proof. Current-Run naming coverage does
not exercise this counterexample.

### 3. Recovery still exposes the storage publication boundary

`lf/commands/run.rs:791–819` publishes a reserved manifest/context before
`publish_run_binding` marks its SQL Run published (`human_session.rs:957`).
An interruption between those writes leaves the reservation unpublished.
`reserve_review_run` retains its ID, but `run_record.rs:2170` publishes via an
exclusive staging directory and rename over the final directory. Existing
staging or final contents can prevent retry. The integrated prepared-file path
(`run_record.rs:1500`) does not reconcile this SQL reservation path.

Jack's recovery action should remain Open, with no manual deletion or new
conversation. Reconcile the exact immutable inputs and publication evidence,
retain mismatches as named failures, and preserve the no-duplicate-launch
boundary. Missing receipts do not prove provider death. This is the slice
review's unresolved implementation finding, not a reason to change Session
identity or add a user-facing preparation lifecycle.

Fault-inject before artifact publication, after artifact publication but before
SQL publication, and after SQL publication before provider start. Assert retained
identity/bytes and exact launch authority. A store-only reservation test cannot
prove any of these filesystem/process boundaries.

### 4. Direct storage discovery is necessary but not yet sufficient

`open_review_sessions` (`store/sqlite/sessions.rs:143`) reads Session/current Run
without decoding unrelated captures. This removes the earlier autonomous-capture
inventory dependency. Exact Session lookup also reads its own Session before
validating that Task (`human_session.rs:1753`).

However, `list_flow_sessions` (`:1635`) calls `review_surface` for every selected
review, which calls fallible `flow_position` at `:1670`. One selected malformed
review aborts the whole list before Ask, ordinary Flow and interactive Sessions
are appended (`:571`). The existing corrupt-neighbor fixture inserts an
autonomous invocation and asserts the store query; it cannot establish isolation
from a malformed human review in the public list.

Keep conversations discoverable through their own records. Validate execution
when an execution action needs it, and expose the broken invocation's identified
recovery problem without dropping its bytes or hiding other Sessions. Complete
Run membership/provider storage so presentation no longer needs a capture decode
or a manifest per row. Prove the public list and Open for a valid Session beside
both malformed autonomous and malformed review invocations. No new human flag,
blanket error swallowing or fallback owner is selected.

### 5. Two binding consequences remain assumptions, with visible effects

| Interaction | Displayed Task | Earlier Run A | Current Run B |
| --- | --- | --- | --- |
| A starts on X; recovery appends B | X | X | X |
| Bind the Session to Y | Y | X | Y |

Usage for A stays on X; usage for B follows Y. Keep this current-Run-only scope
for implementation. If assigning a conversation should move its entire history,
that is a product ownership choice requiring review; it can conflict with
invocation-owned Runs. Stable Session identity approval did not decide it.

Likewise, `Run.invocation.task == Run.task` includes null. Assigning an individual
Run of a taskless Invocation to a Task therefore rejects. The exact future choice
is whether universal bind accepts that invariant rejection or must move such a
Run to a Task. The latter requires changing the ancestry contract; assigning a
whole Invocation affects other Runs and children. No such change was approved.
Prove the two-Run attribution example and atomic null-to-Task rejection; do not
silently reparent execution or present a fresh conversation as equivalent bind.

## Remaining scope and next proof

The retained Invocation and Task review owners are useful internal progress.
`human_session::list` still combines four sources, `flow_run.rs:108,136` still
persists taskless execution in `position.json`, and `lf/commands/runs.rs:95`
still scans manifests through `WorkCatalog`. `session.rs` lacks general Run
outcome/provider/usage and node/tuple fields; `swift/Loopflow/Models/SessionRecord.swift:134`
still exposes the old projection without a current-Run/history contract. Chapter
activation still clears current by Wave (`store/sqlite/chapters.rs:44`).

Complete the coherent Invocation/Run/Session owner and caller cutover, including
shared taskless execution and source-independent review recovery. Then satisfy
repository Chapter rotation, populated offline import/materialization, real-Home
maintenance and configured CLI/app acceptance, pane retention/grouping,
measurements, and deletion research followed by deletion. Preserve all import
inputs until conversion proves them accounted for. This review does not authorize
an intermediate dual-owner publication.

The supplied reports about PR #1296 missing its Task publication record and
stacking already-created Tasks remain in implementation scope. They were not
reproduced here; no delivery state was repaired or inferred from remote success.

Smallest next implementation/proof: fix exact artifact/SQL publication recovery
and extend the real Session lookup path to retain historical Run ownership.
Exercise repeated replacement and stale actor rejection through that path; keep
the public-list corrupt-review counterexample alongside the store proof. These
are refinements of approved ownership, not a new approval requirement.

When TESTING.md resource preflight permits, execute the focused commands already
owed in [slice review](data-model-slice-review.md), including the repeated
replacement, populated Session migration, schema assertion, Task controller,
naming/membership and earlier invocation-preservation tests. Rehearse populated
migration after materialization in a disposable source copy. Compilation is not
behavior, and fixtures are not configured acceptance.

## Evidence boundary

This pass inspected the integrated diff, store transactions, normal rename and
Open paths, publication failure path, old-Run lookup, public inventory, DTOs,
usage docs, relevant builtin guidance and the preceding slice review. Only this
review note changed. No product test, provider, Home inventory/import, promotion,
PR mutation, Task disposition or execution restart ran.

Reused the slice review's resource receipt: active `main-view-task` **15.3 GiB /
12 GiB**, **97.3 GiB** free; safe recovery preserved the active foreign build.
This review did not resample it. Its recorded fmt, all-target Clippy, migration
validation and HTML consistency passes retain their stated scope. Architecture
still has the **32/33** SQLite-owner gap (`wave_chapters`), and branch-range
whitespace still fails on the earlier draft header and copied patch context.
A clean whitespace check of this note clears neither issue. No existing proof
is invalidated by this documentation-only review; the broader identity and
availability claims need the additional behavioral cases above.
