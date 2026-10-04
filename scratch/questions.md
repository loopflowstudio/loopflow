# Assumptions (LOO-375)

- A local branch already contained in `origin/<default>` was never reported as
  `merged` by the old local check (`has_commits && is_ancestor` cannot both
  hold). The rewrite states that directly: `merged` comes only from PR
  evidence. Output is unchanged.
- Remembered squash/diff answers live in `.git/lf-commit-facts`. Treated as
  compatible with "read-only checkout": no checkout, index, ref or config is
  written, and `merge-tree --write-tree` already wrote objects there.
- When GitHub answers, branch existence comes from GitHub instead of
  `git ls-remote`. Assumes `origin` and the GitHub repository are the same
  remote, which `github_repo_nwo` derives from `origin`'s URL.
