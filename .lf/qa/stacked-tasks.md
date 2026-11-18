Read when the change touches stacked Tasks, `lf sync`, `lf pr publish` or a PR's base.

# Stacked Tasks against a fake GitHub

No real GitHub needed. Give the repository a GitHub-shaped origin that Git
rewrites to a local bare repository, and put `.lf/qa/gh-standin.py` on `PATH`
as `gh`:

```sh
Q=/tmp/lf-qa-stack; mkdir -p $Q/bin $Q/home $Q/lfhome
cp target/debug/lf $Q/bin/lf; cp .lf/qa/gh-standin.py $Q/bin/gh
git init -q --bare -b main $Q/origin.git; git init -q -b main $Q/repo; cd $Q/repo
git config url."$Q/origin.git".insteadOf https://github.com/qa/repo.git
git remote add origin https://github.com/qa/repo.git   # lf reads owner/name from this
```

Run every command through a wrapper that unsets `LF_*`, sets `HOME`,
`LF_HOME`, `LF_BIN`, `GH_QA_STATE=$Q/gh.json` and a `PATH` without the real
`claude`/`codex` (so `lf task checkout` warms no conversation), and closes
stdin. `HOME` needs a `.gitconfig` identity. `lf task create '<title>'` gives
short hex identifiers; `lf task checkout <id>` places the worktree beside the
repository. `lf pr open --title … --body …` publishes through the stand-in.

The stand-in keeps PRs in `$GH_QA_STATE` (edit `base`, `state: MERGED`,
`merge_commit`, `merged_head` there to act as GitHub) and logs every call to
`$GH_QA_STATE.log`; `touch $GH_QA_STATE.fail-edit` makes `gh pr edit --base`
fail. lf learns a parent merged only through `gh api …/pulls/<n>`, and caches
that read: 60 s when fresh, 5 min when degraded. A stand-in that refused the
read earlier keeps lf blind for five minutes; check `task_prs.github_observation`.
To land a parent the way GitHub does, merge or squash it into `main`, push,
delete its branch, mark the PR merged in the state file.

Never pipe `lf sync` into `head`: SIGPIPE kills it after it records a stack
and before it merges.
