# Minds: design review

## What was implemented

Waves now own the only durable mind: `lf serve <wave>` keeps one journal-backed
human thread, one resident body, and one durable invocation-stack playhead.
`lf loop <flow> <seed> [--detach]` creates disposable hands in placed
worktrees; they report through a SQLite bus with at-least-once folding, and
`lf radio`/`lf sub` still work while no wave process is running.

The Mac surface projects the same model: thread, active flow and step, body
boundaries, queues and return targets, KRs, PRs, sessions, and backlog. Project
promotion is the explicit path from a measured bet to another resident mind.

The gate pass tightened three edges before review:

- The agent-held exec door now rejects `lf chat` and `lf wavechat`; machine
  speech must use `lf radio`, preserving the human-thread boundary and byline.
- The retired channel-tagged SSE compatibility fixture and Swift filtering path
  are gone. `/events` is the served mind's thread; work-line traffic is bus-only.
- Silent timestamp and SSE serialization fallbacks now fail with an invariant
  message, and user docs, demos, launcher probes, and code comments consistently
  use `serve`, `loop`, `chat`, and `radio` for their actual roles.

## Key choices

- **Continuity belongs to the journal, not a vendor session.** Provider bodies
  may die or change while the wave's thread and logical step survive.
- **Nested work is an invocation stack with local FIFO continuations.** An
  inserted flow completes before returning to its caller; skip and recovery do
  not erase the route home.
- **Hands are execution, not smaller minds.** Placement chooses where a body
  acts. It does not mint another thread, memory, cadence, or project tree.
- **Human and agent speech use separate doors.** `lf chat` is HTTP/SSE into a
  served mind; `lf radio` is a store insert with an explicit arrival channel and
  client-supplied byline.
- **Report delivery is at least once.** The journal write precedes the bus cursor
  commit. A crash in that seam may duplicate one report; reversing the order
  could lose it.
- **Promotion stops at the origin boundary.** A project defined only in a worker
  worktree must land before it can become resident, keeping review policy out of
  `lf project promote`.

## How it fits together

`lf serve` owns the listener, journal fold, and playhead; a resident executes the
current logical step and returns ordered deltas through a token-gated local
door. Detached hands run the same loop primitive in their own worktrees and
publish reports into `lfdb`; the served mind advances a durable cursor as it
folds those rows into its thread. Loopflow Mac reads durable registry and plan
facts directly and follows the served thread over SSE, so CLI and app project
the same state rather than maintaining parallel models.

## Risks and bottlenecks

- Bus delivery is intentionally at least once. A listener crash after journaling
  but before cursor commit can repeat one attributed turn on recovery.
- The one-hour sweep window is enforced on reads and writes. A mind sleeping
  longer gets a visible cursor-jump turn and must recover detail from runs and
  PRs rather than the bus.
- Only Codex supports true mid-turn steering. Claude and OpenCode receive steer
  input at the next body boundary; the UI must continue to describe that delay
  honestly.
- Bus bylines are testimony, not authentication. The arrival channel is retained
  as evidence, but a local writer can claim another label.
- Promotion cannot remove the old `project:<slug>` Linear label because the PM
  provider lacks a remove-label operation.
- The full automated matrix proves the Mac and UI-test targets compile, but this
  headless gate had no rendering environment. The visual playhead/recovery
  walkthrough in `scratch/minds.md` remains a reviewer check.
- Linear reconciliation was blocked by an expired stored token. No PM task was
  mutated; the schedulable follow-ups are recorded in `scratch/questions.md`.

## What's not included

- True mid-turn steering for non-Codex harnesses.
- First-class composite playhead nodes.
- Project-loop-specific timeout/pass defaults or PM label removal on promotion.
- A persisted foreground/background run label.
- A detached hand's private bus cursor or a one-writer worktree lock.
- Consolidation of the overlapping `lf chat` and `lf wavechat` CLI surfaces.

## Validation

`uv run python scripts/test.py --all` passed all six suites:

- Python: 51 passed.
- Rust: 1295 passed, 3 configured skips.
- Website: 59 passed, 3 configured skips.
- Swift: 307 tests across 50 suites plus 5 XCTest cases passed.
- E2E smoke: passed.
- Loopflow Mac: `xcodebuild build-for-testing` passed, including UI-test targets.

Plain `cargo test` also completed with 1295 passed and 3 ignored.
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the
Swift multiplatform boundary check, demo-script syntax checks, and focused bus,
exec-policy, WaveChat, launcher, and registry tests passed. Static contract
checks find no retired broker/channel-frame symbols, no work-line journal path,
and no server/network dependency in `radio.rs` or `sub.rs`.

## Wave alignment

This branch directly advances Product's wave-chat bet: the steward thread and
logical step now survive process restart, reports fold into that one thread, and
send/steer/interrupt/resume resolve through one served mind. It also keeps the
product goal's CLI/Mac/shared-API surfaces on one model and Infrastructure's
architecture legible by deleting the obsolete broker and compatibility path.

The week/month KRs are not claimed by a single gate run. Restart and replay have
automated proof; land, branch, machine-move, long-lived coherence, and source-
cited memory retention still require the stated dogfood trials.
