# LOO-497 — Remaining Desktop integration

Jack Heart authorized only the foundations slice in [design.md](design.md)
while unavailable to review the query-view prototype. The full October 16 draft,
including proposed grammar, commands, acceptance scenarios and source survey,
is preserved at `refs/loopflow/evidence/loo-497-before-compress:desktop-api-coordination.md`
(tree `a0a62123ff32addaf29cc11570b3263a5630fff1`). It is design history, not
additional authority. The accepted product direction remains in
[arrange-navigation-brief.md](arrange-navigation-brief.md) and
[layout-editing-direction.md](layout-editing-direction.md).

## Remaining contract

- Extend the Rust Work observation path and Swift `RegistryQuery` with the shared
  selection result. No Swift membership evaluator or subprocess per tile.
  Query text and repo/Wave defaults must use the same typed expressions.
  Add attention facts and ordering through that shared path: open Tasks before
  Waves, waiting Tasks first in automatic tiles. Current name/rank order is only
  deterministic foundation input; it does not satisfy attention-first acceptance.
- Add headless `view --plan` before the live macOS dispatch gate. Emit canonical
  Work refs, title, attention, primary/zoom content, observed machine/location,
  cwd and argv; report missing primary/location without inventing a shell.
  Plans use current evidence plus saved intent, never claim live Desktop state,
  ensure a primary Session, prepare a checkout or acquire a provider.
- Live `view/open/query` and UI/deep links converge on one selection entry point.
  Preserve exact Session/Changes intents, opening generations and readiness.
  Work navigation alone must not start a conversation. Repo selection targets
  its own window. Pinning changes discovery; rename uses `work/edit.rs` writers.
- Integrate the retained arrangement contract with one native Task layout per
  Work key. Containers reference Work/query members. Primary Session is the
  default tile face; only explicit zoom overrides it. An absent primary is an
  empty tile, never a fallback to the focused shell.
- Apply first-visible-wins to the whole repository window's displayed composition,
  first child then second, skipping hidden members. Suppressed occurrences retain
  query membership and identify their winner. Focus cannot choose the winner;
  promotion must reuse the same Task layout, Session and surface.
- Full-tree replacement includes every occurrence once, including hidden members,
  and checks the observed revision before changing layout or focus. Natural
  split/move/swap/resize edits transform that tree. Hide A, edit B, restore A
  preserves B's newer edits. Structural edits mark touched; focus/zoom/hide do
  not retile. Incomplete evidence retains intent; complete departures prune.
- Exact occurrence/container targeting resolves repeated Work and ambiguity.
  Input/read fences remain independent. Do not automatically retry mutations
  after a lost Apple-event reply; inspect their effects first.
- Persist portable intent through lfdb; save failures remain visible. Native
  handles, window incarnations, attachment claims and liveness are never saved.
  The existing repository/query records need Work/default-view binding and
  persistence callers, plus shared native pane intent (IDs, focus/zoom/hidden,
  recorded content locations) and app-owned file draft recovery. LOO-416 supplies
  continuity/recovery requirements; LOO-497 owns the record/API, LOO-489 its
  save/restore callers. No launch argv or native liveness belongs in these records.
  Cross-machine identity mapping and transport remain
  unimplemented; local storage does not prove restoration on another Machine.
- Preserve the Task's scripting acceptance: JSON results, target resolution and
  exit statuses (0 success, 1 failure, 2 bad arguments, 3 missing/ambiguous,
  4 unreachable, 130 interrupted), scoped LF_TASK/LF_SESSION/LF_REPO context,
  and send/capture/wait/run-wait where needed. Readiness/events remain LOO-453's
  shared contract; program status remains LOO-422/415's. UI readiness, provider
  readiness and process exit stay separate. This is remaining integration, not
  authority to duplicate those owners or infer attention from idle time.

## Delete — do not maintain (later live-integration cut)

- Shell-allocating `MultiplexerStore.split`, `SessionsWorkspace.split`, and their
  exclusive shell-allocation assertions. Scoped shell selection retains location
  and stale-target coverage; arrangement split only groups existing members.
- Membership cases in `DesktopPaneAction/controlPane`, replaced by selection
  commands while preserving input/text/read authority.
- Leaf-dropping `SessionsWorkspaceRegistry.hide`, replaced by retained membership
  plus visibility. Explicit companion removal/Undo still invalidates occurrences.
- `WorkProjection.inWorkingSet` and affected Swift working-set predicates once
  shared Rust results replace their consumers. Defaults include paused/idle
  started unfinished Tasks and empty child Waves.
- Direct view/deep-link selection mutations superseded by the shared entry point;
  retain stale-opening tests, drafts, exact Session/Changes intents and identities.

The `desktop list` → `state` cut is already in the foundations diff. Do not
repair the predecessor paths above during foundations work.

## Open design and coordination

Prototype grammar/parser, sort syntax, layout density, inbox appearance and
query pinning UI remain provisional. Review `query-view/` before production UX.
The archived draft proposes AND before `or`, typed scope/state atoms, container
query digests and revisioned plans; these are not accepted syntax or a mandate
for a second identity layer. Current foundations use authored node paths and
saved query IDs. Resolve that relationship during integration.

LOO-489 owns navigation/presentation and persistence callers; LOO-416 owns
provider/surface lifetime and capacity evidence. LOO-497 owns the shared model,
CLI/DTOs and store contract; LOO-498 consumes plans in external hosts. LOO-453
retains readiness/events, LOO-422/415 status transport and LOO-414 remote launch.
No new daemon, duplicate driver or unrelated owner takeover is selected.

Overlap includes `SessionsView.swift`, `WorkLayout.swift`, `WorkProjection.swift`
and persistence types. Earlier steers to Sessions
`session_17db296946474a3980a127d4fa838972` (489) and
`session_df202fd5754c4954aa659e1158a414fd` (416) remain unacknowledged here.
October 16 sibling reports now record LOO-489's shared renderer/projection and
LOO-416's application-owned registry retention as locally implemented with
focused tests. Neither proves combined native behavior or landed availability.
Persistence callers/record shape and attach-only transport remain incomplete;
earlier message acknowledgments are still unconfirmed. Project crumbs, literal
Dock icons and the Rex chrome inventory remain navigation questions.

## Integration acceptance

Gate will seed a real disposable repository with started running/paused/idle,
backlog and finished Tasks, primary Sessions, a shell, empty/child Waves, nested
queries and incomplete evidence. Produce plans through the built CLI, decode
in Swift, and exercise production selection/arrangement entry points without
mounting windows. Prove click/CLI membership parity, stable native identity,
focus versus zoom, hidden-edit recovery, duplicate promotion, stale/invalid map
atomicity, departure versus unavailable retention, reload, pin/rename propagation
and no read-time execution. Linux proves headless planning too. Mock external
process/network effects only; use the candidate CLI and a private Machine.

Native rendered continuity belongs to demo and LOO-416. The current pursue Flow
may publish foundations and reach demo; full LOO-497 remains open, with no merge
or completion before Jack's review and remaining integration.
