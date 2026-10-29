# LOO-618 assumptions (headless run, 2026-10-28)

- `--fork` is harness-agnostic: a new Session in the same Work seeded with the source's
  conversation log (`conversation_log::continuation`), not Claude's `--fork-session`.
  Reversible: a native fork per harness could replace the seed later.
- `lf session new` takes an optional exact ID besides `--task/--wave`, so Desktop can
  restart any Session (it used scope selectors and refused non-primaries before).
- Takeover of a client lf did not start (raw `claude`/`codex` still open): lf can't
  identify its process, so `lf resume` continues anyway; docs tell people to close it or
  `--fork`. No signalling of foreign processes.
- `--observe` (read-only attach) is gone with `session attach`; the transport protocol
  still supports passive viewers. Nobody in Desktop used it.
- `.lf/wave/product/MEMORY.md` still names `lf connect`; Wave memory is read-only here.
  Proposed: replace with `lf resume` at the next Wave maintenance pass.
- LOO-424 (resume any conversation from anywhere) is covered by `lf resume <provider-id>`;
  closing it into this Task is left to the Task's operator.

## Accepted (Jack Heart, 2026-10-28)

- Harness-agnostic `--fork` (conversation-log seed) is fine for now.
- No signalling of raw `claude`/`codex` clients lf didn't start; `lf resume` continues and docs point to close-or-`--fork`.

## Run refusal (headless implement, 2026-10-29)

- A Graph's structured-output correction runs `lf -b session resume <step-run> --message`
  (`graph.rs`). Refusing every Run would break it, so the Graph process continuing its own
  step's Run (caller's parent = the Run's `graph_lf_process_id`) is the one exception.
  Reversible: the correction could move to a non-resume spelling later.
- A Run is `!interactive`. An `interactive:` Graph step is a Session and stays resumable.
- Desktop selecting a Run row now shows the refusal text (which names `lf resume --task`);
  no Swift change made for it.

## Assignment (headless implement, 2026-10-29)

- `lf ship` claims an unowned Task when it begins, like `lf task run`; only an unassign during
  shipping stops it. A ship never refuses at start for being unassigned.
- `task edit --assignee` stays: it assigns another person in Linear, which claim can't.
- Started, unowned Tasks read as Later (Wave roadmap) / Backlog (Desktop standup). Tasks
  started before the ownership log existed with no claim would also land there.
- `lf task unassign` on a Task nobody owns: settled in the second headless concept review
  (2026-10-29) as a no-op on ownership that still clears the Linear assignee, from Jack
  Heart's decision 5a and AGENTS.md Flexibility. Reversible; see the working design.
- User-facing errors now say "headless Run (-b)" instead of "headless Session (-b)"
  (`agent/session.rs`, `lf/commands/session.rs`); maintainer comments keep LfSession wording.
