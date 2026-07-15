# W2-175 — durable interactive handoff, shared contract

## User-visible outcome

An agent can create one durable interactive handoff beneath its owning Wave,
Project Session, or Task Session. A terminal or app can repeatedly read the same
attach descriptor, record that a human attached, and resolve the handoff as
completed, handed back, or failed. The parent sees one durable waiting marker
and one terminal wake claim; reopening Loopflow or replacing the parent body
does not create a second handoff.

This first serial PR establishes the shared store and `lf handoff` contract. It
does not launch a vendor TUI or render Active Sessions in the Mac app.

## End-to-end proof

Create a Task-owned handoff with a local Home, provider history, body generation,
reason, required environment, and tmux attach argv. Attach twice, reopen the
store, hand it back, then race two parent generations to claim the wake.

The same Session id and attach argv must survive both attaches and the store
reopen; the first terminal result must survive a stale second result; exactly
one generation may claim the wake. The fixture at
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
- Existing Wave, Project, Task, and direct execution bodies remain unchanged.

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

## Operational boundary

All lifecycle mutations are one SQLite transaction and use conditional updates
under the existing five-second busy timeout. No listener, lfd, network request,
or terminal stream is in the path. Environment and argv are bounded by normal
SQLite row limits and are emitted as structured arrays/maps, never shell text.

Store reopen is the recovery boundary for this PR. Later runner integration
will reconcile live tmux/provider evidence against this row and the W2-135 body
lease before adopting, replacing, or waking a process.

## Exclusions

- Launching or supervising Codex, Claude Code, OpenCode, or tmux.
- Changing Project/Task runner status or Wave resident scheduling.
- Transferring provider session ownership between headless and interactive
  bodies.
- Mac Active Sessions UI, embedded Ghostty, external terminal adapters, and
  remote SSH presentation.
- Ten real handoffs and host-restart evidence; those require the launch and
  presentation slices above.
