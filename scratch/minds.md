# Minds: how to verify this branch

The design and its consequences now live in `wave/product/MEMORY.md` ("Minds")
and `wave/infrastructure/MEMORY.md`. What remains here is how to prove the
branch does what it claims. `scratch/pr-body.md` has the short try-it recipe.

## Done when: publishing needs no server

Run with an isolated `LF_HOME` and no `lf serve` anywhere.

```bash
lf sub ship                          # terminal 1: tune in first
lf radio -c ship "hello"             # terminal 2: no server running
```

- The row is in `bus_messages` and prints on the subscriber within one poll
  interval (250 ms). No HTTP in the path.
- Two detached hands exchange messages on each other's channels with no served
  wave. A broadcast on an out-of-prefix channel (`other`) is not heard.
- A row aged past the 1 h window vanishes on the next publish, and the id keeps
  climbing (`AUTOINCREMENT`, so no cursor rewinds).

## Done when: a sleeping mind catches up

```bash
cargo test -p loopflow a_sleeping_mind_catches_up_exactly_once
cargo test -p loopflow a_swept_report_leaves_a_visible_cursor_jump
```

By hand: serve a wave, kill it, have a hand `lf radio` a report, restart the
wave. The report lands in its thread attributed, exactly once — the cursor
decides, not luck. Restart again; still once. A report published beyond the
sweep window is missed *visibly*: the thread carries
`bus cursor jumped 0 → 3: 2 broadcast(s) aged past the sweep window`.

The floor is at-least-once — the journal write and the cursor commit are not one
transaction. A clean restart replays nothing; a crash in that seam duplicates
one row. Don't assert exactly-once anywhere.

## Done when: byline is testimony, channel is evidence

```bash
lf radio --from ci -c ship.a "all green"   # run from inside the ship.a worktree
```

A subscriber prints `[ship.a] ci: all green` — byline `ci`, arrival channel
`ship.a`, the mismatch preserved in the record. No code path derives identity
server-side.

## Done when: the broker is gone

```bash
rg 'family_tx|ChannelFrame|tagged_turn_json|deliver_to_channel|subscribe_channels'   # zero hits, all languages
lf radio --steer "x"                         # error: unexpected argument '--steer'
```

`/events` has no scope query; `/messages` has no `channel` field. `radio.rs`
imports nothing from `wave::server`. `lf sub` opens no socket.

## Done when: no journal exists outside a served mind

Run a detached loop end to end. The only journal files on disk are at
`.lf/journal/waves/<wave>/` under the origin repo — one per served mind, none in
the loop's worktree. `child_worktree_path` no longer exists in the tree.

## Done when: the mind is always somewhere

Open a wave in Loopflow Mac and drive the MVP demo:

1. Watch the default flow advance without selecting a session. The header names
   the active flow and step, the invocation breadcrumb, `now`, and `next`.
2. Enqueue `review-design`; while inside it, enqueue `research`. Both appear in
   `review-design`'s local queue, with the return target shown in `wave`.
   `review-design` finishes its steps, `research` runs, and only then does the
   playhead return to the suspended `wave` invocation.
3. Send a message and watch the active body answer inline (Codex; Claude and
   OpenCode queue to the next body).
4. Skip a step. A `skipped by user` boundary appears, the next body starts, the
   queued `research` survives, and the return point is intact. Output arriving
   from the skipped session cannot append to the new body's span.
5. Close and reopen the app. Thread grouping, invocation stack, current step,
   local queues, and return targets are unchanged.

The MVP holds when that whole demonstration uses one Chat thread and never
requires a terminal attachment or a session picker.

Two more, by injury:

- Kill the active harness. A visible failure boundary appears tied to that body;
  the scheduler retries the *same logical step* in a new body rather than
  advancing the playhead because a process exited.
- Restart the wave server. The abandoned body is marked interrupted, the same
  logical step resumes in a new session, every queued continuation survives, and
  no completed turn is duplicated.

## Full suite

```bash
cargo test                                   # to completion — see below
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
uv run python scripts/test.py --all          # Python, Rust, website, Swift, e2e, Mac build
```

**Run `cargo test` to completion before trusting a green suite.** A failing lib
target makes cargo skip every later target, so lib failures mask bin failures.

Gate run on 2026-07-10: Python 51 passed; Rust 1295 passed with 3 configured
skips; website 59 passed with 3 device-title skips; Swift 307 tests across 50
suites plus 5 XCTest cases passed; the e2e smoke passed; and the Loopflow Mac
scheme completed `xcodebuild build-for-testing`. Formatting, Clippy with
warnings denied, and Swift multiplatform boundary checks also passed.

The automated gate proves the Mac app and UI-test targets build, but this
headless run had no rendering environment. The five-step visual walkthrough
and its two failure-injury variants above remain the reviewer's product check.
