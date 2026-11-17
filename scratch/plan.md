# LOO-924 follow-through: three launch states and a parsed agent choice

Direction from concept review of PR #1966 (`scratch/concept-review.html`).
Jack, 2026-11-17, quoting the review: "Two product ideas still lack one owner in
the code: whether a launch is managed, and which agent was chosen." / "this
seems worth fixing". Both go in this PR.

## 1. Three launch states, one belonging fact

Design note with prior art and every quote: `scratch/launch-states.html`.
Jack, 2026-11-17:

- "lf skill/graph: Bare" / "lf task run ...: not bare"
- "we should try to avoid having too many different knobs that allow you to
  confuse the system"
- "for the purposes of e.g. usage numbers, bare runs in a task checkout do
  count against a task worker"
- "workers are always headless, so there is basically no point to making them
  managed"
- "maybe we want bare [better name available?], managed, and detached as the
  three states?"
- "the worker run should definitely get the task steers"
- "i think its reasoanble if LF_AS or --task or in the worktree, you get the
  steers as well, but at that point i think simplicity and clarity and
  reliability of the system are most importanat"

### Two questions, each with one owner

**Which Task does this run belong to?** One fact, read once. The checkout,
`--task` and `LF_AS` are three ways to say it and mean exactly the same
thing. Belonging gives two things, always together: the run counts toward the
Task (usage, time, ancestry) and its Session gets the Task's steers. Belonging
never decides the state below. `WorkSource` variants stop changing behavior;
delete `WorkSource::is_managed`, and delete the variants too if nothing else
reads them.

**Who keeps it running?** Decided by the command alone, in
`transport::delegate`, and returned as one value:

```rust
enum Run { Bare(Mode), Managed, Worker }   // replaces Holder x Place x Mode
```

| command | state | mode | lives until |
|---|---|---|---|
| `lf`, `lf <skill>`, `lf graph <g>`, `lf -b ...`, with or without `--task`/`LF_AS`, in any checkout | Bare | interactive or headless | its terminal closes or its caller exits |
| a Task, Wave or repository conversation: `lf resume [ID] --task/--wave`, a Session bound with `lf session bind`, a primary conversation | Managed | interactive | someone quits it; survives its viewer |
| `lf task run` and each Graph step it starts | Worker | headless | the step ends; outlives the command that started it |

Names chosen for review: `Worker`, not "detached", because lf already shows
`"state": "detached"` on an interactive Session nobody is viewing. One word
for the middle state in code and docs: `Managed` (rename `Place::Held` uses
that mean this state; "held terminal" stays as the name of the terminal).

### What each state does

| | counts toward its Task | gets Task steers | reports turns to the Machine server | terminal |
|---|---|---|---|---|
| Bare | yes | yes | no | the caller's, or none |
| Managed | yes | yes | yes | held by the server |
| Worker | yes | yes | yes, as Graph steps do today | none |

Worker keeps today's turn reports. The server records Codex turns and
refreshes planning from them, and nothing here has shown that is safe to drop.
Reliability first: Worker differs from Managed in having no held terminal and
no viewer, not in what is recorded.

### Behavior that changes on purpose

- `LF_AS` and `--task` on a skill or graph launch are Bare. Today `--task`
  makes it Managed and `LF_AS` makes it report to the server while running
  here. Replace
  `transport::tests::inherited_declaration_keeps_server_reporting_without_naming_work`
  with tests of the table above.
- A Bare run that belongs to a Task claims that Task's steers. Check
  `steer.rs` and the hook: if a checkout-only Session does not watch its
  Task's marker today, make it. One `steer_delivered` row per Session and
  steer already lets several Sessions read the same steer.
- A store that can't be read at resume no longer fails the reopen
  (`native_session.rs:177`); the conversation opens Managed, as `works()`
  already assumes.

### What it deletes

`Holder`, `Place`, `transport::holder` and its four call sites,
`Delegation::Here { mode }` (it carries `Run`), `WorkSource::is_managed`, the
`named_work` parameter threaded through `delegate`, and `Launch.holder` (it
carries `Run`). `ThreadNaming { caller, server }` keys on `Run` instead.
In-process callers that never go through `delegate` (commit message, PR text,
replay) are `Bare(Headless)`.

Update `server/README.md` "Turn and tool boundaries", `transport/README.md`
and `harness/README.md` to this vocabulary; they still name
`AgentConfig::holder`. `lf pr_landing` already clears `LF_AS` and passes
`--task` to the worker it starts; that worker is started by Task delivery and
is a Worker.

### Demo

In a Task checkout: `lf -b research '...' &`, then `lf steer <TASK> 'note'`.
The bare run prints the steer after its next tool call, `lf ps` shows it as
bare, and the Task's usage includes it. `lf task run <TASK>` shows its step
as a worker and gets the same steer.

## 2. The agent choice parsed once

Today `AgentConfig.agent: Option<String>` (`"claude:opus"`) is re-parsed by
`parse_agent` at 44 production sites and `HarnessKind::parse` at 36, with
`claude_model` and `codex_service_tier` sitting on the shared config.
`agent/mod.rs:1394` can still fail at headless launch with "unsupported
session harness".

- Add `Agent { harness: HarnessKind, model: Option<String> }`, parsed once
  where config resolves. An unknown harness becomes a config/selection error.
- `AgentConfig` carries the resolved `Agent`, and `Launch.harness` comes from
  it. Delete the re-parses. `HarnessKind::parse` remains only at input
  boundaries: CLI `-a`, config files, stored Session provider rows.
- Fold `claude_model` into `Agent.model` where it means the same thing (the
  `resolved_agent` special case). If a per-harness option still has to live on
  the config, keep it, and say in the PR why.
- Wire shapes (`--json`, stored rows) keep their string form. Parse at the
  boundary; don't change DTOs.

## Checks

- `cargo test -p loopflow-execution --lib transport::` and
  `harness::launch_matrix` (16-cell matrix unchanged).
- `cargo test -p loopflow-execution --lib config::` for agent selection and
  fallback order.
- `cargo clippy --all-targets -- -D warnings`.

## Delete — do not maintain

- `Holder`, `Place`, `transport::holder` and its independent Work reads/tests; `Run` from delegation owns the answer.
- String agent parsing in launch consumers and `AgentConfig::resolved_agent`; typed selections own harness and model. Keep strings at config, CLI and persistence boundaries.

## Not in this PR

- Transcript methods stay on `Harness`. Asked whether to split them into a
  per-harness `Transcripts` impl, Jack said "no opinion in this one". One
  trait keeps the change smaller, and it is still one file per provider.
