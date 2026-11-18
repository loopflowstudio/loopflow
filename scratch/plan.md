# LOO-983: headless commands move to the next agent when a usage limit refuses the first

## Outcome

With `agents: [codex:gpt-6.2-sol, codex:gpt-6.2-astra, claude:fable, claude:opus]`
and Codex out of quota, `lf pr open`, `lf commit`, `lf -b <skill>` and sync
conflict resolution print one line per move
(`codex:gpt-6.2-sol: provider usage limit; continuing on codex:gpt-6.2-astra`)
and finish on Claude. They fail only when the order is spent, with the
provider's reason and `Nothing is left in its order after …`. Nothing outside a
Graph step waits for capacity.

`skill.pr-message.default_agent: claude:haiku` leads PR copy, and the rest of
the order follows it, the way `commit-message` already works.

## Finding that reshapes the work: Codex's refusal is not classified at all

The brief's change (drop the early return in `_fallthrough_rungs`) is needed
but would not fix the reported case. Codex's message uses a typographic
apostrophe (`You’ve`, U+2019); `subscription_limit_text`
(`rust/loopflow-execution/src/agent/mod.rs:1352`) matches `you've` with `'`.

- `~/.lf/logs`: 81 occurrences of `You’ve hit your usage limit`, none with `'`.
- Graph steps hit it too: `~/.lf/logs/graph-f5c090aa-….log` (2026-11-18) ends
  `Error: agent exited with code 1: codex_error: You’ve hit your usage limit. …`.
  A classified failure would read `agent stopped after provider usage limit`
  (`rust/loopflow/src/lf/commands/run.rs:833`) and carry the stop advice.
- The only moves ever logged are Claude's (`claude: provider usage limit;
  continuing on claude:opus`). Claude is classified by its structured
  `provider_rate_limited` event (`harness/claude.rs:314`, `harness/drive.rs:81`),
  not by text.
- Both errors quoted in the brief lack the advice line `_run_with_retries`
  appends to a classified limit, so they were unclassified as well.
- Every test spells the message with `'` (`agent/mod.rs:2232`,
  `ops/task_execution.rs:459`, `tests/work/task_graph_launch_tests.rs:1878`),
  which is why nothing caught it.

Codex states the cause in a field. `codex app-server generate-json-schema`
(codex-cli 0.170.0): `TurnError.codexErrorInfo` is an enum with
`usageLimitExceeded`, `modelNotFound` and `unauthorized`.
`harness/codex_decode.rs` reads only `/error/message` (the `error`
notification, line 870) and `/turn/error/message` (`turn/completed`, line 749).
The checked-in traces have `codexErrorInfo: null` for a stream disconnect; no
trace of a live usage refusal exists, so the field being set on one is
unobserved.

Decision: do both. The decoder reads `codexErrorInfo` at both sites and emits
`provider_rate_limited` for `usageLimitExceeded`, the code `drive.rs` already
turns into `AgentFailure::UsageLimit`. The text classifiers fold `’` to `'`
before matching, which is the part the logs prove. `ops/release.rs:2468` keeps
its own copy of the phrase list with the same `'`; it uses the shared
classifier or gets the same fold.

## Design

**One rule.** A headless launch follows its agent order when a provider
refuses it. Only a Graph step also waits for capacity. Three things carry that
rule today (the shared loop behind a flag named for waiting, a second loop in
`ops/commit.rs`, nothing in `ops/pr.rs`) and two places work out the order.
After this change there is one of each.

**The order is resolved once, in `Config::resolve_agents`.** It already
returns the launch agent and its fallbacks, and already gives a bare harness
with no order its other models (`config.rs:522`). `_fallthrough_rungs`
repeats that expansion (`agent/mod.rs:1183`) and disagrees with it: it tests
`model.is_none()` after `with_configured_model`, `resolve_agents` before. So
`_fallthrough_rungs` goes whole. The loop walks `launch.agent`, then
`launch.fallback_agents`, deduplicated, and nothing else. Every caller that
wants moves gets them by resolving: `prompt/process.rs:98` already does
(`lf -b <skill>`, Graph steps, sync conflicts), PR copy and commit messages
start to. A caller that builds `AgentConfig::new(agent)` by hand runs that
agent only.

**`wait_for_capacity` means what it says.** It gates only the
5/15/30/60-minute wait, which `_run_with_retries` already checks on its own
(`agent/mod.rs:1110`). Its doc comment (`agent/mod.rs:451`) loses the half
about moving.

**PR copy** (`ops/pr.rs` `generate_pr_copy`, line 543). Resolve the
`pr-message` skill's settings with `config::settings::resolve(&config,
Some(&skill))`. An agent passed in (`PrOptions.agent`, from `-a`) becomes the
resolved config's `agent`. Then one `config.resolve_agents()` gives the launch
agent and its fallbacks, so `-a` and `agent:` stay pinned through
`Config::fallback_agents` with no branch in `pr.rs`.

**Commit messages** (`ops/commit.rs` `generate_commit_message`, line 256). The
`for agent in agents` loop goes; one `run_agent` with `fallback_agents` from
the same resolution replaces it. `MessageAgent::Chosen(Some)` pins.
`MessageAgent::Fallback(Some(agent))` keeps its meaning: the skill's own order,
else the run's agent.

Made for review: the commit loop moved to the next agent on any failure (a
timeout, a non-zero exit, a reply with no message). After the fold it moves on
a usage limit, an unavailable model or a revoked login only; transient
failures still retry four times on the same agent. Passed over: keeping the
outer loop, which would run each agent's whole fallback order once per outer
agent. A step commit that gets no message still commits as
`lf <step>: checkpoint`.

**Moving provider when nothing has been said.** Every command has a Session:
`run_admitted_agent` begins one when the caller passes no runtime
(`agent/mod.rs:992`) and claims its attachment, so `_history_import` runs for
PR copy and commit messages too. What it returns for a turn refused before any
reply is the wrong first turn for a one-shot prompt: either the transcript is
missing and the next agent is told to "finish any interrupted work", or it is
found and `conversation_log::continuation` sends `Pick up where it left off`
with the log's first 6,000 and last 14,000 characters. A PR prompt carries up
to 20,000 characters of diff, so the middle of the diff is cut and the agent is
pointed at a file under `~/.lf`.

The rule: a conversation with no agent reply has nothing to continue, so the
next provider gets the launch's own `task_prompt` and `skill_invocation`,
unchanged. `continuation` reports "no reply yet" as its own answer
(`Ok(None)`), read from the rendered turns; `_climb` then copies the original
launch's prompt. A transcript with replies imports as now; one that cannot be
read keeps `AGENT_FAILOVER_PROMPT` plus the original task. This holds for a
Graph step refused on its first turn as well, which is the same situation.

Unobserved: a move inside Codex (`codex:a → codex:b`) resumes the refused
thread with `LIMIT_FAILOVER_PROMPT`. No Codex move has ever run, because no
Codex refusal classified. Whether a thread whose first turn was refused holds
the task is unknown; noted in `scratch/questions.md`.

**Recording.** `record_agent_limit_recovery` inserts only for a Graph step's
Process (`rust/loopflow-storage/src/sqlite/graph_inventory.rs:325`). A
command's move is the stderr line and nothing in the store.

## Surfaces

| Caller | Today | After |
| --- | --- | --- |
| Graph step | moves (Claude only, see finding), waits | moves on Codex too, waits |
| `lf -b <skill>`, `lf -b graph` chats and batches | stops | moves, never waits |
| `lf pr open` / `lf pr submit` copy | stops; ignores `skill.pr-message.*` | moves; skill settings lead |
| `lf commit`, step commits | own loop over the order | shared loop |
| `lf sync` conflict resolution | stops | moves |
| `-a` or `agent:` | pinned; bare harness tries its models only in a Graph step | pinned; bare harness tries its models everywhere |
| `lf replay` (`commands/replay.rs`) | the recorded agent | the recorded agent, nothing else: it builds its launch by hand |
| Interactive Sessions | refusal stays visible | unchanged, never reach `run_agent` |

Pi and OpenCode do not classify failures (`classifies_failures: false`), so a
refusal there still stops. An order that leads with one of them never moves.

## Absent and error states

- Order spent: exit non-zero; stderr is the provider's reason, then
  `Nothing is left in its order after a, b, c; add another agent to \`agents\``.
- Pinned: the reason, then the existing `No fallback remains…` advice.
- Revoked login: skips that provider's other models; with no other provider,
  the sign-in advice.
- Codex's usage limit is account-wide, so `codex:a → codex:b → claude` spends
  one extra refused launch on `codex:b`. Same as a Graph step; left alone.

## Exclusions

- Waiting for capacity outside a Graph step.
- Recording command moves in the store or showing them in `lf task status`.
- Failure classification for Pi and OpenCode.
- Interactive Sessions.

## What goes

- `_fallthrough_rungs`, whole. Unit tests in `agent/mod.rs` that leaned on
  its bare-harness expansion (the `pinned` cases near line 2466) pass the
  fallbacks `resolve_agents` would give, or move beside `resolve_agents`.
- The `for agent in agents` loop, `configured` closure and `failure` string in
  `ops/commit.rs`.
- `headless_work_outside_a_graph_stops_at_a_usage_limit_without_moving_or_waiting`
  (`agent/mod.rs:2339`): rewritten as "moves, never waits".
- The `("bare", false, …)` case of
  `model_unavailable_result_falls_through_or_stops_without_spending_quota`
  (`rust/loopflow/tests/agent_tests.rs:193`): it now reaches Opus.
- Doc text saying only a Graph step moves: `agent/mod.rs` comments at 204, 451
  and 1171; `docs/config.md:60`; `docs/troubleshooting.md:289`; the
  `docs/subscriptions.md:62` table row; `docs/orchestration.md:90` if it reads
  as Graph-only. The `agents` description in `config/catalog.rs:274` says "a
  headless step"; reword to cover commands.

Survives: the Graph step's waits and recorded recoveries, pinning by `-a` and
`agent:`, the revoked-login skip, the `checkpoint` step commit.

## Slices

1. **Codex refusals classify, and a command moves.** Fold `’` in the text
   classifiers and read `codexErrorInfo` in `codex_decode.rs`; drop the early
   return. Test in `agent_tests.rs` beside the model-unavailable one: a Codex
   stand-in whose turn fails with the message exactly as Codex sends it
   (`You’ve`), `fallback_agents: [claude]`, `wait_for_capacity: false`; the
   result is Claude's answer, and the prompt the Claude stand-in received is
   the launch's `task_prompt` byte for byte. This is the slice that could prove the plan
   wrong: if the stand-in's refusal does not classify through the real decode
   path, nothing after it matters. Change the `'` in existing fixtures to `’`
   so they match the provider.
2. **PR copy.** Skill settings and fallbacks in `generate_pr_copy`. Two tests
   in `tests/work/pr_tests.rs` through `create_or_update_pr` with `agent:
   None`: `agents: [codex, claude]` with Codex refused publishes Claude's
   title, and the Claude stand-in saw the whole diff; `skill.pr-message.default_agent` picks the stand-in that answers
   while the other would fail the test if launched.
3. **Commit fold.** One launch in `generate_commit_message`, resolved the way
   PR copy is (`Chosen(Some)` sets the resolved config's `agent`); a focused test
   that a refused first agent yields the second's message.
4. **Docs and comments** listed under What goes.

## Docs written ahead of the code

Slice 4's prose is in place and describes the intended behavior; implement
makes it true and still owes the `agent/mod.rs` comments and the `agents`
description in `config/catalog.rs:274`.

- `docs/config.md` "Keep going when a provider runs out": `lf pr open` example
  with the move lines, the spent-order error, commands never wait.
- `docs/subscriptions.md`: refusal table rows for other headless work, Pi and
  OpenCode; a turn refused before any reply restarts from its prompt.
- `docs/troubleshooting.md` "Rate limits".
- `docs/orchestration.md`: `default_agent` for `commit-message` and
  `pr-message`; "headless work", not "a step", moves through `agents`.

The docs state that Pi and OpenCode entries stop at a refusal. `docs/sessions.md`
needed no change.

## Demo

Acceptance check at gate, headless:

```bash
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow --lib pr_tests:: < /dev/null
```

The new test opens a PR whose title came from the Claude stand-in after the
Codex stand-in refused with `You’ve hit your usage limit`. By hand, with Codex
out of quota: `lf pr open` prints the move lines, then the PR URL.

## Checks

- `cargo test -p loopflow-execution --lib agent::` — not run; implement.
- `cargo test -p loopflow --test agent_tests` — not run; implement.
- `cargo test -p loopflow --lib pr_tests::` — not run; implement, then gate.
- `cargo clippy --all-targets -- -D warnings` — not run; lint.
