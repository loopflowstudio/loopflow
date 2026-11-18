# LOO-903's conversation would not open from Rex (2026-11-18)

## Symptom
`lf open --task LOO-903` opened a Rex window and exited before attaching (status 1), twice (~13:00 PST).
The Rex pane printed (read with `rex capture -s 196918`, saved in `rex-pane-loo903.txt`):

    Error: Codex config/read failed: codex CLI not found: codex

## Cause
The Rex server (pid 1927, a launchd job since Nov 17 18:44) has `PATH=/usr/bin:/bin:/usr/sbin:/sbin`.
`lf open` runs `lf session resume …` in a Rex pane with `shell:"none"`, so that lf inherits that PATH, and a
held terminal's child takes its whole environment from the requester (`transport::start_held`). A cold provider
start therefore cannot find `codex` (`~/.local/bin`). `lf server run` already repairs the same launchd PATH for
itself (`server::supervised_path`); the host pane did not. An already-running conversation only attaches, so it
opens fine, which is why most Rex opens work.

Not the cause: today's codex update, the Codex usage limit, Claude Code's environment.

## Reproduction
From any directory, in a PTY, with the conversation not running:

    env -i HOME=$HOME TERM=xterm-256color PATH=/usr/bin:/bin:/usr/sbin:/sbin \
      /Applications/Loopflow.app/Contents/MacOS/lf session resume <session> --agent codex:<model>

Output saved in `open-attempt-2-minimal-path.raw`. Warning: `--agent` moves the conversation to that harness
(see below); reproduce on a throwaway Session.

## What the reproduction disturbed
At 14:09:59 the conversation had been moved to Claude from a Rex shell (`lf resume -a claude:fable`, thread
e2d69fd5, idle since 14:10:26). The 14:11:23 reproduction above carried `--agent codex:gpt-6.1-sol`, took the
conversation from that pane, failed to start Codex, and Desktop then reopened it on a new Codex thread
(01a1f0b7). At 14:13:24 it was moved back with `lf resume 54042b3f… -a claude:fable`: new Claude thread
36106c51, seeded from the log, which answered the seed with a status summary. No typed input was lost; the
conversation has two extra thread forks in its history. Raw output: `restore-claude-move.raw`.

## The Claude case (agent opens another Task in Rex)
Same cause. LOO-980's Session ea3a2511… failed at 14:14:27 in a Rex pane with "'claude' is not installed";
`claude` is in `~/.local/bin`, which the Rex server's PATH lacks. All 28 live `claude` agents sampled at 14:20 had
`~/.local/bin` on their own PATH, so the caller's PATH would have been enough for them; the pane now also gets the
Desktop tool directories (`server::supervised_path`), so a caller with a bare PATH still finds a provider.

## Second defect: the Session could not be opened again
`native_session` recorded the Claude thread id (lf names it, `--session-id`) before spawning, so the failed spawn
left `session_events.provider_thread = fedc5dbe-…` on the captured input. The next `lf resume` ran
`claude --resume fedc5dbe-…` → "No conversation found". `lf session new ea3a2511…` refused as well: "The claude
transcript of Session … was not found. Keep this conversation open and retry after claude saves its first turn."

## Changes (uncommitted, not installed)
- `open/conversation.rs`: the pane command gets `PATH` = caller's PATH plus the Desktop tool directories.
- `harness/native_session.rs`: a launch whose provider CLI is not on PATH is refused before the thread is recorded
  (beside the stopped-server refusal), so the next open starts fresh. `os_process::which_command` resolves on the
  PATH the child will run with.
- `ops/human_session.rs`: `lf session new` on a closed conversation whose recorded thread has no transcript starts
  over from the first prompt instead of refusing. This is the way out for records already stuck (LOO-980).
- `bin/lf.rs`, `open/conversation.rs`: a command `lf open` put in a pane that fails before attaching writes its
  error to `logs/lf.log`; the opener logs the failed receipt.

Tried and dropped: starting fresh whenever a Claude thread has no transcript. It contradicts
`missing_native_history_fails_without_replacing_the_selected_conversation` (a resume never silently replaces a
conversation) and broke 5 launch-contract tests whose stand-in `claude` writes no transcript.

## Checks
- `cargo test -p loopflow --test transport_tests` — 41 passed. New: `a_launch_that_found_no_provider_leaves_nothing_to_resume`
  (before: second launch ran `--resume 673cbe04-…`; after: `--session-id`, and lf.log holds "'claude' is not
  installed"), `a_new_thread_needs_no_log_from_a_conversation_the_provider_never_wrote` (before: no second launch).
- `cargo test -p loopflow --lib open::conversation::` — 4 passed.
- `cargo clippy -p loopflow -p loopflow-execution --all-targets -- -D warnings` — clean.
- `cargo test -p loopflow --test session_launch_contract` — 27 passed, 1 failed:
  `task_conversation_preparation_is_passive_and_connect_uses_shared_launch_settings`, "Holder at … is not answering".
  Fails the same with the provider check disabled; load average was 97. CI.
- `cargo test -p loopflow-execution --lib human_session` — 47 passed, 1 failed:
  `resuming_an_ended_conversation_launches_its_provider_as_the_session`, "Native launch has no admitted invocation".
  Passes alone, fails in the shared-process run with or without this change. CI runs it per process.
- `lf open --task <throwaway>` into Rex with an installed build, conversation stopped — not run; needs `lf install`.
  Gate or the person.
- `lf session new --task LOO-980` after install, then `lf open --task LOO-980` — not run; the installed lf
  (7bb879aff) still refuses, and the branch build was kept off the real store.

## Until the change is installed
Open a stopped conversation with `lf open --task <ID> --here`, `lf resume` in a shell, or Desktop. LOO-980's
Session stays unopenable until then.

# Why it happened (5 whys, 2026-11-18 ~15:00)

Status: open. The change above is uncommitted and not installed, so every cold `lf open` into Rex still
fails on this Machine and LOO-980's Session is still unopenable. Nothing below is changed code; all of it is
proposal.

## Chain 1: the conversation could not start

1. The pane ran `lf session resume` with `PATH=/usr/bin:/bin:/usr/sbin:/sbin`; `codex` and `claude` are in
   `~/.local/bin`. (Pane text; `launchctl print gui/501/com.superlogical.rex.server.10b2…` shows that PATH as
   the job's default environment.)
2. Why that PATH: Rex runs a `shell:"none"` block with its server's environment, and `lf open` passed only
   the variables it had already been bitten by. `Attachment::argv` carried `LF_HOME`, `LF_OPEN_RECEIPT` and
   two `-u`; `Rex::terminal` added `REX_SERVER` with the comment "Rex omits its endpoint from native child
   environments". Each was added after its own failure. Nothing states what environment a conversation started
   by a host is entitled to, so PATH waited for its turn.
3. Why the known repair did not cover it: the bare-PATH problem has been fixed per entry point, three times
   now. Desktop: `GUIProcessEnvironment.enrichedPath` (Swift). The server: `supervised_path`, #1813, Nov 8,
   behind `LF_SUPERVISED`. The host pane: this change. The directory list exists in Swift, in
   `world::DESKTOP_PATH` (a comment says it mirrors the Swift), in the launchd plist text
   (`install/published.rs:764`) and in `machine/ssh.rs REMOTE_PATH`. A new way of starting lf from something
   that is not a terminal gets no repair until it fails.
4. Why no test failed: the one test that opens into a real Rex
   (`rex_open_starts_a_fresh_conversation_in_the_task_worktree`) starts its server with the stand-in provider's
   directory on the server's PATH (`tests/support/rex.rs:63`), and says so: "Rex children find the provider on
   the server's PATH". The fixture hands the host exactly what the production host lacks. It is also
   `#[ignore]` (needs installed Rex), and nothing in CI or `scripts/test.py` runs it. Every other Rex test uses
   a fake `rex` and never runs the pane command.
5. Why it surfaced on Nov 17–18: the Rex server (pid 1927) started at Nov 17 18:44:38 as launchd job
   `com.superlogical.rex.server.10b2…`; `/Applications/Rex Beta.app` is dated Nov 17 02:47 and the server
   bundle `RexServer/Versions/Current` Nov 17 18:41. Whether earlier servers were launchd jobs or children of a
   terminal's `rex` is not recoverable: the server log is rotated and begins Nov 17 19:12. Unknown, and it
   does not change the fix: lf cannot choose how a host starts its server.

Contradicts nothing in the leading explanation. One thing it does not explain: `~/.lf/logs/lf.log` (from Oct
23) has no earlier "not installed" / "CLI not found" line, but it would not have had one either way, since
the installed lf logs nothing on this path.

## Chain 2: the Session could not be opened again

1. `lf resume` ran `claude --resume fedc5dbe-…` for a conversation Claude never wrote.
2. Why: `Harness::open` (claude.rs:1594) picks `--resume` over `--session-id` from whether a thread is
   recorded, and `run_client` records the thread (`write_provider_session`) before the provider starts. The
   record says "this conversation exists" before it does.
3. Why that order was known and still open: #1887 (Nov 12) hit the same thing with a stopped server and put
   `require_server()` ahead of the record, with the comment "Refused before the provider conversation is
   recorded, so the next launch starts it fresh instead of resuming one that never ran". It fixed one way to
   fail after the record. This change adds a second pre-check beside it. Anything else that ends a launch
   between the record and Claude's first saved turn is still there: exec failing for another reason, an auth or
   trust dialog exit, a crash, and possibly quitting before typing (the refusal text says the transcript appears
   "after claude saves its first turn"; not tested here, it needs a live Claude).
4. Why there was no way out: `lf session new` required a transcript to seed from, and `lf resume` never
   replaces a conversation (`missing_native_history_fails_without_replacing_the_selected_conversation`). Both
   rules are reasonable alone; together a thread with no transcript had no exit. The change gives
   `session new` one.

Measured, with a limit: of 122 interactive Claude Sessions created in the last 7 days with a recorded thread,
57 have no `~/.claude/projects/*/<thread>.jsonl` (54 exited, 3 `spawn_requested`). This overcounts the
never-started case: at least one of them has `~/.claude/file-history/<thread>`, so it ran turns and its
transcript is missing for another reason (#1970's child-session marker is one known cause). What the number
does show is that "thread recorded, no transcript" is common on this Machine, and the installed lf answers
every one of them with the same two refusals.

## Chain 3: it took over an hour to see the cause

1. 13:00 first failure; 14:08 the pane text was read. In between, the codex update at 12:59 and the Codex
   usage limit were suspected.
2. Why: the only statement of the cause was in a Rex pane. `lf open` returned "…exited before attaching; see
   the destination for diagnostics", which is written for a person looking at the window. The caller was an
   agent. `lf.log` had nothing.
3. Why the reproduction then disturbed LOO-903: the pane command carries `--agent <harness:model>`, and
   rerunning it is a move, not a view. The debug step replayed it against the live Session at 14:11:23 and
   pulled the conversation off Claude. Nothing in the command marks it as changing anything.

## Prevention (proposals; none approved, none built)

Sized by what each removes.

- **One place adds the tool directories: where lf builds a provider's `Command`.** Then Desktop, the
  supervised server, a host pane, a launchd applet and whatever comes next need no repair of their own, and
  `LF_SUPERVISED`, `conversation_path` and the PATH line in `Attachment::argv` can go. The list lives once in
  Rust; Swift keeps its copy only for what Swift itself spawns. This is the first shape in `questions.md`.
  It also avoids what the current fix does: carrying an agent's PATH (its shims, an activated venv) into a
  person's conversation. Worked if: the reproduction in this note, run with launchd's PATH, starts the
  provider with no `PATH=` in the pane command.
- **Record the Claude thread when Claude has it, not before.** lf already installs hooks on every Claude
  launch (`context.configure_hooks`); SessionStart can confirm the thread, and until it does the next launch
  uses `--session-id` again. That removes `require_server`'s ordering comment and the new `which_command`
  check instead of adding a third guard later. Smaller step if that is too much now: clear the record when
  `run_client` returns an error before a child exists. Worked if: a stand-in `claude` that exits 1 before
  writing anything is followed by a launch with `--session-id`.
- **The Rex fixture's server gets launchd's PATH; the provider is on the caller's only.** One line in
  `tests/support/rex.rs`. The ignored test then fails on the installed lf and passes with the change. It still
  runs only where Rex is installed; `scripts/testing.md` should list it beside the `ssh_shells` line so gate
  runs it for changes under `open/`.
- **`lf open`'s failure carries the pane's error.** The wrapper in `Attachment::argv` already writes
  "Conversation exited before attachment (status N)" into the receipt; have it write the command's stderr
  there too, so the caller's own error message and `--json` say "'claude' is not installed", on every host,
  without `rex capture`. The `lf.log` lines in the change are the fallback, not the answer an agent reads.
- **The pane command stops carrying `--agent` for a conversation that exists.** `lf session resume <id>`
  already knows the Session's harness. Then rerunning a pane command, or reproducing from one, cannot move a
  conversation. (Third item in `questions.md`.)

Not worth doing: anything that checks the host's environment before opening (a `rex` probe, a doctor line).
It adds a precondition for one host, and the first proposal makes the answer not matter.

## Skipped, unexplored

- Herdr and cmux use the same `Attachment::argv`; whether their servers also run with a supervisor's PATH on
  this Machine was not checked. The change covers them either way.
- `transport::start_held` gives a held terminal's child the requester's whole environment. Other variables a
  provider needs from a terminal (`SSH_AUTH_SOCK`, locale, `TERM`) were not compared between a Rex pane and a
  shell. PATH may not be the last one.
- Whether quitting Claude before the first prompt leaves an unresumable thread was not run.
- The LOO-980 row (`ea3a2511…`) did not match `session_ea3a2511%` in `agent_sessions`; its id form was not
  chased.
- Unrelated, seen in `launchctl list`: 13 `com.loopflow.server.<hash>` jobs. AGENTS.md says a launch fixture
  without a `MachineServer` registers one for its temporary home. Not checked which these are.
