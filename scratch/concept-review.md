# LOO-298 concept review

2026-09-26 · Reviewed `cb4ee961ff88784db591758ad87605ca0008cfd8`
against `4cd64be3d0432aa04dd9256293eef7088db97ccc`, including the supplied
uncommitted [slice review](data-model-slice-review.md). That review is preserved
unchanged. This pass changes only this concept review.

**Judgment: keep the amended concepts; the branch is not ready to publish.**
Session is the conversation Jack keeps; Run is an attempt within it. The latest
storage and recovery changes support that distinction. The remaining P1 is that
an action can lose the selected attempt while crossing API layers. Fix that
boundary within the approved model; no additional product noun or lifecycle is
needed. All eight Done When obligations remain.

The [amended design](data-model-one-table-per.md), [review feedback](data-model-review-feedback.md)
and [Session decision](session-runs-and-current-run.md) govern. Jack commissioned
one SQLite owner per main product object. The later participant (name unresolved)
approved taskless Invocations, Session owning Runs plus a current Run, and Flow /
Invocation naming. The older Task title and Wave memory do not override those
corrections. This review supplies evidence and chooses no Flow navigation.

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

Jack starts an unbound conversation, gives it a name, and binds it to INF-123
even after its PR lands. Every view agrees about the current Run's Task. The
same selected Session and live terminal retain the draft. INF-123 is illustrative;
bind and the filtered Session list remain target behavior.

Recovery keeps the same Open action. Resume native history when recoverable;
when exact execution evidence permits replacement, append a Run and select it
without losing the Session's title, feedback or earlier attempts. Missing launch
receipts cannot justify another provider launch. Retaining Session identity does
not recover a dead provider's unsaved native state.

For a review, Ready saves feedback; Complete returns it once to the waiting
boundary. Pane closure and provider exit never complete the review. A broken
neighboring invocation should leave this conversation reachable.

Draft clarification for `docs/lf.md` Sessions, after the implementation proves it:

> Use the Session ID to act on the conversation's current attempt. A Run ID or
> Run prefix selects that attempt: if it has been replaced, the command reports
> its Session and current Run without acting on the replacement. Once Complete
> selects an attempt, it saves that attempt's feedback and tears down that same
> attempt. A concurrent change rejects completion instead of selecting again.

This carries forward the latest slice review's expectation and the design's
existing stale-attempt guarantee; it is not new participant approval. Stable
Session-ID rename remains a conversation operation. The implementation need not
turn a rename into an execution operation, but it must preserve the distinction
between a Session selector and an explicitly selected Run.

Keep the already-clear usage in `docs/lf.md:576` unchanged during this review.
Before shipping, reconcile the preceding prepared-Run paragraph (`:562`) with
the final reservation/publication contract: it still describes the integrated
file-backed path and launch-time context filling. The architecture reference
owns transactions and validators; `docs/waves.md` owns Chapter history. The
builtin headless surface already distinguishes Ready from completion, and
`task/skill/review-design.md:112` requires explicit assumptions. No skill rewrite
is needed to teach storage internals or a new recovery command.

## Core model in one screen

| Action | Identity and owner | State change |
| --- | --- | --- |
| Choose a workflow | Flow template; Project default | Task may explicitly select another Flow |
| Execute or restart it | Invocation; captured graph, cursor, returns, claim | Resume retains capture; restart retains history under a new execution identity |
| Enter a nested loop body | Child Invocation | Runtime nesting only; template composition is expanded before execution |
| Inspect an attempt | Run; nullable Invocation, Task, Wave and Session | Run owns outcome, usage and exact process evidence |
| Name or reopen a conversation | Session; title, feedback, completion, current Run | Replacement appends a member Run and compares the current pointer |
| Complete a review | Selected Run and exact waiting boundary | Settle saved feedback once, then tear down the same attempt |
| Assign the conversation to work | Current Run's Task/Wave | Validated bind; membership and earlier Run attribution remain intact |
| Inspect past plans | Repository Chapter; Project at (Wave, Chapter) | Rotation preserves active identity and freezes predecessor evidence |

A node and launch-time iteration tuple locate a Run. No occurrence object is
needed. Loop return, Run retry, Session reopen and Invocation restart preserve
different facts and remain distinct transitions. The selected Run expectation
is an input to an operation, not another durable object.

## Findings and consequences

Source paths below are relative to `rust/loopflow/src/`. These are source
observations and interleavings, not runtime reproductions.

### 1. P1: the selected Run is discarded before the effect

`ops/human_session.rs:585` resolves retained Run membership and rejects an
already historical selector. But `complete_flow` (`:717`) converts its resolved
position into `FlowSessionToken`. That token (`:69`, constructed at `:1918`)
contains the boundary identity, but neither Run ID nor position version.
`controller/task/mod.rs:764` then reloads a position and uses that new snapshot
as the settlement expectation.

If Run A was selected, then Open publishes replacement B at the same boundary,
the token still matches. An external CLI caller has no superseded provider
`LF_RUN_ID` for `require_current_review_actor` (`human_session.rs:740`) to reject.
Completion can therefore consume B's retained feedback while `stop_flow_run`
receives A's old position. The store compares the exact position in
`store/sqlite/children.rs:235,271` and checks current Run/feedback in
`sessions.rs:352`; those checks protect the snapshot supplied to them, not the
expectation the caller discarded.

The same identity loss appears before other effects. Rename resolves the
original selector, acquires the Session launch lock, then resolves the Session
ID (`human_session.rs:1451`). Open similarly switches to the boundary ID
(`:1083`); its native resume occurs before the later replacement lock in
`open_boundary` (`:1176`). An attempt-specific request can become an action on
the conversation's newer attempt. A read immediately before the effect does
not close the remaining race.

Simpler experience: an explicit attempt reference keeps its meaning throughout
the operation. Carry the selected Run and expected boundary from lookup through
settlement; retain original exact/prefix selector semantics across lock waits.
Coordinate native effects with replacement so the attempt validated is the one
resumed or stopped. A stable Session-ID action may select the current attempt,
but completion and teardown must agree about that selection. Preserve the
existing transaction, invocation and process-ownership fences; do not add a
second current-attempt store or infer control permission from Session state.

The existing naming/membership fixture (`controller/task/mod.rs:2335`) rejects
historical selectors only after replacement and separately checks a superseded
provider actor. It does not exercise this interleaving. The latest slice review's
P1 remains unresolved; a clear concept model cannot downgrade it.

### 2. Earlier lookup and publication findings have advanced at source level

| Earlier counterexample | Current implementation | Remaining proof or limit |
| --- | --- | --- |
| Superseded/completed review Run becomes an independent interactive Session | Exact and manifest-prefix lookup query retained membership first (`human_session.rs:589,606`); historical references identify their owner | Sequential fixtures exist; the action race above remains; final history DTO/inspection still absent |
| Existing artifacts strand an unpublished SQL reservation | `run_record.rs:2205` reconciles immutable manifest/context inputs and original creation time; `:1581` claims SQL before constructing the recorder | Execute partial-staging, artifact-before-SQL, conflicts and duplicate-claim proofs; conflicting bytes remain a named failure |
| Rejected SQL publication writes a false terminal receipt via recorder Drop | Recorder construction follows the publication callback (`run_record.rs:1598`) | Authored publication fixture is compiled, unexecuted |
| Published Run without start evidence gets replaced | Open requires recoverable native history or terminal evidence and preserves uncertainty (`human_session.rs:1196`) | Automatic recovery from an uncertain launch still needs the Run process/outcome owner; a publication flag proves no provider start or death |
| Malformed selected human capture hides valid conversations | `review_surface` represents selected `InvalidData` on its own Session and disables its actions (`human_session.rs:1698`); other errors propagate | Public-list/Open-preparation fixture exists; no actual native Open or CLI dispatch proof |

These supersede the previous concept review's source findings for those paths.
They do not establish that every recovery case works. Keep immutable conflict
inputs, corrupt captures and exact process evidence. Do not generalize the
bounded invalid-capture handling into swallowing database or I/O failures.

### 3. Stable ownership is useful, but remains limited to Task reviews

`store/sqlite/sessions.rs:84` replaces a published Run inside an immediate
transaction and increments the invocation version. Conversation fields remain
on Session. `save_review_in` no longer lets cursor checkpoints overwrite an
existing conversation; Ready (`:245`) requires the exact current published Run
and invalidates an in-flight completion snapshot. Preserve these boundaries.

The full model still has live unconverted consumers: `human_session::list`
(`:571`) combines four sources; Ask Open clears feedback and copies names
(`:1158,1163`); taskless `flow_run.rs:108,136` persists `position.json`;
`lf/commands/runs.rs:87` scans manifests through `WorkCatalog`. General Run
provider/outcome/usage and node/tuple fields are absent from `session.rs`.
Swift `SessionRecord.swift:134` still exposes the old projection without a
current-Run/history contract. Chapter activation clears current by Wave
(`store/sqlite/chapters.rs:44`).

The simplifying result is one conversation list, one Open action, and stable
pane identity through replacement. Achieve it by completing the common owners
and callers, then deleting the live alternate storage paths. Removing those
paths now would remove supported behavior. Keep the full implementation,
import, installed acceptance and deletion scope; no intermediate dual-owner
publication is selected.

## Unresolved product consequences

Current-Run-only bind remains an implementation assumption. If A belonged to X,
replacement B initially belongs to X; binding the Session to Y changes B to Y
while A's attribution and usage stay on X. Session identity approval did not
approve a bulk historical rewrite. Any requirement to move all history needs
an explicit ownership decision before implementation changes that scope.

A taskless Run may still belong to an Invocation. Nullable Task equality then
rejects binding that individual Run to a Task atomically. Orphan does not imply
independent execution. Allowing that bind would require changing the accepted
ancestry contract or deciding how whole-invocation attribution moves; neither
is silently selected here. These choices do not block repairing the action race
or continuing within the recorded assumptions, and this autonomous review opens
no Ask.

## Smallest next action and proof

Repair selected-attempt propagation across lookup, controller settlement and
native effects as one change. Deterministically pause external Complete after
resolving A, publish B with retained feedback at the same boundary, then resume.
Require rejection, unchanged B/cursor/feedback, and no stopped client. Repeat
with rename waiting for its launch lock and Open at the native-effect boundary,
including a Run prefix. Then complete B once and verify its saved feedback and
exact stopped Run. Use synchronization barriers, real lookup/store transactions,
and mocked provider effects; sleeps or mock-call wiring alone are insufficient.
Keep sequential old-Run, stale-provider and Ready rejection coverage.

When TESTING.md preflight permits, execute that proof and the focused publication,
historical lookup, corrupt-neighbor, repeated-replacement, schema, Task controller,
durable-store and populated migration commands already listed in the design and
slice review. Repeat populated preservation after materialization in a disposable
source copy. No test pass is inferred from compilation.

Then continue the shared taskless driver, general Run facts, all Session kinds,
bind/read/launch callers, DTO/Swift pane identity and caches, repository Chapter
operation, populated offline import and real-Home maintenance. Configured CLI/app
acceptance, retained drafts, comparable measurements and deletion research followed
by deletion remain required. The supplied PR #1296 publication-record and
existing-Task stacking reports remain unreproduced implementation scope; this
review changes no delivery state.

## Evidence boundary

Inspected the latest diff and slice review, normal rename and Complete, Open
recovery, token construction, store settlement/replacement, publication ordering,
historical lookup, corrupt-capture rendering, retained fixtures, DTOs and affected
usage/skill guidance. Only this note changed. No implementation, provider, Home,
PR, Task disposition or execution restart changed.

Reused the slice review's resource receipt: active `main-view-task` **15.3 GiB /
12 GiB**, **97.3 GiB** free; safe recovery preserved the active foreign build.
This documentation-only pass did not resample resources or execute product tests.
The note's whitespace check and all four local-link checks pass.
Earlier formatting, Clippy, migration and HTML checks retain only their recorded
scope. SQLite owner coverage still has the recorded **32/33** `wave_chapters`
gap; historical branch-range whitespace findings remain. A clean diff check of
this note clears neither. No existing executed proof is invalidated by the note;
the exact-attempt acceptance claim requires the new interleaving proof above.
