# LOO-935 open choices

## Steers (2026-11-16, word for word)

- Jack Heart: "You can drop everything old in this task as a special case".
- Jack Heart: "we can drop the thing where lf open does anything other than
  just open the task session in other surfaces."
- Jack Heart: "we dont need to try to copy our desktop layouts".
- Jack Heart: "i think workspace should just be for waves and repos now", "and honesrly workspace should be combined w open fr that reason", "do it here"; Wave open: "Conversation; overview by flag". See the lf workspace section of loo-935-store-reduction.md.
- Jack Heart: "lets split the PR" (drops move to a staged contract; see the Split section of loo-935-store-reduction.md).

## Picks made headless

- **Remote Machines on released lf** receive `lf open --workspace --local-workspace` and reject `--workspace` as an unexpected argument until they run this build; no fallback to the old `workspace` argv (no compat for solo dev). Upgrade the remote's `lf`.
- **`lf open --no-focus`** also applies to a single conversation (passed as the focus flag) rather than being refused.

- **The contract Task is LOO-954, not LOO-943**: `lf task reopen LOO-943` answered "An abandoned Task cannot be reopened". LOO-943 stays abandoned.
- **Indexes on leaving `tasks` columns stay** until LOO-954: released lf filters by them.

- **Desktop layouts live in `$LF_HOME/desktop-cache/`, not Application Support.**
  The Mac app has no Application Support directory; `workspace.json` and its
  other caches already live under `WorkCache.homeDirectory`.
- **A saved layout keeps every Session it names** (concept review): the
  first-page filter destroyed valid references, and a Mac-local file needs no
  ownership check. Done: `keeping(sessions:)` is deleted.
- **`lf desktop layout` stays as a read of `query:<id>`**; `--save` is gone
  (`lf desktop dashboard slots` writes slots).
- **Dashboards stay on the Machine** and keep their planning triggers (QA
  2026-11-16: with them dropped, a served `desktop.view.list` watch sent no
  frame after `lf desktop dashboard unpin` from the CLI). Still open, for LOO-940 rather than this PR:
  is a dashboard shared per repository (a saved view everyone sees) or one
  client's screen? `dashboard_arrangements.arrangement_json` is screen state
  keyed by repository, the shape the outcome says screen state should leave;
  if dashboards are per client, both tables move to Desktop like pane layouts.
  Are they used at all?
- **`tasks.regressed_in` stays** (LOO-730): released peers refuse a Task that
  arrives without `task_regressed_in`, and a peer edit writes the column.
- **`due_removals.py` schedule**: `install`/`uninstall`, Monday 09:00, log at
  `~/Library/Logs/loopflow-due-removals.log`, run through `uv` because macOS's
  system Python SQLite rejects the released migrations.
- **SCHEMA.md names Task and marking migration, no date**: a date would change
  at merge and fail `--check` on main.
- **`lf open --available`** opens each conversation through the single-
  conversation path on any host, rather than only Desktop, Rex, Herdr and cmux.

## Task tabs and viewers (2026-11-17, word for word)

- Jack Heart, on `lf task view`: "What even is this"
- Jack Heart: "a task tab was meant to be the task session, with an editable
  input. i think our policy for managed sessions is just you get many editable
  viewers into the same terminal, so you see drafts from A in B"
- Built today (transport/README.md, docs/sessions.md, docs/security.md, #1882):
  one viewer holds input; others attach read-only with **Move here**; `lf open`
  reports an already-open conversation instead of attaching; `lf resume` moves
  input. Overview tabs run `lf task view` (#1814, layout era).
- Jack Heart: "but we should change the ViewerPolicy", "in this diff." See "Every view of a managed Session is editable" in loo-935-store-reduction.md.
- Jack Heart, on ended Sessions in an overview: "opening 12 agents is fine. start provider right away"

## Picks made headless (2026-11-17)

- **`--local-workspace` stays** (hidden) though the follower steer listed it:
  another Machine's part of an overview is requested as `lf open --workspace
  --local-workspace`, which stops it asking peers in turn. The other five
  follower flags are gone.
- **`lf desktop view --task X`** errors like `lf open --workspace --task X`: it
  is the same overview path.
- **`--workspace --session S`** is an error too: one Session is one conversation.

## Sync with main (2026-11-17)

- File measurement attribution follows `WorkModel.repoPath` through `SessionsWorkspace.measurementRepository`; it no longer depends on server layout persistence.
- `swift build --build-tests --package-path swift`: pass; `scripts/test_desktop.sh --skip-build --filter WorkDestinationTests`: 71 pass. Terminal focus checks require a display; gate/CI runs them.
- Jack Heart: "just always have it take over" (native-client Sessions, every caller). See "lf resume does one thing".
