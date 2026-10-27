# LOO-635 hotfix skill: assumptions

- Worktree uses plain `git worktree add <source_revision>`: `lf wt create` takes no base revision. Revisit if it grows one.
- Clone step measures at the statement level with `sqlite3` on a `cp -c` copy; `lf clone` and `LF_HOME` redirection were not verified as a way to run a whole lf against the copy.
- Steps 4 and 6 describe today's commands (env-var build, ps + manual stop). Update when LOO-633 (hotfix provenance) and LOO-634 (rollover) land.
- Observed while writing: `git tag --contains e9b2c2eac1` is empty, so v0.14.17 (8ad349a841) doesn't contain the 0.14.16 hotfix commit. That's db678e's to resolve, not this Task's.

Remaining: the demo is the next local hotfix running `.lf/skills/hotfix.md` end to end (clone before/after, tests before promote, PR to main). Realigned 2026-10-26: origin/main unchanged since 8ad349a841; LOO-633 and LOO-634 still started, so steps 4 and 6 stay on today's commands.

Check: `lf help hotfix` resolves .lf/skills/hotfix.md; active.json one-liner (revision, identity, CLI path) and ps|uniq -c count run against this Machine — pass.

Concept review 2026-10-26: the seven steps match the Task outcome order and its one concept (base = active.json source_revision, never main). Verified against this build: `promote --preview` verdict strings (install.rs:587), build.rs env vars, `session list --available --all`, `session connect`, migrations dir incl. drafts/. No change, no open product choice.

Gate 2026-10-26: prose-only diff, no build. Verified against this tree: `lf help hotfix` resolves; `self install promote --cli-target/--preview` flags; verdict strings (install.rs:587-588); build.rs env vars and `<version>+<9-char sha>` version shape; `session list --available --all`, `session serve`, `session connect`, `pr publish` exist; active.json one-liner prints e9b2c2eac1 release + CLI path — pass.
