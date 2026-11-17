# LOO-924: bare or managed launches and a parsed agent choice

Direction from concept review of PR #1966
(`cd0977a62f:scratch/concept-review.html`; the working file now reviews Session commands).
Jack, 2026-11-17, quoting the review: "Two product ideas still lack one owner in
the code: whether a launch is managed, and which agent was chosen." / "this
seems worth fixing". Both go in this PR.

Status at `f07616744f`: sections 1 and 2, the review answers, the four names
and the glossary are built (the Rust in sections 1 and the review answers still
says `Bare(Mode)`/`Managed`, which "Next implement" renames). Remaining: "Next
implement", then gate and demo.

## 1. Bare or managed, interactive or headless

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

**Who keeps it running?** Transport reads the command before delegation and
returns one value. Bare and managed are the two states; Worker names managed
and headless execution:

```rust
enum Run { Bare(Mode), Managed, Worker }   // replaces Holder x Place x Mode
```

| command | Run | mode | lives until |
|---|---|---|---|
| `lf`, `lf <skill>`, `lf graph <g>`, `lf -b ...`, with or without `--task`/`LF_AS`, in any checkout | Bare | interactive or headless | its terminal closes or its caller exits |
| a Task, Wave or repository conversation: `lf resume --task/--wave`, `lf resume ID` of a Session bound with `lf session bind` or of a primary conversation | Managed | interactive | someone quits it; survives its viewer |
| `lf task run` steps, landing's `ci-fix` run, Discord Wave answers | Worker | headless | the run ends; outlives the command that started it |

Names chosen for review: `Worker`, not "detached", because lf already shows
`"state": "detached"` on an interactive Session nobody is viewing. One word
for launches whose turns the server records: managed (rename `Place::Held` uses
that mean this state; "held terminal" stays as the name of the terminal).

### What each Run variant does

| | counts toward its Task | gets Task steers | reports turns to the Machine server | terminal |
|---|---|---|---|---|
| Bare | yes | yes | no | the caller's, or none |
| Managed | yes | yes | yes | held by the server |
| Worker | yes | yes | yes, as Graph steps do today | none |

Worker keeps today's turn reports. The server records Codex turns and
refreshes planning from them, and nothing here has shown that is safe to drop.
The `Worker` variant is managed and headless, with no held terminal or viewer;
the `Managed` variant is managed and interactive. Both report turns.

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
  the one place this is read, before `delegate`, into `Run`.
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
`lf/commands/run.rs`). `lf -b resume <ID> --message` is `Bare(Headless)`, except a Graph's
correction turn in a Task run, which is the step's Worker; with no
message, resume keeps the Session's state. The transport, harness and server READMEs say so.

Resume reads the Session's binding and primary scope once before `delegate`,
then carries the resulting `Run` through native launch. A held child keeps the
parent's Managed decision without rereading it. `SqliteStore::is_bare`
uses `bound_at` and primary scope, not attribution provenance: a declared Bare
launch must remain Bare when reopened. An unreadable store logs a recording
gap and retains the Managed fallback. `runs_in_its_terminal` shares the same
classification so a managed conversation cannot be treated as an independent
terminal launch while its held terminal is absent.

Managed native fixtures use a real Task binding and open through `resume`,
retaining their selected cwd and configuration; direct skills remain Bare
with Task attribution. WorkSource remains for context provenance and primary
selection, with no launch-lifetime authority. Fresh resume carries invocation
settings and a literal inline-prompt delimiter across exec.

Earlier implementation and check results: `2a414ca0f3:scratch/plan.md`.
Gate owns broader transport, configuration and Task performance checks; demo
owns live steering and terminal lifetime checks. The older batch → interactive
resume demo is not available: `run_refusal` rejects headless Sessions before
transport classification (see `scratch/questions.md`, 2026-11-17).

## Delete — do not maintain

- Credential filters `provider_env_allowed_for_program`, `api_key_env_allowed_for_program`
  and `normalize_program_name`: typed harness launch options own credential policy;
  preserve exported-key filtering and stored GitHub/provider env behavior.

- `SessionConnectionArgs::work_flag` and prepared-command work flag forwarding; the selected Session ID owns resume identity.
- Resume row classification inside `delegate`; both resume entry points use `session_run` before delegation.
- `Holder`, `Place`, `transport::holder` and its independent Work reads/tests; `Run` from delegation owns the answer.
- String agent parsing in launch consumers and `AgentConfig::resolved_agent`; typed selections own harness and model. Keep strings at config, CLI and persistence boundaries.
- `PromptBuild.harness`, `.model` and `.mode` (`lf/commands/run.rs`) and the `prepared_config` argument of `begin_process_runtime` and `run_headless_prompt`: `agent_config.agent` and `Run` already say it. Removed in compress.

## Not in this PR

- A launch-state label in `lf ps`. It needs `Run` recorded at launch, not
  worked out again from argv. Nothing in this change needs it.

- Transcript methods stay on `Harness`. Asked whether to split them into a
  per-harness `Transcripts` impl, Jack said "no opinion in this one". One
  trait keeps the change smaller, and it is still one file per provider.

## Review answers, 2026-11-17

Jack answered `scratch/concept-review.html` with "1c 2b 3c -- give info to the
"remove process rows" task which delete lf s 4) a 5)a".

- **1. Worker is a name for managed and headless.** Jack first answered 1c
  ("Drop the third name"), then "wait im not sure about 1c", then: "i guess
  wohat i would say is WOrker = (Manage & Headless)". So the word stays and
  it is a definition, not a third behavior:
  - bare: the server is not told about its turns; interactive or headless.
  - managed: the server is told about each turn. A managed launch that is
    interactive is a conversation in a held terminal; one that is headless
    is a worker and runs detached.
  Concept review then chose four variants named for the cells (see "Next
  implement"; choice in `scratch/questions.md`); either way no code may give
  Worker a behavior that "managed and headless" doesn't
  explain. Say the definition once in `harness/README.md` and use it in
  `transport/README.md`, `server/README.md`, `docs/sessions.md` and
  `AGENTS.md`; the docs should not describe three independent states.
- **2b. `harness::Run` keeps its name.** LOO-953 chooses other names for its
  `Run`/`GraphRun`; it has been told.
- **3c. No state column** in `lf session list` or `lf ps` (LOO-953 deletes
  `lf ps`). Remove the `lf ps` line from the demo.
- **4a. Check every remaining `parse_agent` and `HarnessKind::parse` call
  site** (20 and 25 in production code at 74a96d6176). Keep only those where
  text comes in: CLI `-a`, config files, stored rows, wire input. Replace the
  rest with the typed `Agent`. List what stayed, and why, in the PR body.
- **5a. Rename `Admitted` to `Recording`** (`Launch.recording`), with a
  one-line doc comment saying it holds the store, the Session and its
  attachment that this launch records under.

## The four names (2026-11-17)

Jack: "my suggestions: work session, worker / chat, run", then, after `run`
was shown to clash with `harness::Run`, the Graph run and `lf task run`:
"ok, batch is fine for bare headless".

| | someone typing | headless |
|---|---|---|
| managed | work session | worker |
| bare | chat | batch |

Two states, managed and bare; each name is one cell and says both axes. Use
these four words in `harness/README.md` (linking `docs/glossary.md`), `transport/README.md`,
`server/README.md`, `docs/sessions.md` and `AGENTS.md`, replacing "bare
conversation", "managed conversation", "bare headless run" and similar. In
code, name constructors or variants after the cells where it reads truer
(`Run::Worker` stays; the next implement section names all four); no cell gets
a behavior its two axes don't explain.
Prior art checked (T3 Code, Orchestrator, Paseo, HumanLayer): all are always
daemon-backed and name none of the cells; Orchestrator says "worker session".

## Next implement: one vocabulary in code and in the model doc

Concept review of `55416284eb`. The four names are in the glossary and the
READMEs; three places still speak an older vocabulary. No behavior changes.

1. **`Run` variants are the four cells.** The glossary's Managed is a row
   (work session and worker); `Run::Managed` is one cell of it, and the
   predicate that means managed is spelled `reports_turns()`. The comment on
   `harness/context.rs::claude_args` already reads `Run::Managed` as the row
   while the code tests `reports_turns()`.

   ```rust
   enum Run { Chat, Batch, WorkSession, Worker }
   impl Run {
       fn bare(mode: Mode) -> Run      // Chat or Batch
       fn is_managed(self) -> bool     // WorkSession | Worker; replaces reports_turns()
       fn mode(self) -> Mode
   }
   ```

   `matches!(run, Run::Bare(_))` becomes `!run.is_managed()`;
   `ThreadNaming { bare, reporting }` becomes `{ bare, managed }`. The
   `harness/README.md` table then maps each name to the variant of the same
   name and can shrink to a sentence. At `f07616744f`: 27 `Run::Managed`,
   11 `Bare(Mode::…)` plus 7 `Bare(input.mode())`/`Bare(mode)`/`Bare(_)`
   sites, 8 `Run::Worker`, 11 `reports_turns()`.

2. **Managed and bare are defined by turn reports alone.** That is the
   definition quoted under review answer 1. The glossary rows add a lifetime
   ("outlives its initiating command or viewer", "follows its caller") that
   the Discord Wave answer contradicts: it is a worker, runs in the bridge's
   process and returns its answer to it. Managed: the Machine server is told
   each turn. Bare: it is not. Lifetime stays in the four cell rows, and the
   Worker row says most workers are Graph steps.

3. **`docs/dev/index.md`, "What runs on a Machine", still teaches the model
   this change deletes.** It is untouched on this branch, and
   `transport/README.md` calls it "the model". It names three places (Here,
   Held, Detached), says "Work decides between Here and the other two: a
   command that names a Task or Wave ... is Work", and "Hooks follow Work,
   not the place". Here `lf research --task LOO-12` is a chat. Rewrite the
   launch table and the two paragraphs after it with managed/bare and the
   four names, linking the glossary; keep the process diagram, the process
   table and "what survives what". Drop "Detached" as a launch word, so
   `detached` means only the Session state `lf session list` prints. Line 12
   of that file points at the section; check it still reads true.
   That table lists one launch the others omit: a plain conversation a
   Desktop pane or framed `lf --machine` view opens is held. Read
   `transport::delegate_current` (`current().is_some()` selects managed) and
   say which cell it is in the glossary's Work session row and the transport
   README table.

4. **"Held launch" where the sentence is about turn reports.** Workers
   report turns and are not held. `harness/README.md` ("A held launch's hook
   first sends one `session.boundary` request") and `server/README.md` ("A
   held launch's prompt hook always runs") say managed launch, after checking
   the second is true of workers; `transport/README.md` ("A held launch calls
   `session.start`") says work session, and the `session.start` row in
   `server/README.md` lists `lf task run` under "a managed launch" though a
   worker is started detached, not through `session.start`: check and fix
   whichever is wrong. "Held terminal" keeps its meaning.

Checks: the focused commands under "Implementation checks" again, plus
`cargo clippy --all-targets -- -D warnings`.

## Glossary (this PR)

Asked "is there a task for having a GLOSSARY.md", then "if not we can do it
here". There is no Task; `docs/glossary.md` exists (61 lines, "Loopflow's
words"). Put the launch vocabulary there, as the one definition the READMEs
link to:

- Add **Managed** and **Bare** (who keeps a launch alive; whether the server
  is told each turn), and the four cells: **Work session**, **Worker**,
  **Chat**, **Batch**, each one line saying both axes and the command that
  makes it (`lf resume --task`, `lf task run`, `lf <skill>`, `lf -b <skill>`).
- Rewrite the rows that say the same thing in older words: **Headless
  Session**, **AgentProcess** ("An `lf <skill>` run's is the foreground
  program…"), **Server**, **Holder**. One idea, one row.
- Say that belonging to a Task (checkout, `--task`, `LF_AS`) is separate from
  managed or bare.
- `harness/README.md` then links the glossary rather than defining the words
  a second time.

## Implementation checks, 2026-11-17

The review changes and glossary vocabulary are implemented. At `55416284eb`
`Run` still had `Bare(Mode)`, `Managed`, `Worker`; "Next implement" renames them; `Recording` replaces the launch attachment type. The
production parsing audit and its retained call sites are in [PR #1966](https://github.com/loopflowstudio/loopflow/pull/1966).
Broader configuration/transport/performance checks remain with gate; live
steering and viewer lifetime remain with demo.

All commands cleared inherited `LF_*`; application and credential checks used
the prepared fixture CLI. The initial matrix built its fixture CLI inside the
tests, accounting for its two slow results.

- `cargo test -p loopflow-execution --lib --no-run`: passed.
- `cargo nextest run -p loopflow-execution --lib -E 'test(harness::launch_matrix::) | test(transport::launch_mode_tests::)'`: all eight checks passed in the focused execution run.
- `cargo nextest run -p loopflow -p loopflow-execution --lib --test integration -E 'test(run::launch_policy_tests::) | test(transcript::tests::old_account_transcripts_resume_with_native_auth_without_copying_credentials) | test(documented_commands::)'`: 11 passed; macOS linking warned about unwind-table size and succeeded.
- `cargo nextest run -p loopflow-execution --lib -E 'test(provider_auth::tests::api_key_env_filter_is_harness_specific) | test(harness::native_session::tests::session_launch_tui_preserves_native_oauth_and_routes_stored_api_keys)'`: 2 passed.
- `target/debug/deps/integration-e149becb33146c3a --exact documented_commands::documented_invocations_have_no_ambiguous_command_shorthand`: passed against the final documentation edits.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`: passed.
