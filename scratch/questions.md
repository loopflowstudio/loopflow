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
- Production timing is a bounded JSON-lines file at `<Home>/perf/wt-list.jsonl`,
  not Exec rows and not the Desktop `studio.loopflow`/`perf` signposts. Exec
  rows are lost under exactly the contention that makes a listing slow, and
  signposts need Instruments to read. The sample stores the repository root
  path; the brief allows paths within the repository.
- "One receipt deadline per process" is implemented as one 15 s wait shared by
  an Exec's receipts, not a wall-clock deadline from process start: a long
  command would otherwise reach its finish receipt with no wait left.
- The report command is `lf wt timing`, a sibling of `lf wt list`, rather than
  a flag on the listing or a general `lf perf` surface with one instrument.
- The Flow's final step lands with `-c`. Completion still needs ordinary-use
  timing read after an installed release; landing this PR should keep the Task
  open.
- `.lf/flows/pursue-auto.yaml` entered this branch through the restart
  checkpoint and nothing in the repository references it. Left in place: the
  running Flow was started from it, and removing a Flow definition mid-run is
  not this Task's call. Whether it should land on main is unresolved.
