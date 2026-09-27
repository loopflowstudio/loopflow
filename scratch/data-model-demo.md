# LOO-298 demo — one table per main object

Prepared for Jack on 2026-09-27. Attribution of the delegated decisions below
is to the supervising Claude session acting while Jack was away; Jack has
reviewed none of them.

## What to try

Everything runs against a copy of the installed Home, never the real one.

```sh
cd /Users/jack/src/loopflow.data-model-one-table-per
export LF_HOME=/private/tmp/claude-501/-Users-jack-src-loopflow-main-view-task/f80b50d6-c50a-4489-8d81-d10a36f31ae3/scratchpad/home-f
LF=$PWD/target/debug/lf

$LF session import --dry-run     # a second import changes nothing
$LF session list --json          # one query: interactive, Ask, Flow review, Task review
$LF runs --task LOO-298 --json   # one query over runs
$LF session bind <orphan-session> --task <unstarted-issue>
$LF runs --task <unstarted-issue> --json   # the bound Run; the Task is now started
$LF session bind <same-session> --task <other-issue>   # refused: write-once
```

## What is built

| Object | Table | Notes |
| --- | --- | --- |
| Session | `sessions` | owns Runs; `current_run_id`; all four kinds are rows |
| Run | `runs` | every launch stores a row; nullable invocation, task, wave, session; filled upward |
| Flow invocation | `flow_invocations` | nullable Task; saved Flows write a row at each step |
| Task started | `tasks.started_at` | set once; trigger validator against `runs` |

Commands added: `lf session bind`, `lf session import`.

## What is not built

- The saved Flow's `position.json` still owns its cursor. The invocation row is
  a copy. Four decisions in `cutover/cut-g-deletion-research.md` block the move.
- `task_flow_positions`, the `prepared` marker, `terminal.json` as the state
  recovery reads, and string subjects in manifests remain.
- Swift and DTO shapes are unchanged. Nothing in the app uses bind yet; that is
  LOO-303's next slice.
- Migration drafts are not materialized or rehearsed beyond opening a Home copy.

## The honest size

The branch adds about 1,376 production lines over its merge base. Cut 3 removed
635 and Cut G removed 63; the store, bind, import (538 lines, deletable once
every Home is imported) and the Run constructor added more. The large
deletion is the saved Flow driver, estimated at −330 against +150.

## Decisions waiting for Jack

Full text in `questions.md`, sections Cut 1 through Cut G. The ones that change
behavior a user can see:

1. A launch never fails on an unwritable store: it warns once and is never
   recorded. An Ask refuses instead. No sidecar replays it.
2. A launch is no longer refused for an unready Task or a non-current Chapter.
3. `lf session bind` does not confirm, refuses a Flow review Session, and
   refuses when any Run of the Session already has a Task.
4. A Flow step Run may name a Task while its invocation names none; the review
   Run of the same saved Flow carries the Wave and no Task.
5. A Run that names no Work inherits its caller's Task and Wave.
6. Import: a Task first named by an imported Run gets the import time as
   `started_at`; six finished Flow step Runs with no Session stop listing as
   Sessions.
7. Wave, Project, usage and activity listings show only Runs with rows, so an
   old Home lists nothing there until it is imported.
