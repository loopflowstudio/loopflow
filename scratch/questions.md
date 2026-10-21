# LOO-539 open questions

- **Retired release debris (open for Jack).** There are 36 `jack-heart/{prepare,publish}-default-v0-12-*/v0-13-*`
  branches on origin. Their heads are all contained in `main`, and their tags exist.
  No current lf record names them, so the plan retains them and lists the reason.
  Getting rid of them takes a one-time `git push origin --delete …`. Teaching prune
  the retired naming scheme would also work, but the plan rejects that option.
- **Assumption: command surface.** The plan uses `lf wt prune --branches` as the
  third prune mode next to `--builds`, because `wt` already owns branch deletion.
  The flag is named `--branches`, not `--remote`, because the command also removes
  finished local refs. This is reversible.
- **Assumption: local refs are in scope.** The plan reads "plus their stale refs"
  as covering local branches without a worktree that have the same finished-work
  evidence (120 in the 2026-10-20 dry run), plus `git remote prune origin`.
- **Assumption: failure exit code.** A failure in authentication, rate limiting,
  or a deletion exits 1, so the hourly log shows it. A GraphQL budget below 200
  also exits 1, as a skipped run.
