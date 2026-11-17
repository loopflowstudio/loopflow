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

- `LF_AS` and `--task` on a skill or graph launch are Bare. Previously `--task`
  made it Managed and `LF_AS` made it report to the server while running
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
- Reopening is Managed whatever started the Session. `SqliteStore::is_bare`
  is gone, so a conversation begun with `lf <skill>` at a terminal and later
  opened with `lf resume` is held by the server and outlives its viewer;
  on main it ran in the terminal and ended with it. Show this at human
  review: it is the one change here a person meets without touching a Task.
- `lf resume <ID> 'message'` is a Bare headless turn, with or without
  `--task`. On main `--task` made that turn report to the server.

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
The bare run prints the steer after its next tool call and the Task's usage
includes it. `lf task run <TASK>` gets the same steer in its step. Then
`lf research` at a terminal, quit, `lf resume`: close the viewer and the
conversation is still there.

`Run` is not stored or sent, so no process listing names the state; the
launch matrix prints it per cell.

## 2. The agent choice parsed once

`AgentConfig` carries `Agent { harness: HarnessKind, model: Option<String> }`.
Config resolution parses the choice and fallback order before launch. Launch
consumers use the typed selection; native admission and missing-CLI errors no
longer parse it again. Claude's configured model folds into `Agent.model`.

CLI, authored preferences and persistence retain string forms at their input
boundaries. Wire DTOs are unchanged. `chrome` and `codex_service_tier` remain
separate provider launch options because neither is a model choice.

## Next implement slice: say what Worker is

Concept review of `cb6aee93e5`. The code holds the model: `Run` has one
definition, `delegate` returns it, `Holder`/`Place`/`is_managed`/`named_work`
are gone, and attribution no longer reaches lifetime. Two places where the
words and the code disagree; both are fixed in the words.

1. **Worker is wider than "a Task Graph step".** Three things are Workers:
   steps under `lf task run`, the `ci-fix` run landing starts
   (`pr_landing.rs`, `LF_GRAPH_STEP=worker`), and each Discord answer for a
   Wave (`run::answer_bound`, built in-process, never through `delegate`).
   What they share: lf started them for a Task or Wave, nobody views them,
   and the server records their turns. The table above and
   `transport/README.md`, `harness/README.md`, `server/README.md` say
   "headless Task Graph step"; make them say that definition and list all
   three. `answer_bound` keeps `Run::Worker`: it reported turns before this
   branch, and the same caution that kept Worker's turn reports applies.
   Give it a one-line comment naming why it is not `Bare(Headless)`, since
   it is the only `Run` chosen outside `delegate` that isn't Bare.
2. **Resume with a message is Bare.** `transport/README.md` says every
   resume is Managed. Add the row: `lf resume <ID> 'message'` is
   `Bare(Headless)`; `lf resume` with no message is Managed.

No code path changes. After this slice nothing remains for implement; gate
owns the broader transport, configuration and Task performance suites, demo
owns live provider steering.

Managed native fixtures open through `resume`; direct skills remain Bare
with Task attribution. WorkSource remains for context provenance and primary
selection, with no launch-lifetime authority. Fresh resume carries invocation
settings and a literal inline-prompt delimiter across exec.

Checks, 2026-11-17 (inherited `LF_*` cleared):

- `cargo build -p loopflow --bin lf`: passed.
- Focused `cargo nextest run --lib` selections: 11 core tests passed, including
  the 16-cell launch matrix, agent fallback order, steer attribution and query plans.
- Focused CLI nextest selections: startup, server-free Task attribution,
  documented commands, Codex service tiers and document bytes, Claude/Codex/OpenCode
  launch/resume policy, linked worktrees and all three native Pi cases passed.
- Task-worker, launch-mode and steer-capability nextest selections: 3 passed.
  macOS debug libtest linking warned about the size of `__eh_frame`; linking succeeded.
- The native launch/resume policy test passed after moving its preparation
  fixture into execution's shared test support.
- `cargo clippy --all-targets -- -D warnings`: passed on the final tree.

## Delete — do not maintain

- `Holder`, `Place`, `transport::holder` and its independent Work reads/tests; `Run` from delegation owns the answer.
- String agent parsing in launch consumers and `AgentConfig::resolved_agent`; typed selections own harness and model. Keep strings at config, CLI and persistence boundaries.

## Not in this PR

- A launch-state label in `lf ps`. It needs `Run` recorded at launch, not
  worked out again from argv. Nothing in this change needs it.

- Transcript methods stay on `Harness`. Asked whether to split them into a
  per-harness `Transcripts` impl, Jack said "no opinion in this one". One
  trait keeps the change smaller, and it is still one file per provider.
