# Session readiness from every caller

October 16, 2026. Jack Heart requested prefetch/cache cost analysis for every
ensured Session, then clarified that creation may occur through arbitrarily nested
lf invocations without Desktop. The most basic creation path and death of its
caller must be covered. This extends LOO-416's investigation; policies below are
proposals, not measured improvements or a selected background-start policy.

## Expanded scope and uniform behavior

Jack explicitly included changing `lf task create` semantics, Session `ensure`
behavior, and every caller/usage in LOO-416. Current command boundaries are evidence,
not constraints on the design. Jack then required repository, Wave and Task
conversations to auto-create through the same mechanism and follow the same lifecycle.
This is accepted scope, not approval of a particular eager-provider policy.

Define one shared operation on the existing Work scope. Specify its creation
trigger, identity/reuse rule, promised readiness on return, caller-death behavior,
and retry identity. Use it from every domain creation/entry path, nested CLI and
Desktop. Context, placement and primary selection may vary by scope; lifecycle
reliability and auto-creation guarantees must not. Determine the analogous repository
and Wave admission triggers alongside Task creation; no Desktop-only first-entry
initializer can be the sole guarantee.

| Caller family found | Required reconciliation |
|---|---|
| `bin/lf.rs` → `ops/task::task_create`; capture, planning and operator skills | Decide whether creation reserves a conversation or also requests readiness. Return durable Task identity even if later preparation fails; retry preparation against it rather than file another Task. |
| `session ensure`, `connect --task/--wave --prepare`, direct connect | Consolidate selection/admission/start semantics and readiness reporting. Concurrent callers reuse one primary/provider without taking input control merely to warm it. |
| Desktop `openPrimaryConversation` and repository-entry hook; `WorkModel.prepareLinkedTask` | Both call the common lifecycle. Repo/Wave use ensure today; Task uses connect/prepare and additional location/Task reads. Remove accidental behavioral differences and duplicate work. |
| Direct/taskless runs, Flow steps, nested agent calls | Use the same durable admission and execution-owner handoff, preserving explicit execution requests, step results and cancellation. Primary selection is not a requirement for durable execution. |

Current asymmetry: repo/Wave ensure calls `prepare` after reuse; Task ensure returns
`surface` directly when it chooses an existing conversation. Desktop alone also
has a repository first-entry auto-open hook. These paths require deliberate alignment,
not three separate fixes.

The capture skill's false exact-input deduplication claim is corrected. Source,
planning documentation and Infrastructure's recorded LOO-406 decision agree:
each explicit create has a fresh Task ID; internal synchronization retries reuse
the saved identity. Uncertain filing requires identity reconciliation, not another
create. Session preparation retries must reuse that Task; warming failure never
authorizes duplicate creation. No command retry semantics changed.

## Existing boundaries

- `ops/task.rs::task_create` saves a Task and synchronizes planning; it does not
  create a Session or launch a provider. The current Task return does not prove ready execution. The existing planning test confirms creation without a checkout.
- `store/sqlite/sessions.rs::create_session` is the durable admission boundary:
  it reserves a Session/capture and commits before provider effects. The primary
  ensure transaction additionally chooses/reuses the scope's primary Session.
  Durable state survives the creating process; an uncommitted transaction does not.
- `human_session/primary.rs::ensure_task` may restore a missing checkout, choose
  an existing conversation or reserve a new one. `prepare` publishes initial-input
  artifacts, without provider launch. `CaptureHandle::start_prepared` binds the
  launch's prompt/runtime/model, rather than freezing those at ensure time.
- Direct CLI execution also creates Sessions: `commands/run.rs::begin_capture`
  reaches `CaptureHandle::begin_with_context` and the shared store. It does not
  depend on primary ensure, Task association, or Desktop. `journal` records nested
  Process/caller ancestry separately; ancestry is not lifetime/control authority.
- Current headless launch keeps the provider lifeline in the launching lf invocation
  (`os_process/lifeline.rs`). Its last holder's exit triggers provider-group cleanup.
  Thus current persisted identity does not establish survival of the launching
  process. Moving only Desktop launch to a holder would leave this gap.

Preserve explicit cancellation and Flow results. Completed Runs need not remain
running interactive conversations.

## Contract to prove, independent of origin

| Crash boundary | Required result |
|---|---|
| Before durable admission commits | No committed Session from that transaction; report failure/uncertainty honestly. |
| After admission, before owner starts | Same discoverable unopened Session; no phantom running provider. A later explicit start uses that identity. |
| During owner startup/handoff | Persist a launch generation and exact owner/provider identity before provider effects. Retry resolves that attempt under existing fences; a missing acknowledgement does not authorize a second launch. |
| After owner accepts execution | The owner holds provider transport, lifeline and output draining independently of creator and viewers. Caller exit does not kill accepted durable execution. |
| Owner itself dies | Preserve history and delivery uncertainty; report lost execution. No automatic turn replay or inference of success. |

The acknowledgement must mean the owner has assumed responsibility, not merely
that an OS spawn succeeded. Use existing Process/AgentProcess/capture owners and
fences; determine required additions in the transport implementation rather than
inventing a second job table. Keep idempotent retry identity available to the
caller: `create_session` reuses a supplied ID, but a client that loses its reply
and mints a new ID on retry has not obtained exactly-once creation.

No global watcher is needed for an explicit create-and-start operation: its shared
Rust path can launch the owner on demand. If raw record creation is supposed to
trigger eventual warming after every creator dies, that is a stronger contract:
it requires a durable pending request plus a surviving consumer, or a later reader
that resumes it. SQLite alone does not perform that work. Do not promise eager
readiness from a database insert while relying on Desktop to notice it.

## What can be prepared before clicking?

| Level | Benefit | Costs and limits |
|---|---|---|
| Durable Session/primary and location | Avoid admission/selection on click | Store contention, checkout restoration and planning reads may already be nontrivial. Existence is not input readiness. |
| Prepared context and metadata | Move reusable reads/assembly off the click path | File/config/account changes invalidate derived material. Keep current launch-time binding; don't cache authority, credentials or launch commands. |
| Running provider under owner | Avoid provider startup and native-history recovery | Idle memory/CPU, account limits and remote connectivity; starting the Task-session instruction may execute work or consume tokens. Distinguish transport startup from submitting the first turn. |
| Attached terminal with restored state | Avoid attachment/snapshot work | Stream bandwidth, parser/scrollback memory and snapshot size. Observation must not steal input control. |
| Retained native surface | Immediate return, including draft/viewport | Native/GPU memory; off-window display-link suspension already exists. Separate display cost from provider lifetime. |

Compare retaining opened surfaces, preparing metadata, starting owners and
attaching terminals ahead of selection. Prioritize selected work, visible tiles,
then other explicitly ensured work. Coalesce concurrent ensure/click requests by
shared identity, with bounded concurrency so speculation cannot delay a click.
Query matching alone grants no execution authority.

Do not call live providers an evictable cache. Disposable metadata/snapshots can
be rebuilt; dropping runtime state or drafts changes continuity. Starting fewer
speculative providers is different from killing already running work to hit an
LRU limit. Define cleanup for never-used preparations and stopped-owner artifacts
without retiring an ensured conversation or deleting history.

## Measurement and acceptance

Compare on-demand, metadata-only, owner-ready and terminal-ready policies at
12 actual Sessions, then 24/48. Include new versus history-bearing Sessions,
local/remote, idle/working/awaiting approval, and misses never clicked. Measure:
ensure latency; click-to-visible and click-to-editable p50/p95; app+owner+provider
RSS/CPU; retained terminal bytes; I/O/network; SQLite wait; provider requests/tokens;
wasted preparation and first-click delay under competing warmups. Report cold and
warm separately. Existing Desktop benchmarks use three retained cat PTYs or one
synthetic-provider snapshot Session; they do not choose this policy.

Exercise nested CLI → CLI → creation/launch with Desktop absent. Kill the creator
at each boundary above, then discover/open from a fresh CLI and Desktop. Race
prefetch with click: one Session and one provider, original Work/native identity,
no duplicate initial turn. Repeat for direct/taskless launch and a Flow step;
viewer detach must not alter step settlement. Test creator exit versus explicit
cancel separately. Include context changes between preparation and click, failed
warming, unknown startup acknowledgement and an unavailable remote Machine.

Repeat creation, concurrent ensure and caller-death proofs for repository, Wave
and Task scopes with Desktop absent. No runtime measurements or implementation
changes were performed.
