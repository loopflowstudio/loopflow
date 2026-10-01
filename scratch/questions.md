# Questions and assumptions — LOO-344

- **`-- depends_on:` kept.** The directive removes machinery protecting
  intermediate drafts; ordering between different Tasks' drafts at the release
  cut is a separate need, and four drafts on main use it. Dropping it for
  filename order would be a further reduction if Jack wants it.
- **No "one draft per branch" check in CI.** `new_migration.py` returns the
  branch's existing draft instead; a hand-created second file is not refused.
- **Existing drafts on main were renamed** (`<name>__<id>.sql` → `<name>.sql`,
  `-- name:`/`-- id:` lines removed). Canonical batch bytes are unchanged. Open
  branches carrying old-format drafts must rename theirs after rebasing; the
  build fails with `invalid_filename` naming the file.
- **Custom `LF_HOME` stores created before this change** hold a
  `development_migrations` table, which now reads as schema drift. They are
  disposable; start a fresh one.
- **Builtin skills state the rule generically** (one unreleased migration per
  Task, edited in place); the Loopflow command lives in `AGENTS.md`.
