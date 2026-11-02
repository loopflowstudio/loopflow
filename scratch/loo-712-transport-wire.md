# LOO-712: Session transport speaks Rex's shape

Jack Heart asked (2026-11-02) for the owner (`lf session serve`) and its viewers
to adopt the parts of Rex's protocol worth copying. Status: accepted design;
core stream implemented, successor integration incomplete (reviewed 2026-11-02).
Integration and landing follow LOO-705, then LOO-708/LOO-709.
Evidence: `scratch/research/rex-architecture.md` in the LOO-672 checkout
(read 2026-11-02).

## Contract

The [transport README](../rust/loopflow/src/transport/README.md#viewer-stream)
owns the wire description; [wire.rs](../rust/loopflow/src/transport/wire.rs)
defines requests, replies and events. Binary framing stays independent of the
snapshot format (LOO-710). A fresh viewer gets snapshot/pipe tail before live
output; a native client also supplies `return {detach}`. Attach with `take`
uses the broadcast resize/owner events rather than duplicating them.

Nonce APC events share stream 1 and its byte count, including in pipe mode;
sequence boundaries protect terminal parsing. Replies carry the last queued
event's sequence, using `--watch`'s request `id` / reply `answers` shape.
The bounded spool survives EOF for 60 s; control and size/color ownership
release immediately. Resume asks for those claims again; an unavailable byte
requires a fresh snapshot. The CLI retains LOO-618's leave-on-control-loss
behavior until the successor integration below.

LOOPFLOW.md adds the missing agent guidance: the person may be viewing another
Session, and never `lf resume` your own. Assumptions and reasons remain in
[questions.md](questions.md).

## Delete — do not maintain

Done (2026-11-02): the JSON-lines `Request`/`Response` enums, `Viewer::queue`,
`Output::attach`, and the JSON-lines test fixtures are gone; binary framing and
events replaced them.

Compression removes the parallel per-viewer result vector and the fixture's
second incremental frame buffer. Retention handles each request result directly;
blocking fixture reads use `read_exact`, which also exposes truncated frames.
Wire tags carry their byte discriminants, event serialization consumes its
already-owned value, and the CLI tracks only whether its connection may type.

Remaining integration deletions: `Event::Control.successor: Option<PathBuf>`
and the `let _ = successor` arm in `transport::attach`; replace them with
LOO-705's full command and its existing `follow` behavior. After integration,
LOO-705's `Response::Moved` and JSON-lines fixtures also disappear into the
single event path. Preserve its owner retirement, terminal restoration,
same-Session resume and successor exit/error behavior.

Preserve: every existing test's behavior (input fencing, Busy, automatic
attach after disconnect, pipe EOF, slow observer disconnect, native pending
request, silent owner), OSC 7501 reports, snapshot pending-prefix carry.

## Done when

Each line names the `transport_tests` case that proves it (all passing at compress).

- [x] Two viewers both start with the snapshot and both see live output and input
  — `every_viewer_starts_with_the_snapshot_and_shares_live_output`.
- [x] Resume by byte count continues exactly; spool exhaustion refuses; fresh attach
  repaints — `a_dropped_viewer_resumes_from_its_byte_count_without_a_snapshot`,
  `a_resume_past_the_spool_is_refused_and_a_fresh_attach_repaints`.
- [x] Resize reaches every viewer as a nonce APC; a program printing the same bytes
  does not — `resize_reaches_every_viewer_as_its_own_event_and_program_bytes_do_not`.
- [x] `request` behind `take` is granted on leave; `take` loser gets `owner held:false`
  — `ownership_queues_behind_a_taker_and_tells_the_loser_who_took_it`.
- [x] 1 MB burst → wire within 10% — `a_megabyte_burst_costs_within_ten_percent_on_the_wire`.
- [x] `transport_tests` and `golden_prompt` pass; `transport/README.md` documents the stream.
- [ ] Owner retirement carries the full successor command in `control`; the
  viewer restores its terminal and executes that command, preserving its result.
- [ ] `session_lifecycle_tests` and `session_launch_contract` pass — gate runs them.

## Remaining

- **Land order** (Jack Heart, 2026-11-02, via the Task comment): LOO-705 first,
  then LOO-708/LOO-709, then this Task. None of the three is on origin/main as of
  2026-11-02 14:40; the branch is otherwise even with main.
- **LOO-705 successor correction (2026-11-02).** Task status reports PR #1799
  open at `demo`, not armed for landing. Its source at
  `c788a6ea32e93f180f827441eba103ad53551886` uses `Option<Vec<String>>`:
  `[entry, "session", "resume", session_id]`. This branch's `Option<PathBuf>`
  placeholder cannot carry that contract. After integrating the dependencies,
  the lifetime owner sends this complete command through `Event::Control`,
  and the viewer restores its terminal and runs LOO-705's `follow(command)`.
  Replaying viewer argv is invalid: LOO-708 changes it to attach to the old
  owner. This corrects the earlier plan's invocation-replay wording and follows
  Jack Heart's supplied steer. Dependent implementation is pending integration;
  no additional retirement policy or successor runner is needed.
- **LOO-708 coordination.** Both Tasks edit `transport.rs`; this one owns the
  wire format, LOO-708 which processes exist. Expect a conflict-bearing rebase.
- `session_lifecycle_tests` and `session_launch_contract` are gate's.

Check (2026-11-02): `uv run python scripts/test_network.py cargo test -p loopflow` with `--lib transport::` (18 passed; linker unwind-size warning) and `--test transport_tests` (26 passed), `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` pass; `lf context` is within token/byte limits; successor integration and lifecycle/launch acceptance remain for integration/gate.
