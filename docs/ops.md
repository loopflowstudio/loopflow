# PRs, checkouts, machines and accounts

Your Task came back as a pull request. This page takes it the rest of the way:
landed, with failing CI repaired for you, then released. It also covers the
environment that work runs in: checkouts, other Machines, native provider logins
and service authentication.

Land, publish, CI repair and release are Loopflow's own verbs, so Skills and
Graphs that use them don't care which code host is behind them. GitHub is the
code host today; you need [`gh`](https://cli.github.com) signed in.

## Pull requests

### Land and repair CI automatically

```bash
lf land --wait-and-fix                 # sync, mark ready, enable auto-merge, repair CI until merged
lf --task LOO-12 land --wait-and-fix   # same, from any directory
lf task auto on LOO-12                 # auto on, then remaining Graph steps: reflect → gate → land --wait-and-fix
```

`lf land --wait-and-fix` polls the PR every 15 seconds and launches a `ci-fix`
agent whenever a required check fails or GitHub drops auto-merge. It stays
until the PR merges, closes, hits a blocker `ci-fix` cannot clear, or you press
`Ctrl+C` (which leaves the landing intent recorded so a later waited landing
picks it up). Bare `lf land` requests auto-merge and returns immediately with
no repair watcher.

If a Task's run dies, auto on or not, the Machine server starts it again as
`lf task run` within a minute, up to five times before it waits for you. Repeated failed repairs back off from 1 to 30 minutes; cap startup
retries per repair with `automation: {retries: 1}` in `.lf/config.yaml`, or
stop repair on one Task with `lf task unassign LOO-12`. Inspect past repairs
with `lf repo ci --since 7d`.

Auto-merge uses the repository's allowed merge method (squash, then merge, then
rebase); set `merge_method` in `.lf/config.yaml` to prefer one. Branch
protection and merge queues remain GitHub's job.

### Stop short of landing

Pick the commitment level you want (all work on ordinary branches without a
Task too):

```bash
lf pr open                          # draft PR, opened in your browser
lf pr publish                       # ready PR, prints state + URL; leaves upstream alone
lf pr submit                        # sync, clear added scratch/ files, mark ready for human merge
lf pr arm                           # submit, then enable auto-merge (no CI repair)
lf pr abandon                       # close the PR, remove the worktree, delete the branch
```

Teammates review on GitHub without installing `lf`:

```bash
lf request-review -r alice -r acme/platform   # request or re-request review
lf reviews                                    # each review and the exact commit it approved
lf checks --required                          # required checks only (exits 8 while pending; --logs for failures)
```

### Review while CI runs (`lf gate`)

```bash
lf gate                  # publish first so CI starts, then review the diff while CI runs
```

```text
Tests that matter most
- retry_budget::stops_at_three      the limit this change moves
- refresh::expired_token_retries    most-edited file; a 401 used to be swallowed
- ship_api::acceptance              the design's acceptance check

4c1e9a2  superseded. CI passed 14 checks; says nothing about the fix below.
b70d3f5  CI pending: rust-test, tests-result. Passing: rust-lint, python-test.
         Skipped: macos (paths not reached). Local: refresh:: passed.
         Fixed in review: refresh dropped the retry count after a 401.
Auto-merge not requested.
```

`gate` commits a coherent candidate, syncs, publishes a ready PR so CI starts
immediately, and verifies that the relevant suites actually queued (not
skipped). While CI runs, it reviews security, correctness, failure handling,
interfaces, and docs, fixing in-scope issues in one batch push. It names the
2–5 highest-value tests and reports CI status strictly per commit (never
carrying a green check from an older head). Customize it with
`lf skill customize gate`.

## Checkouts and stacked PRs

```bash
lf commit -m "retry budget" -p      # commit and push; trailing paths commit only those files
lf sync --plan                      # preview what sync would do without touching git
lf sync                             # merge main (or stack parent) into this branch
lf sync LOO-12                      # stack this Task on LOO-12 and merge its branch
lf wt create spike                  # create a sibling worktree
lf wt create spike --stack          # branch from this checkout's branch; sync and PRs follow it
lf wt prune --branches --dry-run    # preview cleanup of merged PRs and finished Tasks
```

Automatic sync checkpoints staged and unstaged edits before merging while
leaving generated `.lf/tmp/` telemetry local. Resolve merge conflicts normally,
then run `lf sync --continue` (or `lf sync --abort`).

In a checkout created with `--stack`, run `lf sync main` to follow the default
branch for future syncs and new PRs. The base changes when the sync completes.

### Stack dependent Tasks

Stack one Task on any other open Task that has a branch, before the parent
lands or even opens a PR:

```bash
lf task run LOO-13 --stack-on LOO-12   # new Task forked from LOO-12's branch
lf sync LOO-12                         # stack an existing Task on LOO-12 from its worktree
lf sync main                           # detach LOO-13 and deliver it independently on main
```

```text
$ lf sync LOO-12
Stacked LOO-13 on LOO-12; PR #1972 now targets jack/loo-12-retry-budget
Merging origin/jack/loo-12-retry-budget...
Synced onto origin/jack/loo-12-retry-budget.
```

- `lf sync` records the stack parent, retargets the child PR's base to the
  parent branch, and carries the child cleanly through the parent's updates and
  squash merge without replaying parent commits.
- Moving a stacked Task to a different parent (`lf sync LOO-14`) or detaching to
  `main` (`lf sync main`) removes inherited parent commits and keeps only the
  child's own work, preserving the Task, branch, and PR identity.
- A stack names a Task, not one of its PRs. When LOO-12's PR closes without
  merging and LOO-12 opens another, the next `lf sync` in LOO-13 follows it and
  records the new PR; landing and `lf pr publish` follow the same way, and a
  new PR that has already merged carries the child onto the default branch.
  When LOO-12 has no PR left to follow, sync stops and names both ways forward:

  ```text
  $ lf sync
  stack parent LOO-12's PR #1971 closed without merging; `lf sync LOO-12` restacks this Task on its branch, `lf sync main` detaches it
  ```
- Always record stacks with `lf task run --stack-on`, `lf task checkout --stack-on`,
  or `lf sync <ID>`, not manual `git merge` or `gh pr edit --base`. If a PR's
  base disagrees with the recorded stack, `lf pr publish` leaves the base
  untouched and prints the command to reconcile either side.
- If the parent has unpublished local commits, stacking bases on
  `origin/<branch>` and reports how many local commits were left out; run
  `lf commit -p` in the parent checkout to include them.

## Releases

```bash
lf repo release check                     # has anything merged since the last tag?
lf repo release run patch                 # bump, release notes, PR, build, tag, publish
lf repo release run minor --target cli    # scoped to one target's area and tag prefix
lf repo release status                    # where the current release stands
```

`lf repo release run` bumps version manifests, writes release notes, lands a
release PR, builds the exact merged commit, and tags it only after the build
and publisher succeed. If interrupted, rerunning resumes the same version
instead of bumping again.

Manage regressions and the `stable` release marker:

```bash
lf repo release regress v0.15.3 "Sessions lose their name"  # holds stable below v0.15.3
lf repo release issues                                      # open regressions
lf repo release promote --dry-run                           # preview moving stable
lf repo release promote                                     # move stable now
```

Declare per-target build and publish commands in `.lf/config.yaml` (see
[Releases](config.md#releases) for all fields):

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

## Machines

### Local server vs. execution Machines

`lf server status` inspects the local client server (used by Desktop and held
conversations); `lf install` updates it. Machine commands choose **where work
runs**: your laptop, an always-on SSH box, a pool of boxes, or an on-demand
rental. See [Connect clients](server.md) for server lifecycle and phone access.

### SSH machines, pools, and on-demand rentals

```bash
lf machine add mini                              # register an SSH host; offers to install lf
ssh mini                                         # sign in with Claude/Codex on mini
lf --machine mini task list                      # run any lf command on mini
lf task move LOO-12 --machine mini               # move a Task and its Session to mini
lf --wave product set machine mini               # default new Tasks in `product` to mini
lf machine hub mini                              # exchange this Machine's plan with mini, live
```

`--machine` runs the entire command on the target host in its saved repository.
`lf machine remove mini` forgets the connection without stopping work running
there. Machines that clone the same repository share its plan through Git; a
hub makes that take seconds. See
[Share the plan](work.md#share-the-plan-with-your-team).

Group owned machines into a pool and fall back to on-demand rentals when none
are free:

```bash
lf pool add lab && lf machine add jack@mini --pool lab
lf ondemand add code --provider daytona --vcpus 4 --memory-gb 8 --max 2 --max-usd-per-hour 0.40
lf ondemand add modal --provider modal --vcpus 8 --memory-gb 16 --max 2 --max-usd-per-hour 1
lf ondemand add gpu --provider runpod --vcpus 8 --memory-gb 32 --gpu A40 --max 1 --max-usd-per-hour 1
lf task run LOO-570 --pool lab --rent modal --quote   # preview placement, burst ceiling, and max cost
lf task run LOO-570 --pool lab --rent modal           # use a free lab machine, else rent one
lf --wave infrastructure set machine.capacity modal   # Wave Tasks rent from `modal`
lf machine end modal-1                                # copy records home and return early
```

```text
$ lf task run LOO-570 --rent modal --quote
would rent 8 vCPU 16 GB burst on modal at $0.95184/h ceiling, ≤ $3.80736 until 18:40; runs nothing

$ lf machine end modal-1
Returned modal-1 (2 sessions copied; ceiling estimate $0.24)
```

- **Supported rental providers:** Daytona (`DAYTONA_API_KEY`), Modal
  (`MODAL_TOKEN_ID`/`MODAL_TOKEN_SECRET` or `~/.modal.toml`), RunPod
  (`RUNPOD_API_KEY`), and AWS (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`,
  `AWS_REGION`). Pass keys via `doppler run -- lf …`.
- **Options:** `--gpu` requests a GPU (RunPod, AWS, Modal); `--spot` rents
  interruptible capacity (AWS, RunPod); `--reserved` (Modal only) pins the CPU
  and memory floor to `--vcpus` and `--memory-gb` instead of bursting.
- **Modal burst sizing and cost estimates:** By default, Modal sandboxes start
  at Modal's minimum floor (`0.125` physical core, `128 MiB`) and burst up to
  `--vcpus` and `--memory-gb`, so charges follow usage above that floor.
  Pass `--reserved` to pin floor equal to ceiling. Quotes, hourly limits,
  dollar caps, and timeouts still price the full ceiling; uptime-derived cost
  in `lf ondemand list` and `lf machine end` is labeled as a ceiling estimate.
  In rental JSON, `ceiling_estimate_usd` gives that estimate. Rentals created
  before Modal limits were recorded keep their original `cost_usd`.
- A rental claims the Task once its repository is ready. If the launch connection
  drops, repeat the placement command. It checks the recorded Machine and reports
  a Graph that already started. Plain `lf task run` advances that Graph.
- A rental runs the Linux build matching the caller. Your Machine fetches the
  release or dev package before forwarding logins. For a local build or an
  expired CI artifact, set `LF_RENT_LINUX_BIN` to the Linux executable.
- From the first SSH shell, `lf config show` reports the caller’s global agent
  preferences. Use `lf config show --skill <name>` to see a skill’s route.
  The rental keeps its own installation stream.
- **Credentials on rentals:** Owned SSH machines use their own native logins.
  For unattended rentals, forward explicitly named tokens or auth files via
  `forward.env` / `forward.files`, and push rotated files to a live Machine
  with `lf machine forward <label>`
  ([Remote Machines and rentals](subscriptions.md#remote-machines-and-rentals)).
  Linear receives a short-lived access token refreshed from your laptop (the
  refresh grant stays local).
- Your Machine collects rental Session history while its server runs. After a
  sandbox disappears, `lf session list` names the origin (`from modal-1`).
  Search the collected text with `lf session history search <text> --task <id>`.
  Read the last collected transcript with `lf session history show <id> --transcript`
  and its token and cost totals with `lf session usage --session <id>`.
- Task rentals return 15 minutes after their last Task finishes. Give-back
  makes a final copy. Unpushed work blocks return unless you pass `--force`,
  including when SSH cannot reach a sandbox that is still running.
- If the provider already ended the sandbox, `lf machine end modal-1` settles it
  without `--force` and brings its Tasks back to your Machine. Ended rentals
  leave `lf machine list` and `lf machine status`; `lf machine status modal-1`
  and `lf ondemand list` still show them.

## Updates and scheduling

```bash
lf schedule hourly             # change update cadence: 5min, hourly, daily (default), weekly
tail ~/.lf/logs/refresh.log    # inspect recent scheduled updates
```

`lf install` registers one update schedule (`com.loopflow.refresh` LaunchAgent
on macOS, crontab on Linux). Disable it on macOS with:

```bash
launchctl disable gui/$(id -u)/com.loopflow.refresh
launchctl bootout gui/$(id -u)/com.loopflow.refresh
```

Re-enable with `launchctl enable gui/$(id -u)/com.loopflow.refresh && lf schedule`.
On Linux, remove the `# loopflow install schedule` crontab line. When
`crontab` is missing, `lf install` finishes and reports that automatic
updates and cleanup are unscheduled. An explicit `lf install schedule`
reports the missing command. On a container without a systemd user manager,
the first connection starts a detached server and logs to `~/.lf/logs/server.log`.

For any other recurring job, such as settling landings with Desktop closed
(`lf pr reconcile`) or running a daily `wave-operate` or release, use your OS
scheduler. On Linux, crontab works directly; on macOS, use a LaunchAgent in
`~/Library/LaunchAgents` whenever the job needs Keychain credentials or runs an
agent.

## Authentication

```bash
codex login                                      # native login on the execution Machine
lf -a codex implement
lf account connect linear --chrome-profile Work  # service auth
```

Claude Code and Codex own their native logins and credential refresh. `lf account`
manages service authentication for GitHub, Linear, Asana, Notion, Doppler, and
OpenCode. See [Native authentication](subscriptions.md).

## Reference

| Command | What it does |
|---|---|
| `lf land [--wait-and-fix]` | Enable auto-merge; with `--wait-and-fix`, watch and repair CI until merged |
| `lf pr publish` / `open` / `submit` / `arm` / `abandon` | PR lifecycle at each commitment level |
| `lf pr reconcile` | Settle recorded landings once; recover a Task's missing PR |
| `lf task auto on\|off [ISSUE]` | Turn [auto](orchestration.md#auto) on and run the Task to merge / off: wait at the next interactive or merging step |
| `lf repo ci [--since 7d]` | Report CI failures and automated repairs |
| `lf repo release run\|check\|status\|regress\|promote` | End-to-end releases and stable promotion |
| `lf commit`, `lf sync [TASK\|BRANCH]`, `lf wt …` | Git operations with Task and stack awareness |
| `lf machine …`, `lf pool …`, `lf ondemand …` | SSH machines, pools, and cloud rentals |
| `lf server status`, `lf install` | Local client server and updates |
| `lf account` | Service authentication (GitHub, Linear, Asana, OpenCode) |

Full flags: [`lf pr`](lf-reference.md#lf-pr), [`lf repo`](lf-reference.md#lf-repo),
[`lf wt`](lf-reference.md#lf-wt), [`lf machine`](lf-reference.md#lf-machine),
[`lf account`](lf-reference.md#lf-account).

Next: [Surfaces](surfaces.md): Loopflow Desktop, Rex, cmux, Herdr, tmux, and
building your own UI.
