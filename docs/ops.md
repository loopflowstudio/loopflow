# Ops

Your Task came back as a pull request. This page takes it the rest of the way:
landed, with failing CI repaired for you, then released. It also covers the
environment that work runs in: branches and worktrees, other machines, and the
provider accounts that pay for it.

Land, publish, CI repair and release are Loopflow's own verbs, so Skills and
Graphs that use them don't care which code host is behind them. GitHub is the
code host today; you need [`gh`](https://cli.github.com) signed in.

## Land it

```bash
lf land --wait-and-fix   # sync, mark ready, auto-merge; repair failing CI until it merges
```

Run it in the Task's checkout, or from anywhere with `lf --task LOO-12 land --wait-and-fix`.
It checks the PR every 15 seconds and starts a `ci-fix` agent when a required
check fails. Elapsed time never ends the wait; a verified merge, a closed PR, a
blocker `ci-fix` cannot clear, or Ctrl-C does. Interrupting leaves the landing intent in
place, so a later watcher picks it up.

Bare `lf land` requests auto-merge, records that you want it landed, and returns.
Repair then comes from the CI watcher (below) or the next waited landing.

To go from review-ready to merged in one step, run the built-in `ship` Graph:

```bash
lf -b task run LOO-12 ship   # gate (ready to ship and review), then land --wait-and-fix
```

A merge doesn't complete the Task on its own; it stays open for follow-up work in
the same checkout. Close it with `lf task complete LOO-12` once the PR has merged.

Running `lf land` again after new commits prepares the Task's next PR; the earlier
one stays in its history. Auto-merge uses the repository's allowed merge method
(squash, then merge, then rebase); set `merge_method` in `.lf/config.yaml` to
prefer one. Branch protection and merge queues stay the code host's job.

## Stop short of landing

Not every PR should merge itself. Pick the endpoint:

```bash
lf pr open                          # draft PR, opened in the browser
lf pr publish                       # ready PR, printed as state + URL; opens nothing
lf submit                           # sync, clear added scratch/ files, mark ready; a person merges
lf arm                              # submit, then request auto-merge; no repair
lf pr abandon                       # close the PR, remove the worktree, delete the branch
```

`publish` makes work reviewable and leaves upstream alone. `submit`, `arm` and
`land` own preparation: they sync with main (or the stack parent) and run
`lf scratch clear`, which removes scratch files the branch added and keeps
existing ones. All of these work on an ordinary branch with no Task.

Teammates review on GitHub without installing lf:

```bash
lf request-review -r alice -r acme/platform   # request or re-request on this PR
lf reviews                                    # each review and the commit it approved
lf checks --required                          # only the checks protection requires
```

An approval of an older commit is not an approval of the current head; `reviews`
shows which commit each one covered. `checks` exits 8 while checks are pending,
which is not a failure. `--logs` pulls failed GitHub Actions logs.

## When CI fails

```bash
lf ci watch               # watch this repository's PRs; repair landings that fail
lf ci watch --install     # keep it running as a launchd service
lf ci watch --status      # live or not, last poll, what it started and why
lf ci --since 7d          # how each failure was detected, repaired and landed
```

The watcher polls about once a minute and starts one `ci-fix` when a PR you
landed or armed fails its required checks. A failing PR nobody asked to land is
reported, not repaired. Loopflow Desktop runs the same watcher for each open
repository. Nothing depends on it: `land --wait-and-fix` admits the same repair
on its own.

`lf ci` reads the local record, not GitHub: how much of CI repair happened without you.

Hold repair for one Task, or set how many times a repair may retry:

```bash
lf task automate LOO-12 off   # hold future repair; running work continues
lf task automate LOO-12 on
```

```yaml
# .lf/config.yaml
automation: {retries: 1}
```

Pending or missing checks keep waiting; time alone never starts a repair. For
progress through a closed laptop, land from an always-on Machine (below).

## Schedule it yourself

Loopflow keeps no scheduler. Any lf command runs from your own crontab, launchd
or systemd timer:

```bash
# crontab -e
* * * * *  cd ~/src/product && lf task reconcile --json    # settle landings with Desktop closed
0 9 * * 1  cd ~/src/product && lf release run patch        # Monday's release
```

`lf task reconcile` checks every recorded landing once: it records CI failures and
settles verified merges, then exits.

## Release

```bash
lf release check          # has anything merged since the last tag?
lf release run            # patch release, end to end
lf release run minor
lf release status         # where the release workflow stands
```

`release run` bumps manifests, writes release notes, lands a release PR, builds
the exact merged commit, and tags it only after that build and your publisher
succeed. Interrupted, it resumes the same release from where it stopped; a
failed build is fixed with another commit under the same version, never by
bumping. `lf skill release-run minor` hands the same command to an agent.

Your repository owns the build and publish steps. Declare them per target:

```yaml
# .lf/config.yaml
release:
  targets:
    cli:
      area: [packages/cli/]
      tag_prefix: cli/
      verify: [scripts/check-release cli]
      prepare: [scripts/prepare-release cli {version}]
      workflow: .github/workflows/release-cli.yml
      completion: github-release
```

```bash
lf release run patch --target cli   # changes since cli/v<previous> only
```

[Configuration](config.md#releases) has every field. The first release of a
target needs an explicit `X.Y.Z`.

## Branches and worktrees

```bash
lf commit -m "retry budget" -p      # commit and push; paths after it commit only those
lf sync --plan                      # what sync would do, without touching git
lf sync                             # merge main, or the stack parent, into this branch
lf wt create spike                  # a worktree beside the repository, in its layout
lf wt prune --branches --dry-run    # branches of merged PRs and finished Tasks
```

Sync follows a stack parent through its squash merge. Stack one Task on
another's open PR, even a draft:

```bash
lf task run LOO-13 --stack-on LOO-12
```

Conflicts stay visible: resolve them, then `lf sync --continue`, or `lf sync --abort`.

## Other machines

```bash
lf machine add mini                              # save an SSH destination; offers to install lf
lf machine connect mini claude jack@example.com  # a login of its own, approved in this laptop's browser
lf --machine mini task list                      # any command, run there
lf machine use mini --task LOO-12                # hand a Task to it; you stay its person
lf machine use mini product                      # new Tasks in Wave `product` run there
```

`--machine` runs the whole command on the target, in its saved repository.
Removing a machine (`lf machine remove mini`) forgets the connection and leaves
its work running.

Group machines you own into a pool, and rent more when none is free:

```bash
lf pool add lab && lf machine add jack@mini --pool lab
lf account token claude jack@example.com          # once a year: Claude for rented machines
lf ondemand add code --provider runpod --vcpus 8 --memory-gb 16 --max 1 --max-usd-per-hour 0.40
lf task run LOO-570 --pool lab --rent code --quote   # where it would run, the most it could cost
lf task run LOO-570 --pool lab --rent code           # a free lab machine, else rent one
lf machine end code-1                                # give a rented machine back now
```

Renting reads `RUNPOD_API_KEY` from the environment (`doppler run -- lf …`). A
rented machine deletes itself at its deadline or when its budget runs out. Before
any give-back, its Sessions and Processes are copied home, so `lf session list`
shows them "from code-1". Unpushed work refuses the give-back unless you pass
`--force`.

## Accounts

```bash
lf account                                         # access and capacity for every login
lf account connect codex work@example.com --chrome-profile Work
lf account route set codex work@ personal@         # this repository: work first
lf account codex use work@                         # sign ~/.codex in as work@, for lf and plain codex
```

Connect registers a login you already have. It runs the provider's sign-in in
the Chrome profile you name and refuses a login that doesn't match the email.
Routes try accounts in order; one at 95% of a usage window drops behind the
rest. `lf account --cached` reads saved evidence without contacting providers.
Unknown capacity stays unknown, never zero.

For one launch, `--account codex=work@` prefers an account and keeps fallback;
`--only-account` restricts the launch and its children. See
[Sessions](sessions.md) for how a launch picks its account.

## Reference

| Command | What it does |
|---|---|
| `lf land [--wait-and-fix]` | Request auto-merge; with the flag, wait and repair CI until merged |
| `lf arm` | Prepare and request auto-merge; return without watching |
| `lf submit` | Prepare for a person's merge click |
| `lf pr publish` / `lf pr open` | Ready PR, printed / draft PR, opened |
| `lf pr abandon [BRANCH]` | Close the PR, remove the worktree, delete the branch |
| `lf pr reconcile` | Check recorded landings once; recover a Task's missing PR |
| `lf ci watch [--once\|--install\|--status]` | Repair failing landings |
| `lf ci [--since 7d]` | Report CI incidents and their repairs |
| `lf release run\|check\|status\|notes\|bump\|tag\|publish` | Release stages, together or one at a time |
| `lf commit`, `lf sync`, `lf wt create\|switch\|list\|prune\|delete` | Git, with Task and stack awareness |
| `lf machine add\|list\|status\|use\|connect\|end` | Where work runs |
| `lf pool …`, `lf ondemand …` | Machines you own, machines you rent |
| `lf account [connect\|route\|use\|token\|set]` | Provider logins, capacity and routing |

Every flag is in the [`lf` reference](lf-reference.md#lf-ops).

Next: [Surfaces](surfaces.md): see this work in Desktop, cmux, Herdr or tmux.
