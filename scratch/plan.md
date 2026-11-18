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

## Why the fix is wider than the brief: Codex's refusal never classified

Dropping the early return in `_fallthrough_rungs`, as the brief had it, would
not have fixed the reported case. Codex's message uses a typographic
apostrophe (`You’ve`, U+2019) and the text classifier matched `you've` with
`'`. What showed it, before the change:

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
- Every test spelled the message with `'`, which is why nothing caught it.

Codex also states the cause in a field. `codex app-server
generate-json-schema` (codex-cli 0.170.0): `TurnError.codexErrorInfo` is an
enum with `usageLimitExceeded`, `modelNotFound` and `unauthorized`. Both are
read: the fold is what the logs prove, the field is what the schema promises.

## What exists

**One rule, one order, one loop.** A headless launch follows its agent order
when a provider refuses it; only a Graph step also waits for capacity.

- **Order.** `Config::resolve_agents` gives the launch agent and its
  fallbacks, including a bare harness's other models. `_run_with_retries`
  (`agent/mod.rs`) walks `launch.agent`, then `launch.fallback_agents`,
  deduplicated. `_fallthrough_rungs` is deleted. A caller that builds
  `AgentConfig::new(agent)` by hand (`lf replay`) runs that agent only.
- **Waiting.** `ProcessConfig.wait_for_capacity` gates only the
  5/15/30/60-minute wait; `commands/run.rs` sets it for a Graph step alone.
- **Classification.** `codex_decode::turn_error_code` reads `codexErrorInfo`
  at both sites (`error`, `turn/completed`) and emits `provider_rate_limited`
  for `usageLimitExceeded`. The text classifier (`agent/mod.rs`) and
  `ops/release.rs` fold `’` to `'` before matching.
- **PR copy and commit messages** build their launch in
  `ops::commit::message_launch`: the skill's settings, the chosen agent, then
  `resolve_agents`. The commit loop over agents is deleted. `-a` pins;
  `MessageAgent::Fallback(Some(agent))` is the skill's own order, else the
  run's agent. A step commit with no message still commits as
  `lf <step>: checkpoint`.
- **Moving provider before any reply.**
  `conversation_log::continuation_of_work` returns `None` when the transcript
  holds no agent item, and `_climb` then gives the next provider the launch's
  own `task_prompt` and `skill_invocation`. A transcript with replies imports
  as before; one that cannot be read gets `AGENT_FAILOVER_PROMPT` plus the
  original task. `continuation` keeps its signature for `lf session new -a`
  and resume. The reason: a PR prompt carries up to 20,000 characters of diff
  and the import keeps the log's first 6,000 and last 14,000.
- **The Session follows the agent.** A retry that changes provider writes
  the Session's provider and model (`SqliteStore::record_session_agent`,
  called from `LfProcessRuntime::retry_agent_process`). Before this a move
  from Codex to Claude could not finish: stopping the Claude that followed
  asked Codex's close check for an endpoint and failed with `Codex
  AgentProcess connection is unavailable`. Claude to Codex had worked only
  because the default close check asks nothing.
- **Recording.** `record_agent_limit_recovery` inserts only for a Graph
  step's Process. A command's move is the stderr line and nothing in the
  store.
- **Docs** (`docs/config.md`, `subscriptions.md`, `troubleshooting.md`,
  `orchestration.md`) and the `agents` setting description say headless work
  moves and only a Graph step waits.

| Caller | Behavior |
| --- | --- |
| Graph step | moves, on Codex too; waits |
| `lf -b <skill>`, `lf -b graph` chats and batches | moves, never waits |
| `lf pr open` / `lf pr submit` copy | moves; `skill.pr-message.*` leads |
| `lf commit`, step commits | shared loop; `skill.commit-message.*` leads |
| `lf sync` conflict resolution | moves |
| `-a` or `agent:` | pinned; a bare harness tries its other models |
| `lf replay` | the recorded agent, nothing else |
| Interactive Sessions | never reach `run_agent` |

Pi and OpenCode do not classify failures (`classifies_failures: false`), so a
refusal there stops, and an order that leads with one never moves.

Stops:

- Order spent: exit non-zero; stderr is the provider's reason, then
  `Nothing is left in its order after a, b, c; add another agent to \`agents\``.
- Pinned: the reason, then the `No fallback remains…` advice.
- Revoked login: skips that provider's other models; with no other provider,
  the sign-in advice.
- Codex's usage limit is account-wide, so `codex:a → codex:b → claude` spends
  one extra refused launch on `codex:b`. Same as a Graph step; left alone.

Tests: `agent::tests::headless_work_outside_a_graph_moves_through_its_order_and_never_waits`,
`codex_decode::tests::a_spent_quota_is_a_usage_limit_wherever_codex_reports_it`,
`agent_tests` (`model_unavailable…`), `commit_tests::a_commit_message…`
and `pr_tests` (a PR whose title came from the Claude stand-in after the Codex
stand-in refused). Every test spells the refusal `You’ve`. The Codex stand-in
(`support::refused_codex_script`) leaves `codexErrorInfo` null, so the
integration tests cover the apostrophe fold and the unit test covers the field.

Changed for review: commit messages used to move to the next agent on any
failure (timeout, non-zero exit, a reply with no message). They now move on a
usage limit, an unavailable model or a revoked login only; transient failures
retry four times on the same agent.

## Outside this change

- Waiting for capacity outside a Graph step.
- Recording command moves in the store or showing them in `lf task status`.
- Failure classification for Pi and OpenCode.
- Interactive Sessions.

## What remains

No building. Lint and gate run the checks below that implement did not.

Unobserved against the provider, each noted in `scratch/questions.md`:

- Whether a live Codex usage refusal sets `codexErrorInfo`. No trace of one
  exists; the checked-in traces have `null` for a stream disconnect.
- A move inside Codex (`codex:a → codex:b`) resumes the refused thread with
  `LIMIT_FAILOVER_PROMPT`. No Codex refusal classified before this change, so
  that path has never run against the provider.

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

- `cargo test -p loopflow --test agent_tests -- model_unavailable` — passed; implement.
- `cargo test -p loopflow --lib pr_tests::pr_` — passed; implement, then gate.
- `cargo test -p loopflow --test integration commit_tests::a_commit_message` — passed; implement.
- `cargo test -p loopflow-execution --lib -- agent::tests harness::codex_decode ops::task_execution ops::human_session::conversation_log` — passed; implement.
- `cargo clippy --all-targets -- -D warnings` — not run; lint.
- `test(store::) | test(session_record::capture_tests::) | test(process::runtime::tests::)` — not run; gate. The retry path now writes the Session's provider.
