# Conversations opened into a host pane start, and a failed start can be opened again

Incident and 5 whys: `scratch/incident-loo-903-open.md`. One PR from `jack-heart/session-opening`, no Task
attached. Base is origin/main a0548bfe28; nothing to rebase yet.

## Lands in this PR as it is

- `open/conversation.rs`: pane command carries `PATH` = caller's PATH plus Desktop tool directories
  (`server::supervised_path`).
- `native_session.rs`, `os_process.rs`: a launch whose provider CLI is not on the child's PATH is refused before
  the thread is recorded.
- `ops/human_session.rs`: `lf session new` on a closed conversation whose thread has no transcript starts over
  from its first prompt.
- `bin/lf.rs`, `open/conversation.rs`: failed pane opens log to `logs/lf.log`.
- Two new `transport_tests`.

## Change before it lands (implement step; all small)

1. **The new unit test holds the env lock.** `conversation_resolves_commands_on_the_callers_path_not_the_hosts`
   reads the process `PATH` twice (inside `conversation_path`, then for the assert) in the `loopflow` lib test
   binary, where `run.rs`, `replay.rs` and `install/published.rs` tests `set_var("PATH", …)` under
   `journal::test_env_lock()`. Under `cargo test -p loopflow --lib` (one process) it can read two different
   values. Take the same lock. This PR's incident note already lists one order-dependent test; it should not add
   another.
2. **Check that `LF_OPEN_RECEIPT` does not reach the provider child; fix if it does.** Not verified.
   `transport::start_held` builds the child's environment from the requester's `std::env::vars_os()`, and
   `acknowledge_open` removes the variable from the viewer only on attach, which is later. If the child inherits
   it, every failing `lf` command an agent runs inside a conversation that `lf open` cold-started writes
   "conversation opened by lf open failed before attaching" to `lf.log`, which is false. `ps eww` showed no live
   process holding the variable, but cold opens into Rex fail on the installed lf, so that says nothing. If it
   leaks: remove it from the child's environment where the held terminal's env is built. If it does not: nothing.
3. **The new-thread message says what happened.** `human_session.rs:1327` prints "Started a new … thread for …,
   seeded with what the {from} thread knew." also when `thread_unwritten` made it start from the first prompt.
   That case includes a real conversation whose transcript is gone (57 of 122 recent Claude Sessions here had a
   thread and no transcript; at least one had run turns). Say it started over from the first prompt because no
   {from} transcript was found.
4. **The Rex fixture's server gets launchd's PATH** (`rust/loopflow-execution/tests/support/rex.rs:63`: drop the
   stand-in provider's directory from the server's PATH, keep it on the `lf open` caller's), and
   `scripts/testing.md` lists `cargo test -p loopflow --test transport_tests rex_open_starts_a_fresh -- --ignored`
   beside the `ssh_shells` line for changes under `open/`. Then the one test that opens into a real Rex fails
   without this change. Run it once here (Rex is installed; the fixture isolates a server, not a second app). If
   it needs more than the PATH lines, leave it to Task 4a34ee and say so in the PR.

Not changing: the pane carries the caller's PATH, an agent's shims and venv included. Known cost; Task 4a34ee
removes the line.

## Checks owed before publishing

- `cargo test -p loopflow --test transport_tests` and `cargo test -p loopflow --lib open::conversation::` after
  1–3; clippy.
- `rex_open_starts_a_fresh … --ignored` after 4.
- `lf open --task <throwaway>` into Rex with the conversation stopped, on an installed build: not run; needs
  `lf install`. Gate or a person. LOO-980 is not a check for this PR.

## Tasks filed (backlog, not run, standalone)

| Task | Outcome | From |
| --- | --- | --- |
| 4a34ee | A provider starts from wherever lf is started, with no PATH repair per entry point | proposal 1 |
| 26cd2a | A launch that ends before the provider's first turn leaves a conversation that opens again | proposal 2 |
| e4df95 | `lf open` says why a pane's conversation failed | proposal 4 |
| 16d016 | Rerunning a pane's command never moves a conversation to another harness | proposal 5 |
| f557e4 | Two Session tests pass whatever runs beside them | the two order/load-dependent tests |

Each brief names the open PRs on its files. The heavy overlaps: #1978 (LOO-923, moves harness code to a sibling
crate) and #1967 (LOO-958, session verbs) both touch `native_session.rs`, `human_session.rs` and `claude.rs`;
4a34ee and 26cd2a should start after those merge or on top of them. This PR conflicts with the same two
wherever they land first; #1972, #1973 and #1976 also touch its files.

## Waits

- 13 `com.loopflow.server.<hash>` launchd jobs seen in `launchctl list` on 2026-11-18: LOO-898 ("Make test runs
  leave no launchd jobs or servers behind") and LOO-780 already own this. No new Task. Which fixtures made
  these 13 was not checked.
- `lf session new` vs `lf resume` on a conversation whose transcript the provider deleted (fourth item in
  `questions.md`): ships as the diff has it; 26cd2a revisits when the record moves.
- Headless usage-limit fallback and PR copy using the pr-message skill's agent: LOO-983.
- Other terminal variables in a host pane (`SSH_AUTH_SOCK`, locale), Herdr and cmux servers: named in 4a34ee,
  unexplored.
