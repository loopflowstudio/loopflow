# LOO-984: a launch that ends before Claude's first turn opens again

## Outcome

A Claude conversation whose launch died before Claude saved anything (CLI
missing, exec failed, auth or trust exit, crash, quit before typing) opens on
the next `lf resume` / `lf session resume` / pane open: Claude starts with
`--session-id` on a new thread with the Session's first request, not
`--resume` into "No conversation found". A conversation Claude did save still
resumes, and a provider that rejects that resume still leaves the Session as
it was.

## What the probes showed (2026-11-19)

- **Quit before typing writes nothing.** Real Claude 2.1.329 under a PTY,
  `claude --session-id <uuid>`, Ctrl-C twice before any prompt: no
  `~/.claude/projects/*/<uuid>.jsonl`, no `file-history/<uuid>`.
  `claude --resume <uuid> -p x` then prints
  `No conversation found with session ID: <uuid>`. Starting the same uuid
  again with `--session-id` is accepted (no "already in use"). Probe script:
  `/tmp/lf984probe/quit.py` (unset `CLAUDE_CODE*`/`LF_*` first or Claude
  saves no transcript; see the child-session memory).
- **Recovery already exists, so the PATH guard is dead weight.** #1972
  (merged after this brief was written) added `lost_thread`
  (`ops/human_session.rs`): on open, a recorded thread that
  `transcript::thread_lost` says neither home holds starts a fresh thread
  with the original request. With the `which_command` pre-check deleted from
  `native_session::run_client`, both
  `a_launch_that_found_no_provider_leaves_nothing_to_resume` and
  `a_launch_refused_for_a_stopped_server_leaves_nothing_to_resume` still pass
  (`cargo test -p loopflow --test transport_tests`, 2 passed, 6.6s).
- **`require_server()` in `run_client` only orders the record.**
  `agent_process::context_environment` already calls `require_server()` for
  every managed launch, on both `exec_native` and `spawn_native`. A failed exec
  is already recorded as a spawn that never ran (`exec_native`), and NotFound
  is already mapped to `missing_agent_message` after spawn and exec.

So the record written before start stops mattering once one rule holds:
**a recorded thread counts as a thread only if the provider holds its
transcript.** The SessionStart-hook idea does not cover quit-before-typing:
the hook fires on startup, before Claude writes the transcript.

## Design

Source of truth: Claude's transcript file, read through
`transcript::thread_lost` (native home, then the recorded conversation
home). lf's `write_provider_session` record stays as written, before start.
It names the thread lf chose; it no longer means "resumable".

1. **One decider.** A *saved thread* is one function in
   `ops/human_session.rs`:
   `fn saved_thread(store, artifact_key, provider) -> Result<Option<ProviderSessionRef>>`,
   `input_provider_session(key)?` filtered by `!thread_lost(..)`. Its two
   callers are the two places that ask "does this conversation have provider
   history?" at open time: `resume_native_session` (the only place that
   turns a recorded thread into `Start::Resume`) and `new_thread`'s
   `threaded` (continuation log vs original request), which today inlines
   the same `input_provider_session` + `thread_lost` pair. A Claude input
   with no usable thread returns `false`, and `open_waiting_locked` takes its
   existing "consumed launch without native history" branch: `append_input`
   gives the same Session a new input carrying the original request, and
   `fresh_launch` starts it with `--session-id <new>`. It prints one
   diagnostic: `This conversation has no saved claude transcript; starting
   it again from its first request.` (provider named as the Session records
   it). That is
   accurate both when the thread never ran and when its transcript was
   deleted.
2. **Delete `lost_thread`** and its branch in `open` (the `fresh` tuple, the
   `fresh-thread:<session>:<thread>` request id). Every opener reaches
   `open_waiting_locked`, the held-terminal holder included, so one predicate
   covers `lf resume`, `lf session resume`, pane opens and the recovery
   watcher. *Made for review:* keeping `lost_thread` and deleting only the
   guards also passes acceptance. It was passed over because it leaves two
   places deciding whether a thread resumes, and openers that skip `open`
   would keep the old behaviour.
3. **Delete the guards** in `run_client`: the `require_server()` call and
   its ordering comment, the `which_command` pre-check, then
   `agent_process::record_missing_provider` and `os_process::which_command`
   (no other callers).

Codex: `thread_lost` covers Codex too. A lost Codex thread now goes through
`resume_native_session`'s existing no-thread branch (reopen the captured
request when the manifest has a process, else a fresh input), not
`new_thread`. Codex records its thread only after the hook reports it, so
this case is rare.

## Acceptance check at gate

- `transport_tests::a_provider_that_exits_before_saving_starts_fresh_next_time`
  (new): a stand-in `claude` on `PATH` exits 1 on its first run without
  writing a transcript, then waits. `lf -a claude resume` fails, and the next
  `lf -a claude resume` launches with `--session-id ` and no `--resume `.
- `a_launch_that_found_no_provider_leaves_nothing_to_resume` and
  `a_launch_refused_for_a_stopped_server_leaves_nothing_to_resume` pass with
  the guards gone. They now exercise the recovery, not the refusal.
- `session_launch_contract::provider_rejected_resume_keeps_the_selected_conversation`
  passes unchanged: the transcript exists, the resume fails, the artifact key
  is the same and there is one `agent_sessions` row. (The brief's
  `missing_native_history_fails_without_replacing_the_selected_conversation`
  was renamed to this in #1972.)
- `session_cli_tests::provider_switch_without_transcript_seeds_from_the_original_request`
  and the `transcript::thread_lost` unit test still pass.
- Real Claude (qa or gate, needs a person's Claude login): in a scratch
  repository, `lf -a claude resume`, Ctrl-C twice before typing, then
  `lf resume`. Claude's prompt appears with no "No conversation found".

## Exclusions

- Headless stream runs (`claude::stream_args`) record the `session_id`
  Claude itself reports. Not touched.
- Dead input rows from past failed launches (57 of 122 on one Machine) stay.
  Each one resumes correctly on its next open.
- `has_provider_history` (the Session projection, `sessions.rs`) still
  means "a thread is recorded", so `resume_intent`, Desktop's action label
  and `resume_explanation` call a lost thread "Resume" while opening starts
  it fresh. Fixing it means the projection reading Claude's files for every
  listed Session, which the 10,000-Task load rule rules out; the honest fix
  is recording the thread only once the provider saved it (a later Task).
  Noted on `scratch/concept-review.html`, not changed here.

## Status

Built (commits 4b607058ed, c87ed69001): guards and `lost_thread` deleted,
`saved_thread` decides for `resume_native_session` and `new_thread`, docs
updated. Slice 1 showed `lost_thread` already recovered a provider that
started and exited 1; `saved_thread` now owns that decision. Passing:
the three transport tests, `provider_rejected_resume_keeps_the_selected_conversation`,
`provider_switch_without_transcript_seeds_from_the_original_request`,
`a_killed_task_conversation_comes_back_once_and_a_quit_one_stays_quit` (the
recovery watcher's lost-transcript case, now through `append_input`) and
`transcript::`.

Remaining: nothing to build. Gate still owes the acceptance tests in CI.

Kept as built, from the concept review (`scratch/concept-review.html`):

- `saved_thread`'s "no saved … transcript" line is printed by the held terminal's own
  `lf`, after `delegate_session` and before a view attaches (qa D: present
  only in `$LF_HOME/screens/*.ghostsnp`), so nobody running `lf resume` sees
  it. The docs no longer promise it (52d3183121). *Made for review:* the line
  stays where the decision is; passed over printing from the viewing process
  (a second place deciding, able to disagree with the holder) and deleting
  it (the only trace of why, in the held screen).
- `new_thread` on a lost thread prints "no saved … transcript" then "seeded with its
  original request". It runs before the hand-off, both show, and they read
  as one account.

Done in qa (`scratch/qa.html`): real Claude 2.1.329, exit at the first
screen then `lf -a claude resume` started a new `--session-id` thread with
the first request; a saved thread reopened with `--resume`.

Larger than this Task, on the page with what each would take: open-time lines
printed after `delegate_session` have no reader (TakeOver's "Stopped the …
client" likely among them, not checked in a terminal); `has_provider_history`
means recorded, not saved.

## Docs (written first)

- `docs/sessions.md`, the paragraph after the `lf task status` "no server" example: any launch that ended before the provider saved (missing CLI, login/trust exit, crash, Ctrl+C before typing) opens again from its first request on every opener, a saved conversation that the provider refuses to resume stays as it was.
- `docs/subscriptions.md`, Resume paragraph: "Missing history is reported" replaced by "starts again from its first request; it never opens empty."
