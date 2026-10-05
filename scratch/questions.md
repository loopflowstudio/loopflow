# Open choices — October 5, 2026

For [the design](focus-on-your-own-work.md). None blocks the next pass; each
has a default the pass follows. Defaults are the agent's, not Jack Heart's.
Earlier interpretations are at `6f246fda4:scratch/questions.md`.

## Answered by Jack

- **Who backgrounds, now that `-b` blocks?** Jack (October 5): "cant you just
  background with &". The caller does: `lf --task X -b <flow> &`. No detached
  launch mode. Agent's reading of the consequences: delete `task create
  --run/--flow`; Desktop already spawns a child process and reads Execs for
  the result. Jack: "like llms know how to background things" — skills name
  the plain command and do not teach backgrounding.

- **Task helpers.** Jack (October 5), on deleting `task create --run`: "Im
  fine with this. i am fine with task helpers that sit on top of the lf
  system, but i dont want to introduce parallel paths or drivers." The test
  for any Task command: it may prepare, then it enters the ordinary path; it
  never owns a second launcher, driver or record.

## Asked of Jack, unanswered

- **Planning refusal** (terminal, moved or removed Task). Default: apply once
  at Task resolution, for every launch that resolves to a Task.
- **`task automate`/`automation`, `task interrupt`, `task wait`:** worker APIs
  or ordinary Task commands? Default: keep; `automate` holds CI repair only.
- **Name of the Task entry.** Default: `lf task run ISSUE [FLOW]`.
- **A past Flow after its YAML changed** is drawn from its Exec sequence, not
  a pinned graph. Default: accepted, as it follows from Jack's requirement.
- **`lf commit`:** Jack offered always-push or `-p`. `-p` is restored; plain
  commit stays local.

## Interpretations still awaiting Jack's review

- Builtin `feature` ends at a published PR with no design-review pause; `ship`
  lands after the demo; `code` equals `pursue`. Open: whether `feature` should
  stop after kickoff, and whether `code` should remain.
- A Flow that stops at a watched landing ends when its driver exits; nothing
  resumes it. Landing cleanup proceeds as for a standalone landing.
- Main's `lf flow end` (#1435, LOO-326) is dropped: a stopped Flow blocks
  nothing. Open: whether Infrastructure wants a mark on stopped history.
- The per-Task automation on/off field is kept for CI repair only.
- Implementation defaults: 120-second Waiting fallback, most-recent
  interactive Task primary, optional occurrence names, backward-target loops,
  source-only workflow YAML.
- Global `-w` and `lf session open`, also removed by #1356, are left alone as
  outside this Task.
