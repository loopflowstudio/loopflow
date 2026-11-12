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

## Stack

main ← LOO-823 (#1877) ← LOO-824 (#1887) ← this branch. LOO-821 (#1879)
merged November 11 and is in this branch. LOO-823's launch replaced this
branch's first cut (`ForegroundAgent`, `on_interrupt` are gone): a bare
interactive terminal launch (`transport::delegate` declines to delegate)
runs in the calling process and
`os_process::foreground::hand_to` makes the provider the terminal's
foreground process-group leader. Headless, JSON and framed launches keep
their owner: `-b` runs and Graph steps rely on it to outlive their caller
(`a_headless_run_outlives_the_detached_limit`,
`headless_graph_settles_its_original_step_after_creator_teardown`).

## Built

- `SqliteStore::is_bare` / `runs_in_its_terminal`: interactive, no managed
  WorkSource, no owner or held terminal. `resume_session_attachment` attaches
  a new launch beside a still-running bare agent instead of refusing; the
  first agent's row stays open and its exit writes become best-effort
  attachment-lost gaps. `human_session` resume skips `stop_session_agent`
  for such a Session.
- `open_without_replacing` re-locks and opens in this terminal when no
  transport starts, instead of bailing.
- `session::Holder { Caller, Owner, Server }` replaces both `managed`
  fields. `Holder::of(work, interactive)` is decided in `run.rs` and
  `resume_session_with_env`. `ContextDelivery::hook_settings(holder)` gives
  `Caller` only the start and compact hooks; `claude_args`, `codex_hooks`,
  `pi_args`, `add_opencode_context`, `native_args` and `steer_environment`
  read `holder`. Bare interactive launches reserve no steer capability; bare
  OpenCode gets no turn hooks. Differs from the planned
  `{ Terminal, Server(PrimaryScope), Run }` (reversible, `questions.md`):
  `Run` folded into `Caller`/`Server`, since a headless run differs only in
  lifetime, which `batch` carries; `Owner` stays while bare Desktop-shell
  and framed viewers keep owners; `Server` carries no scope because a
  `--task` launch has no primary Session to name.
- One decider (e01693a21): `transport::delegate` chooses holder, owner or
  this terminal from `managed || !is_bare(row)`. Task run steps carry
  `--task`, so a launch's `WorkSource` decides its Holder. Deleted:
  `Cli::names_work`, `resumes_work`, `journal::task_run_step`,
  `bare_conversation`, `opens_bare`; `runs_in_terminal` is private to
  transport. Bare Graphs at a terminal now run there (item 5).
- Bare Pi (f69514edf): `--session-id <minted id>`; resume and the
  transcript reader resolve `*_<id>.jsonl` under `PI_CODING_AGENT_DIR`
  (default `~/.pi/agent`). A minted id with no file (no reply yet) resumes
  fresh on the same `--session-id` (`pi::resume_args`); a held Pi's
  recorded path still refuses when missing.
- Bare exits (QA follow-up): a failed bare provider returns its own status
  (`CommandExit`, 128+signal) with "<program> exited with …"; the "another
  client" hint is for held launches only. A signal to lf while its provider
  leads the terminal (`foreground::end_leader`, called first in
  `exit_on_interrupt`) SIGTERMs the provider group (SIGKILL after 5 s) and
  leaves lf's own return to record the exit; the handler exits 130 only if
  that return hangs 30 s.
- Bare Codex (`Holder::Caller`): `codex [OPTIONS] -- <prompt>` /
  `codex resume -- <id> <prompt>`, options from
  `agent::codex_terminal_options`; no app-server, `open_thread`,
  `--remote` or `__codex-observe`. The start hook writes `session_id` to
  `<delivery>.thread` (created only for this launch); lf records it at
  exit through `write_provider_session`.
- Item 4: server-first's "Every view types" row now names Desktop's relay
  handshake among what it removes.

## Remaining

- Bare Codex has no end-to-end fixture of the hook → thread → `lf resume`
  path; unit tests cover the argv and Pi resolution only. Codex 0.167.1's
  hook input has `session_id` (strings in the binary), not run live.
- Item 5 by construction: `transport::delegate` sends any bare execution at a
  terminal direct, Graphs included; a bare Graph's headless steps run `-b`
  and keep owners. Not exercised by a test.
- Owner-held bare Codex (Desktop shell) still uses the app-server; it goes
  direct when the "Every view types" row removes the Desktop handshake.
- A bare Codex launch with no `conversation_context` installs no hooks, so
  it records no thread; reading the rollout at exit would cover it.

## One exit status for bare launches (concept review 4), built

"for a bare session, it jsut does whatever claude or codex would do" covers
the exit status. `SessionCommandOutcome::bare_exit_code` is the one decider:
for `Holder::Caller` the fresh Claude path (`run_interactive`), the fresh
Codex/OpenCode path (`run_native_agent`) and resume (`result`) all exit with
`transport::provider_exit_code`, so a hung-up provider gives the shell 129.
`interrupted` decides only the "completed" record and whether lf prints
"<program> exited with …"; `run.rs` returns a bare terminal launch's code
without its own message. A mover taking a bare conversation still ends that
launch cleanly (lf's doing, not the provider's). Held launches and headless
`-b` runs keep today's rules.
Check: `session_cli_tests::a_hung_up_bare_provider_exits_lf_with_its_own_status`
(fresh and resume in a PTY, 129; 0 before the change), `--lib
lf::commands::util::`, the bare-launch session_cli_tests, clippy, fmt: pass.

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
  them through `transport.rs`; bare launches no longer reach them.
- Deleted from `transport_tests.rs`, bare-owner behavior by design:
  `codex_original_request_channel_and_turn_survive_all_viewers`,
  `nested_cli_execution_survives_its_calling_provider_and_viewer`,
  `prepared_start_and_take_back_keep_one_thread_with_or_without_a_session`.
  Adapted: `first_native_launch_keeps_only_valid_desktop_terminal_placement`
  (asserts no owner), `sessions_keep_serving_when_their_lf_executable_goes`
  (resume reopens in the terminal, no new owner).

## Concept review leftovers, built

- `SqliteStore::is_work(named_work, session)` beside `is_bare`; `transport::works`
  and `resume_session_with_env` call it. `run.rs` keeps
  `work.source.is_managed()`: a fresh launch has no row, so its command's
  WorkSource is the whole answer and opening a store would only return it.
- The threaded `managed: bool` is `named_work` through `human_session`,
  `commands::session`, `util::resume_session*`, `run::resume` and `transport`.
- The hook spec key is `turns` (`harness/context.rs`, `context_opencode.js`,
  `context_pi.mjs`).

## Open

Repository and Wave primaries record `WorkSource::Declared` since LOO-824's
`196faad43`, so they stay managed. Primaries recorded before it carry no
WorkSource and resume bare in the terminal; nothing migrates them.

With turn hooks gone, a steer to a bare Session arrives only at its next start
or resume, or at compaction. Should `lf session steer` / `lf task steer` say
so at send time, or skip bare Sessions? Jack Heart leaned toward "remove lf
session steer and only allow lf task steer" (`questions.md`), which leaves
Wave and repository conversations without a steer command. That is its own
Task; this PR ships with today's delivery (next start, resume or
compaction), which loses nothing.

## Checks

- `cargo build -p loopflow --bin lf` — clean; implement.
- `cargo test -p loopflow --lib session_record::capture_tests::resume` — 3 passed; implement.
- `cargo test -p loopflow --test transport_tests` — 15 failed against the first cut; after keeping owners for headless/JSON and the test changes above, the 15 rerun: 12 pass, 3 deleted; full binary rerun left to gate.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt` — clean; implement.
- realign: no tests; plan repairs only.
- realign 2: no tests; plan repaired against code (turn hooks gone for `Caller`, LOO-823 foreground launch synced, #1879 merged).
- compress 2: `opens_bare` and `runs_in_its_terminal` share `SqliteStore::is_bare` (a pre-LOO-824 primary is no longer "beside"); probes take `Holder`. `cargo test --lib` resume + `harness::context::` + `ops::human_session::` — 57 passed; clippy clean.
- compress: `cargo build -p loopflow --bin lf` — clean; `cargo test -p loopflow --lib session_record::capture_tests::resume` — 3 passed.
- Holder: `cargo test --lib` `harness::context::` 11, `context_block::` 12, `harness::agent_process::` 19 (fixture now a Task's Session: a bare one resumes beside its stale attachment), `session_record::capture_tests::` 26, `ops::human_session::` 43 — pass; `--test integration session_steers::` 9 pass; `--test session_launch_contract` 29/30 twice, a different fixture each time (server-start timeout, pid count) at load 25, each passes alone — gate reruns; clippy, fmt, `documented_commands::` clean.
- implement 3 (one decider, Pi, Codex): `cargo test --lib` `ops::human_session::` + `harness::context::` + `lf::commands::graph::` + resume — 63 passed; `pi_` — 32 passed (new `bare_pi_names_its_thread_and_resumes_it_by_that_id`); `agent::` + `harness::context::` + `harness::codex` + `lf::commands::context::` + `lf::commands::util::` — 112 passed (new `bare_codex_runs_plain_codex_and_a_held_one_keeps_its_app_server`); `--test session_launch_contract` 29/30, the failing `a_running_claude_named_onto_another_login…` passes alone (pid-count flake seen before); `documented_commands::` 4 passed; clippy, fmt clean. Gate: `transport_tests`, `binary(lf)`, `lf::tests::`, `session_cli_tests::`.
- compress 3: `configure_hooks` returns its delivery path (no env lookup for the Codex `.thread` file; the hook's `launched` test dropped, the file's existence is the request); `delegate` returns early on `cfg!(test)`, no `#[cfg(test)]` arm or `dead_code` allows. clippy clean; `--lib` `harness::context::` 11, `lf::commands::context::` 3, `agent::session::` 9, `bare_pi_` 1, `ops::human_session::` 43, `context_block::` 12 — pass.
- implement 5: docs only (sessions.md direct list and table, troubleshooting `lf session serve`), no tests; `documented_commands::` 4 passed. Gate: `transport_tests`, `binary(lf)`, `lf::tests::`, `session_cli_tests::`.
- realign 3: no tests; grep confirms `bare_conversation`, `opens_bare`, `names_work`, `resumes_work`, `task_run_step` are gone and `runs_in_terminal` is private to transport; `simplification-opportunities.md` trimmed to what remains (Opportunity 1 → LOO-824, Server-only observers).
- concept review: no tests; design-only (one Work predicate, `named_work`, hook spec `turns`).
- implement 4 (those three): `cargo build --bin lf` clean; `--lib` `harness::context::` + `ops::human_session::` + `session_record::capture_tests::resume` + `bare_pi_` — 58 passed; clippy, fmt clean.
- compress 4: the direction's three items were already in `8a516adad2`; this pass only imports `Holder` in `agent_process` tests (18+/40−). clippy clean; `--lib harness::agent_process::` 19 passed.
- concept review 2: no tests; design-only (two `docs/sessions.md` rows, one troubleshooting line).
- realign 4: no tests; grep confirms one `is_work` (transport, `util::resume_session_with_env`), no `managed` flag in `human_session`/`commands::session`/`util`, no `"managed"` hook spec key.
- realign 5: no tests; the three docs items were already in `4f9b7796ff` (sessions.md list and table, troubleshooting `lf session serve`), matching `transport::runs_in_terminal` (`LF_SERVER_STATUS`, `-b`, no tty keep an owner).
- concept review 3: no tests; no change. `Holder::of` reads where `transport::delegate` put the launch (inside an owner → `Owner`, Work → `Server`, else `Caller`), so one decider still holds; the steer question stays its own Task.
- qa: live PTY walkthrough in `scratch/qa.html`. Fixed: a bare provider ended by SIGINT/SIGHUP/SIGTERM is interrupted, not "another client still owns this session"; bare Codex resume no longer re-sends its first turn (`bare_codex_resume_does_not_resend_its_first_turn`). `--lib` `lf::commands::util::` + `agent::session::` + `ops::human_session::` — 64 passed; clippy, fmt clean. Open from QA: a closed terminal leaves the agent row unfinished; nonzero exits still print the "another client" hint.
- implement 6 (QA's three): `--lib lf::commands::util::` 13 passed (new `a_bare_launch_returns_its_providers_own_status`; the SIGTERM test now expects 143); `--test integration session_cli_tests::a_closed_terminal_finishes_a_bare_launchs_agent` passed, and failed (agent row open) with `end_leader` disabled; `--lib pi_` 32 passed; clippy, fmt clean. Gate: `transport_tests`, `binary(lf)`, `lf::tests::`, `session_cli_tests::`.
- compress 5: the bare exit code reuses `transport::provider_exit_code`; `Foreground` clears the leader on drop (the `Leading` guard is gone); Pi `resume_args` reads one `conversation_file`. `--lib lf::commands::util:: pi_` 44 passed; `session_cli_tests::a_closed_terminal_finishes_a_bare_launchs_agent` passed; clippy, fmt clean.
- realign 6: no tests; QA's three items are in `2d972e033d`/`79b456f453` (grep: the "another client" hint only on the held arm of `util.rs`, `end_leader`, both new tests). The parent's merge (holder moved under `server/`) changes nothing here.
- concept review 4: no tests; design-only (one bare exit status, above). The steer question and pre-LOO-824 primaries stay as written in Open.
