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
`lf land --title … --body …` arms against it: `gh pr merge --auto` stores the
matched head as `auto` on the PR and the `LoopflowPrMerge` query reports it, so
land returns "handed off" without merging. Pass `--title`/`--body` or land asks
an agent for PR copy.

For a stacked land, give `origin.git` an `update` hook that appends
`$1 $2 $3` to a log: one line per pushed ref. A child whose parent squash-landed
and whose parent branch was deleted lands with "Dropped branch-added scratch
conflicts" and no "Launching sync agent". To see what `sync-conflicts` gets on a
real code conflict, put a stand-in `claude` on `PATH` that answers `--version`
and `auth status`, logs `ls scratch` and `git status --short`, and exits 1; run
`lf land -a claude`. `lf sync --abort` then `lf sync main --manual`, edit,
`lf sync --continue` resolves it by hand.

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

A parent that closes and reopens: set the PR's `state: CLOSED` in the state
file, commit in the parent's checkout and run `lf pr open` there again. lf
marks the old `task_prs` row `abandoned_at` and adds a new `sequence` row with
the next number, while the child's `task_checkouts.parent_pr_id` still names
the old row: the real-world shape. `lf task status <child> --json` shows the
recorded row's `stack_parent.pr_number` until a sync records the followed one.

`sqlite3 "file:…/loopflow.db?mode=ro"` answers "unable to open database file"
in a fresh private home with no `-wal`; with no lf process running, open it
plainly. A merge lf read as open less than 60 s earlier stays open to it: the
next sync fetches the deleted branch and fails with `couldn't find remote ref`.
Wait out the cache before landing-then-syncing.

# `lf wt create --stack` checkouts

Same world. From a Task's checkout, `lf wt create <name> --stack` branches
from that checkout's branch and records `branch.<new>.loopflow-base` in the
repository's Git config (`--persistent` beside it adds `loopflow-persistent`).
`lf sync` there merges the local base branch, not `origin/<base>`, so it works
before the parent is pushed. `lf pr open` there pushes a base origin lacks
(`git ls-remote <origin.git>` shows it) and targets it; the stand-in's state
file shows `base`. To land the parent with its checkout still on disk, squash
it into `main`, delete the remote branch and mark the PR `MERGED`: `lf sync`
in the child then merges main, and `lf pr open` retargets its PR to main.
`lf task status` in the stacked checkout says "this checkout has no Task".

`lf sync main` (or `origin/main`) there unsets `loopflow-base` once the sync
completes: a conflict without an agent leaves it set until `lf sync --continue`,
and `lf sync --abort` keeps it. Resolve by hand with `lf sync main --manual`,
edit, then `--continue`; without `--manual` the sync launches an agent to
resolve and fails with no harness selected. `lf sync --plan` prints `base:`.

# A default branch other than `main`

`git init -q --bare -b trunk origin.git`, clone-shaped repo on `trunk`, push,
then `git remote set-head origin trunk`: `get_default_branch` reads
`refs/remotes/origin/HEAD`, and without it falls back to `init.defaultBranch`
or `main`. Use a second `GH_QA_STATE` file per world so PR numbers don't mix.
