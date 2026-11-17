# LOO-924 follow-through: three launch states and a parsed agent choice

Direction from concept review of PR #1966
(`cd0977a62f:scratch/concept-review.html`; the working file now reviews Session commands).
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
| `lf task run` steps, landing's `ci-fix` run, Discord Wave answers | Worker | headless | the run ends; outlives the command that started it |

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
- `lf resume` connects to an existing Session and keeps the state that
  Session has. A conversation that started Bare (`lf`, `lf <skill>` at a
  terminal) reopens Bare, in the terminal, and ends with it, as on main. A
  Task, Wave or repository conversation reopens Managed. Jack, 2026-11-17:
  "resume connects to existing sessions". The first pass made every reopen
  Managed and deleted `SqliteStore::is_bare`; undo that. The Session row is
  the one place this is read, inside `delegate`, into `Run`.
- `lf -b resume <ID> --message TEXT` keeps main's behavior in this PR: one
  headless turn. Jack: "lf resume shouldnt take messages, i dont think?" /
  "that seems like a mixed API". Taking the message off `resume` and giving
  "send one turn" its own command is a separate Task (Graph retries in
  `graph.rs`, docs and `.lf/qa/provider-switching.md` use it); don't build on
  `--message` here or add a second special case for it.

### What it deletes

`Holder`, `Place`, `transport::holder` and its four call sites,
`Delegation::Here { mode }` (it carries `Run`), `WorkSource::is_managed`, the
`named_work` parameter threaded through `delegate`, and `Launch.holder` (it
carries `Run`). `ThreadNaming { caller, server }` keys on `Run` instead.
In-process callers that never go through `delegate` (commit message, PR text,
replay) are `Bare(Headless)`.


### Demo

In a Task checkout: `lf -b research '...' &`, then `lf steer <TASK> 'note'`.
The bare run prints the steer after its next tool call and the Task's usage
includes it. `lf task run <TASK>` gets the same steer in its step. Then
`lf research` at a terminal, quit, `lf resume`: the Bare conversation returns
in that terminal. Open `lf resume --task <TASK>`: closing its viewer leaves the
Managed conversation running.

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

## Worker and resume, in the words

Concept review of `cb6aee93e5` found the docs narrower than the code; realign
fixed the words, no code path changed. Worker is any headless run lf starts
for a Task or Wave that nobody views and whose turns the server records:
`lf task run` steps, the `ci-fix` run `pr_landing.rs` starts
(`LF_GRAPH_STEP=worker`), and `run::answer_bound` for Discord Wave answers,
the one `Run` chosen outside `delegate` that isn't Bare (commented in
`lf/commands/run.rs`). `lf -b resume <ID> --message` is `Bare(Headless)`; with no
message, resume keeps the Session's state. The transport, harness and server READMEs say so.

Resume now reads the Session's binding and primary scope once in `delegate`,
then carries the resulting `Run` through native launch. A held child keeps the
parent's Managed decision without rereading it. `SqliteStore::is_bare`
uses `bound_at` and primary scope, not attribution provenance: a declared Bare
launch must remain Bare when reopened. An unreadable store logs a recording
gap and retains the Managed fallback. `runs_in_its_terminal` shares the same
classification so a managed conversation cannot be treated as an independent
terminal launch while its held terminal is absent.

Gate owns the broader transport, configuration and Task performance suites;
demo owns live provider steering and the two resume lifetimes.

Managed native fixtures use a real Task binding and open through `resume`,
retaining their selected cwd and configuration; direct skills remain Bare
with Task attribution. WorkSource remains for context provenance and primary
selection, with no launch-lifetime authority. Fresh resume carries invocation
settings and a literal inline-prompt delimiter across exec.

Earlier build, launch matrix, agent fallback, steering, native launch/resume and
lint results: `cd0977a62f:scratch/plan.md`. Current resume checks follow below.

- `cargo nextest run -p loopflow -p loopflow-execution --lib --test session_launch_contract -E 'test(transport::launch_mode_tests::) | test(terminal_launch_and_resume_use_current_policy_and_preserve_native_identity) | test(pi_native_callbacks_retain_identity_final_output_and_usage)'`: 7 passed, 2026-11-17, inherited `LF_*` cleared; macOS libtest linking warned about `__eh_frame` size and succeeded.
- `cargo nextest run -p loopflow --test integration -E 'test(documented_commands::)'`: 4 passed in the combined native/docs selection, 2026-11-17, inherited `LF_*` cleared.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings`: passed, 2026-11-17, inherited `LF_*` cleared for Clippy.

## Delete — do not maintain

- `Holder`, `Place`, `transport::holder` and its independent Work reads/tests; `Run` from delegation owns the answer.
- String agent parsing in launch consumers and `AgentConfig::resolved_agent`; typed selections own harness and model. Keep strings at config, CLI and persistence boundaries.

## Not in this PR

- A launch-state label in `lf ps`. It needs `Run` recorded at launch, not
  worked out again from argv. Nothing in this change needs it.

- Transcript methods stay on `Harness`. Asked whether to split them into a
  per-harness `Transcripts` impl, Jack said "no opinion in this one". One
  trait keeps the change smaller, and it is still one file per provider.
