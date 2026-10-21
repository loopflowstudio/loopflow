# LOO-553 · Operators work the Wave's existing work

Status: implemented (2026-10-21, commits `7bb419141a`, `82c45d5e2b`). Product
direction accepted by Jack Heart on 2026-10-21; decisions 1–3 and 6 proceed on
recorded assumptions awaiting Jack's confirmation (`scratch/questions.md`).

Remaining: concept review 3's due-follow-up and reply-order wording (below), then the demo (agent run, demo/review) and the gate checks.

## Concept reviews (2026-10-21): implemented

1. `78f5db2b89`: `lf config user` prints `config::participant_name()`, so
   `LF_USER_NAME` forwarded by `lf --machine` counts (test
   `cli_discovery::config_user_reports_the_forwarded_participant`);
   wave-operate judges ownership before `next_move`.
2. `83183c468d`: s1–s5 (vsm-operate) report plan and capacity changes as
   proposals, s3 uses Task operations only on your Tasks, `docs/waves.md`
   states vsm-operate keeps the operator boundaries, and repo-operate lists
   due follow-ups before backlog like wave-operate.

## Concept review 3 (2026-10-21): next implement direction

A due follow-up you already started and own falls under two rules.
wave-operate's "Return to due follow-ups" reads due Tasks "including unstarted"
and says "Do not start a due follow-up". Meanwhile every started Task you own
gets one disposition, and the reply lists each started Task once. Decision 3
places the follow-up list under unowned backlog, so the settled reading is:

- The due follow-up list holds only unstarted Tasks, which are unowned backlog,
  for the person to select. Rewrite the rule as "Do not start one". A started
  Task you own whose due date has arrived gets its ordinary disposition and is
  not listed again. Before that date its disposition is dependency, with the
  date named. Make `wave_operate.md` "Return to due follow-ups" and Finish
  ("unstarted due follow-ups"), `wave_session.md` line 21 and
  `docs/waves.md` ~459 say the same thing.
- Use one reply order in both operators. repo-operate's "Summarize current
  activity" puts "what finished" between stuck and others' work, while
  `repo_session.md` and wave-operate use Available → Moving → Stuck → Others'
  work. Move "finished" after others' work in repo-operate: Available, Moving,
  Stuck, Others' work, finished, due follow-ups, backlog.

Checked and kept: started Tasks are owned. Checkout allocation claims the Task
(`children.rs` `claim_in`), and the 0.14.12 backfill records person-less local
entries, which `is_mine` treats as yours and never delivers to a teammate. So
"unowned = backlog" never strands started work of yours. The exception is
deliberate `lf task unclaim`, and backlog is the right reading there.

Considered and kept: repo-operate cleanup can `lf task abandon` your own
Tasks while wave-operate never abandons. That is the cleanup duty decision 6
keeps, scoped to your work; questions.md 1 covers reversing it.

## Problem

Jack (2026-10-21): "repo and wave operate should both be focused on the work in
that wave. For now lets just drop anything about KRs or generating new work or
claiming unassigned work."

Today `repo-operate` (`rust/loopflow/src/builtins/ops/skill/repo_operate.md`)
and `wave-operate` (`rust/loopflow/src/builtins/wave/skill/wave_operate.md`)
also check KRs and metric targets, edit the plan (`lf update-plan`), rotate
chapters, file Tasks, and run authorized due follow-ups that are still
unstarted. With two people in one repository the "every owned Task gets a
disposition" rule also covers a teammate's Task: their Flow runs on their
Machine, so it looks idle here, and `lf task run` then appends a claim for this
Machine (`store/sqlite/task_ownership.rs` `claim_in`, `writer_in`;
LOO-550 `collab-review-dividing.md` finding 2).

Beneficiaries: whoever runs an operator (a shorter, predictable pass) and the
teammate whose work it must not take.

## Demo

In an lf-world repository with Wave `demo` holding: an unowned backlog Task, a
Task claimed under `LF_USER_NAME=Dustin`, a Task you own whose last Flow failed,
and a current Project with one KR and one metric target:

```bash
lf wave show demo --json > /tmp/before.json
lf -b --wave demo wave-operate
lf wave show demo --json > /tmp/after.json
```

The reply lists your Task as acted (or stuck with the failure named), Dustin's
Task under **Others' work** with "Dustin · left alone", and the unowned Task
under backlog. Diffing ownership, KRs, metric targets and Task count between
before/after shows only your Task's execution changed. Repeat with
`lf -b repo-operate` for the repository view. This is a demo/review judgment
(an agent run), not a gate check.

## Decisions

1. **Operators act only on your Tasks.** "Yours" = `runtime.owner.person`
   equals `lf config user`, or the owner names no person (the same rule as
   `is_mine` in `task_ownership.rs`). Every Task you own that is unfinished gets
   one disposition and the existing actions.
2. **Another person's Task is reported, never touched.** It is listed with its
   owner and Machine as "theirs · left alone". No `task run`, recovery,
   completion, claim, `machine use --task`, interrupt or comment. Its liveness
   is not judged: their Processes are not visible here, so idle-looking is not
   evidence. (Answers the Task's "how operators treat a Task owned by another
   person".)
3. **Unowned Tasks are backlog, listed only.** Due or overdue follow-ups are
   listed separately for selection; the "already authorized unattended check"
   exception is deleted. Starting backlog is the person's selection
   (`lf task run` outside the operator).
4. **No KR, metric-target, plan or chapter work.** Operators do not check KRs,
   decide on moved metrics, call `lf update-plan`, preview or apply
   `lf repo new-chapter`, or run `lf wave ensure`. `start-chapter`,
   `review-chapter` and the Wave Session's own planning stay the owners.
5. **No new work.** Operators do not file Tasks, run capture or launch-plan
   style generation, or hand off designs into new Tasks. Gaps they find are
   reported in the reply (repo-operate also in `scratch/open-work.md`).
   Filing on request stays with `capture-tasks`; design handoff stays with
   `advance`/`launch-plan`.
6. **repo-operate keeps cleanup of your own work** (assumption, see questions):
   it is already scoped to your PRs/branches and is not KR, generation or
   claiming. "Capture it as a Task" in Database upkeep becomes "record the gap".
7. **Session skills keep user-directed capture.** `repo-session` and
   `wave-session` inline the operate body (`build.rs` `SESSION_OPERATE_PAIRS`).
   Their "Develop direction" section remains the place where a person asks for
   Tasks; `repo-session` points to `capture-tasks` like `wave-session` does.
   Operating passes inside a session follow decisions 1–5.

## Target skill shapes

### wave-operate (keep / change / delete)

- Keep: scope and evidence; live-Flow-first disposition table; recover, Flow
  ends where authored, retry completion, finish merged delivery, reviews stay in
  the Task conversation; Finish reply format and link rules.
- Change: "Keep started work moving" applies to Tasks you own; add a short
  ownership paragraph (decisions 1–3) and an **Others' work** reply group after
  Stuck. Due follow-ups: list only. "Five functions" shrinks to judgment over
  existing Tasks (delivery, collisions, capacity, outside change, purpose),
  reported as findings/proposals in the reply; delete "select an existing next
  Task".
- Delete: metric/KR paragraph; "Identity travels both ways" plan reconciliation;
  intent-graph bullets for `lf task create`, `lf update-plan`, chapter rotation,
  Planned/Completed Project editing; the whole "Task briefs" section (owned by
  `capture-tasks`); `lf cron list` schedule guidance beyond "no schedule is
  installed by this pass" (keep one sentence).

### repo-operate

- Description: "Clear your obsolete work state, report activity, and keep your
  started Tasks moving across the repository's Waves."
- Keep: Clear old work state (scoped to your work), Summarize current activity,
  Drive work through Waves (with decisions 1–3; backlog listed), Sessions and
  reviews, Diagnose execution and auth, a compact Machines paragraph (must keep
  `lf machine use`; asserted by `skills/mod.rs` export test).
- Compress: "Drive work through Waves" states the ownership rule once and
  defers dispositions and recovery to `lf help wave-operate` (one owner, no
  drifting second vocabulary); "Advancing your Tasks", the shared-readers and
  bounded-contribution paragraphs are gone; "Machines" keeps `lf machine use`
  and `lf task interrupt` for your own Tasks.
- Delete: "Capture new Tasks"; from "Launching and advancing work": filing,
  `lf update-plan`, chapter rotation, `lf task delete`; "Existing-design
  handoff"; bounded-contribution paragraphs that teach starting other work
  (`--stack-on` child creation). Keep `lf task run <issue> <flow> --reason` and
  `lf task interrupt` for your own Tasks.

### Other builtins and docs that must agree

| File | Change |
| --- | --- |
| `builtins/ops/skill/repo_session.md` | description drops "capture new work" as operator duty; Develop direction → `lf help capture-tasks`; "Begin work in a Wave" unchanged (person-directed) |
| `builtins/wave/skill/wave_session.md` | description wording; follow-ups line matches decision 3 |
| `builtins/ops/skill/work.md` | supervision line: operators move your existing work; no change to Task-agnostic rules |
| `builtins/ops/skill/init.md` | "wave-operate makes a finite pass" wording only |
| `builtins/wave/skill/s1.md`–`s5.md` (vsm-operate) | s1: start only Tasks a person selected, never another person's; s3: no KR check, priority/capacity tensions are proposals; all: plan changes are proposals |
| `README.md` | "Keep work moving" GOAL example no longer "pursue the highest-leverage open KR"; operate comments |
| `docs/waves.md` | Operate manually + due follow-up paragraph (line ~459) match decisions 1–3 |
| `docs/lf.md` (~538–548) | operator paragraph: your Tasks, others' listed, backlog unstarted |
| `docs/architecture.md`, `docs/architecture/planning.md`, `docs/getting-started.md`, `docs/agent-api.md` | "planning pass"/"reviews the plan"/"reassess Project priorities" examples become operating examples |
| `tests/goldens/with_docs.md` | regenerated for the `work.md` supervision line |

## Source of truth

Ownership: the Task ownership log, read through `runtime.owner {person,
machine}` in `lf wave show --json` / `lf task status --json`. Identity:
`lf config user`. Skill text is the only behavior changed; no store, CLI or DTO
changes.

## Absent and error states

- `lf config user` unset or owner person unreadable → ownership unknown; the
  Task is reported as unknown and not acted on.
- Owner person absent → yours (legacy single-person entries).
- Failed status read → unknown, as today.

## Exclusions

- No code change to `lf task run`/`claim_in`. A refusal guard contradicts the
  repo's flexibility rule and would block deliberate takeover (`lf task claim`
  remains the explicit path). Revisit if skill-level rules prove insufficient.
- `task-operate` is explicitly selected per Task; unchanged.
- `start-chapter`, `review-chapter`, `capture-tasks`, `launch-plan` unchanged.
- Desktop: no change.

## Forbidden outcomes

- Moving the deleted procedures into a new "planner" skill or a flag.
- Operators keeping a softened "propose a KR check" or "file when obvious" path.
- Session skills re-adding the deleted operate guidance inline.

## Acceptance check at gate

```bash
cargo test -p loopflow --lib skills::   # export test still finds `lf machine use` in repo-operate
cargo test -p loopflow --lib operator_skills_stay_on_existing_work
```

New unit test (next to the export test in `rust/loopflow/src/skills/mod.rs`):
for the `repo-operate` and `wave-operate` registered bodies (the session skills
inline them; their own "Begin work in a Wave" text legitimately names
start-chapter's KR planning), assert neither contains `lf task create`, `update-plan`, `new-chapter`,
`lf task claim`, or `KR`; for `s1`–`s5`, assert none contains the first three. Expected: pass. Docs: `git grep -n -e "highest-leverage open KR" -e "one finite planning pass" -- ':!scratch'` returns nothing.
`cargo test -p loopflow --test golden_prompt` passes against the regenerated golden.

## Slices

One indivisible text change, done: both operator skills, session and other
builtins, docs, the guard test and the golden.

## Done when

Running either operator over a Wave with unowned backlog, a KR and a teammate's
Task leaves the plan, ownership and backlog unchanged, names the teammate's
Task as theirs, and moves only your own started Tasks.

Check results: `cargo test -p loopflow --test golden_prompt` — passed after regenerating `with_docs.md` (realign, 2026-10-21); `skills::` 28 passed at compress; docs grep clean outside scratch. Concept-review repairs (compress, 2026-10-21): `--test integration config_user` 1 passed, `--lib skills::` 28 passed, `--test golden_prompt` passed without regeneration. Concept review 2 (implement, 2026-10-21): `--lib skills::` 28 passed with the s1–s5 guard, `--test golden_prompt` passed without regeneration, `cargo fmt --check` clean.
