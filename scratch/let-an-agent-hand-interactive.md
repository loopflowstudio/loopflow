# W2-175 — durable interactive human handoff

## User-visible outcome

An agent can deliberately create one interactive child beneath its owning Wave,
Project Session, or Task Session in the exact Home and worktree. The parent
blocks at a replay-safe boundary until the human completes the work or hands it
back, then the same parent resumes exactly once. A terminal or app can repeatedly
open the same attach descriptor without creating a second agent or lifecycle.

This first serial PR establishes the shared store and `lf handoff` contract. It
does not launch a vendor TUI or render Active Sessions in the Mac app.

## End-to-end proof

A running Task body reaches work that requires a human and opens an interactive
child while preserving its provider history. The shared transaction creates the
handoff and the durable parent-wait boundary. The human opens its descriptor
twice, including once after an app/store restart, in the recorded Home and
worktree. Completion or explicit hand-back resolves the child; even if the
parent body was replaced or two completion writers race, one current parent body
resumes once with the winning terminal evidence.

Deterministic tests cover local handoff, repeated attach, parent process
replacement, app/store restart, child body death, explicit hand-back,
completion, concurrent completion, and unchanged non-interactive execution.
The operational proof opens ten real handoffs and observes the same Session id,
provider history, worktree, and single parent resume each time.

### First-PR proof

Create a Task-owned handoff with a local Home, provider history, body generation,
reason, required environment, and tmux attach argv. Attach twice, reopen the
store, hand it back, then race two parent generations to claim the wake.

For the shared-store slice, the same Session id and attach argv must survive
both attaches and the store reopen; the first terminal result must survive a
stale second result; exactly one generation may claim the wake. The fixture at
`tests/fixtures/dto/interactive_handoff_attach.json` must decode and round-trip
in Rust and Swift.

Proof commands:

```bash
cargo test -p loopflow interactive_handoff
cargo test -p loopflow cli_parses_interactive_handoff_contract
swift test --package-path swift --filter DTOFixtureTests
cargo fmt --check
cargo clippy -p loopflow -- -D warnings
```

## Source of truth

`interactive_handoffs` in the machine shared SQLite store is authoritative.
One row owns:

- a stable handoff Session id;
- exactly one parent reference (`wave`, `project`, or `task`) and owning Wave id;
- the canonical Home, worktree/cwd, provider, optional provider session id, and
  referenced W2-135 body generation;
- why human input is required;
- required environment and attach argv (terminal bytes remain in tmux/vendor);
- waiting/attached/terminal state and terminal outcome;
- the one parent-generation wake claim.

The active row is the durable parent-waiting marker. A partial unique index
allows at most one unresolved handoff per parent. The row references the
existing body generation but owns no PID, process group, lease token, or
provider process: W2-135 remains the process authority.

`InteractiveHandoffAttach` is a derived wire DTO. It contains the Session id,
status, cwd, host, environment, and argv a presentation needs; it contains no
terminal stream or completion capability.

## State contract

- `waiting` — created and blocking the parent, not yet attached.
- `attached` — at least one presentation asked to attach; repeated attach is a
  read of the same Session and does not advance lifecycle again.
- `completed`, `handed_back`, `failed` — terminal outcomes. The first terminal
  transition wins. Repeating the same transition is idempotent; a conflicting
  stale transition fails without mutation.
- A terminal transition creates one pending parent wake. A generation claims it
  with compare-and-set. The claim is durable, so a second process or app restart
  cannot wake the parent again.

## Affected surfaces and consumers

- Rust domain types: handoff identity, parent reference, state, outcome, attach
  descriptor, and open request.
- Shared SQLite store: migration, create/get/list/attach/finish/claim operations,
  parent validation, active-parent uniqueness, and transactional idempotency.
- CLI: `lf handoff open|status|attach|complete|back|fail`. `attach --json`
  returns the shared descriptor; no command carries terminal bytes.
- Swift: decode-only attach DTO for the later Mac Active Sessions/presentation
  adapters, pinned by the shared fixture.
- Wave, Project, and Task runners: a later serial slice consumes the active row
  as a replay-safe wait boundary and claims its terminal wake.
- Existing non-interactive Wave, Project, Task, and direct execution bodies
  remain unchanged unless they deliberately invoke the handoff primitive.

## Absent and error states

- A malformed Session or parent id is invalid input, not “not found.”
- A syntactically valid missing parent returns not found; a terminal Project or
  Task parent refuses a new handoff.
- Empty reason, provider, cwd, attach argv, environment key, terminal summary,
  or generation zero is invalid.
- Opening against a parent that already has an unresolved handoff returns that
  row unchanged. It never creates a sibling lifecycle.
- Missing handoff lookup returns not found.
- Attaching a terminal handoff returns its descriptor and terminal status; it
  never rewrites the outcome.
- Completion before attachment is valid: an external presentation may finish
  without first calling the attach read.
- Claiming before a terminal outcome fails. Claiming after another generation
  returns `false` with the original evidence intact.
- Interactive child death records one failed terminal outcome and wakes the
  parent once; restart reconciliation must not create a replacement child unless
  the parent deliberately opens a new handoff after the terminal row.
- Missing tmux/provider evidence after restart is a reconciliation input, not
  permission to discard the durable handoff or provider history.

## Operational boundary

All lifecycle mutations are one SQLite transaction and use conditional updates
under the existing five-second busy timeout. No listener, lfd, network request,
or terminal stream is in the path. Environment and argv are bounded by normal
SQLite row limits and are emitted as structured arrays/maps, never shell text.

Store reopen is the recovery boundary for the first PR. Runner integration must
reconcile live tmux/provider evidence against this row and the W2-135 body lease
before adopting, replacing, or waking a process. Parent blocking and terminal
wake are durable state transitions, not listener memory or a long-lived RPC.

## Ordered delivery

1. **Shared store and CLI — PR #935 (open).** Land the model, migration,
   replay-safe lifecycle operations, `lf handoff` contract, Rust tests, and
   shared Rust/Swift descriptor fixture.
2. **Parent block/resume and child reconciliation — next serial PR in W2-175.**
   Connect deliberate Wave/Project/Task escape points to the store, preserve the
   W2-135 provider/body authority, reconcile child death and restart, and consume
   the one wake claim so the same parent resumes once.
3. **Operational proof — before W2-175 closes.** Exercise ten real handoffs
   across supported vendors/presentations available at that point. Record the
   exact Session, provider history, worktree, and parent-resume evidence; convert
   any failure into the owning implementation slice.

Mac Active Sessions, embedded Ghostty, and supported external presentation
adapters follow as separate Tasks, as required by the Task delivery contract.
They consume this Task's descriptor and lifecycle rather than extending it.

## Task exclusions

- Restoring lfd or the deleted parallel generic Session catalog.
- Granting attach, completion, or hand-back controls to non-interactive bodies.
- Building Mac Active Sessions UI, embedded Ghostty, or external/remote terminal
  presentation adapters; those are separate consumer Tasks.
- Replacing W2-135 process leases, recovery, or provider-session authority with
  handoff-owned process supervision.
