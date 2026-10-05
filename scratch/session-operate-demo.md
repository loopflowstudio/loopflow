# Session/operate prompts: demo review (LOO-383)

Reviewed 2026-10-05 against branch head `4af724769`. Design:
[make-session-and-operate-prompts.md](make-session-and-operate-prompts.md).
Simulated walk-through: [docs/reviews/session-operate-prompts.md](../docs/reviews/session-operate-prompts.md).

Status: reviewed with Jack Heart on 2026-10-05. Jack's statements are under
"Jack's feedback"; everything else is observation or the branch's choice.

## Jack's feedback (2026-10-05)

- Shown the six defects below: "Ok, great. just continue to iterate and
  experiment."
- On the dry-run first turn: "i would prefer a little more structure to that,
  with like, links to PRs, sorted by waiting on me vs currently moving vs stuck
  in some way."
- "but once youre done, this is approved."

Jack did not answer whether an operator should arm a merge that status
recommends, or whether the installed demo precedes landing. The branch chose
the conservative merge rule (below); the installed demo remains unshown.

## What was demonstrated

Three levels, none claimed to be another.

1. **Branch build, headless — passed.** `lf list --json` lists all six pair
   members. `lf help wave/session` prints the session text plus one
   `# Operating procedure: wave/operate` section (306 lines; `repo/session`
   432, `task/session` 131). An export into a disposable home wrote 112 skills
   including `task/session/SKILL.md` for both vendors; each session export
   carries its operate procedure once; none contains `lf skill show`,
   `lf task run`, `run --directive` or "one or two useful moves".
   `LOOPFLOW.md` is 114 lines.
2. **A real model on the new text, read-only — ran.** A fresh agent was given
   the composed `wave/session` text as its instructions, told nothing about a
   good answer, and produced the Product conversation's first turn from Jack's
   live status (20:36–20:43 UTC). Mutations were forbidden; it recorded the
   command it would have run.
3. **Installed conversation — not shown.** Needs an install on Jack's Home and
   `lf session replace` on the Product Wave conversation.

Trap for the export check: a dev build with no explicit `LF_HOME` re-execs the
installed `lf` for ordinary commands (`machine_install::dispatch_default_cli`),
so `HOME=<tmp> target/debug/lf home sync-skills` silently writes the *installed*
text. Use `env -i PATH=/usr/bin:/bin HOME=$H LF_HOME=$H/.lf target/debug/lf home
sync-skills --yes`. `lf help` and `lf list` answer before that hand-off.

## What the dry run did

The observed failure did not recur: no line handed work to "operations" or
another role, and every Task read got one named disposition with evidence.

| Disposition | Tasks |
| --- | --- |
| moving (left alone) | LOO-382, LOO-381, LOO-371 |
| waiting on Jack | LOO-353 and LOO-383 (demo reviews), LOO-380 (PR #1443 published, unarmed), LOO-330 (design unpublished), LOO-291 (PR merged; done or next PR?) |
| waiting on a dependency | LOO-376 (PR #1447 armed, checks unreported) |
| paused | LOO-378 (held), LOO-293 (Jack's 2026-09-24 interrupt) |
| would act | LOO-368: `lf --task LOO-368 flow start`, expecting a refusal because PR #1418 already merged |
| unknown | LOO-278: planning read fails, "refresh planning" |
| backlog, not started | LOO-333 |

It also said which background help was active (minute check disabled, CI
watcher polling) and promised no follow-up.

It refused the action status recommended for LOO-371 (`lf flow start`) because
a direct `pursue` was live there under another operator. That was correct, but
it found the live driver only by reading `lf task status` and the `lf ps`
parent chain; `lf wave status` alone said "ready, no worker claimed".

## Defects the dry run exposed in the new text

Checked against source:

- `wave_operate.md:63` and `task_operate.md:76` cite `lf pr next` as the way to
  report remaining scope. It rotates the Task to a new PR and has no dry run.
- `wave_operate.md:174` says take the title from `task.title`; the field is
  `task.name`.
- `wave_operate.md:6` says "Nobody else continues its started Tasks." On
  Jack's Home a 22-hour `wave/operate` conversation was driving LOO-371 and
  LOO-330 while this one ran, and GOAL.md declares an 08:00 `wave/operate` cron.

Reported by the agent, not independently checked:

- **Owner `wave` versus a reserved merge.** LOO-380 has `next_move.owner: wave`
  with recommended action `lf pr land -c`. "That owner is you: act" and "the
  Task waits on a person… do not choose a merge the person reserved" give
  opposite answers; the text never says how to tell a merge is reserved. The
  agent chose to wait on Jack.
- **Started Task with no Flow** (LOO-291, LOO-293): neither "a defined Flow
  proceeds" nor the finished-Flow rule covers it.
- **Liveness is not a precondition of `flow start`.** The `lf top`/`lf ps`
  reads appear only under recovery. Unmanaged direct Flows are invisible to
  the managed-Flow reason in `lf wave status`.
- **Held versus other dispositions.** `lf task automation` lists LOO-371 and
  LOO-376 as held while one is moving and one waits on CI; no precedence is
  given, and the text does not say whether a hold on the periodic check binds
  the operator.
- **`flow start` on a Task whose PR already merged** (LOO-368): status's own
  controls disagree with the skill's command; unsettled without running it.
- Status reads are 437 KB (`lf wave status`) and up to 170 KB per Task; the
  agent had to script extraction. The Wave's memory "arrives with this
  conversation" was untrue in the dry-run harness only.

## Changes made after the first dry run

In `wave/operate`, `task/operate`, `repo/operate`, the two sessions and
`wave-report`:

- A live-driver check precedes any continue or retry: every Flow in
  `execution.work.flows` (including `managed: false`) and every unfinished Exec,
  against `lf ps --json`. `lf wave status` describes only the managed Flow.
- "Nobody else continues its started Tasks" is gone; another conversation or
  scheduled pass may operate the same Wave, and its work is moving.
- `next_move.owner: wave` makes the disposition the reader's to establish; the
  recommended action is a suggestion checked against the rules.
- A merge nobody armed and `lf pr next` are the person's to choose (my
  choice of rule, not Jack's stated one). A started Task with no Flow waits on
  a person. A refused `flow start` is evidence, not something to work around.
- Dispositions resolve by the first row that fits, so a hold never hides
  moving or dependency-blocked work.
- Report shape, per Jack: **Waiting on you**, **Moving**, **Stuck**, then
  backlog; each row links its published PR from `prs[].publication.github`.
- `task.title` → `task.name` (also in `wave-report`).

Check result (demo, 2026-10-05): `cargo test -p loopflow --lib -- engine::prompt
engine::builtins engine::skills` 94 passed; `--test discovery_tests --test
documented_commands --test golden_prompt` pass with the launching agent's
`LF_*` variables cleared; goldens unchanged. Gate still owns the full plan.

## Second dry run (21:10 UTC, revised text)

The first turn came back grouped as Jack asked — seven Tasks waiting on him,
four moving, two stuck, backlog after — with a PR link on each published row
and no mutating command proposed. It refused `flow start` on LOO-371 again,
this time because of the pending review and hold, but called it a near-miss:
"a stricter reader would have" run it. It also filed LOO-383 and LOO-353 under
Moving because their review Sessions were connected. Both led to a last edit:
a connected conversation is not a driver, and "a defined Flow proceeds" applies
only with no live driver, no pending review on any Flow and no hold. That last
edit has tests but no third model run.

The run also reported, unverified here: PR #1439 (LOO-353) replaces
`lf --task <issue> flow start` and Session Ready/Complete with `lf task run`,
which the prompts in this PR instruct. Whichever lands second must reconcile.

## Still open

- Installed behavior: install this build, `lf session replace` the Product
  Wave conversation, and watch it act. Not shown.
- Whether an operator should arm a merge that status recommends.
- Status itself recommends `lf flow start` while a direct Flow is live, says
  "resume" with no pinned Flow, and returns 437 KB per Wave read. Those are
  `lf` defects the prompt now works around; no Task owns them.
- LOO-278's planning read fails with "refresh planning" and names no command.
