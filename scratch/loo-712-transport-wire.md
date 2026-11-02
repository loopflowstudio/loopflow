# LOO-712: Session transport speaks Rex's shape

Jack Heart asked (2026-11-02) for the owner (`lf session serve`) and its viewers
to adopt the parts of Rex's protocol worth copying. Status: accepted design;
core stream, successor handoff and concept-review color ownership implemented
(2026-11-02). Local gate is checked; dependency integration and live acceptance remain.
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
- [x] Core `transport_tests` and `golden_prompt` passed before the color correction;
  the five focused checks below cover the latest implementation. The README
  documents the stream; gate still owns the full affected suites on the integrated tree.
- [x] Owner retirement carries the full successor command in `control`; the
  viewer restores its terminal and executes that command, preserving its result
  — `sessions_end_when_their_lf_executable_goes` and
  `successor_handoff_restores_the_terminal_and_preserves_command_results`.
  The latter covers terminal attributes and alternate-screen restoration,
  exact argv, success/nonzero exit, signal, missing/empty command and no successor.
- [x] `session_lifecycle_tests` and `session_launch_contract` passed in the affected gate run; transport checks were refreshed after the immediate-resume correction.
- [ ] Two viewers of one running Claude conversation receive the snapshot and
  live output, including controlling-viewer input — demo/review evidence remains.

## Remaining

- **Land order** (Jack Heart, 2026-11-02, via the Task comment): LOO-705 first,
  then LOO-708/LOO-709, then this Task. Shared Task reads during realign
  (2026-11-02) show LOO-705 at demo with open PR #1799, LOO-708 running compress,
  LOO-709 idle with its latest Graph stopped at loop-or-next, and LOO-710 running
  implement. The latter three have no PR. A fresh `git fetch origin main`
  still resolves to `4e9ade2d04`; dependency integration remains outstanding.
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
- Dependency integration must refresh the affected acceptance on the combined tree before landing.

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
remain. Realign (2026-11-02) confirms the implementation matches this correction
and records its report-retention lesson in Infrastructure memory. Infrastructure
and its sole immediate child, Release, were read in full; Release's distinction
between source fixtures and installed acceptance still applies. No new product
decision is needed for reconciliation; the unknown-color assumption remains
explicit for demo/review.

Headless concept review after the color correction: **no further design change
and no open product choice**. The normal interaction remains one Session viewed
from two connections: both receive its snapshot and output, one may type, and a
separately selected viewer supplies size and known colors. Losing appearance
ownership alone leaves the viewer attached. The CLI's existing departure on
input-control loss remains separate from Desktop's eventual presentation.

The recovery counterexamples still require these distinctions. If a disconnected
viewer returns after another takes over, restoring its output must not silently
restore its input or appearance claims. If the spool no longer contains its
offset, a fresh snapshot recovers viewing without resubmitting input. If the owner
ends, its in-memory stream cannot recover the conversation; the successor command
addresses the durable Session and provider history instead. No new persistent
viewer record or second successor lifecycle is warranted. Byte resume is a wire
capability exercised by the fixtures; this review does not claim the CLI
automatically reconnects a broken socket.

The current README and LOOPFLOW.md already express the accepted interaction;
no additional usage or agent instruction is proposed. Next proof remains the
integrated dependency checks and live Claude interaction listed above. The review
page is `scratch/concept-review.html`; implementation and Wave memory are
unchanged by this review.

Check (2026-11-02): `git diff --check` and `lf context` pass; prose-only realign reuses the 5 focused transport passes, build, formatting and Clippy recorded at `d1eeb41e1d6cbfc39c543598468382a8ae28cec8:scratch/loo-712-transport-wire.md` (snapshot waits for socket EOF after the earlier leaky pass); earlier core/handoff evidence: `76654874b332ff0874dd125735b15d957b6e00a4:scratch/loo-712-transport-wire.md`. Gate owns affected transport/lifecycle/launch suites and owner-boundary startup/interactive-Graph checks after dependency integration; CI owns Linux/installation proofs, and live Claude acceptance remains demo/review's.

## Gate plan (2026-11-02)

Review comparison: supplied checkout base `4e9ade2d04`, including working edits.
Changed transport owner, lifetime, output and wire modules reach `transport::`,
`transport_tests`, `session_lifecycle_tests`, `session_launch_contract`,
`agent_startup_tests`, `session_cli_tests`, and the retained-terminal interactive
Graph fixture. LOOPFLOW.md and its five prompt goldens reach `golden_prompt`.
The changed-aware runner also selects architecture, inventory, formatting and
Clippy. No Swift, website or Python production paths changed. The installation
boundary requires `declared_agent_can_start_another_tasks_graph`; Linux transport
and Clippy remain CI proofs. Live Claude interaction remains demo/review's.

Gate review found two wire/ownership edge cases: a maximum-size frame sharing a
read with the next frame was rejected by an aggregate-buffer limit, and taking
ownership left a previously queued request eligible for another grant. Focused
regressions cover legal coalesced frames and one-time fulfillment of a request.

Fresh shared Task reads: LOO-705 still has open PR #1799 at demo; LOO-708 is
running implement without a PR; LOO-709 is idle without a PR. This gate checks
the current tree; integration and Jack Heart's landing order remain required.

The first full gate passed 125 tests plus architecture, inventory, fmt and stable
Clippy (`.lf/tmp/gate/run-69292`). Further review found same-pass EOF/resume
could see the old input claim and appearance owner. A paused-owner fixture
confirmed the resume reply had no input token (its initial failure cleanup then
encountered a removed Process row). Release claims and expired streams before
resume; the regression checks restored input and new PTY dimensions. The rerun
narrows to transport modules and fixtures, plus required static checks. Other
Session/launch/prompt suites retain their first-gate evidence.

Check (2026-11-02): `scripts/test.py --reuse-passing --rust-filter "test(transport::) | binary(transport_tests) | binary(session_lifecycle_tests) | binary(session_launch_contract) | binary(golden_prompt) | test(agent_startup_tests::) | test(session_cli_tests::) | test(interactive_graph_keeps_input_and_advances_only_after_each_provider_exits)"` via `uv run python`, LF variables unset, stable Rust 1.99.1 — PASS, 125 tests plus architecture/inventory/fmt/build/Clippy; after the resume correction the same runner with `"test(transport::) | binary(transport_tests)"` passed all 49 tests and static checks (`.lf/tmp/gate/run-60047`); Linux transport/Clippy remain CI-owned, configured Claude interaction remains demo/review-owned.
Check (2026-11-02): `uv run python scripts/test_task_installation.py --test declared_agent_can_start_another_tasks_graph` with inherited LF variables unset — PASS on disposable Linux/Rust 1.99.1, including its two prerequisite store tests; container removed (`.lf/tmp/gate-installation.log`).

Gate disposition: three defects repaired with regressions (coalesced maximum
frames, fulfilled ownership requests, immediate byte-resume claims/size).
The documented stream remains accurate; no extra API or compatibility path was
introduced. All selected local checks pass. This is not integrated dependency
or configured-provider acceptance: preserve the required landing order above,
then test the combined source and complete the live Claude walkthrough. No
current-gate fixture owners or background commands remain; older processes
observed during the exit check were left untouched.
