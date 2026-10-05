# Session and operate prompts keep work moving together (LOO-383)

Status: implemented on the branch and approved by Jack Heart in demo review on
2026-10-05 ("once youre done, this is approved"). Gate, the installed demo and
reconciliation with PR #1439 remain. The full design, audit and both dry-run
records are at
[commit 831b979ec](https://github.com/loopflowstudio/loopflow/tree/831b979ec/scratch);
the durable account is [the review](../docs/reviews/session-operate-prompts.md).

## Decisions (Jack Heart, 2026-10-05)

1. **Started work must keep moving.** A Wave makes sure every started Task
   keeps moving. It does not start unstarted work "for now at least." The goal
   is "reliably finishing stuff ive started and isnt blocked on me."
2. **The procedure is inline in the session.** "No strong opinions; just make
   it work reliably." No run-time `lf help` read.
3. **A defined Flow proceeds.** "We can assume good flows for now that
   basically end at landing." A Task whose Flow ended before landing waits on
   Jack.
4. **`task/session` is a plain skill**, `lf --task <issue> skill task/session`.
   A primary Task session is "just a smaller wrapper around this that saves
   that id in a field" (LOO-364, not built here).
5. **How a Task changes its Flow stays TBD.** Jack wants it; no design exists.
6. **Report shape** (demo review): "a little more structure… links to PRs,
   sorted by waiting on me vs currently moving vs stuck in some way."

Jack did not answer whether an operator should arm a merge that status
recommends. The branch chose the conservative rule: an unarmed merge and
`lf pr next` are the person's. That is the branch's choice, not Jack's.

## What the branch does

| Scope | Operate (finite procedure) | Session (ongoing conversation) |
| --- | --- | --- |
| Repository | `repo/operate` | `repo/session` |
| Wave | `wave/operate` | `wave/session` |
| Task | `task/operate` | `task/session` (new) |

- `build.rs` composes each session from its session file plus its operate body
  under a generated `# Operating procedure: <scope>/operate` heading. Session
  files hold no Task-operation steps; operate skills stay standalone.
- Every started unfinished Task ends a pass with the first disposition that
  fits: moving, acted, waiting on a person, waiting on a dependency or
  capacity, paused, unknown. Unstarted backlog is listed and left alone.
- A live-driver check precedes any continue or retry: every Flow in
  `execution.work.flows` (including `managed: false`) and every unfinished
  Exec, against `lf ps --json`. `lf wave status` describes only the managed
  Flow, and the first dry run nearly started a second driver from it.
- `next_move.owner: wave` is the reader; its recommended action is a suggestion.
- A started Task with no Flow, or whose Flow finished unlanded, waits on a
  person. A refused `flow start` is evidence, not something to work around.
- Sessions say that nothing schedules their next turn and name which background
  checks are installed instead of promising to watch.
- LOOPFLOW.md is 114 lines, with Task/Session/Flow/Exec vocabulary.

Forbidden outcomes still binding on later edits: a hand-written second copy of
the procedure in a session file; an operator starting unstarted backlog or
selecting a Flow for a Task; a session that promises to watch; completing or
approving a review without the person; a new scheduler, controller or DTO.

Kept deliberately: `wave/operate`'s Task-brief section (`capture-tasks` does
not cover its description-versus-comment rules); the sessions' workspace
section (`sync-conflicts` does not cover the `.lf-sync-N` recovery); operate's
one-pass wording inside the composed sessions, where the session text governs
repetition.

## Evidence so far

- Branch build, headless: `lf list --json` lists all six; each session's export
  carries its operate procedure once; no export contains `lf skill show`,
  `lf task run`, `run --directive` or "one or two useful moves".
- Two read-only model runs on the composed `wave/session` text against Jack's
  live Product status. Neither handed work to another role; the second produced
  the grouped report. The last edit after it (a connected conversation is not a
  driver; "a defined Flow proceeds" only with no live driver, pending review or
  hold) has tests and no third run.
- Installed conversation behavior: not shown.

Export-check trap: a dev build with no explicit `LF_HOME` re-execs the
installed `lf`, so a bare `HOME=<tmp>` sync writes the installed text. Use
`env -i PATH=/usr/bin:/bin HOME=<tmp> LF_HOME=<tmp>/.lf target/debug/lf home
sync-skills --yes`.

## Remaining

- **Gate**, once:

  ```bash
  cargo fmt --check && cargo clippy --all-targets -- -D warnings
  cargo test -p loopflow --lib -- engine::prompt engine::skills engine::builtins ops::task_automation
  cargo test -p loopflow --test discovery_tests --test documented_commands --test golden_prompt --test default_conversation_tests
  ```

  `default_conversation_tests` passes only with the launching agent's `LF_*`
  variables cleared.
- **Installed demo** (post-merge, not a gate): install, `lf session replace`
  the Product Wave conversation, say nothing, and watch its first turn give
  every started Task a disposition; then ask "what's running?" and file an idea.
- **PR #1439 (LOO-353)** reportedly replaces `lf --task <issue> flow start` and
  Session Ready/Complete with `lf task run`; unverified here. Whichever lands
  second reconciles the prompts and `ops/task_automation.rs`'s hint.
- `lf` defects the prompt works around, owned by no Task: status recommends
  `lf flow start` while a direct Flow is live and says "resume" with no pinned
  Flow; `lf wave status` returns 437 KB; LOO-278's planning read fails with
  "refresh planning" and names no command; `flow start` on a Task whose PR
  already merged (LOO-368) is unsettled.

Check result (compress, 2026-10-05): prose reduction after the demo commit;
`cargo test -p loopflow --lib -- engine::prompt engine::builtins` (90 passed)
and `--test golden_prompt --test documented_commands` pass. Gate owns the full
plan.
