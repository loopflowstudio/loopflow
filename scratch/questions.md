# Open questions and assumptions (LOO-624 repair)

- Resolved 2026-10-27: Jack: "just delete the fly rentals". The draft
  `forget_fly_rentals` deletes 0.14.17's Fly on-demands and rentals and clears
  scopes that sent Tasks to one; no reader parses `fly`.

- Found 2026-10-27 in the AWS proof: `LF_RENT_LINUX_BIN` only copied the
  binary over a default home, where no unpublished build may create the store
  and a published install would re-exec every command into itself. Fixed: the
  rental runs the uploaded build on a disposable home (`LF_HOME=~/.lf-dev`,
  `LF_BIN`) selected in `/etc/environment`, as branch builds run on a laptop.
- `~/.lf-qa` (made by a pre-release dev build) no longer opens with any
  current build. Its Claude long-lived token and Linear grant were moved to
  `~/.lf-qa2`, and its db renamed `loopflow.db.retired-moved-to-lf-qa2`.
- Fixed 2026-10-27: rental `prepare` looked the repository up by
  `CanonicalRepo` and failed "has no repository identity yet" on a fresh home
  first used from a worktree. It now binds the Task's `repository_id`. The
  worktree-path recording on a fresh home itself is unchanged.
