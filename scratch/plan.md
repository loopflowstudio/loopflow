# Antigravity as a fifth harness

`lf -a antigravity`, `lf task run LOO-12 -a antigravity` and `agents:
[..., antigravity]` work the way they do for Claude, Codex, OpenCode and Pi:
chat, work session, batch and worker; context and steers; resume; transcript
and usage; failover. The CLI is `agy` (Homebrew cask `antigravity-cli`, 1.5.13
here; the process it execs is named `antigravity`).

Two changes ride with it:

- **Failure classification is part of the Harness API.** Each harness answers
  `classify_failure(line)`; the `classifies_failures` capability is gone. A
  spent quota, a revoked login or an unserved model moves a run to the next
  agent on every harness.
- **OpenCode running out of credits fails over.** Today it stops the step.

The demo: `lf task run <Task> -a antigravity` runs a step on real `agy`, the
step hears `lf steer` after its next tool, and `lf history usage --task` shows
its tokens. Then a step whose first agent is `opencode` with no credits moves
to the next agent in `agents` and finishes.

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
| `cargo test -p loopflow-execution --lib agent::tests::` | OpenCode's balance line and Antigravity's quota, sign-in and model lines each move the run to the next agent; Pi still retries nothing | implement, CI |
| `cargo test -p loopflow-execution --lib harness::antigravity` | decoder trace from real 1.5.13 output; a subagent's hook payload gets `{}` | implement, CI |

On real `agy` (needs its sign-in; qa or demo, never a required check): the
demo above.

## Done in this checkout (uncommitted, does not compile yet)

- `harness/mod.rs`: `HarnessKind::Antigravity` (`"antigravity"`), `ALL` has
  five, `Harness::program()` (default: the kind's name; Antigravity answers
  `agy`), required `Harness::classify_failure`, `classifies_failures` removed.
  `pub mod antigravity; mod antigravity_decode;` are declared; neither file
  exists yet.
- `agent/mod.rs`: `_classify_agent_failure(harness, result)` asks the harness
  per stderr line and keeps the old precedence (credential, usage limit, model,
  transient). `routing_failure` (no transient) and `provider_failure` (with
  transient) are the shared phrase lists. `check_cli_available` maps a harness
  name to its program.
- `claude.rs`, `codex.rs`: `provider_failure`. `pi.rs`: `routing_failure` (Pi
  retries transient failures itself; `pi_failure_keeps_exact_history_…` holds
  that). `opencode.rs`: "insufficient balance" is a usage limit, then
  `routing_failure`.

## Left to build

1. `harness/antigravity.rs`: `Antigravity` harness, headless `Driver`, the
   per-launch folder, hook payload adapter.
2. `harness/antigravity_decode.rs`: stream-json lines to `Decoded`.
3. `transcript/antigravity.rs`: `transcript_full.jsonl` to `Turn`s.
4. `lf __context-block --harness <name>`
   (`rust/loopflow/src/lf/commands/context.rs`, `lf/mod.rs`): the named
   harness reads the callback and shapes the reply.
5. Program name: `native_session.rs` (lines near 31, 276, 492, 530) builds
   `Command::new(kind.as_str())`; use `program()`. Generalize the "Codex
   reported no conversation" warning.
6. Every list of harness names: `error.rs`, `config.rs` tests,
   `prompt/process.rs`, `loopflow-test-support/src/lib.rs`,
   `config/catalog.rs` (`yolo`, `chrome` help), `launch_policy_tests.rs`,
   Swift (`WorkspacePanesView.swift`, `SessionProgramStatus.swift`), docs,
   `skills/loopflow`.
7. Tests: a stand-in `agy` under `rust/loopflow-execution/tests/support/`
   (model on `opencode_cli.py`), `harness::launch_matrix` cells,
   `task_graph_launch_tests::a_task_step_runs_as_a_worker_on_every_harness`,
   decoder trace under `harness/testdata/`, a failover test for OpenCode's
   balance line and Antigravity's quota line, `conformance_tests.rs`.
8. `harness/README.md`: the launch table, the Task-step table (five columns),
   the limits row now "yes" for OpenCode and Pi.

## Design

**Headless: one `agy` process per turn**, as OpenCode does.

```
agy --input-format stream-json --output-format stream-json
    [--dangerously-skip-permissions]   # config.skip_permissions
    [--model <slug>] [--conversation <id>] --add-dir <launch folder>
```

One line on stdin, `{"event":"user","message":{"content":"…"}}`, then close
stdin: the turn runs and `agy` exits 0. Interrupt and stop end the process
group (`agent_process::stop`); the next input continues with `--conversation`.

**The provider names the thread** (bare and managed: `ThreadName::Reported`).
Headless reads `conversation_id` from the `init` line. A bare terminal launch
gets it from its first hook through `reported_thread_path`, as bare Codex does.

**Terminal:** `agy -i "<prompt>"` (argv; `validate_terminal_turn`),
`--conversation <id>` to resume, same `--model`, permission and `--add-dir`
flags.

**The launch folder** carries instructions and hooks with nothing written to
`~/.gemini` and no hook file in the checkout root:
`<launch cwd>/.lf/prompts/<stamp>-antigravity/.agents/`

- `AGENTS.md`: `system_prompt_with_structured_replies(config)`.
- `hooks.json` + a hook script: only when the launch has a
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

- The hook script keeps the launch's conversation: the first `PreInvocation`
  writes its `conversationId` beside `hooks.json` (the hook's cwd). **A
  payload from any other conversation gets `{}` and starts no lf.** Subagents
  run the same hooks under their own id (seen live, below); without this a
  subagent's `Stop` would end the worker's turn and a subagent would claim the
  Session's steers. Claude's hook does the same on `agent_id`.
- First call of the launch: `lf __context-block --moment start`. Later calls:
  `invocationNum` 0 opens a turn (managed: `--moment tool`, ungated);
  otherwise the gated `--moment steer`, with the marker gate `hook_settings`
  builds. The script reads `"invocationNum":0` with a shell `case`; a quote
  inside `lastUserInput` is escaped, so user text cannot match it.
- `Stop` (managed only): `--moment stop`.
- The script always exits 0 and prints `{}` when lf prints nothing.
- The script passes `--harness antigravity`. `emit_block` asks that harness
  to read the callback and to shape the reply, two `Harness` methods whose
  defaults are today's Claude/Codex behavior (`hook_callback`: the payload as
  sent; `hook_reply`: `hookSpecificOutput.additionalContext`). Antigravity's
  read `conversationId` → `session_id`, name the event `SessionStart` /
  `UserPromptSubmit` / `PostToolUse` / `Stop`, set `source` `startup` when
  `initialNumSteps <= 1`, else `resume`, and answer
  `{"injectSteps":[{"systemMessage":{"systemMessage":"<text>"}}]}`. The
  server receives the normalized callback, so nothing past `emit_block`
  learns a fifth payload shape.
- A managed start is also the first prompt: after the Start boundary, report
  a Tool boundary named `UserPromptSubmit` so the server marks the turn
  working.

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
`tool_calls[]`); the row after a tool call (`GENERIC`, or a named type) is
its result. A `PLANNER_RESPONSE` with no `tool_calls` does not end the turn:
a subagent's report or a timer arrives as a `SYSTEM_MESSAGE` and the model
answers again (seen live: steps 9, 10, 11 of one turn). The turn's answer is
its last `PLANNER_RESPONSE` with `content`. Rows carry no cwd, so
`recent_transcripts` answers nothing (see Exclusions).

**Failures** (`classify_failure`), then `routing_failure`:

- usage limit: "exhausted your quota", "quota reached", `QUOTA_EXHAUSTED`,
  `RESOURCE_EXHAUSTED`, "usage limit reached", "out of credits"
- credential: "authentication required", "authentication failed"
- model: "is not recognized as a known model"

Antigravity's phrases other than the model and authentication lines come from
the binary's strings, not from a refusal seen live.

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

## Absent and error states

| state | what happens |
|---|---|
| `agy` not installed | passed over for the next agent; named once when it was the only choice (`check_cli_available` through `program()`) |
| not signed in | `result.status` `ERROR`, stderr "authentication required": `CredentialInvalidated`, next agent. Exit is 0 when stdin is `/dev/null`, so the driver fails the turn on `status`, not on the exit code |
| model not served | `ModelUnavailable`, next agent |
| quota spent | `UsageLimit`, next agent (phrases from the binary, not seen live) |
| `agy` exits with no `result` line | the turn fails with the last stderr line as its reason, as OpenCode's does |
| no `init` line before the failure | no thread recorded; the turn gets a UUID name; nothing to resume |
| hook cannot start lf, or the server does not answer | the script prints `{}` and exits 0; a recording gap in `logs/lf.log`, never a failed turn |
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
- README rows "no: failures unclassified (LOO-518)" and "(Task 98edc6)", and
  the sentence "A 'no' for OpenCode or Pi means the step stops with the
  provider's error (limits)". Those two Tasks close with this change.
- Every `Command::new(kind.as_str())` and `["claude", "codex", "opencode",
  "pi"]` literal that `HarnessKind::ALL` or `program()` can answer: the Swift
  picker reads its list from the wire if one already carries it, else gains
  the fifth name.

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
- **Trust by session `--settings`** for lf-created worktrees only.
- **`agy` is assumed to retry transient failures itself** (`routing_failure`
  after its own phrases). If a live 503 shows it does not, switch to
  `provider_failure`: one line.

## Slices

1. **A batch on real `agy`, and the tree compiles.** `antigravity.rs` with
   the `Driver` and `classify_failure`, `antigravity_decode.rs`, a transcript
   reader that returns no turns yet, `native_session.rs` on `program()`,
   `open` returning "not built yet". Stand-in `agy`
   (`tests/support/antigravity_cli.py`) and the failover tests for OpenCode's
   balance line and Antigravity's three refusals.
   `lf -b default 'reply pong' -a antigravity` prints pong and records the
   conversation id; `-a antigravity:no-such-model` with a second agent lands
   on the second. This is the slice that can prove the plan wrong: it shows
   whether one process per turn, `status`-not-exit failure and the reported
   thread fit `drive::run` without a new concept.
2. **A worker hears lf.** The launch folder, the hook script with the
   conversation filter, `--harness` on `__context-block`.
   `lf task run <Task> -a antigravity`, then `lf steer` mid-step; the worker
   test's Antigravity cell. A second resume turn shows `--conversation` with a
   fresh launch folder.
3. **Usage.** `transcript/antigravity.rs`, `step-<n>` turn names,
   `locate_transcript`; `lf history usage --task` shows the step.
4. **The terminal.** `open` (`-i`, trust setting, resume), bare thread report
   through `reported_thread_path`, `recognizes` (`agy` or `antigravity`),
   launch matrix chat and work-session cells.
5. **Names.** Every list in "Left to build" 6, Swift, docs, `skills/loopflow`,
   README tables with five columns.

## Not checked

- Whether `agy` retries transient failures itself (see Choices).
- An exhausted-quota refusal, live.
- Whether `trustedWorkspaces` wants the path as given or canonical: the probe
  passed both `/tmp/…` and `/private/tmp/…` and the dialog did not appear.
  Pass both.

## Unrelated to this change

"Agent execution unavailable: agent opencode:opencode/gemini-4-carbon uses
harness OpenCode, which does not support a managed execution boundary" comes
from the installed `lf 0.16.0`. `main` removed that refusal in #1985; `lf
install` from `main` clears it.
