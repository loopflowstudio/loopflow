# Open questions and assumptions

The model, the built/not-built ledger, and this branch's durable learnings are
folded into `wave/product/MEMORY.md` and `wave/infrastructure/MEMORY.md`. What
remains here is what a reviewer would otherwise re-litigate.

## Blocked: Linear could not be reconciled

`lf pm show` fails with `Stored linear token has expired. Run `lf auth linear``
again.` — this run is headless, so no task was closed, corrected, or filed. The
work below should become tasks once auth is restored. Each is schedulable now;
nothing waits on anyone else.

Under `project:wave-chat`:

1. **A hand's ear.** No detached driver polls its own channel, so a steer on a
   hand's channel reaches only live `lf sub` listeners. Either hold a poll cursor
   at the driver's pass boundary, or decide a hand is not addressable and let the
   wave's thread be its one ear. Open fork, deliberately.
2. **Collapse `lf wavechat` into `lf chat` + `lf sub`.** Two surfaces claim the
   thread; the verb split says one should go.

Under `project:loopflow-api`:

3. **Project-loop caps.** `lf loop project` still inherits the generic 8-pass /
   2-hour task defaults. Run one real project loop first, then pick — a
   weeks-scale timeout guessed in advance is worse than the wrong default.
4. **Composite playhead frames.** `and`/`or`/`xor`/`loop` nodes run through the
   internal headless `__flow-step` fallback. Promote them when the Mac's
   breadcrumb starts lying about nested flows.
5. **PM provider label removal.** The abstraction has no remove-label op, so
   promotion records residual `project:<slug>` labels instead of clearing them.

Under `project:technical-architecture`:

6. **Reduction leftovers** (review findings, triaged): factor the shared
   inbox-interrupt arms and lift the lease-renewal block; merge
   `interrupt_child`/`interrupt_harness` behind one `begin_interrupt`; finish
   the endpoint-resolver consolidation; inline `require_loop_flow`.
   `heartbeat_idle` stays.
7. **Live sessions per worktree in `lf status`.** The store knows every session's
   cwd. Visibility, not a lease.

## Assumptions taken while implementing the store bus

- **The sweep window is one hour, enforced by readers as well as writers.** The
  design says "a wall-clock window" and names none. An hour is long enough that a
  mind asleep between passes catches its hands' reports, short enough that the
  table never reads as a log. The sweep rides `publish_bus` *and* every read
  (`read_bus_after`, `bus_head`, `bus_floor`) rather than a background task — a
  timer would be a process in the path this design exists to empty.
  Publish-only sweeping was the first cut and was wrong: it ignores the lone
  report that ages out while the bus is quiet, which a waking mind would then
  fold into its thread an hour late as though it were fresh.
- **The gap survives an emptied bus, via `sqlite_sequence`.** `bus_messages` is
  `AUTOINCREMENT`, so the high-water mark outlives every row the sweeper takes;
  `bus_floor()` returns the oldest surviving id, or `high_water + 1` on an empty
  bus. Two honest limits: the count is of *all* channels, not just the ones this
  mind hears, so it can overstate what would have been folded; and dropping and
  recreating `bus_messages` resets `sqlite_sequence` and that history with it.
  Neither is worth a second counter table — the announcement's job is to send a
  reader to `lf runs` and the PRs, and it does that whether it says one or three.
- **A mind skips rows bylined with its own channel.** Not in the design. Without
  it, a mind steering a hand folds its own steer back into its own thread and
  wakes its own loop with it.
- **A cursor jump is announced as a `say` bylined `bus`.** The cheapest thing
  that both journals the miss and puts it in front of the loop. Costs one turn in
  the thread of a wave that slept past the window.
- **The bus needs the store to exist, not to create it.** `lf radio` uses
  `open_existing_store`, so with no `~/.lf/lfd.db` it drops with exit 0 and a
  note, exactly as `lf chat` drops with no wave. Publishing never mints a
  registry.
- **`item_line` stays duplicated between `thread.rs` and `journal.rs`.** The
  three format strings are verbatim copies, but `journal.rs` wraps each in a
  `Narration` and also handles the prose variants `thread.rs` drops. One shared
  `Option<String>` helper would force an `unwrap_or_default()` at the journal's
  call site — twelve duplicated lines traded for a silent-empty path when
  `ConversationItem` grows a variant. Revisit if a third renderer appears.

## Judgment calls

- **Residency reads wave definitions from the main checkout.** Promotion authored
  in a worker worktree stops with an explicit land-before-residency error rather
  than launching a child against files the listener cannot see. Automating the
  handoff would make `lf project promote` own the review/merge policy.
- **`lf sub` is a bus verb only.** The demo requires it to work with zero
  processes, which an SSE follower cannot. That follower moved verbatim to
  `lf/commands/thread.rs`, where it still backs `lf wavechat`.
- **Two agents wrote this worktree at once**, and the resolution is dispatch
  discipline, not a lock: want a second writer, place a second worktree. The
  store contributes visibility (item 7 above), not enforcement. Rebases inherit
  the rule — the driver that owns the worktree owns its `.git` sequencer.
