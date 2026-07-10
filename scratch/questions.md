# Unresolved scope

The built/not-built ledger lives in `scratch/minds.md` under **Status**. This
file carries only the judgment calls a reader would otherwise re-litigate.

- Skill steps run in the live harness and Codex steering journals and streams
  incrementally. Claude and OpenCode expose no true mid-turn steer capability,
  so their input queues to the next body. Composite top-level flow nodes still
  run through the internal headless step fallback; promoting branch/loop
  internals into first-class playhead paths is separate graph-runtime work.
- Project promotion can create the child PM project and move its labeled tasks,
  but the PM provider abstraction has no remove-label operation. The promotion
  skill records residual `project:<slug>` labels here when it cannot remove
  them safely; adding provider-level label removal is separate API work.
- Residency reads wave definitions from the main checkout. When promotion is
  authored in a worker worktree, the command now stops with an explicit
  land-before-residency error instead of launching a child against files the
  listener cannot see. Fully automating that handoff would make project
  promotion own the review/merge policy, which is outside this command.
- `lf loop project` still inherits the generic 8-pass / 2-hour caps. The new
  command exposes explicit cap overrides and the doctrine requires durable
  reports/memory, but selecting longer project defaults needs real dogfood data
  rather than a guessed weeks-scale timeout.
- The run ledger now exposes a loop's current pass, but it does not
  persist whether the owner was the foreground wave pass or the listener's
  detached tmux supervisor. The Mac screen therefore shows pass/worktree and
  liveness without inventing an unreliable foreground/background label.
- The bus/thread scope changed mid-branch, deliberately: channels became
  ephemeral pubsub wires and journaling became a property of served minds only
  (`scratch/minds.md` §8). That landed. Child journals are deleted rather than
  moved, the prefix write-gate sketch is dropped in favor of open publish with
  honest bylines, and the verb splits with the wires: `lf radio` is the agent
  bus, `lf chat` is the human client for a served mind's thread, `lf serve`
  stays lifecycle. §9 then split the *transports* too: radio and chat no longer
  share a door. Radio is an INSERT into `bus_messages`; chat is `POST /messages`
  on the mind's listener. The postgres/psql analogy held only while the bus was
  a broker; once the db is the bus, sharing a wire would mean putting a server
  back in the publish path.
- **The byline is stamped from the channel, not from the token.** §8 says "the
  token names the writer"; it cannot yet. `SubagentDoor` mints one token per
  boot and hands it to every descendant, so the server has no way to tell which
  hand presented it. Deriving the byline from the addressed channel is
  unforgeable and is exactly right for a report — a hand speaks on its own
  channel, and "this is goals.148e reporting" is how radio speech works. It is
  wrong in one direction: a mind that publishes a recorded `say` on a hand's
  channel is bylined as that hand. Steering therefore belongs on the wave's own
  thread, and the doctrine now points there. Superseded by `minds.md` §9: the
  bus moves to the shared store, no server sits in the publish path, and the
  byline becomes client-submitted testimony recorded beside the channel's
  evidence — per-hand tokens stop being needed. §9 landed; the byline is now
  whatever the client wrote, next to the channel it arrived on.
- **A detached loop's driver holds no subscription.** §8's model assumed one,
  live for the loop's lifetime, queueing steers in memory. The driver has no
  such thing, so `lf radio --channel <hand>` reaches `lf sub` listeners and
  nobody else. The design's own slow path still holds and is what the doctrine
  teaches: a hand re-reads the wave's memory and thread at every pass boundary,
  so speech on the wave's thread is the ear a hand reliably has. Building the
  fast path is a decision about whether a hand deserves a private steer channel
  at all, not a bug to fix on the way past. `minds.md` §9 makes both answers
  cheap: on a store bus the hand's ear is a poll cursor, held or deliberately
  not. §9 landed and the fork stays unresolved: no driver polls its own channel,
  so a steer on a hand's channel still reaches only live `lf sub` listeners. The
  wave's thread remains the ear a hand reliably has, and the doctrine still says
  so.
- **`lf wavechat` and `lf chat` now overlap.** The rebase onto main picked up
  `lf wavechat` — a one-pane TUI that both follows a wave's events and posts
  typed lines into its thread. This branch splits that surface three ways:
  `lf serve` boots the mind, `lf chat` attaches to its thread, `lf sub` reads
  the stream. `wavechat` is `chat` + `sub` fused, which is the fusion §8 argues
  against. Both were kept through the rebase; deleting a verb that just landed
  on main is not a rebase's call. One of them should go, and the split is the
  one this design defends.
- Two agents wrote this worktree at once. HEAD advanced under a running skill
  while unrelated files were being edited, and the two writers left a
  self-contradicting test behind — one updated the frame it waited for, the
  other left the assertion on the old shape. Nothing enforces one writer per
  worktree. Whether that is a wave-home invariant or a lock is open; until it
  is settled, check for a live agent before working a wave worktree.

  It recurred during the rebase onto main, and a rebase is the worse case. Two
  drivers shared one `rebase-merge` state dir: conflicts resolved themselves
  between one command and the next, and `done` advanced 6→22 with no
  `--continue` from this session. Nothing was lost — the resolutions were
  right, and the loser noticed in time to stop touching git — but interleaved
  `--continue` calls have no reason to land on a coherent tree, and a
  mid-rebase write to `scratch/` would have wedged the other driver's next
  checkout. Concurrent editing corrupts a file; concurrent rebasing corrupts
  history. The store lease in `minds.md` §9 / Next-session item 5 is the fix;
  it should cover `.git`'s sequencer state, not just the worktree's files.

Assumptions taken while implementing §9 (`minds.md`, "the bus is the store"):

- **Sweep window is one hour, swept on publish.** The design says "a wall-clock
  window" and does not name it. One hour is long enough that a mind asleep
  between passes catches its hands' reports and short enough that the table
  never reads as a log. The sweep rides every `publish_bus` rather than a
  background task, because a bus nobody writes to needs no cleaning — and a
  timer would be a process in a path the whole design exists to empty.
- **A mind skips rows bylined with its own channel.** Not in the design. Without
  it, a mind steering a hand (`lf radio -c goals.<run>`) folds its own steer back
  into its own thread and wakes its own loop with it. The rule is honest — a mind
  does not report to itself — and it is what makes §9's "a mind speaking on a
  hand's channel is bylined as the mind" safe to land.
- **`lf sub` is now a bus verb only.** The design's demo requires it to work with
  zero processes, which the SSE follower cannot. That follower moved verbatim to
  `lf/commands/thread.rs`, where it still backs `lf wavechat` (replay, backoff,
  reconnect, the human render). `lf sub` never opens a socket. The overlap
  question above ("`lf wavechat` and `lf chat` now overlap") is untouched by
  this: wavechat is still chat + a thread reader, just no longer chat + sub.
- **A cursor jump is announced as a `say` from `bus`.** The design asks for the
  miss to be "visible (the journal shows the cursor jump)". A journaled `say`
  bylined `bus` is the cheapest thing that both journals it and puts it in front
  of the loop. It costs one turn in the thread on a wave that slept past the
  window.
- **The bus needs the store to exist, not to be created.** `lf radio` uses
  `open_existing_store`, so on a machine with no `~/.lf/lfd.db` it drops with
  exit 0 and a note, exactly as `lf chat` drops with no wave. Publishing does not
  mint a registry.
