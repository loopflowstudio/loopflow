# LOO-860 docs (written, describe intended behavior)

- docs/sessions.md "Switch agents mid-conversation": the switch sticks; plain resume reopens on the latest input's provider, its conversation and account.
- docs/config.md: "Existing Sessions resume on the provider their latest input ran on"; stopped-session resume paragraph links the switch section.

# Implementation (done)

- Cause found in the store: f505abc0's capture came from `lf --agent codex session resume` (process 42b65c77). `append_input` captured the row's provider (claude) and published the prepared manifest as claude; `conversation_launch_args` then launched `--agent codex`, and `start_prepared` rewrote only the manifest harness. Captured events are immutable (`preserve_capture_update`), so the provider must be right at capture.
- `append_input` applies `handoff_agent`; `open_waiting_locked` appends a fresh input instead of launching a prepared one when another harness is selected.
- `resolve_session_config` takes the input's thread and account only when the input's manifest harness is the resumed provider.
- Migration `resume_on_latest_input_provider` moves a Session to the only provider that recorded its `provider_thread` (model cleared). Dry run on a copy of ~/.lf/loopflow.db changed exactly 3 rows: f505abc0…, session_b5b0a13d…, session_c9a397ed… → codex.
- `provider_account.rs` `select_route` needed no change: it looks up `provider_session_account(provider, thread)`, which matches once the provider and thread agree.
- Deleted uncalled `retarget_unpublished_capture` (sqlite + async wrapper) and `capture_seq_in`.
- Compress: `new_thread` and `append_input` share `switch_agent` for the provider/model rewrite. `cargo test -p loopflow --lib ops::human_session::`: 51 pass.

Checks: `cargo test -p loopflow --lib` for `ops::human_session::` (51), `agent::session::` (9), `store::migrations::` (132): pass. New regression test fails with the `append_input` fix removed. Gate still owns `documented_commands::`, clippy and the affected-suite run.
