# e46a80 demo: the store never ends a conversation (headless, 2026-11-05)

Experience under review: a Codex or Claude conversation keeps accepting input,
answering and quitting while lf's store fails or is locked, and the record
catches up when the store answers. Design: `scratch/e46a80-keep-idle-conversations.md`.

## What ran

Headless demo step, Jack Heart not present; the agent stood in for the decision.
The real-path demo (`lf -a codex` on the installed build, store broken under it
by a newer floor, type, reply, quit, `lf resume`) needs an installed build, a
provider login and a terminal, so it stays Jack's to run (steps in the design's
"Demo" section). The fixtures that drive the real relay, owner input path and
store stood in:

`cargo test -p loopflow --lib -- harness::codex_connection:: transport::input_retry_tests:: store::sqlite::session_events::tests::a_recovered_gap harness::fact_writer::tests::a_yielded_reading` — 20 passed in 116 s, at `ba6295a0c3`.

Observed in those runs: an idle relay survives a failing store and accepts a
reconnect (`idle_relay_survives_store_failure_and_accepts_a_reconnect`); typed
bytes reach the provider unrecorded on a failed store and within the 1 s budget
on a locked one, then record on recovery; a recovered gap writes one `store-gap`
observation; a refused Codex reading is saved when the store answers.

## Decision

Advance. The preceding pass (fourth concept review) intended no new code and
confirmed the catch-up; its evidence holds on re-run. No repairable gap found.
Open, non-blocking: the `store-gap` observation is not yet rendered; a turn
answered during a gap retains its owner until quit (questions.md).

Next: gate runs the affected suites; Jack runs the real-path demo above on the
installed build after landing.
