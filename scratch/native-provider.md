# LOO-820: Bare launches run the agent directly in the terminal

## Direction, November 11, 2026

Jack Heart: "i dont think this task really needs anything rex specfiic, its
about just taking pty ownership away from lf for raw skills"

This Task is `docs/dev/server-first.md`'s "Bare launches go direct" row: a
bare `lf <skill>` compiles prompt and context, starts the agent as a
foreground child of the calling terminal, waits, and reads the transcript at
exit. No owner, no relay, no LF PTY, whatever the terminal is. Managed
launches are LOO-824's holder ("managed should = held"); turn hooks of bare
interactive launches go with the owner (LOO-821, "dont really care about
intermediate states as long as we remove the owner eventually"); exit-time
usage/completion rows come from LOO-822's transcript read; "with bare lf,
noting holds its at all" (LOO-823). Jack Heart: "repo and wave sessions
should be managed", so bare-direct covers only launches that are not Task,
Wave or repository conversations.

The Rex-owner exploration (probe, receipts contract, failure matrix) is
retired; it reads at `fdb457e38:scratch/native-provider.md`, with
`scratch/probe_rex.py`, `rex-probe-receipts.json` and `explore.html`.

## Built

- `transport::delegate`: a bare interactive terminal launch (not held, not
  `-b`, not piped, not `--json`, no framed `lf ssh` viewer) runs in the
  calling process; the in-process native path already spawns the agent with
  the terminal's stdin/stdout/stderr in lf's process group and waits.
  Headless, JSON and framed launches keep their owner: the first full
  `transport_tests` run showed `-b` runs and Graph steps rely on it to
  outlive their caller (`a_headless_run_outlives_the_detached_limit`,
  `headless_graph_settles_its_original_step_after_creator_teardown`). The
  `docs/sessions.md` "Runs in" row now says so.
- `agent::ForegroundAgent` / `agent::on_interrupt`: while lf waits on a
  foreground agent, the CLI's Ctrl+C/SIGTERM/SIGHUP handler leaves the
  signal to the agent and keeps waiting to record the exit. Before, the
  first Ctrl+C killed lf and returned the shell while the agent still read
  the terminal. A SIGTERM aimed at lf alone is ignored until the agent exits.
- `SqliteStore::runs_in_its_terminal`: interactive, no managed WorkSource, no
  owner or held terminal. `resume_session_attachment` then attaches the new
  launch beside a still-running agent instead of refusing ("attached
  LfProcess; connect to it"); the first agent's row stays open and its lf's
  exit writes become best-effort attachment-lost gaps.
  `human_session` resume skips `stop_session_agent` for such a Session.
- `open_without_replacing` no longer bails when no transport starts; it
  re-locks and opens in this terminal.

## Remaining

1. After LOO-821 (#1879) lands and LOO-824 syncs onto it: remove the turn and
   tool hooks from bare interactive launches; keep start and compact.
2. Pi: launch bare interactive Pi with `--session-id <minted id>` (probe in
   `questions.md`) once the turn hook that reports `sessionFile` is gone;
   the thread becomes the id, resolved to `*_<id>.jsonl` by LOO-822's reader.
3. The repository conversation records no WorkSource, so a CLI
   `lf resume <repo session>` reads as bare (LOO-824 concept review, open).

## Delete — do not maintain

- Bare use of `Host::Owner(start(..))` in `transport::delegate` (done).
- `ExitRecord`/`DropExitRecord`/`hand_exit_record` stay: held terminals use
  them through `transport/hosted.rs`; bare launches no longer reach them.
- Deleted from `transport_tests.rs`, bare-owner behavior by design:
  `codex_original_request_channel_and_turn_survive_all_viewers`,
  `nested_cli_execution_survives_its_calling_provider_and_viewer`,
  `prepared_start_and_take_back_keep_one_thread_with_or_without_a_session`.
  Adapted: `first_native_launch_keeps_only_valid_desktop_terminal_placement`
  (asserts no owner), `sessions_keep_serving_when_their_lf_executable_goes`
  (resume reopens in the terminal, no new owner).

## Open

With turn hooks gone, a steer to a bare Session arrives only at its next start
or resume, or at compaction. Should `lf session steer` / `lf task steer` say
so at send time, or skip bare Sessions?

## Checks

- `cargo build -p loopflow --bin lf` — clean; implement.
- `cargo test -p loopflow --lib session_record::capture_tests::resume` — 3 passed; implement.
- `cargo test -p loopflow --test transport_tests` — 15 failed against the first cut; after keeping owners for headless/JSON and the test changes above, the 15 rerun: 12 pass, 3 deleted; full binary rerun left to gate.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt` — clean; implement.
