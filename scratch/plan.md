# Finish the skills rewrite: commits that say what changed, extensions everywhere, QA notes by surface

Branch `jack-heart/rewrite-skills`, draft PR #1955. The branch already holds:
`loop:` blocks and the `plan`/`build`/`code`/`feature`/`deliver`/`deep-clean`
Graphs, the rewritten always-loaded text and skills (implement,
concept-review, kickoff, realign, reflect, debug, reduce, polish), `extends:
builtin` skills with `lf skill customize` writing one by default, the skill
version on each run, the per-Task summary row, and a commit after every skill
step. This plan is what is left before it lands.

## What people said (word for word)

- On step commits: "i would prefer commits that exlain that actually changed.
  i think we can have a commit skill that we automatically follow all other
  skills with and then in my personal config, set its agents to haiku and
  terra". Then: "I think I would prefer it not as an actual... Can you look at
  how some of the PR-related skills happen? ... When you run `LF PR open`, for
  example, then we automatically fill up the PR. Is there a lighter-weight
  inner function of a skill that we could be calling?"
- "I do want the auto-commits to be prefixed with "LF skill" LOF commit,
  though, doesn't need such a prefix since, for LOF, it should just have the
  body".
- Commit applies to bare skill runs and Graphs outside a Task: "yes". "and
  yes, it should push too". "i think in the lf repo at least i do gating
  behind empty-scratch to avoid unnecessary ci".
- "My policy is that committing and pushing should always be free or should
  always be encouraged and should never be considered risky."
- When told the model-written message had been dropped because an agent
  launch records a Session: "how does pr work ten", "can we figure out a
  different plan", "we can redesign the overall architecutre if necessary".
- On overrides: "we just migrate the same location to use extends:, but
  without extends still works (it just has drift potential)". "i dont think
  there should be grpah extensions". "graph rewrites rewrite".
- On QA notes per surface (`.lf/qa/<surface>.md`, first line says when to
  read): "Agree with A. but then how do these get read or loaded?" then
  "thats ifne".
- On the follow-up Tasks filed for all of this: "why did these get filed. No.
  Do them!"

## Facts that shaped the design

- `lf pr open` and `lf commit` (no `-m`) make their one model call from their
  own `lf` process; the Graph runner spawns `cmd:` steps as child `lf`
  processes with `self.spawn(label, StepKind::Operation, &args)`, so the step
  commit is one more such child (`rust/loopflow/src/lf/commands/graph.rs`,
  `RunningGraph::commit_step`).
- The headless agent bound is `ProcessConfig.timeout` (tokio timeout, then
  `harness.stop()`); commit messages use 300 s.
- Installed `lf` is 0.16.0 and reads this repository's `.lf/skills/` and
  `.lf/graphs/`. It does not know `extends: builtin` or `loop:` blocks.
- loopflow's `.lf/skills` gate, implement, pr-message, demo, verify are full
  copies; qa is now a copy of the new builtin. Repository Graphs using the old
  `loop: <name>` form: `.lf/graphs/pursue-auto.yaml`,
  `recover-transient-faults.yaml`.

## Outcome

1. Every skill step's edits land as a commit titled `lf <skill>: <what
   changed>` with a body, pushed, written by the `commit-message` skill on
   its own configured agents. Plain `lf commit` has no prefix.
2. `lf list` shows which repository skills extend a builtin and which replace
   one.
3. QA lessons live per surface under `.lf/qa/`, and the builtin `qa` reads
   and writes them.
4. `task-session` and `explore` read in the same voice as the other rewritten
   skills.
5. Nothing refers to the retired lifecycle scorecard or the removed `work`
   guidance.
6. The branch's own Graphs have been run once, end to end, in an isolated
   home.

## Design

### 1. Step commit as a child `lf commit --step <skill>`

- `lf commit --step <SKILL>`: stage everything; ask `generate_commit_message`
  (the `commit-message` skill, its own `agent`/`default_agent`/`agents`); commit
  as `lf <skill>: <title>` plus body; push unless on the default branch. On
  any failure to get a message, commit as `lf <skill>: checkpoint`. Clean
  tree: no commit; unpushed commits off the default branch still push. The logic lives in `ops::commit_step`; the command
  is a thin call.
- Graph runner: after a finished skill step, when `step_commit_pending` (edits,
  or unpushed commits off the default branch), `self.spawn("commit", StepKind::Operation, ["commit", "--step",
  skill])`. Keep the existing skips (a Graph contributing to a Task another
  run carries; an intervention in progress). Any failure is a warning and the
  Graph continues; an interrupted Graph still stops.
- Bare skill run (rust/loopflow/src/bin/lf.rs ~496): run the same child with
  `resolve_lf_binary()`, stdin null, best-effort.
- Bound the model call so a hung provider cannot hold a Graph. Use the
  existing bound for headless launches if there is one; if not, report it and
  do not invent a kill mechanism.
- The child's process and its one Session belong to the Graph and Task the
  way a `cmd: sync` step's do. Confirm from how `spawn` sets the environment.

The demo: in a repository with a remote, `lf -b implement 'add a line to
README'` then `git log -1` shows `lf implement: <title>` and a body, and the
branch is pushed.

### 2. `lf list` shows extends or replaces

For each repository skill whose name is also a builtin: `extends builtin`, or
`replaces builtin` with `same as builtin` when its text is identical. Same in
`--json` if the listing has JSON, under the DTO rules. No version tracking in
files.

### 3. QA notes by surface

- `.lf/qa/<surface>.md`, one per surface (server, served launches, Desktop,
  iPhone, Linear sync, hooks, held terminals, account routing, …), split from
  the 25 sections of `.lf/skills/qa.md`. First line of each: when to read it.
  Keep every lesson; this is a move, not an edit.
- Builtin `qa` (rust/loopflow-definitions/src/builtins/**/skill/qa.md): list
  the notes with `head -1 .lf/qa/*.md`, read the ones for the surfaces this
  change touches, and write what was learned back into that surface's file.
  A repository with no `.lf/qa/` starts one when it has a lesson to keep.
- `.lf/skills/qa.md` shrinks to what is method and not a surface note. It
  stays a full file (see "Released lf" below).

### 4. Voice pass: `task-session`, `explore`

The voice chosen in this branch: bold imperatives, one or two plain sentences
under each; a "The test:" sentence only where the question says more than the
statement (one or two per skill at most); nothing about what `lf` is for;
rules the runner enforces mechanically stay out. `explore` should agree with
the slicing in kickoff and implement; `task-session` with the Task model now
carried in `LOOPFLOW.md` and with `concept-review` being the interactive step
after `build`.

### 5. Leftovers

- Delete `.lf/skills/telemetry-scorecard.md`; remove the scorecard step from
  `.lf/graphs/telemetry-daily.yaml` (if nothing is left, say so and leave the
  file for a person).
- `scripts/evals/instruction_levels.json` and its test: drop the `work_task`
  cases and `load_work_guidance` actions.
- `outcome` is an `lf` process field and stays as it is. It is meaningless
  for agent processes, and the branch already stopped writing and reading it
  for them (the agent process record has no such field). Nothing more to do
  here: don't clear old rows, don't change how `lf` processes use it. Check
  only that no agent-process code path or wire type still carries it.
- No reference left to `lifecycle_scorecard`, `telemetry-scorecard`,
  `work-guidance`, `--work-task`, `lf help skill work`.

### 6. Run it once

Build the branch `lf`. In an isolated `LF_HOME` with its own store and a
fixture repository with a bare remote (never the real `~/.lf`: this branch
adds migrations the installed `lf` cannot read), run `lf -b graph code` on a
one-line change with a stand-in or cheap agent. Record what happened at each
step: the loop blocks, the commit after each step, the page from
concept-review, `reflect` in `deliver`. Fix what breaks.

## Released lf

`.lf/skills/` and `.lf/graphs/` are read by the installed `lf` too. A file
with `extends: builtin` is, to 0.16.0, the whole skill; a `loop:` block does
not parse. So in this PR:

- loopflow's own overrides stay full files, and repository Graphs keep the
  old `loop: <name>` form, which the new parser still reads.
- After this lands and is installed: convert the six overrides to `extends:
  builtin` (the short sections are already known: about 650 words in place of
  2,740), rewrite `pursue-auto` and `recover-transient-faults` as `loop:`
  blocks or delete them where `plan`/`build` cover them, then remove the old
  parser. manabot and etude are edited by hand in their repositories. That is
  Task 91930a.

## Slices

All six are built (commits f10c0b6a74, a36284d737, b38466264c and the docs
commit after them). What is left is gate: publish so CI runs the suites.

## What the runs showed (2026-11-16, branch `lf`, isolated `LF_HOME`, `claude:haiku`)

- `lf -b code '<add one README line>'` in a fixture with a bare origin ran all
  15 steps and stopped at `land --wait-and-fix` (`gh` finds no GitHub host).
  Each step that left edits got `lf <step>: <title>` with a body and the branch
  was pushed (remote head = local HEAD). Interactive `concept-review` ran with
  the headless stand-in.
- That run's commits also carried lf's own files: `.lf/tmp/metrics/ops.jsonl`
  (`lf realign: Record ops metrics…`) and the `.gitignore` line lf wrote for
  `.lf/prompts/`. Fixed: both go in `info/exclude` through `git::exclude`;
  lf no longer edits `.gitignore`.
- `lf -b implement` committed its own work (the skill says to), so the tree was
  clean, no step commit ran and the branch stayed unpushed. Fixed:
  `step_commit_pending` is also true for unpushed commits off the default
  branch, and `lf commit --step` then only pushes. Rerun: `lf implement: Note
  in README that the repository is a fixture`, pushed, only `README.md`.
- Every step of a bare Graph receives the original message, so steps after
  kickoff re-answer it ("the line is already there"). Not changed here; noted in
  `scratch/questions.md`.
- No launchd job or process was left for the temporary homes.

## Checks

- `cargo clippy --all-targets -p loopflow -p loopflow-execution -- -D warnings`: clean.
- `commit_tests::` (13), `ops::commit::`, `skills::` (35), `listing::`,
  `cli_discovery::list`, `server::schema`, `lf::commands::graph::` (6),
  `the_research_graph_stops_at_its_findings_then_reaches_end`, `prompt::
  context_block:: git::` (160), `ops::telemetry:: journal::` (39),
  `documented_commands::`, `golden_prompt` (no golden changed), pytest
  instruction_levels + skill_alignment (21): pass, run here.
- Full suites and Swift (the skill-version and `outcome` mirrors were never
  built): CI.
