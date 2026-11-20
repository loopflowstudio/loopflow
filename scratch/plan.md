# Antigravity as a fifth harness

`lf -a antigravity`, `lf task run LOO-12 -a antigravity` and `agents:
[..., antigravity]` work the way they do for Claude, Codex, OpenCode and Pi:
chat, work session, batch and worker; context and steers; resume; transcript
and usage; failover. The CLI is `agy` (Homebrew cask `antigravity-cli`, 1.5.13
here; the process it execs is named `antigravity`).

Two changes ride with it:

- **Failure classification is part of the Harness API.** Each harness answers
  `classify_failure(code, message)` once, for each error its own decoder
  reports; the `classifies_failures` capability is gone. A spent quota, a
  revoked login or an unserved model moves a run to the next agent on every
  harness.
- **OpenCode running out of credits fails over.** Today it stops the step.

The demo: `lf task run <Task> -a antigravity` runs a step on real `agy`, the
step hears `lf steer` after its next tool, and `lf history usage --task` shows
its tokens. Then a step whose first agent is `opencode` with no credits moves
to the next agent in `agents` and finishes.

## The model

What a person knows: an **agent** they name (`antigravity`,
`antigravity:gemini-3.8-flash-high`), the **order** in `agents`, and three
things lf tells them: why a step moved to the next agent, what to do when it
could not (install, sign in, add an agent), and what the step cost.

What the code has, one answer each:

| the person's idea | the code's one owner |
|---|---|
| the agent's name and the command it runs | `HarnessKind::as_str`, `Harness::program`, `Launcher::installed` |
| a provider refused, and why | the harness, asked once per error event: `classify_failure(code, message)`; the answer travels as `AgentProcessResult.failure` |
| what happens next (move, wait, retry, stop with advice) | `_run_with_retries`, which reads `result.failure` and nothing else |
| the conversation a launch owns (resume, and "not a subagent's") | the thread file beside the launch's delivery file |
| what a hook callback means (which moment, whose conversation, what shape the reply takes) | the harness: `hook_callback`, `hook_reply` |

Three places where the first draft of this plan had two owners, and what
replaces them:

1. **A failure was classified from a line lf wrote itself.** `drive::run`
   turns each `ConversationEvent::Error { code, message }` into the text
   `"{code}: {message}\n"` and calls it `stderr`; `_classify_agent_failure`
   split that text back into lines and asked the harness about each. The
   harness's decoder made the event; the harness is asked about the event.
   See "Failures".
2. **A hook's moment was decided in a shell script and its payload read
   again in lf.** lf reads the payload; the script only decides whether
   starting lf is worth it. See "Hooks".
3. **`check_cli_available(cli)` took a harness name or any program.** All
   three callers mean a harness. `Launcher::installed()` replaces it.

## Done when

- `lf task run <Task> -a antigravity` finishes a step on real `agy`: the
  answer and the conversation id are recorded, the Graph advances, a steer
  sent mid-step shows up in what the step does, and `lf history usage --task`
  lists its tokens.
- `lf -a antigravity` opens the TUI in a Task's worktree with no trust dialog,
  and `lf resume` reopens the same conversation.
- A step with `agents: [opencode, claude]` and no OpenCode credits finishes on
  Claude, and `lf task status` says why it moved. The same for an Antigravity
  quota refusal, a signed-out `agy` and a model `agy` does not serve.
- A Machine without `agy` passes Antigravity over, or names it once when it
  was the only choice.

## Acceptance check at gate

Headless, against the stand-in `agy` (no sign-in, no display):

| command | shows | runs at |
|---|---|---|
| `cargo test -p loopflow-execution --lib harness::launch_matrix` | five harnesses × chat, work session, batch, worker: argv, hook ownership, recorded thread | implement, CI |
| `cargo test -p loopflow --lib task_graph_launch_tests::a_task_step_runs_as_a_worker_on_every_harness` | an Antigravity step settles, records answer, thread and usage, gets its context, hears a steer after its next tool | implement, CI |
| `cargo test -p loopflow-execution --lib agent::tests::` | a run whose result says usage limit, revoked login or unserved model moves to the next agent; a transient one is retried | implement, CI |
| `cargo test -p loopflow-execution --lib harness::conformance_tests` | each harness's own refusal words, through its `classify_failure`: OpenCode's balance line, Antigravity's quota, sign-in and model errors; Pi, OpenCode and Antigravity answer `None` for a transient failure | implement, CI |
| `cargo test -p loopflow-execution --lib harness::antigravity` | decoder trace from real 1.5.13 output; a `result` with `status: ERROR` fails the turn with a classified failure; a subagent's hook payload is not this launch's and gets `{}`; first callback is Start, `invocationNum` 0 opens a turn | implement, CI |

On real `agy` (needs its sign-in; qa or demo, never a required check): the
demo above.

## Built

Slice 1, and it runs on real `agy` 1.5.13 (2026-11-20):

- `lf -b default 'Reply with exactly: pong' -a antigravity` printed `pong`,
  exit 0; the Session row holds `provider=antigravity` and the conversation id
  `agy` reported in `init`.
- `agents: [antigravity:no-such-model-xyz, antigravity]`, same command with no
  `-a`: stderr "antigravity:no-such-model-xyz: model unavailable; continuing
  on antigravity", then `pong`.
- `-a antigravity:no-such-model-xyz` alone: "agent stopped after model
  unavailable. antigravity_error: invalid model selection (…) is not
  recognized as a known model or custom model in settings" and the "No
  fallback remains" advice. The "Available models:" list is cut.
- The instructions reached `agy` as
  `.lf/prompts/<stamp>-<rand>-antigravity/.agents/AGENTS.md` through
  `--add-dir`.

What is in the tree:

- `classify_failure(code, message)` on every harness, asked in `drive::run` at
  each error event, gravest kept (`AgentFailure::graver`).
  `_classify_agent_failure`, the `from_code` call in `drive::run` and
  `check_cli_available` are gone; `Launcher::installed()` replaces the last.
- After the sync onto main's `loopflow-harness` split (LOO-923), what `agy`
  writes is in that crate: `antigravity.rs` (`program`, `transcript_home`,
  `recognizes_driver`), `antigravity_decode.rs`, `transcript/antigravity.rs`,
  two real traces under `src/testdata/`, and `sanitize_error_message` in
  `redact.rs`, shared by the OpenCode and Antigravity decoders. Execution's
  `harness/antigravity.rs` is the `Launcher`: driver, `classify_failure`,
  hooks, AGENTS.md launch folder, `open`. The stand-in is
  `tests/support/antigravity_cli.py`; the `<harness>.stderr.log` opener is in
  `harness/common.rs`.
- The launch matrix has an Antigravity Batch cell.
- `harness/README.md`: the limits row is "yes" for OpenCode and Pi and
  `classify_failure` has its own row.

Slice 2, a worker hears lf (2026-11-20):

- Real `agy` 1.5.13, a throwaway repo with its own `LF_HOME`:
  `lf -b default 'Your context names a passphrase in scratch/plan.md. Reply
  with the passphrase only; do not read any file.' -a antigravity` printed the
  passphrase. The transcript has three rows: `USER_INPUT`, a `SYSTEM_MESSAGE`
  "injected by a PreInvocation hook" holding the context block, and one
  `PLANNER_RESPONSE` with no tool call. So `--add-dir` loaded lf's
  `hooks.json`, the script ran, and `injectSteps` reached the model.
- Not run on real `agy`: a Task step (`lf task run … -a antigravity`) and a
  steer mid-step. They need a Machine server for the test home; qa or demo
  runs them. The stand-in covers both.
- `a_task_step_runs_as_a_worker_on_every_harness` has an Antigravity cell: the
  step settles, the Graph advances, the thread is recorded, the first model
  call gets "This Session" and its Task, the call after the tool gets the
  steer, and the stand-in fails the run if a subagent's hook is answered with
  anything.
- The launch matrix's Antigravity Worker cell runs.
- Run at realign (2026-11-20), both ok: `cargo test -p loopflow-execution
  --lib -- harness::antigravity harness::launch_matrix` (14 passed) and
  `cargo test -p loopflow --lib
  task_graph_launch_tests::a_task_step_runs_as_a_worker_on_every_harness`.
  CI runs them after.

What is in the tree for it:

- `HookCallback` and `Harness::hook_callback` / `hook_reply`
  (`harness/mod.rs`); the trait's default `hook_callback` holds the
  `agent_id` check that was in `emit_block`. `lf __context-block --harness
  <name>` picks the harness; without the flag it is Claude, so `emit_block`
  asks one harness both questions (`HookCallback::plain` and `plain_reply`
  are gone). Run after that change, ok: `cargo test -p loopflow --test
  integration -- session_steers::public_context_capability_rejects_children_and_replacement
  context_block_tests::`, the worker test again, and `cargo clippy -p
  loopflow -p loopflow-execution --all-targets -- -D warnings`.
- `antigravity.rs`: `launch_folder` (made for instructions or a conversation
  context), `hooks`, `hook_script`, `claim_thread`, and the two trait methods.
  The script is `<launch folder>/hook.sh`, outside `.agents`; `hooks.json`
  runs it as `/bin/sh '<path>' tool|steer|stop`.
- `ContextDelivery::hook_command` and `steer_gate`, taken out of
  `hook_settings` so the script and Claude's hooks share one gate.

What slice 2 changed in the design (the "Hooks" section below is corrected):

- **lf never writes the thread file at launch.** The Machine server accepts a
  turn or a steer claim only from a process whose adapter a start activated
  (`with_provider_context`), and every headless turn is a new process. So the
  first callback of every launch is Start, with `source` `resume` when
  `initialNumSteps > 1`, and a resumed turn gets the context block again, as
  OpenCode's does. The launch's own conversation always calls before any
  subagent, so the first id written is the owner.
- The start's extra prompt boundary is `HookCallback.prompt`; `emit_block`
  sends it only for a managed launch.
- `${reply:-{}}` is not `{}` in sh: it expands to the reply plus a stray `}`.
  The script tests the reply instead.

What slice 1 taught:

- One process per turn, `status`-not-exit failure and the reported thread fit
  `drive::run` with no new concept. `drive::run` already reads
  `agent_session()` after every event, so a thread named in the first line is
  recorded.
- **A headless run always passes `--dangerously-skip-permissions`**, as
  OpenCode's always passes `--auto`; `config.skip_permissions` is not what
  decides it. Without the flag `init` says `"permission_mode":"request-review"`
  (`echo hi` still ran), and nobody is there to review. `docs/security.md`
  already says this. The terminal (slice 4) follows `config.skip_permissions`.
- A tool that finishes fast prints only its `DONE` step, and an
  `agent_response` that only calls a tool prints `DONE` with no text.
- `/usr/bin/python3` here is 3.9: `native_codex.py` had one `str | None`
  annotation that failed the matrix's Codex cell on this Machine before any
  Antigravity cell ran. Quoted now.

Slice 3, transcript and usage (2026-11-20):

- Real `agy` 1.5.13, a throwaway repo with its own `LF_HOME`:
  `lf -b default 'Run the shell command `echo hi`, then reply with exactly:
  done' -a antigravity` printed `done`. For that Session, `lf history show
  <session> --final` printed `done`; `lf history usage --days 0 --json`
  listed turn `step-0` with `input_tokens` 13503, `cache_read_tokens` 9077,
  `total_input_tokens` 22580, `output_tokens` 273; `lf history show
  --transcript --json` showed one ended turn, model `gemini-3.8-flash-high`,
  a request and three agent items.
- Not run on real `agy`: `lf history usage --task` for a Task step. The
  stand-in covers it.
- `a_task_step_runs_as_a_worker_on_every_harness` asserts Antigravity's final
  answer and usage like the others; the `transcript` guard is gone. The
  stand-in keeps a `transcript_full.jsonl` under `HOME`, its tool turn as two
  model calls (12 in / 2 out, 8 / 3) that the turn sums to 20 / 5.
- Run, all ok: `cargo test -p loopflow-execution --lib --
  transcript::antigravity harness::antigravity harness::launch_matrix` (16
  passed), `cargo test -p loopflow --lib
  task_graph_launch_tests::a_task_step_runs_as_a_worker_on_every_harness`,
  `cargo clippy -p loopflow-execution --all-targets -- -D warnings`. CI runs
  the suites after.

What is in the tree for it:

- `transcript/antigravity.rs`: `file(home, id)` and `read`. The harness
  answers `locate_transcript`, `direct_transcript` and empty
  `transcript_markers` (every row counts in a tail read).
- `Source::last_written` reads `created_at` when a row has no `timestamp`,
  so a turn cut off at exit is completed at Antigravity's last row.

What slice 3 taught (from 25 real transcripts on this Machine):

- `input_tokens` is apart from `cache_read_tokens` (9565 + 0, then 2366 +
  7526, against the stream's totals 11931 and 7526), so it fits `Calls` as
  Pi's and OpenCode's do. The transcript has no thinking-token count; the
  stream's `thinking_tokens` is not kept, and `reasoning_tokens` is 0.
- `step_index` is contiguous and every tool result seen is `GENERIC`, one
  per `PLANNER_RESPONSE` (no call with two tools was seen). Results are
  matched to calls in order. A denied or failed tool is `status: ERROR`; a
  background one is `RUNNING` and its row is what the model was told.
- A `PLANNER_RESPONSE` with no `tool_calls` ends the turn, and a later one
  in the same turn reopens it (`Turns::current`), so a live read says
  Running exactly while a tool or a model call is outstanding. A
  `USER_INPUT` that arrives while a tool is unanswered marks the turn
  interrupted.

Slice 4, the terminal (2026-11-20):

- Real `agy` 1.5.13 in a pseudo-terminal, a throwaway repo with its own
  `LF_HOME`. In the main checkout, `lf default 'Reply with exactly: pong' -a
  antigravity` stopped on "Do you trust the contents of this project?". In a
  worktree made with `lf wt create`, the same command went straight to the
  prompt and answered `pong` with no dialog. After ctrl+c twice, both
  Sessions hold `provider=antigravity` and the conversation id `agy` printed
  in "Resume with … --conversation=<id>". `lf resume <session>` reopened that
  conversation: the TUI showed the first prompt and `pong`.
- Not run on real `agy`: a work session (a Task's conversation) and a steer
  into an open TUI. qa or demo.
- Run, all ok: `cargo test -p loopflow-execution --lib -- harness::antigravity
  harness::launch_matrix` (17 passed; all four Antigravity cells run) and
  `cargo clippy -p loopflow-execution --all-targets -- -D warnings`.

What is in the tree for it:

- `Antigravity::open`: the options a headless turn passes, then
  `--dangerously-skip-permissions` when `config.skip_permissions`,
  `--settings {"trustedWorkspaces":[cwd, canonical cwd]}` in a worktree lf
  created (`trust`), and `-i <prompt>`. A launch with no known thread sets
  `reported_thread`; `claim_thread` fills it at the first callback and
  `native_session::run` records it when `agy` exits.
- The stand-in `agy` without `--output-format` is the TUI: one turn from
  `-i`, its hooks, its transcript, exit.
- The matrix's `Mode::Interactive` skip is gone.

What slice 4 taught:

- `each_turn_is_one_run_that_continues_the_conversation_agy_named` takes 4 s
  alone. Its 60 s was the wait for the ledger the launch matrix holds when
  both run in one invocation.
- Go-style flags: `-i` takes the next argument whatever it starts with (a
  prompt beginning with `-` is under test as argv, not seen live).

Slice 5, names (2026-11-20):

- `antigravity` is in every list of harness names: the "No agent selected"
  and "unsupported agent" messages (`error.rs`, `config/settings.rs`), the
  `yolo` and `chrome` help (`config/catalog.rs`), the Desktop picker and pane
  heading, `loopflow-test-support`'s installed-harness filter (`agy`), and
  the docs that named four. Tests that listed four names read
  `HarnessKind::ALL` where they can.
- `harness/README.md`: the launch table has an Antigravity row and the
  Task-step table a fifth column.
- `a_worker_stops_on_interrupt_and_the_next_run_starts_its_step_again_on_antigravity`:
  the stand-in holds a turn on `hold-turn`, so the README's interrupt cell is
  "yes" with a test behind it.
- Run, ok: that test; `cargo test -p loopflow-execution --lib --
  config::tests::bare_harnesses prompt::process::tests::preferred_name
  harness::launch_matrix::every`; `cargo test -p loopflow --lib --
  launch_policy_tests::`; `cargo test -p loopflow --test integration --
  documented_commands::`; `cargo clippy -p loopflow -p loopflow-execution -p
  loopflow-test-support --all-targets -- -D warnings`.
- Not run here, CI runs them:
  `settings_launch_tests::automatic_agent_selection_requires_exactly_one_installed_harness`
  (this Machine has `opencode` and `agy` in `/opt/homebrew/bin`, which lf adds
  to every PATH, so no fixture here ever has exactly one harness; it fails the
  same way without this change) and the Swift suite
  (`scripts/test_desktop.sh --filter SessionsStoreTests`).

After the merge of main (c7f2724, the `loopflow-harness` split; 30 files
touched on both sides), 2026-11-20:

- Run on the merged tree, all ok: `cargo test -p loopflow-harness` (90
  passed); `cargo test -p loopflow-execution --lib -- harness::antigravity
  harness::launch_matrix harness::conformance_tests agent::tests::` (52
  passed); `cargo test -p loopflow --lib --
  task_graph_launch_tests::a_task_step_runs_as_a_worker_on_every_harness
  task_graph_launch_tests::a_worker_stops_on_interrupt`; `cargo test -p
  loopflow --test session_launch_contract -- opencode`. Clippy was not run
  after the merge; gate or CI runs it.
- `a_worker_stops_on_interrupt_…_on_opencode_and_pi` failed at least 9 runs of 11
  here with attempts `["failed", "interrupted"]`: "opencode_error:
  json.decoder.JSONDecodeError: Extra data: line 1 column 187". `lf task run`
  starts the Task's own conversation (`lf holder run`, the OpenCode TUI) and
  the worker's `opencode run` within the same millisecond; both stand-in
  processes rewrote `native-opencode-config.json` and one read the other's
  half-written copy. The stand-in file is the same on main; nothing in this
  branch's diff reaches it. Fixed in `tests/support/opencode_cli.py`: that
  file is gone (nothing else read it) and the sessions, locations and model
  files are written whole by rename. `antigravity_cli.py` writes
  `agy-steps.json` the same way. After: 7 of 7 ok.

QA on real `agy` 1.5.13 (2026-11-20), a private Machine with its server;
`scratch/qa.html` has the transcripts and `.lf/qa/antigravity.md` the route:

- `lf task run <Task> -a antigravity` finished its step; a steer sent 17 s in
  reached the worker 2 s later, after its next tool, and the step did what it
  said. `lf history usage --task` listed the step's tokens. `lf task
  interrupt` left the turn `interrupted` and no `agy` process; the next
  `lf task run` started the step again. With `agents:
  [antigravity:no-such-model-xyz, antigravity]`, `lf task status` read
  "implement went on under antigravity after: antigravity_error: invalid
  model selection …".
- The Task's own conversation is an `agy` TUI in a held terminal. It got the
  context block, the steer and the Task update, and its `Stop` hook ran.
- Fixed: `lf resume --task` on a quit Antigravity conversation started a new
  `agy` conversation and sent the opening request again. A held terminal's
  launcher becomes `agy`, so nothing published the conversation the first
  hook named. A provider start now publishes it for an input that has none
  (`with_provider_context`).
- Fixed: in a worktree outside `/tmp`, every Git command in the TUI answered
  "fatal: not a git repository": `agy` runs a terminal's commands in a
  sandbox that reaches only workspace folders. `open` adds the main
  repository's `.git` with `--add-dir` (`main_git_dir`).
- Not reached: a real OpenCode balance refusal (the account has credits; its
  `gemini-4-carbon` answered), a spent Antigravity quota, a signed-out `agy`,
  a Machine without `agy` (lf's own PATH finds it here).

## Left to build

From the review of the built change (`scratch/concept-review.html`,
2026-11-20). One pass, no new behavior:

1. **Say what a Task's conversation on Antigravity meets.** In the terminal
   `agy` runs commands in its own sandbox, which does not reach `~/.lf`; the
   first `lf` command fails there and the model asks "Run this command?" to
   run it outside. A headless step is not sandboxed. Today only
   `scratch/questions.md` and `.lf/qa/antigravity.md` say so.
   - `docs/config.md`, "Antigravity": one paragraph. The prompt, the answer
     that allows `lf` for the conversation, and that `yolo` removes it.
   - `docs/troubleshooting.md`: one row for "Run this command?" /
     "Operation not permitted" from `lf` inside `agy`.
   - `rust/loopflow-execution/src/harness/README.md`: a clause on the
     Antigravity launch row.
   - Result: `cargo test -p loopflow --test integration --
     documented_commands::` still passes.
2. **One true statement of who writes the thread file.** The comment above
   the write in `emit_block` (`rust/loopflow/src/lf/commands/context.rs`,
   "lf asks only a bare Codex launch for its thread") is false since
   Antigravity: say that a launch whose provider names the conversation keeps
   it beside its delivery, that Codex's launch creates the file to ask and
   Antigravity's first callback claims it. `Launcher::hook_callback`'s doc
   (`harness/mod.rs`) says an implementation may claim the file. Comments
   only.
3. **`harness/README.md`'s method table** gains `hook_callback` and
   `hook_reply` beside `classify_failure`.

Then the demo on real `agy`, for qa or demo: a Task's work session in the
TUI; a step whose first agent is `opencode` with no credits moving to the
next agent.

## Design

**Headless: one `agy` process per turn**, as OpenCode does.

```
agy --input-format stream-json --output-format stream-json
    --dangerously-skip-permissions     # always headless; see "What it taught"
    [--model <slug>] [--conversation <id>] [--add-dir <launch folder>]
```

One line on stdin, `{"event":"user","message":{"content":"…"}}`, then close
stdin: the turn runs and `agy` exits 0. Interrupt and stop end the process
group (`agent_process::stop`); the next input continues with `--conversation`.

**The provider names the thread** (bare and managed: `ThreadName::Reported`).
Headless reads `conversation_id` from the `init` line. A bare terminal launch
gets it from its first hook through `reported_thread_path`, as bare Codex does.

**Terminal:** `agy -i "<prompt>"` (argv; `validate_terminal_turn`),
`--conversation <id>` to resume, same `--model` and `--add-dir` flags;
`--dangerously-skip-permissions` follows `config.skip_permissions`.

**The launch folder** carries instructions and hooks with nothing written to
`~/.gemini` and no hook file in the checkout root:
`<launch cwd>/.lf/prompts/<stamp>-<rand>-antigravity/.agents/`

- `AGENTS.md`: `system_prompt_with_structured_replies(config)`.
- `hooks.json`, and `hook.sh` beside `.agents`: only when the launch has a
  `conversation_context`.
- It must be under the launch cwd (see "workspace order" below), and
  `git::exclude(cwd, ".lf/prompts/")` keeps it out of Git.

**Hooks.** Only `SessionStart`, `PreInvocation` and `PostInvocation` can put
text in front of the model (`injectSteps`); `PostToolUse` must answer `{}`,
and `SessionStart` does not run on `--conversation`. `PreInvocation` runs
before every model call, so it is the start hook, the prompt hook and the
after-a-tool hook. lf declares `PreInvocation` and, for a managed launch,
`Stop`. Nothing else: a `PreToolUse` handler that answers `{}` denies every
tool ("tool call denied by pre-tool hook", seen live).

The script decides one thing: whether starting lf is worth it. lf reads the
payload.

- **The script.** `PreInvocation`: read stdin; when the payload has
  `"invocationNum":` followed by 1-9 (a model call inside a turn), run the
  marker gate `hook_settings` builds and exit with `{}` when no steer may be
  waiting; otherwise pipe the payload to
  `lf __context-block --harness antigravity`. `Stop` (managed only): the same
  command, ungated. It always exits 0 and prints `{}` when lf prints nothing.
  The gate fails open: a payload the pattern does not recognize starts lf, so
  a changed payload costs a process, never a missed context block or an
  unreported turn. A quote inside `lastUserInput` is escaped, so user text
  cannot match the pattern.
- **`Harness::hook_callback(payload, declared moment, thread file)`** answers
  the moment and the callback in the shape `emit_block` already reads
  (`session_id`, `hook_event_name`, `source`), or `None` when the callback is
  not this launch's conversation. The default is today's behavior: the
  declared `--moment`, the payload as sent, `None` when `agent_id` is set
  (Claude's subagents; that check leaves `emit_block`). Antigravity's:
  - **Whose conversation.** The thread file is `reported_thread_path` beside
    the launch's delivery file (the hook inherits `LF_CONTEXT_DELIVERY`). The
    launch's first callback writes its `conversationId` there, resumed or
    not. A payload with any other `conversationId` is `None`: `{}`, no
    boundary, no steers claimed.
    Subagents run the same hooks under their own id (seen live, below);
    without this a subagent's `Stop` would end the worker's turn and a
    subagent would claim the Session's steers. The same file is how a bare
    launch reports its thread for resume, so one file answers both.
  - **Which moment.** `Stop` is Stop. A `PreInvocation` that wrote the thread
    file is Start: the first of every launch, so of every headless turn.
    `invocationNum` 0 after that opens a later turn in the same process: the
    moment its hook declared, Tool for a managed launch (the server marks the
    turn working), Steer for a bare one. Anything else is Steer.
  - `source` is `startup` when `initialNumSteps <= 1`, else `resume`.
- **`Harness::hook_reply(event, text)`**: default
  `hookSpecificOutput.additionalContext`; Antigravity's
  `{"injectSteps":[{"systemMessage":{"systemMessage":"<text>"}}]}`.
- The server receives the normalized callback, so nothing past `emit_block`
  learns a fifth payload shape.
- A managed start is also the first prompt (`HookCallback.prompt`): after
  the Start boundary, `emit_block` reports a Tool boundary named
  `UserPromptSubmit` so the server marks the turn working.

**Folder trust.** The TUI stops on "Do you trust the contents of this
project?" in a folder it has not seen; headless runs do not ask. A terminal
launch in an lf-created worktree (`git::lf_created_worktree`, the rule
`claude_trust` uses) passes `--settings '{"trustedWorkspaces":["<worktree>"]}'`,
which lasts for that session and writes nothing (`settings.json` was unchanged
after the probe). Main checkouts and hand-made worktrees keep the dialog.

**Turn names.** A turn is named `step-<n>`, the `step_index` of its
`user_input` step: the stream's first `step_update` and the transcript's
`USER_INPUT` row carry the same index, so the driver's turn and the
transcript's usage join without Pi's time matching. A run that fails before
any step gets a UUID.

**Transcript.**
`~/.gemini/antigravity-cli/brain/<id>/.system_generated/logs/transcript_full.jsonl`,
found by id alone (`direct_transcript`). `USER_INPUT` starts a turn (strip the
`<USER_REQUEST>` wrapper for the request text) and the next `USER_INPUT` or
the end of the file ends it; `SYSTEM_MESSAGE` is text the system or a hook put
in mid-turn, never a request; `PLANNER_RESPONSE` is one model call (`input_tokens`,
`cache_read_tokens`, `output_tokens`, `model`, `content`, `thinking`,
`tool_calls[]`); the rows after a tool call (`GENERIC`, or a named type) are
its results, matched to the calls in order. A `PLANNER_RESPONSE` with no `tool_calls` does not end the turn:
a subagent's report or a timer arrives as a `SYSTEM_MESSAGE` and the model
answers again (seen live: steps 9, 10, 11 of one turn). The turn's answer is
its last `PLANNER_RESPONSE` with `content`. Rows carry no cwd, so
`recent_transcripts` answers nothing (see Exclusions).

**Failures.** A provider's failure is classified once, where lf first sees
it. Each decoder already reports it as `ConversationEvent::Error { code,
message }`; `drive::run` asks `launch.harness.classify_failure(&code,
&message)` at that event and keeps the gravest answer of the turn
(credential, usage limit, model, then transient) in
`AgentProcessResult.failure`, dropped when the turn completed anyway.
`_run_with_retries` reads `result.failure` and never the error text;
`result.stderr` stays what a person reads as the reason.

- `routing_failure(code, message)` and `provider_failure(code, message)`:
  `AgentFailure::from_code(code)`, else the phrase lists over the message's
  lines. Claude and Codex answer `provider_failure`; Pi and OpenCode
  `routing_failure`, OpenCode after its own "insufficient balance".
- The direct `AgentFailure::from_code` call in `drive::run` goes: the harness
  method is the one question.
- Antigravity's decoder reports a `result` whose `status` is `ERROR` as
  `Error { code: "antigravity_error", message: <first line of result.error> }`
  and fails the turn; `agy` exiting with no `result` line reports the last
  stderr line the same way. Its `classify_failure`, then `routing_failure`:
  - usage limit: "exhausted your quota", "quota reached", `QUOTA_EXHAUSTED`,
    `RESOURCE_EXHAUSTED`, "usage limit reached", "out of credits"
  - credential: "authentication required", "authentication failed"
  - model: "is not recognized as a known model"
- What changes on purpose: when one turn reports two different failures, the
  gravest wins whichever came first. Today a coded failure wins over a worded
  one (`failure.or`). A turn with one failure classifies as it does now.

Antigravity's phrases other than the model and authentication lines come from
the binary's strings, not from a refusal seen live.

**What the person reads at a dead end.** `install_hint`: "Install it with
`brew install --cask antigravity-cli`; the command is `agy`."
`sign_in_advice`: "run `agy` and sign in". The agent is named `antigravity`
everywhere lf prints it.

**Capabilities:** no native skills, no turn limit, no output schema (`agy`
has `--json-schema`; not wired), no respawn after crash, no `Provider` (native
sign-in, like Pi). `recognizes`: process command `agy` or `antigravity`.

## What real `agy` 1.5.13 showed (2026-11-20, this Machine)

Stream (`--output-format stream-json`), one prompt that ran `echo hi`:

```
{"event":"init","conversation_id":"b8673fd6-…","init":{"cwd":"…/work","tools":[…60],"permission_mode":"always-proceed"}}
{"event":"step_update","step_update":{"conversation_id":"…","step_index":0,"state":"DONE","step_type":"user_input"}}
{"event":"step_update","step_update":{…,"step_index":1,"state":"DONE","step_type":"system_message","duration_seconds":5.9e-05}}
{"event":"step_update","step_update":{…,"step_index":3,"state":"DONE","step_type":"agent_response","duration_seconds":3.55,"usage":{"input_tokens":9565,"output_tokens":169,"thinking_tokens":134,"cache_read_tokens":0,"total_tokens":9734}}}
{"event":"step_update","step_update":{…,"step_index":4,"state":"ACTIVE","step_type":"tool","tool_name":"run_command","tool_info":{"name":"run_command","parameters":{"CommandLine":"echo hi"}}}}
{"event":"step_update","step_update":{…,"step_index":4,"state":"DONE","step_type":"tool","tool_name":"run_command","duration_seconds":0.137,"tool_info":{"name":"run_command","parameters":{"CommandLine":"echo hi"},"output":"Created At: …\nCompleted At: …\nCommand exited with code 0.\n\nCommand output:\nhi\r\n"}}}
{"event":"step_update","step_update":{…,"step_index":6,"state":"ACTIVE","step_type":"agent_response","text_delta":"The shell command `echo hi` was executed"}}
{"event":"step_update","step_update":{…,"step_index":6,"state":"DONE","step_type":"agent_response","text_delta":"\n","duration_seconds":4.4,"usage":{…}}}
{"event":"result","result":{"conversation_id":"…","status":"SUCCESS","response":"…","duration_seconds":8.3,"num_turns":1,"usage":{"input_tokens":11931,"output_tokens":477,"thinking_tokens":397,"cache_read_tokens":7526,"total_tokens":12408}}}
```

- `text_delta` is incremental; the `DONE` line can carry a last delta.
- A resumed run (`--conversation`) continues `step_index` (7, 8, …) and its
  `result.usage` and `num_turns` are totals for the whole conversation.
- Not signed in: `{"event":"result","result":{"conversation_id":"","status":"ERROR","error":"authentication failed or timed out",…,"finished_reason":"error"}}`;
  stderr `Error: authentication required. Run 'agy' to log in, then retry.`
  Exit 1 with a piped stdin, 0 with `/dev/null`.
- Unknown model: `status":"ERROR"`, exit 1, `"error":"invalid model selection
  (--model \"no-such-model-xyz\" --effort \"\"): model no-such-model-xyz is
  not recognized as a known model or custom model in settings\nAvailable
  models:\n  Gemini 3.8 Flash (High)…"`.
- A headless question: the `ask_question` step prints as
  `"step_type":"unknown"` with no text, the turn ends `SUCCESS` and the answer
  restates the question. The transcript has it:
  `"tool_calls":[{"name":"ask_question","args":{"questions":[{"question":"Which color do you prefer?","options":[…],"is_multi_select":false}]}}]`.
  So the stream cannot fail a turn on a question; the README row is "no" as
  for Claude and Codex unless a `PreToolUse` hook on `ask_question` is added.
- Subagents (`invoke_subagent`): the stream prints
  `"step_type":"subagent","tool_name":"invoke_subagent","subagent_info":{"subagents":[{"type_name":"self","role":"…","initial_prompt":"…"}]}`.
  The subagent is its own conversation with its own `brain/<id>/` transcript.
  Hooks from one launch, in order (parent `3a0a…`, subagent `795a…`):
  `SessionStart` P, `PreInvocation` P (0, 1), `PreInvocation` P (1, 3),
  `SessionStart` S, `PreInvocation` S (0, 0), `PreInvocation` S (1, 3),
  `PreInvocation` P (2, 5), `PreInvocation` S (2, 5), `PreInvocation` P (3, 8),
  `PreInvocation` P (4, 10), `Stop` S, `Stop` P. The subagent's payload
  differs only in `conversationId`, its paths and an empty `lastUserInput`.
  The parent's `Stop` ran once, at the end.
- `init.init.model` carries the model slug.
- `agy models`: `gemini-3.8-flash-high`, `gemini-3.8-flash-medium`,
  `gemini-3.8-flash-low`, `claude-sonnet-4-6`, `claude-opus-4-6-thinking`,
  `gpt-oss-120b-medium` (slug, tab, label).

Hooks:

- `--add-dir <dir>` loads `<dir>/.agents/hooks.json` and
  `<dir>/.agents/AGENTS.md` (the model answered a passphrase that only that
  file held). File shape: `{"<name>":{"PreInvocation":[{"type":"command","command":"…"}],"Stop":[…],"PostToolUse":[{"matcher":"*","hooks":[…]}]}}`.
- **Workspace order.** Antigravity sorts workspace folders by cleaned path
  (symlinks unresolved) and runs commands in the first. `--add-dir
  …/hookdir` from cwd `…/work` ran `pwd` in `hookdir`; `…/zzhooks` and
  `…/work/.lf/prompts/h1` left it in `work`. A folder under the launch cwd
  always sorts after it.
- Order in one turn with one tool: `SessionStart`, `PreInvocation`
  (`invocationNum` 0, `initialNumSteps` 1), `PostInvocation`, `PostToolUse`,
  `PreInvocation` (1, 5), `PostInvocation`, `Stop`. On `--conversation`
  resume there is no `SessionStart`; `PreInvocation` restarts at
  `invocationNum` 0 with `initialNumSteps` 8.
- Payload (camelCase): `conversationId`, `hookEventName`, `cwd`,
  `workspacePaths`, `transcriptPath` (the `transcript_full.jsonl`),
  `artifactDirectoryPath`, `modelName`, `lastUserInput`; `PreInvocation` adds
  `invocationNum`, `initialNumSteps`; `PostToolUse` adds
  `toolCall{id,name,args}`, `stepIdx`, `error`; `Stop` adds `executionNum`,
  `terminationReason` (`NO_TOOL_CALL`), `fullyIdle`, `continuationActive`.
- A hook's cwd is its `.agents` directory; it inherits the launch's
  environment plus `ANTIGRAVITY_CONVERSATION_ID`.
- `injectSteps` from `SessionStart` and `PreInvocation` reached the model and
  persist as `SYSTEM_MESSAGE` rows ("The following system message was
  injected by a PreInvocation hook: …").
- TUI: `agy -i '<prompt>' --add-dir <dir>` ran the prompt and the hooks. In a
  folder it has not seen it first asks "Do you trust the contents of this
  project?" ("Yes, I trust this folder" / "No, exit"). With
  `--settings '{"trustedWorkspaces":["<dir>","<canonical dir>"]}'` the same
  launch in an unseen folder went straight to the prompt and answered, and
  `~/.gemini/antigravity-cli/settings.json` was unchanged afterwards.
- `agy` here is a 227 MB file at `~/.local/bin/agy`, a directory lf's tool
  PATH already searches (`os_process.rs`), as is `/opt/homebrew/bin`.

OpenCode's refusal, from `~/.lf/runs/6c/6c7c…/opencode.stderr.log`:
"Insufficient balance. This request is pay-as-you-go, billed to OpenCode
Models credits. Top up at https://opencode.ai/console/…/billing?credits=1.
OpenCode Go covers only requests to the OpenCode Go provider."

The hooks guide inside the binary (`strings` of
`/opt/homebrew/Caskroom/antigravity-cli/*/antigravity`, "# Lifecycle Hooks")
is the fullest reference; docs: https://antigravity.google/docs/hooks/,
https://antigravity.google/docs/cli/headless/,
https://antigravity.google/docs/cli/reference.

## Docs written ahead of the build

They describe the behavior above as intended; the build reads them as the
user-facing contract.

- `docs/config.md`: "Antigravity" section (install, sign-in, `-a`, models,
  steer, usage, launch folder, trust dialog, failover, limits); OpenCode's
  balance refusal moving on under "Keep going when a provider runs out"; the
  two "Pi and OpenCode stop there" passages removed; harness list, model
  strings, `supported_harnesses`, argument cap, `yolo`.
- `docs/subscriptions.md`: the two "Pi or OpenCode: stop" rows removed.
- `docs/troubleshooting.md`: `opencode_error:` and `antigravity_error:` in the
  reason table; the `antigravity` not-installed message.
- `docs/telemetry.md` (no cost on Antigravity rows; subagent tokens not
  added), `docs/sessions.md` (plain `agy` not listed; updates at the next
  boundary), `docs/security.md` (headless floor).
- Names only: `README.md`, `docs/index.md`, `docs/getting-started.md`,
  `docs/glossary.md`, `skills/loopflow/README.md`, builtin `init.md`
  (`command -v … pi agy`).

Details the docs settled, for the build to match or correct:

- A moved step's reason keeps the decoder's code and the error's first line:
  `antigravity_error: invalid model selection (…): model … is not recognized…`
  without the "Available models:" list; `opencode_error: Insufficient
  balance. …`.
- `docs/security.md` says a headless Antigravity run skips permission
  prompts. Built so: the headless driver always passes the flag.
- `docs/config.md` says Antigravity terminal prompts share the 122,880-byte
  argument cap (`validate_terminal_turn`).

## Absent and error states

| state | what happens |
|---|---|
| `agy` not installed | passed over for the next agent; named once, with the install hint, when it was the only choice (`Launcher::installed()`) |
| not signed in | `result.status` `ERROR`, "authentication failed": `CredentialInvalidated`, next agent, or the sign-in advice when none is left. Exit is 0 when stdin is `/dev/null`, so the driver fails the turn on `status`, not on the exit code |
| model not served | `ModelUnavailable`, next agent |
| quota spent | `UsageLimit`, next agent (phrases from the binary, not seen live) |
| `agy` exits with no `result` line | the turn fails with the last stderr line as its reason, as OpenCode's does |
| no `init` line before the failure | no thread recorded; the turn gets a UUID name; nothing to resume |
| hook cannot start lf, or the server does not answer | the script prints `{}` and exits 0; a recording gap in `logs/lf.log`, never a failed turn |
| the hook payload changes shape | the script's gate does not match and lf starts at every model call; an unreadable payload gets `{}` |
| transcript missing or half-written | `Located::Gone(Gap::FileMissing)`; usage absent for that turn, the turn itself stays as the driver recorded it |
| a headless question (`ask_question`) | not read: the turn ends `SUCCESS`. README row "no", with Claude and Codex (Task 48edb9) |

## Exclusions

- **Subagent usage.** A subagent's tokens are in its own conversation's
  transcript; the parent's `result.usage` and transcript leave them out (seen
  live: 18,630 input tokens = the parent's five model calls exactly). lf
  records the parent's.
- **Conversations started outside lf.** `recent_transcripts` answers nothing:
  transcript rows carry no cwd. Passed over: reading `workspace_uris` from
  `~/.gemini/antigravity-cli/conversation_summaries.db`.
- `--json-schema` (output schema), `--effort`, `agy acp`, waking an idle work
  session (Task 540073 covers OpenCode and Pi; Antigravity joins that row as
  "no: next turn only").
- No store migration: no table or wire type restricts harness names (checked
  `migrations/` and `loopflow-storage`).

## What goes

- `Capabilities::classifies_failures` and the early return in
  `_run_with_retries` (done).
- `_find_provider_error` (done).
- `_classify_agent_failure`, the `AgentFailure::from_code` call in
  `drive::run` and `check_cli_available` (done).
- The `agent_id` check in `emit_block` (done: the trait's default
  `hook_callback` holds it).
- README rows "no: failures unclassified (LOO-518)" and "(Task 98edc6)", and
  the sentence "A 'no' for OpenCode or Pi means the step stops with the
  provider's error (limits)" (done). Those two Tasks close with this change.
- Every `["claude", "codex", "opencode", "pi"]` literal that
  `HarnessKind::ALL` or `program()` can answer (done; the Swift picker gained
  the fifth name).

What must survive: Pi and OpenCode still retry their own transient failures
(`routing_failure`, not `provider_failure`); Claude and Codex classification
is unchanged line for line.

## Choices made for review

- **One `agy` process per turn**, not one long-lived `--input-format
  stream-json` process (which runs a turn per stdin line) and not `agy acp`.
  Interrupt is a process-group kill either way, `--conversation` resume is
  seen working, and OpenCode's driver is the model to copy.
- **Hook harness named by flag** (`--harness antigravity`), not sniffed from
  camelCase keys.
- **The harness is asked about the error event, not about a line.** Passed
  over: keeping `classify_failure(line)` over the text `drive::run` composes
  (works, but the harness parses lf's own formatting and a multi-line
  provider error is cut at its newlines), and putting a typed failure on
  `ConversationEvent::Error` (the truest shape, but that is a recorded wire
  type in `loopflow-model`; a separate change).
- **lf reads the hook payload; the script only gates.** Passed over: the
  script choosing the moment and filtering subagents in shell. That costs an
  lf start at each subagent's first model call and buys one place that reads
  the payload, under test in Rust.
- **One name, `antigravity`.** `-a agy` is not accepted; the missing-CLI
  message names the command. Passed over: `agy` as an alias (one idea under
  two names in config and history).
- **Usage counts the parent conversation only** (see Exclusions). Nothing in
  `lf history usage` marks the number as partial.
- **Trust by session `--settings`** for lf-created worktrees only.
- **A Task's conversation on Antigravity keeps `agy`'s own permission
  prompts**, so its first `lf` command asks before running outside the
  sandbox, and the docs say so. Passed over: `yolo` for every Task
  conversation on Antigravity (interactive sessions use each vendor's own
  prompts on every other harness), and a session grant for `lf` through
  `agy`'s `permissions` settings (not explored; a separate outcome).
- **`hook_callback` may claim the thread file.** Passed over: a pure
  `hook_callback` with `emit_block` owning every write, which needs one more
  trait answer to save one `fs::write`.
- **A provider start publishes the resume reference when the launch
  published none, on every harness.** Only a provider-named terminal reaches
  it today. Passed over: limiting it to `ThreadName::Reported`.
- **`agy` is assumed to retry transient failures itself** (`routing_failure`
  after its own phrases). If a live 503 shows it does not, switch to
  `provider_failure`: one line.

## Slices

1. **Built (see "Built").** A batch on real `agy`; classification at the
   error event on every harness.
2. **Built (see "Built").** A worker hears lf: launch folder, gate script,
   `hook_callback` / `hook_reply` behind `--harness`.
3. **Built (see "Built").** Usage: `transcript/antigravity.rs`, `step-<n>`
   turn names, `locate_transcript`; `lf history usage --task` shows the step.
4. **Built (see "Built").** The terminal: `open` (`-i`, trust setting,
   resume), the thread reported through `reported_thread_path`, launch
   matrix chat and work-session cells.
5. **Built (see "Built").** Names.

## Not checked

- Whether a subagent's hook sees its own id or the parent's in
  `ANTIGRAVITY_CONVERSATION_ID`. Nothing depends on it: lf compares the
  payload's `conversationId`.

- Whether `agy` retries transient failures itself (see Choices).
- Whether `agy` always prints `"invocationNum":N` without a space. The gate
  fails open if it does not.
- An exhausted-quota refusal, live.
- Whether `trustedWorkspaces` wants the path as given or canonical: both are
  passed and the dialog did not appear.
- A bare launch in a held terminal (the launcher becomes `agy`, so no lf
  process reads the thread file at exit) is in bare Codex's position. A
  managed one was run and fixed (see QA above).
- A launch from a subdirectory of an lf-created worktree: the trusted folder
  is the launch cwd, not the worktree root.

## Unrelated to this change

A headless `ask_question` ends the turn `SUCCESS` and the Graph advances on
an answer that restates the question. Claude and Codex have the same gap;
Task 48edb9 owns all three.

"Agent execution unavailable: agent opencode:opencode/gemini-4-carbon uses
harness OpenCode, which does not support a managed execution boundary" comes
from the installed `lf 0.16.0`. `main` removed that refusal in #1985; `lf
install` from `main` clears it.
