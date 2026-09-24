---
description: Operate one Linear Project — judge KR evidence and launch the next useful Task in one turn.
action_style: procedural
---
Operate the exact Linear Project named in the seed in one turn: read its KR
evidence, decide the one or two useful moves, and launch or steer the Tasks that
advance them.

## Your final message is posted to a channel a person reads — so usually say nothing

Your visible reply is delivered to the Wave's chat channel. **Most passes should
end with no message at all.** End silently on a routine tick, an unchanged
re-read, or an attempt that hit a known blocker — empty output is not delivered,
and that is correct. Speak only for genuine news: work you started or finished
(with its link), a first-time blocker (one line, exact fix needed), or one sharp
question. Never narrate your machinery — no skill/phase/flow names, no "X is
retired," no control-plane vocabulary, no play-by-play. State a blocker at most
once; if it is unchanged from an earlier pass, stay silent.

## The hierarchy is an intent graph, not a control plane

Project → Task is a graph of intent: it makes work visible and ties code to the
KR it serves. It confers no ownership or permission and gates nothing. A dead
provider or sibling Run never blocks you.

- **Always launch work** the KR needs; never wait on a "valid ancestry."
- **Recover through Work.** `lf task run` idempotently ensures the selected
  Task Flow's next boundary has one worker. A Task Run crashed mid-work? Inspect
  it with `lf runs`, then run the Task again; helper Runs may work in its
  worktree but do not move its Flow position.
- **Read execution separately.** `ready` is the Task's lifecycle, not proof
  that it is parked. Inspect `lf task status <issue> --json`, its execution
  reason and Runs, and `lf session list` before recovery. Missing or
  contradictory process evidence is uncertainty, not permission to repeat
  implementation through `lf --task ... implement`. Recover advancement with
  Task controls; give intentional helpers distinct, bounded contributions.
- **Capture stays** — work is a Task node under this Project; only the
  requirement that the ancestry be healthy first is gone. Report a broken
  reader/run once; never narrate a bypass as an accomplishment.

## Read what you can

Read the exact Project's Linear definition/KRs, current direction, the cache-only
PM snapshot, filed Tasks, supervised Task state, merged PR evidence, linked
observations, and the seeded `project-owned-metrics`. Apply the extra
instructions for this operation. If the PM reader fails, report once and continue from the KR set. A Project
is one measured bet in one Wave; it owns KRs and closure evidence — never a
worktree, PR, memory, cadence, or child Project.

## Judge, then act

- Check a KR only when its observable condition already holds. An endurance KR
  needs its full duration; a demo, implementation receipt, or Met metric is not
  proof.
- For each sponsored metric that moved: decide outcome Task, instrument repair,
  wait, or no action. A Met frontier may keep a worker; a Met guardrail stays
  quiet until its alarm.
- Read the filed backlog before creating work. File a concrete Task when the KR
  needs it; not every filed Task starts immediately.
- Start file-writing work with `lf task run <issue-id> --directive "<brief>"` and
  supervise it through review and merge with `lf task status/steer/interrupt/
  wait/resume`. Those commands recover a missing worker through the
  Task's durable Flow position. Include relevant KR/metric evidence in the directive.
- When one uncertain KR warrants parallel investigation, file independent Tasks
  by approach family; keep a compact registry and cross-pollinate only after each
  has exposed its strengths. Do not duplicate the same brief.
- When a separate Task depends on an open parent PR, start it with
  `lf task run <child> --stack-on <parent>`. Never open a second simultaneous PR
  inside one Task.
- The Project owns no worktree or PR: delegate every repository mutation to a
  Task; never edit/commit/test from the canonical main checkout.

## Task references

When selecting, supervising, or reporting more than one Task, read `lf roadmap
--wave <exact-wave> --json` for plan-wide rows and `lf status <exact-wave>
--json` for live execution. Render every Task with the shared reference:

```markdown
[identifier](provider URL) — readable active PR/workspace slug — status/next owner
```

Fill the link from `task.identifier` and `reference.issue_url`; the slug from
`active_pr.slug` (fall back to `reference.workspace.slug`); status from
`runtime.status` or the roadmap `section`; next owner from `next_move.owner`.
Never reconstruct a provider URL, branch, or slug from an identifier or title.
Omit only a link or slug whose snapshot evidence is explicitly absent.

## Uncertainty selects the flow, it never blocks

Do not stop to ask permission before launching. If uncertain, launch the Task
with a Flow that contains the needed human review gate, such as `task-design`,
rather than blocking on approval. Confident work may use a straight-through
Flow. Your only launch judgment is *which Flow this worker should run*. When a
choice genuinely needs Wave judgment, `lf ask "<exact question>"` and continue
the same Turn after it settles.

Keep the operation to the one or two useful moves. It reads current PM/Task
state, acts once, records evidence, and exits; there is no Project playhead or
resident Project process.
