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
`scratch/probe_rex.py` and `scratch/rex-probe-receipts.json` at the same
commit. LOO-824's scratch (`holder.md`, `explore.html`, `concept-review.html`,
`qa.html`) is on its branch; `holder.md` reads at `d7901ea6e:scratch/holder.md`.

## LOO-823 ships the launch (November 11)

Jack Heart, on LOO-823: "if we already have it working we should just kee it
here and hsip it here". LOO-823 lands bare interactive `lf` at a terminal with
no owner and the provider as foreground process-group leader. When syncing
onto it, its launch replaces this branch's `transport::delegate` /
`ForegroundAgent` below; keep only what it lacks (`runs_in_its_terminal`
resume beside a running agent, `open_without_replacing`). Item 3 under
Remaining is LOO-823's.

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

## Holder, built November 11

`session::Holder { Caller, Owner, Server }` replaces `AgentConfig::managed` and
`SessionCommand::managed`. `Holder::of(work, interactive)` decides it where the
launch is configured: `run.rs` (`names_work`, `!batch`) and
`resume_session_with_env` (`names_work` or the row's `WorkSource::is_managed`).
`hook_settings(holder)`, `claude_args`, `codex_hooks`, `pi_args`,
`add_opencode_context`, `native_args` and `steer_environment` read it. Fixed on
the way: bare interactive launches no longer reserve a steer capability, bare
OpenCode no longer gets turn hooks (`native || managed`), Codex and
`native_args` no longer read `transport::current()` themselves.
Deleted: `require_server_for`, `session_environment`'s `native`.

Differs from the plan's `{ Terminal, Server(PrimaryScope), Run }`
(reversible, `questions.md`): `Run` folded into `Caller`/`Server`, since a
headless run's only difference is lifetime, which `batch` already carries in
delegation; `Owner` stays while bare Desktop-shell and framed viewers keep
owners; `Server` carries no scope because a `--task` launch has no primary
Session to name.

## Remaining

1. After LOO-821 (#1879) lands and LOO-824 syncs onto it: remove the turn and
   tool hooks from bare interactive launches; keep start and compact.
2. Pi: launch bare interactive Pi with `--session-id <minted id>` (probe in
   `questions.md`) once the turn hook that reports `sessionFile` is gone;
   the thread becomes the id, resolved to `*_<id>.jsonl` by LOO-822's reader.

3. The provider becomes the terminal's foreground process-group leader (own
   group + `tcsetpgrp`, or exec), so Rex, which reads the `tcgetpgrp` leader,
   shows Claude/Codex. Today `delegate` keeps the agent in lf's group, so lf
   is the leader. No argv0/symlink naming. One launch path for every harness.
   LOO-823's prototype: `59fdcdaee0` (`os_process/foreground.rs`,
   `run_interactive`) on `jack/loo-823-find-what-leaks-lf-processes-and-fix-the-tests`.
   With its own group the agent gets Ctrl+C/SIGHUP directly; recheck
   `ForegroundAgent`'s signal handling against that.
4. Desktop pane opening a bare Session still waits for the owner relay
   handshake (`LF_SERVER_STATUS`, "Opening… Input paused"). It stays "for
   now ... until server-first's Desktop pieces replace the handshake"; name
   the `docs/dev/server-first.md` row that removes it.

5. Bare Graph targets (`lf code`, `lf graph …`) still keep an owner. Call,
   reversible: an interactive step of a bare Graph goes direct like a bare
   skill (closing the terminal ends it); headless steps keep their owner,
   which `headless_graph_settles_its_original_step_after_creator_teardown`
   relies on.
6. Bare Codex drops `__codex-observe`. Jack Heart, asked whether bare Codex
   should keep starting it: "no they shoudlnt". It sits in its own
   setsid session, outlives the terminal and writes turns live, which is
   neither "behaves like the underlying harness" nor bare telemetry, which
   comes from LOO-822's exit-time transcript read. Bare Codex launches like Claude:
   no lf app-server, no `open_thread`, no observer; lf runs `codex [OPTIONS]
   <prompt>` (resume: `codex resume <id> <prompt>`, both on 0.167.1), so the
   first turn is argv. Jack Heart: "why cant we do something more similar to
   what we do with claude or any other provider". Gap vs Claude/Pi: Codex has
   no `--session-id`, so the thread id comes from the kept start context
   hook's `session_id` (unverified) or the rollout at exit.
   Managed Codex follows
   LOO-826's server registration.

`scratch/simplification-opportunities.md` (read at LOO-823 `8e1c71d6d5`)
assigns LOO-820 Opportunity 2's bare half. Still to delete: `bare_conversation`,
`opens_bare`, `runs_in_terminal` as decider, the exit-record handoff for bare
(context hooks only for bare and no bare steer are done, via `Holder`). Asked whether `Holder` and bare Codex fold
into LOO-820 or become separate Tasks, Jack Heart: "fold it in here". So
LOO-820 also builds `Holder { Terminal, Server(PrimaryScope), Run }`, decided
once where the Session id is known, and deletes `names_work`, `resumes_work`,
`journal::task_run_step` and both `managed` fields; harnesses read their hook
set from `hook_settings(holder)`. Calls on the doc's open choices (reversible):
headless `--task` runs get every hook; a `Terminal` launch on non-tty stdin
runs on the caller's pipes, no owner PTY.

## LOO-823 review, moved here (Jack Heart, November 11, "let 820 own it")

- "for a bare session, it jsut does whatever claude or codex would do"
- "agian, just behaves like the underlying harness woudl": closing the
  terminal ends it, mid-turn included; Ctrl+] is an ordinary key.
- "if its a managed task session, then neither ends the session"
- Managed is only a scope's ongoing conversation (Task/Wave/repository); a
  skill typed in a Task checkout is bare ("2a").
- "shoudl be independent of provider too"

## Delete — do not maintain

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

Repository and Wave primaries record `WorkSource::Declared` since LOO-824's
`196faad43`, so they stay managed. Primaries recorded before it carry no
WorkSource and resume bare in the terminal; nothing migrates them.

With turn hooks gone, a steer to a bare Session arrives only at its next start
or resume, or at compaction. Should `lf session steer` / `lf task steer` say
so at send time, or skip bare Sessions?

## Checks

- `cargo build -p loopflow --bin lf` — clean; implement.
- `cargo test -p loopflow --lib session_record::capture_tests::resume` — 3 passed; implement.
- `cargo test -p loopflow --test transport_tests` — 15 failed against the first cut; after keeping owners for headless/JSON and the test changes above, the 15 rerun: 12 pass, 3 deleted; full binary rerun left to gate.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt` — clean; implement.
- realign: no tests; plan repairs only.
- compress 2: `opens_bare` and `runs_in_its_terminal` share `SqliteStore::is_bare` (a pre-LOO-824 primary is no longer "beside"); probes take `Holder`. `cargo test --lib` resume + `harness::context::` + `ops::human_session::` — 57 passed; clippy clean.
- compress: `cargo build -p loopflow --bin lf` — clean; `cargo test -p loopflow --lib session_record::capture_tests::resume` — 3 passed.
- Holder: `cargo test --lib` `harness::context::` 11, `context_block::` 12, `harness::agent_process::` 19 (fixture now a Task's Session: a bare one resumes beside its stale attachment), `session_record::capture_tests::` 26, `ops::human_session::` 43 — pass; `--test integration session_steers::` 9 pass; `--test session_launch_contract` 29/30 twice, a different fixture each time (server-start timeout, pid count) at load 25, each passes alone — gate reruns; clippy, fmt, `documented_commands::` clean.
