# LOO-609: cmux workspaces are named after their Task

Status: accepted plan (Jack Heart reviewed it October 26 and moved the ID to the end). Product direction accepted by Jack Heart
on October 26; the implementation choices below are reversible and recorded in
`scratch/questions.md`.

## Problem

Jack Heart (October 26): "when using lf to launch new windows in cmux (either in
convo with the loopflow skill or through api) we make the workspace titles start
with the names rather than ids, and always stay that (only update if task name
changes. dont send changes back as task name changes."

Today a cmux workspace for a Task is titled by its identifier and keeps changing:

1. `lf open --host cmux` (`open.rs::open_cmux`) creates it as
   `INF-123 · Prove Task PR transitions · Explore` (`Resolved::destination_name` =
   label `identifier · title` + `standing`).
2. The `lf session connect` running inside it (`TerminalTitle::prepare`, also the
   transport viewer via `ViewerTitle`) immediately renames the workspace **and**
   tab to the Session display name, `INF-123 <3-word purpose> · Available · Design`,
   and re-renames every 500 ms whenever the Session name, availability or Workflow
   node changes.

Both conversation launches (agent using `lf open`) and client/API opens (Desktop's
host enum includes `cmux`; both reach `lf open`) go through these two writers.

## Demo

```
lf open loopflow://loopflow/task/LOO-609 --host cmux
```
A new cmux workspace appears titled `Name cmux workspaces lf opens after their
Task, not its ID · LOO-609`. The Session inside renames itself (`lf session rename …`) and
becomes Available: the workspace title does not move (the tab does). `lf task edit
LOO-609 --title "Task-named cmux workspaces"` (or a Linear title sync) retitles the
workspace within a second. Renaming the workspace in cmux changes nothing in lf.

## Approach

One rule: **a cmux workspace's name is `<Task title> · <identifier>`** (Jack Heart,
October 26: the ID goes at the end, not the beginning). Only the Task title feeds
it; the identifier never changes. Sessions own tabs.

- `open.rs`: `Resolved::destination_name` returns `<title> · <identifier>` for Task targets
  and the existing label for Wave/repository/taskless-Session targets. `standing`
  leaves destination names; the `Resolved.standing` field and its
  `terminal_title::standing` call in `describe` are deleted. This is the one name
  every host gets at creation (cmux, Herdr, tmux); see questions.
- `terminal_title.rs`: `TerminalTitle` tracks two published names — `workspace`
  (`<Task title> · <identifier>`, `None` for taskless Sessions) and `name` (the existing Session
  terminal title). `HostNames::Cmux` renames the workspace only when the Task title
  differs from the last one it published, and the tab when the Session name
  differs. Taskless Sessions never rename a cmux workspace. tmux is unchanged
  (its window is the Session's, like a cmux tab).
- Split `publish` into a pure "which renames are due" step (previous published
  names vs. current store reads → `Vec<HostRename>`) and the `host_command`
  execution, so the rule is unit-testable without spawning cmux.
- Control characters are stripped from the Task title as `display_title` does
  for Session names. An empty Task title leaves the identifier alone.

### Write-back inventory

No path reads a cmux name back. The only cmux invocations in the tree are
`workspace create` and `workspace select` (open.rs) and `rename-workspace` /
`rename-tab` (terminal_title.rs); Task titles change only through lf's Task
writers and Linear sync. Nothing to delete; the proof test pins it.

## Risk findings

- **Two writers, one name.** Creation and runtime both name the workspace; if
  they diverge the workspace flickers on attach. Both now read the Task title, so
  the runtime's first publish is a no-op rename.
- **Standing loses its cmux surface.** Availability and Workflow node were
  visible in the sidebar; they stay visible on the tab (Session terminal title)
  and in the `lf open` receipt. Jack's "always stay that" outranks it.
- **Existing workspaces.** A workspace opened before this change keeps its old
  title until its Session reconnects, then gets the Task title. No migration.
- **Long Task titles** are passed whole; cmux truncates in its sidebar.

## Current → target

| | Before | After |
|---|---|---|
| Create name (`destination_name`) | `ID · title · standing` | `title · ID` (Task); label otherwise |
| Runtime cmux workspace | Session name + ID + standing, follows Session rename/standing | `title · ID`, follows only Task title |
| Runtime cmux tab | same as workspace | Session terminal title (unchanged) |
| Taskless Session in cmux | renames workspace to Session name | leaves workspace name alone |
| tmux window / OSC title | Session terminal title | unchanged |

## Delete — do not maintain

- `Resolved.standing` and its `terminal_title::standing` call in `open.rs::describe`.
- The workspace half of `HostNames::Cmux::rename` driven by the Session name.

`terminal_title::standing` itself survives (Session terminal title uses it).

## Forbidden outcomes

- Any `cmux` read (`list-workspaces`, `current-workspace`, …) feeding a store write.
  (Reading the current name only to skip renaming a hand-edited workspace is allowed;
  Jack Heart, October 26.)
- A second name for the same workspace (e.g. creation with ID, runtime with title).
- Overwriting a workspace name someone edited by hand (Jack Heart, October 26,
  reversing the earlier "no guard" line): lf renames only names it wrote.
- `<identifier> · <title>` in `lf open` receipts or other Task labels; they read
  `<title> · <identifier>` too (Jack Heart, October 26).

## Slices

One diff. Implemented October 26 (steps 1–4 below): `terminal_title::workspace_name`
is the one name both writers use; `renames_due(previous, current)` is the pure step
(no `published` field: a failed rename detaches the host, so the previous names are
what it shows). Committed as 7367f02689; the later main merge touches none of
these files. Remaining: concept-review, gate, pr-publish.

Checks: `cargo test --lib terminal_title` 5 ok (after compress); `--test integration work_open` 13 ok;
clippy clean; `session_cli` deferred to gate.

1. `terminal_title.rs`: workspace/tab split, pure due-renames step, unit test.
2. `open.rs`: `destination_name` → `<title> · <identifier>`; delete `standing` field.
3. Tests: update `work_open_tests::cmux_opens_the_task_conversation_through_session_connect`
   to expect `workspace create --name Prove Task PR transitions · INF-123 --cwd`; adjust any
   Herdr/tmux create-name assertions the same way.
4. Docs: `docs/sessions.md` (was `docs/lf.md`) ("In cmux, the attached workspace and tab follow `lf session
   rename`") → workspace follows the Task title, tab follows the Session name;
   `docs/architecture-reference.md` LfSession row (`process:cmux` projects the
   Task title onto the workspace, the Session name onto the tab).

## Concept review (October 26, headless)

Implemented change matches the accepted model: one `workspace_name` for creation
and runtime, Task title is its only input, Sessions own tabs and tmux windows,
no cmux read path (`grep '"cmux"'` shows only create/select/rename). Nothing to
change; no open product choice. Noted, not changed:

- tmux and cmux-tab destinations are created with the Task workspace name and
  renamed to the Session terminal title on attach. Before this change they also
  differed (`ID · title · standing`), and Herdr, with no runtime rename, keeps it.
- Attaching a Task Session to a workspace someone named by hand renames it to
  the Task name once (first publish); this is the accepted "no guard" rule.
- `Resolved.label` (receipts) stays `ID · title`, like terminal titles; only the
  workspace leads with the title, as Jack asked.

## Done when

- `lf open … --host cmux` for a Task creates a workspace named `<Task title> · <identifier>`.
- A Session rename or standing change in that workspace issues no
  `rename-workspace`; a Task title change issues exactly one.
- No lf code path reads a cmux name.

Acceptance at gate (headless, `LF_*` unset, `< /dev/null`):

```
cargo test -p loopflow --lib terminal_title
cargo test -p loopflow --test integration work_open
cargo test -p loopflow --test integration session_cli
```

Expected: the unit test asserts — for a Task-bound Session with fake prior names —
Session rename ⇒ only `rename-tab` due; Task title change ⇒ `rename-workspace`
with the new title due; taskless Session ⇒ never `rename-workspace`. The open test
asserts the create name; a `grep -rn '"cmux"' rust/loopflow/src` review shows only
create/select/rename invocations.

## Exclusions

- Herdr runtime renaming (none exists) and tmux window naming.
- Desktop's own pane/tab titles.
- Writing cmux names back into Tasks or Sessions (explicitly forbidden).
