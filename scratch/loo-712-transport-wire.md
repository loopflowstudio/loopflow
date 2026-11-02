# LOO-712: Session transport speaks Rex's shape

Jack Heart asked (2026-11-02) for the owner (`lf session serve`) and its viewers
to adopt the parts of Rex's protocol worth copying. Status: accepted design;
core stream, successor handoff and concept-review color ownership implemented
(2026-11-02). Dependency integration, gate and live acceptance remain.
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
behavior when another viewer takes control. An owner retiring its executable
sends the full same-Session resume command; the viewer restores its terminal
and follows that command, preserving its exit or error.

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
Final events share `end_viewers` for retirement, detach and child exit; frame
encoding writes into either the queued or immediate buffer without converting
through a temporary deque. Event placement only borrows retained output.
Request processing now consumes the frame decoder directly, without a second
list of decoded requests. Size and theme both come from one settled-owner lookup;
resize requests retain dimensions only on their viewer, not in a second pending
size value. The test owner-event reader checks the arriving event directly.

Removed (2026-11-02): `Event::Control.successor: Option<PathBuf>`, the ignored
successor arm in `transport::attach`, lifetime's hint-only ending channel, and
the exclusive `Viewer::hints` fixture helper.
The control event carries LOO-705's full command and uses its `follow` behavior.
On dependency integration, absorb LOO-705's `Response::Moved` and JSON-lines
fixtures into this single event path. Preserve its owner retirement policy,
terminal restoration, same-Session resume and successor exit/error behavior.

Removed (concept-review implementation, 2026-11-02): the serve-wide `theme`
cache and the owner-only `Request::Theme` arm that discarded waiting reports.
`Viewer::theme` retains reports through reconnect grace; the settled ownership
selects fresh-stream and broadcast colors on one appearance-event path.

Preserve: every existing test's behavior (input fencing, Busy, automatic
attach after disconnect, pipe EOF, slow observer disconnect, native pending
request, silent owner), OSC 7501 reports, snapshot pending-prefix carry.

## Done when

The wire checks name passing `transport_tests` fixtures. They use a shell,
not a configured Claude conversation; native acceptance remains below.

- [x] Two viewers both start with the snapshot and both see live output and input
  — `every_viewer_starts_with_the_snapshot_and_shares_live_output`.
- [x] Resume by byte count continues exactly; spool exhaustion refuses; fresh attach
  repaints — `a_dropped_viewer_resumes_from_its_byte_count_without_a_snapshot`,
  `a_resume_past_the_spool_is_refused_and_a_fresh_attach_repaints`.
- [x] Resize reaches every viewer as a nonce APC; a program printing the same bytes
  does not — `resize_reaches_every_viewer_as_its_own_event_and_program_bytes_do_not`.
- [x] `request` behind `take` is granted on leave; `take` loser gets `owner held:false`
  — `ownership_queues_behind_a_taker_and_tells_the_loser_who_took_it`.
- [x] Waiting color reports remain inactive; promotion broadcasts them and fresh
  observers receive only the selected owner's known theme. Resume retains its
  colors without applying them until takeover, preserving byte replay and event
  sequence — the ownership and byte-resume fixtures above.
- [x] 1 MB burst → wire within 10% — `a_megabyte_burst_costs_within_ten_percent_on_the_wire`.
- [x] `transport_tests` and `golden_prompt` pass; `transport/README.md` documents the stream.
- [x] Owner retirement carries the full successor command in `control`; the
  viewer restores its terminal and executes that command, preserving its result
  — `sessions_end_when_their_lf_executable_goes` and
  `successor_handoff_restores_the_terminal_and_preserves_command_results`.
  The latter covers terminal attributes and alternate-screen restoration,
  exact argv, success/nonzero exit, signal, missing/empty command and no successor.
- [ ] `session_lifecycle_tests` and `session_launch_contract` pass — gate runs them.
- [ ] Two viewers of one running Claude conversation receive the snapshot and
  live output, including controlling-viewer input — demo/review evidence remains.

## Remaining

- **Land order** (Jack Heart, 2026-11-02, via the Task comment): LOO-705 first,
  then LOO-708/LOO-709, then this Task. On 2026-11-02, shared Task reads still
  show LOO-705 at demo with open PR #1799, LOO-708 running implement and LOO-709
  running concept-review, both without a PR. LOO-710 is running compress without
  a PR. The local origin/main ref remains `4e9ade2d04`; it was not fetched
  during reconciliation and is not fresh remote merge evidence.
- **LOO-705 integration (2026-11-02).** Its source at
  `c788a6ea32e93f180f827441eba103ad53551886` supplies the full
  `[entry, "session", "resume", session_id]` command and `follow` implementation,
  now carried end to end by `Event::Control`. Replaying viewer argv is invalid:
  LOO-708 changes it to attach to the old owner. The current branch still has
  its original removed/replaced retirement policy. Integrating LOO-705 must
  preserve its superseded-install detection and retry behavior, translating its
  ending message to the existing control event; no second successor runner.
  The current `sessions_end_when_their_lf_executable_goes` fixture deliberately
  keeps an owner alive after an installed-entry symlink swap. That assertion
  describes this branch's pre-integration policy; it must adopt LOO-705's
  superseded-install expectation while retaining busy-turn preservation.
- **LOO-708 coordination.** Both Tasks edit `transport.rs`; this one owns the
  wire format, LOO-708 which processes exist. Expect a conflict-bearing rebase.
- **LOO-709/LOO-710 snapshot boundary.** Their November 2 coordination names
  `transport::snapshot::leave` and `last_screen`, with one
  `screens/<session>.ghostsnp` file. The second to land removes the `.vt` path.
  This Task preserves that writer/reader on integration; framing remains opaque
  to the snapshot format. Their snapshot storage work stays in those Tasks.
- `session_lifecycle_tests` and `session_launch_contract` are gate's.

## Concept review (2026-11-02)

Accepted correction implemented: Jack Heart's contract selects size and colors
independently of input control. A waiting viewer's reports remain inactive until
it owns appearance. Disconnect releases its claims immediately but retains its
reported theme with the stream through reconnect grace. Resuming without an
ownership claim preserves the current appearance; taking ownership broadcasts
the resumed viewer's known colors. Unknown colors supply no override; fresh
observers receive only the selected owner's known theme (questions.md).

The ownership fixture now checks queued colors, promotion broadcasts, fresh
observers, and takeover by a viewer without a color report. The byte-resume
fixture retains its exact replay assertion and checks that only takeover restores
the resumed viewer's colors, with gapless event sequence and reply ordering.
No new persistence, claim or Desktop behavior is introduced.

Other reviewed boundaries remain accepted: Session identity and provider history
survive an executable handoff through the full successor command; a stream token
and byte count recover only an in-memory viewer stream. They are different
recovery scopes. Spool exhaustion requires a fresh snapshot, never replay of
input. Snapshot format and owner retirement policy remain with the named
dependencies; the existing integration and live-acceptance obligations above
remain. Wave memory is unchanged by this implementation pass.

Check (2026-11-02): after decoder/appearance reduction, network-isolated `cargo nextest run -p loopflow --test transport_tests -E 'test(ownership_queues_behind_a_taker) | test(a_dropped_viewer_resumes) | test(every_viewer_starts_with_the_snapshot) | test(resize_reaches_every_viewer) | test(native_pending_request_survives_creator_exit)'` — 5 pass (snapshot waits for socket EOF after the earlier leaky pass); build, `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` and `lf context` pass. Earlier core/handoff evidence: `76654874b332ff0874dd125735b15d957b6e00a4:scratch/loo-712-transport-wire.md`. Gate owns affected transport/lifecycle/launch suites and owner-boundary startup/interactive-Graph checks after dependency integration; CI owns Linux/installation proofs, and live Claude acceptance remains demo/review's.
