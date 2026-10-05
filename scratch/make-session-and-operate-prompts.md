# Session and operate prompts keep work moving together (LOO-383)

Status: design reviewed with Jack Heart, 2026-10-05. Jack accepted the two-part
contract in the Task brief and made the three decisions under "Decisions"
below. Mechanisms not listed there remain proposals.

## Decisions (Jack Heart, 2026-10-05)

1. **Started work must keep moving.** A Wave makes sure every started Task in
   its Wave/Project keeps moving. It is not asked to start unstarted work "for
   now at least"; that "will evolve over time." The goal is "reliably finishing
   stuff ive started and isnt blocked on me."
2. **The procedure is inline in the session.** Jack is "ok with collapsing
   operate into just inline in session. No strong opinions; just make it work
   reliably." Loading operate by `lf help` at run time is dropped.
3. **A defined Flow proceeds.** "We can assume good flows for now that
   basically end at landing." If a Task's selected Flow does not end at
   landing, the Task either changes its Flow (wanted, not yet designed) or
   waits on Jack. "If there is a defined flow, we need to proceed."

## Problem

Jack Heart's ongoing conversations end turns with runnable Tasks untouched,
reporting that "operations owns continuation." Jack clarified that the Wave
conversation *is* operations. The prompts disagree about who acts, and the
operating document no longer describes the shipped system.

## What the audit found (v0.13.3 source = installed runtime, 2026-10-05)

Installed exports under `~/.claude/skills` match this checkout's builtins except
generated frontmatter (exported Oct 5 00:33). The drift named in the Task was
against the older Product Wave checkout, not against the installed runtime.

1. **`task/session` does not exist** in source, `lf list --json`, or exports.
   `PrimaryScope` has only `Repository` and `Wave`; `session ensure` has no Task
   form. Two of three pairs exist.
2. **"Next owner: Wave" is read as someone else.** `wave/operate` tells the
   report to take the next owner from `next_move.owner`. `lf wave status`
   assigns `Wave` to a Task that is ready to start, to blocked or unknown
   execution, and to a published PR with passing CI and no merge request
   (`lf/commands/waves.rs`, `next_move_for_task`). The Wave operator therefore
   reports its own queue as a handoff. This is the observed failure's mechanism.
3. **`wave/operate` permits stopping early.** "Aim for one or two useful moves…
   No action is a valid result" has no requirement to dispose of runnable work.
4. **`wave/operate` states "Each Task's selected Flow carries its work through
   landing."** False for the builtin catalog:

   | Flow | Ends at | Lands | Completes Task |
   | --- | --- | --- | --- |
   | `feature` | `pr land -c` after design review, demo review, queue | yes | yes |
   | `ship`, `ship-demo` | `pr land -c` | yes | yes |
   | `deploy` | `pr land` | yes | no |
   | `pursue` | `pr-publish` | no | no |
   | `code` | `pr-review` (human) | no | no |
   | `queue`, `refresh`, `task-design`, `incident` | gate / realign / review / plan | no | no |

5. **The sessions carry a competing copy of the procedure.** `wave/session` and
   `repo/session` each restate reconcile → status → recover in ten bullets that
   differ from their operate skill (no five functions, no cleanup, no placement).
6. **`repo/session` deflects.** "Point the user there for work that belongs to
   one Wave instead of absorbing it here" contradicts `repo/operate`, which
   applies `wave/operate` to each Wave that needs attention.
7. **`lf task reconcile` is described as "the same check the minute schedule
   runs."** The schedule is optional and here disabled (`lf task automation`:
   `disabled · every 60 seconds`); `lf cron list` shows no installed
   `wave/operate` job although Product's GOAL declares one. Reconcile also only
   resumes *enrolled* Tasks with an *unfinished captured Flow*: it reports
   `no unfinished captured Flow` for LOO-380, LOO-375 and LOO-285, and
   `Flow failed … inspect and explicitly retry` for LOO-367. Those are exactly
   the Tasks an operator must act on, and nothing else will.
8. **Broken commands in shipped text.** `wave/session` says
   `lf skill show capture-tasks`; the runtime answers `skill not found: show`
   (`lf help capture-tasks` works). `ops/task_automation.rs` tells readers to
   `select a Flow with lf task run`; no such command exists
   (`lf --task <issue> flow start <flow>`). `repo/operate` mentions a rejected
   `run --directive`.
9. **Review completion disagrees.** `wave/session`: "this chat does not complete
   it on their behalf." `task/operate`: collect feedback and complete the exact
   review. `repo/operate`: complete "when the User asks to proceed."
10. **No wake exists for a conversation.** Nothing in Loopflow re-invokes an
    interactive Session. Between turns, only independent mechanisms run: a Task's
    own worker, the optional minute check, the optional `lf ci watch`, and
    installed Wave crons (finite headless passes, a different conversation).
    Within a turn, `lf task wait <issue> --until submitted|terminal --timeout`
    blocks without polling a model.
11. **LOOPFLOW.md** (114 lines, paid on every run) is accurate on delivery
    commands but: names only two operate skills; has no Task/Session/Flow
    vocabulary although `builtins.rs` says it carries it; never says a Flow ends
    where it is authored to end; never says background work is independent and
    optional; and says "the Wave operator reads existing logs" without saying
    the Wave conversation is that operator.

## Demo

Prompt behavior, shown at two levels; neither is claimed to be the other.

- **Headless, at gate:** `lf help task/session` prints the new skill;
  `lf list --json` lists all six pair members; a skill sync into a temporary Home
  writes `task/session/SKILL.md`; `lf context --skill wave/session` shows the
  assembled prompt with one `<lf:loopflow>` block.
- **For Jack, after install:** run `lf session replace <product-wave-session>`
  and say nothing. The Wave conversation's first turn ends with every *started*
  unfinished Product Task in one named disposition — e.g. LOO-367's failure
  read and retried or reported with its log evidence, a Task whose Flow stopped
  mid-way continued and verified live, LOO-382 named as waiting on Jack's
  kickoff review with its open command — and no line that says another role
  owns the next step. Unstarted backlog is listed, not started. Then ask
  "what's running?" mid-turn and file an idea; operation resumes afterwards.

## Chosen approach

### One contract, three scopes

| Scope | Operate (finite procedure) | Session (ongoing conversation) | Operates through |
| --- | --- | --- | --- |
| Repository | `repo/operate` | `repo/session` | `wave/operate` per Wave; `task/operate` for waveless Tasks |
| Wave | `wave/operate` | `wave/session` | `task/operate` per Task |
| Task | `task/operate` | `task/session` (new) | the Task's own Flow |

**Operate** owns the whole procedure for its scope and is complete standalone:
read current evidence, act, recover through supported controls, verify, exit.
It stays the headless and cron entry point.

**Session** owns continuity and responsiveness only: apply the procedure on
start, on each return after a pause, and after anything it changed; answer the
person first; capture ideas; then resume operating.

The procedure reaches the session inline, from one source. Each session's
builtin content is its short session file followed by its operate file's body,
composed where builtins are registered (`build.rs` already generates the skill
map). `lf help wave/session`, the vendor export and the launch prompt all carry
the composed text; `wave/operate` remains its own skill. No frontmatter include
feature, no run-time `lf help` read, and no hand-kept condensed copy or inline
invariant. The session file itself contains no operating steps.

Paying for this: `wave/session`'s 44-line persistent-workspace and publication
section is mechanics, not conversation. It moves to the skill that exercises it
or to user docs if one already covers it; otherwise it stays and the composed
prompt is simply longer. Cross-scope application (`wave/operate` applying
`task/operate` per Task, `repo/operate` applying `wave/operate` per Wave) keeps
whatever form it has today; only the same-scope pairing is composed.

### The action contract (stated once, in each operate skill at its scope)

An operate pass ends only when every *started* unfinished Task in scope has
exactly one disposition, each with its evidence. Started is the recorded fact
(a reservation set it once), independent of whether a worker is alive now.
Unstarted backlog is reported as backlog and left alone: starting it is the
person's selection for now.

- **moving** — a live worker or driver was observed; left alone;
- **acted** — this pass started, continued, recovered or delivered it through a
  supported control, and reread status to see the effect;
- **waiting on a person** — a named review, decision or merge click, with the
  Session or PR to open; never completed or approved on their behalf;
- **waiting on a dependency or capacity** — the named Task, PR, check, account
  or limit;
- **paused** — an explicit hold (`lf task automate <issue> off`) or instruction;
- **unknown** — the named read or liveness evidence that is missing. Unknown is
  not idle, and not a reason to start a second driver.

"Ready," "needs reconciliation," or "the Wave/operations owns this" is not a
disposition. When status names this scope as next owner (`next_move.owner` of
`wave` for a Wave operator; a Task with no worker for `task/operate`), the
operator *is* that owner: act, then report the result. No useful action is a
valid outcome only when every item already holds one of the dispositions above.
This replaces "one or two useful moves" without adding a quota — consistent
with Jack's 2026-09-28 "soften, dont harden": the measure is covered work, not
a count of actions.

Recovery before escalation: read `lf task status`, the failed step's log and
`lf top`/`lf ps` evidence; retry through `lf --task <issue> flow start --reason
"<what changed>"` only with new evidence or a repaired cause; one unchanged
failure is not retried again. Authorization stays where it is: operators start
and continue selected Flows without asking; they do not approve reviews, choose
a merge the user reserved, cancel or abandon Tasks, or change direction.

### A defined Flow proceeds

A started Task with a selected Flow that has steps left and no live driver is
continued (`lf --task <issue> flow start`) and verified. That is the whole
rule for the common case, and it needs no permission.

Builtin delivery Flows are assumed to end at landing. The operate skills stop
claiming it as a fact about every Flow: a finished Flow, an accepted launch and
a published PR are evidence to inspect. When a Task's Flow has finished and its
work has not landed, the disposition is **waiting on a person**, naming the PR
and what remains. The operator does not pick a different Flow for the Task;
letting a Task change its Flow is wanted by Jack and not designed here.
Publication, review, landing, Task completion and remaining scope
(`lf pr next`) are reported as separate facts.

### Continuation, honestly

Each session skill states that an ongoing Session acts only during a turn and
that nothing schedules its next one. Within a turn it may wait on a state change with
`lf task wait`. Between turns, Task workers continue on their own; the minute
check and CI watcher help only where `lf task automation` and
`lf ci watch --status` show them installed. Say which are active instead of
promising to "keep watching." Each return re-runs the operation from fresh
reads. No resident, wake service or second Flow cursor is added.

### `task/session`

A new builtin skill at `task/skill/task_session.md`, launched with
`lf --task <issue> skill task/session`, and listed automatically by discovery,
the Desktop skill picker and exports. It is an ordinary Task Session: any number
may exist, it never becomes the Flow's driver, and it holds no primary scope.
It keeps applying `task/operate`, answers questions about the Task, takes
direction, and gives review feedback a home without completing a review the
person has not decided. A primary Task conversation (`session ensure` for a
Task, Ctrl-C semantics) stays with LOO-364/LOO-353.

### Review completion, unified

A review belongs to its own Session. Operators surface it with its open command.
An operator completes that exact Session (`lf session complete <id>`) only when
the participant explicitly decides that review with the operator and the
feedback is saved; the following `loop-decide` owns navigation. Headless, a
pending review is a waiting disposition.

### LOOPFLOW.md

Rewrite in place, no longer than today's 114 lines. Keep: execute-here-first,
the delivery command block (verified against Clap), checkpointing, checks and
Flow boundaries, naming people, headless failure, state readers, secrets,
context budgets, scratch and memory owners. Change:

- add a five-line vocabulary: Wave, Task, Session, Flow, Exec — one clause each;
- name all three operate skills as owners of supervision and recovery, and say
  the ongoing repository/Wave/Task conversation is that scope's operator;
- one sentence: a Flow ends where it is authored to end; finishing or
  publishing is not Task completion;
- one sentence: Task workers and optional scheduled checks run independently;
  nothing re-invokes a conversation;
- remove nothing that a test or golden depends on without updating it
  (`engine/prompt.rs` asserts "Execute Here First", "Checks and Flow
  boundaries", "Gate owns\nverification once", "Checks must run headless",
  "lf land"; `tests/goldens/` embeds the document).

## Alternatives considered

- **Load operate at run time with `lf help <scope>/operate`.** An installed fix
  would reach an existing conversation, but the observed failure was an
  instruction not followed, and this adds one more. Dropped by decision 2.
- **A general frontmatter include.** More machinery than three fixed pairs need.
- **Delete the operate skills and keep only sessions.** Headless passes, crons
  and `lf/commands/run.rs` placement depend on `wave/operate` and `repo/operate`.
- **Operators choose the next Flow for a Task whose Flow ended before landing.**
  Dropped by decision 3; such a Task waits on Jack.
- **Keep a condensed procedure in each session.** Today's state; it drifted
  within a week of #1383.
- **Have status stop saying "Wave" as next owner.** A DTO change across Rust,
  Swift and fixtures to fix a reading error; the label is accurate. Rejected.
- **A wake mechanism for conversations.** Excluded by the Task; LOO-364 owns
  primary-Session runtime.

## Scope

Source of truth: builtin skill files and `LOOPFLOW.md` embedded in the `lf`
binary. Derived: `lf help`/`lf list`, vendor exports written by install and by
launch-time skill sync, assembled launch prompts, goldens.

Edit together:

- `engine/builtins/LOOPFLOW.md`
- `wave/skill/wave_operate.md`, `wave/skill/wave_session.md`
- `ops/skill/repo_operate.md`, `ops/skill/repo_session.md`
- `task/skill/task_operate.md`, new `task/skill/task_session.md`
- `build.rs` (and `engine/builtins.rs` tests): compose each session from its
  session file plus its operate body
- `ops/task_automation.rs`: replace the `lf task run` hint with
  `lf --task <issue> flow start <flow>`
- `engine/prompt.rs` assembled-prompt test: add the three sessions and
  `task/operate`; assert each session's content contains its own operate
  body exactly once and none contains `lf skill show`
- `tests/discovery_tests.rs`: `task/session` is listed
- `tests/goldens/` via `tests/goldens/update_goldens.py`
- Docs: `README.md` (pairs table beside `lf task/operate`), `docs/lf.md`
  (session section, background checks), `docs/waves.md` and
  `docs/getting-started.md` ("bounded next action" wording), `docs/conducting.md`,
  `skills/loopflow/SKILL.md`, `docs/architecture/execution.md` only where it
  states who continues work
- `docs/reviews/session-operate-prompts.md`: the nine scenario walk-throughs,
  labelled as simulations against assembled prompts, with what each role does
  and which boundary it keeps

Delete — do not maintain: the "Start by reconciling" and "Keep work moving"
bullet procedures in both sessions; "one or two useful moves"; "carries its work
through landing"; "point the user there … instead of absorbing it here";
`lf skill show`; `lf task run`; the `run --directive` remark. `wave/operate`'s
41-line Task-brief section moves behind `capture-tasks` only if that skill
already covers every rule in it; otherwise it stays (see questions).

Forbidden outcomes: a hand-written second copy of the procedure in a session
file; an operator starting unstarted backlog or selecting a Flow for a Task; a
fourth "operations" role; a session that promises to watch; a prompt that
completes or approves a review without the person; a new scheduler, controller
or DTO; a warning paragraph layered over a contradiction instead of removing it.

Exclusions: primary Task Sessions and wake runtime (LOO-364); Flow and review
changes in LOO-353; S1–S5 skill content (LOO-333) beyond keeping the five
functions intact in `wave/operate`; the default New Session skill, which stays
`capture-tasks` per Jack's 2026-10-02 selection; installing schedules on any Home.

## Risks

- **Instruction-following, not authority, caused the failure.** Prompt edits
  improve odds; they prove nothing about runtime behavior. Mitigation: the
  inline composed procedure, the post-install demo, and labelling simulations as such.
- **The disposition rule can over-act** on stale reads. Mitigation: unknown is a
  disposition, live work is never duplicated, unchanged failures are not retried.
- **A long conversation keeps the procedure it launched with.** A released fix
  reaches it only through `lf session replace`.
- **Composed prompts are long.** `repo/session` carries `repo/operate`'s 323
  lines. Accepted for reliability; trim operate itself, not the composition.
- **LOO-353 changes Flows and reviews underneath.** The prompts continue a
  Flow's remaining steps without naming endpoints, so they survive that change.
- **Token cost.** LOOPFLOW.md must not grow; sessions shrink; `task/session` is
  new but short.

## Rollout

Builtins ship in the binary: nothing changes until a release is installed.
Install refreshes vendor exports. A conversation assembled before the install
keeps its old session and operating text until `lf session replace <id>`. Homes with the
minute check or Wave crons disabled gain no background progress from this Task.

## Done when

- The six skills exist in `lf list --json` and a fresh export, and each
  session's exported text contains its matching operate procedure once.
- A started Task with remaining Flow steps and no live driver is continued and
  verified; unstarted backlog is not started; a Task whose Flow ended before
  landing waits on Jack.
- Given the nine scenarios — idle runnable work, live worker, recoverable
  failure, unresolved liveness, pending user review, published PR with
  unfinished delivery, no useful action, returning user, question or capture
  mid-operation — the reviewed walk-through shows the action taken and the
  boundary kept, with no "another role owns it" outcome.
- No builtin, doc or runtime string names a command Clap rejects.
- LOOPFLOW.md is at or under 114 lines and contradicts no operate skill.

Gate (headless, once):

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo test -p loopflow --lib -- engine::prompt engine::skills engine::builtins ops::task_automation
cargo test -p loopflow --test discovery_tests --test documented_commands --test golden_prompt --test default_conversation_tests
```

Expected: all pass; goldens regenerated in the same change. Demo/review holds
the post-install conversation behavior; it is not a gate.

Check result (kickoff, 2026-10-05): no build or test run — plan only.

## Internal slices

One coherent change; order for the implementer:

1. **This slice — contract and deletion.** Rewrite the three operate skills to
   the action contract and defined-Flow rule; cut the session files to
   continuity only and compose operate into them; add `task/session`; fix the two broken commands. Focused test:
   `cargo test -p loopflow --lib assembled_prompts_deliver_procedures_to_the_owning_skill`.
2. LOOPFLOW.md rewrite and goldens.
3. User docs and the scenario review document.
