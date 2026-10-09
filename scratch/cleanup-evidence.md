# Cleanup investigation — 2026-10-09

Read-only investigation requested by Jack Heart. Source snapshot: `3e1e6245c`;
installed laptop CLI: `lf 0.13.10`. Running work can change counts during scanning.
No worktrees, branches, caches, customer processes or histories were deleted or
stopped. Only the investigation's stalled read-only roadmap command was canceled.

## Laptop observations

- `df -h /System/Volumes/Data`: 926 GiB capacity, 815 GiB used, 60 GiB available.
  This is whole-machine storage, not an attribution to Loopflow.
- Git registration reads initially found 72 Loopflow, five Etude and 43 Fantasia
  checkouts. A later `lf wt list --json` found 73 Loopflow rows. Several Etude-named
  directories belong to Fantasia's Git common directory; names are not ownership.
- Loopflow listing: 28 dirty, 45 clean; 21 merged, five squash-merged, 39
  remote-gone, three open-PR and two closed-PR rows (flags overlap).
  Nineteen clean merged and ten additional clean remote-gone rows match the
  current terminal-prune classifier **before** Task, process and persistent
  protections. This is not a safe-to-delete count.
- Three prefixed directories are absent from the three repositories' Git
  registrations: `loopflow.context-lab` contains a 35,459,104-byte release binary,
  `loopflow.agent` contains a residual `.venv`, and `loopflow.remote` contains a
  `deploy/Caddyfile` directory. Their creation/removal history is unproven.
- Top-level source scan finds 18 `target` and 89 `.venv` directories. Registrations
  alone miss residue; arbitrary prefix scans cannot authorize deletion.
- `du -sk` observations: main Loopflow `target` 4,954,924 KiB; `swift/.build`
  1,038,888 KiB; `.lf` 1,777,400 KiB; machine `~/.lf/runs` 5,520,432 KiB;
  `~/.lf/logs` 955,276 KiB; shared uv cache 15,373,284 KiB; Cargo 1,155,980 KiB.
  Sizes are allocated-directory estimates; sharing and hardlinks limit summation.
- Active `loopflow.db` file length is 4,609,712,128 bytes. Two release migration
  backup lengths are 3,956,523,008 and 2,860,216,320 bytes. These are durable data
  and recovery generations, not automatically junk. Current migration code keeps
  two recognizable generations; unrelated historical backups remain untouched.

Largest measured checkout footprints (`du -sk`, GiB rounded):

| Checkout | GiB | Observed Git state |
|---|---:|---|
| `loopflow.focus-on-your-own-work` | 31.9 | Clean, merged |
| `etude.measure-whether-mirror-match-training` | 19.2 | Disposition not inspected |
| `loopflow.make-lf-wt-list-fast` | 14.7 | Clean, remote gone; PR unknown |
| `loopflow.update-the-workspace-automatically-when` | 13.7 | Clean, merged |
| `loopflow.keep-every-wave-ready-for` | 13.5 | Clean, merged |
| `loopflow.explore-loopflow-s-own-store` | 13.3 | Clean, merged |
| `loopflow.make-a-task-up-to` | 13.0 | Dirty |
| `loopflow.compare-cmux-s-command-line` | 10.7 | Dirty |
| `loopflow.run-a-task-on-a` | 10.6 | Dirty |
| `loopflow.data-model-one-table-per` | 9.0 | Clean, merged |

The five clean merged rows above total approximately **81.4 GiB**, before Task,
execution, ignored-content and persistent protections. This is a promising
inspection set, not an approved reclaim total. A process-table sample also found
a test executable in `keep-every-wave-ready-for` running for over three days;
its ownership and health were not established, and it was not stopped.

Build artifacts dominate the largest Loopflow examples: `focus-on-your-own-work`
has 29.0 GiB in `target`; `make-lf-wt-list-fast` 14.4 GiB; `update-the-workspace-automatically-when`
10.8 GiB; `explore-loopflow-s-own-store` 11.1 GiB. In contrast, the largest Etude
example has **17.9 GiB in `.runs`**, experiment output rather than compiler cache.
Jack Heart subsequently classified this as disposable too. The initial blanket
exclusion was too conservative; idle experiment output belongs in the artifact
retention policy, with active-use protection and explicit retention proposed.

`~/.lf` totals **62.0 GiB**. Largest directory categories: accounts 18.2 GiB,
traces 17.6 GiB, backups 6.8 GiB, runs 5.3 GiB, logs 0.9 GiB. The total also includes
root database files. Account contents were not read. Trace/history and ad hoc
recovery backups need ownership/retention investigation, not wholesale deletion.
Other measured caches: Xcode DerivedData 2.8 GiB and SwiftPM cache 3.7 GiB.

After Jack Heart asked about rotation, a metadata-only account-home breakdown
found 9.56 GiB under `sessions`, 0.45 GiB under `projects`, 0.45 GiB in temporary
directories and 0.39 GiB in caches. Across those homes, `.jsonl` files total
10.0 GiB and `.sqlite` files 4.76 GiB (plus 0.35 GiB WAL). Largest individual
database lengths include `logs_2.sqlite` at 1.44 GiB and `thread_history_1.sqlite`
at 1.35 GiB. Credentials were not read. These provider-owned stores cannot safely
be rotated by blindly deleting the largest file. Current local code in
`ops/human_session/provider_conversation.rs` discovers native transcripts and
reads provider histories; archive work must preserve those readers and resume.

A read-only compression sample of one `provider.jsonl` (no transcript
text emitted, no files changed) reduced 45,163,625 bytes to 16,957,119 bytes with
zlib level 3: **62.5% smaller**. This proves useful lossless compression for that
sample, not a guaranteed reduction for the entire 17.6 GiB traces directory.

2026-10-09: `lf roadmap --json` produced neither output nor an error after more
than five minutes and was interrupted. Task ownership/disposition is therefore
not established for the size-ranked candidates. No SQLite repair or mutation
was attempted. `lf wave list --json` succeeded and supplied design placement.

## Product behavior and gaps

1. **Event-driven cleanup, incomplete retry coverage.**
   `ops/task/lifecycle.rs::cleanup_completed_task`, Task movement and
   `lf/commands/flow.rs::run` provide completion attempts. A successful Task Flow
   retries cleanup after its steps; this corrects the initial hypothesis that
   every live-Flow deferral needed a manual retry. Taskless PR cleanup retains
   active execution and tells the caller to use `lf wt delete` later.
   `store/sqlite/pr_landings.rs::pending_pr_landings` excludes `merged` and
   `closed`, so repository delivery reconciliation cannot revisit a merged row
   solely because cleanup was deferred. An existing every-minute repository tick
   runs `lf task reconcile --json`, currently reaping orphan engines and observing
   delivery, not collecting all terminal checkouts. It is the proposed integration
   point instead of another timer/service.
2. **Unused automatic pruning.** `WorktreePrunePolicy::automatic`,
   `prune_branch_worktree`, `prune_terminal_worktree` and
   `prune_abandoned_prompt_logs` have no production callers in the Rust search.
3. **Manual pruning is not safe to schedule unchanged.** Seven-day branch
   inactivity, a closed PR or disappearance of the upstream branch is sufficient
   for its classifier, subject to dirty/ownership/persistence protections.
   `running_workspace_paths` returns an empty set on `lsof` failure.
   `prune_worktrees` prunes Git metadata even when `dry_run` is true; no prune
   preview was executed during this read-only investigation.
4. **Clean does not mean disposable.** `is_clean` uses ordinary porcelain status,
   which excludes ignored files. Prune eventually invokes forced Git removal.
   Clean merged examples contain ignored `.lf` state, environments and artifacts;
   the same mechanism could erase ignored customer data.
5. **Development-only cache recovery.** `resource_envelope.py`, `TESTING.md` and
   `performance/budgets.json` describe recovery before this repo's verification:
   64 GiB target free, 32 GiB emergency reserve, cold builds after 24 hours,
   24 GiB per-root signal, eight roots/64 GiB per pass. Recognized roots are
   `target`, `.build`, `swift/.build`, `website/node_modules`; `.venv` is omitted.
   Large roots over the per-pass byte ceiling are skipped. Activity/recency can
   retain caches indefinitely. These developer choices are not customer defaults.
6. **Other ownership already exists.** Migration backup pruning in
   `store/migrations.rs` retains two generations. Durable Session payloads are
   explicitly protected by the resource script. Neither should be indiscriminately
   absorbed into a generic “delete old files” operation.

## Mini access evidence

2026-10-09: `lf machine list --json` returned `[]`; `ssh -o BatchMode=yes
-o ConnectTimeout=10 mini ...` failed “Could not resolve hostname mini.”
`mini.local` also failed resolution. Interactive zsh reported no `mini` alias.
The installed `tailscale` wrapper points at a missing application executable;
no VPN/auth repair attempted. Jack Heart was asked for the underlying address.

Jack Heart supplied `export mini=100.96.227.95`; direct SSH to that address
succeeded. Alias resolution is no longer a blocker. Host: `mini-heart.local`;
installed CLI: `lf 0.13.10`. No remote configuration was changed.

## Mini observations

- `df -h /System/Volumes/Data`: 926 GiB capacity, 795 GiB used, 86 GiB available.
- Eight registered Loopflow checkouts, including main; three Etude checkouts.
  Seven Loopflow sibling tips date from July 1–6. All eight Loopflow checkouts
  have zero ordinary porcelain-status entries. This does not classify ignored
  artifacts or prove terminal Task state.
- Largest Loopflow siblings: desktop 7,636,772 KiB (7.3 GiB), workflows
  6,514,480 KiB (6.2 GiB), mobile 5,648,504 KiB (5.4 GiB). Main is
  5,433,400 KiB (5.2 GiB). Other July sibling trees are small, about 23–27 MiB.
- Etude main is 6,819,048 KiB (6.5 GiB) and has two dirty entries. The two
  October 8 controller checkouts are about 0.8 and 1.0 GiB; one has three dirty
  entries. Current experiment directories are not assumed inactive.
- `~/.lf` is only 130,444 KiB (127 MiB), mostly executables. Shared uv cache is
  9,845,436 KiB (9.4 GiB); Cargo cache is 110,860 KiB (108 MiB). Unlike the laptop,
  the mini's disk use is not concentrated in Loopflow's home.
- LaunchAgent filenames include legacy `com.loopflow.lfd` and `lfd.update` jobs;
  no current repository-tick filename was found. Filenames alone do not prove
  which jobs are loaded. Existing-install schedule migration matters.

The three largest July Loopflow siblings total **18.9 GiB**. Their `target`
directories account for **18.8 GiB**, over 99% of that footprint. `lf wt list
--json` reports all three clean and remote-gone, but no merged PR. Their heads
are not ancestors of cached local main. No automatic branch-discard conclusion
follows; independent artifact eviction avoids needing that conclusion.
The release-default worktree has merged PR evidence but is only about 25 MiB.

Six registered Cadenza checkouts bring the three inspected repository families
to **17 checkouts**. The initial filesystem scan found 19 `.git` entries total;
that is not identical to registration count. One Cadenza sibling has dirty files.

Broader attribution: `~/src` is **41.2 GiB**, `~/Library` **491.1 GiB** and
`~/Parallels` **24.3 GiB**. Whole-disk use must not be blamed on worktrees.
Library categories include Application Support 206.1 GiB, CloudStorage 84.9 GiB,
Containers 63.0 GiB, Messages 55.3 GiB, Developer 49.2 GiB and Caches 19.4 GiB.
These broad application categories were measured, not declared disposable or
attributed to Loopflow. No unrelated application storage was changed. Neither
workstation has been cleaned during this investigation.

## Alternatives considered

- Schedule today's `lf wt prune`: small implementation, unsafe abandonment
  semantics and no machine-wide cache policy. Reject unchanged.
- Put cleanup in agents/repository operation: useful for judgment about ambiguous
  work, unsuitable as the customer's automatic storage lifecycle.
- Share every build directory: might reduce duplication but changes build
  concurrency and toolchain behavior, and does not remove abandoned checkouts.
- One deterministic collector, triggered by lifecycle plus periodic reconciliation:
  best coverage without another execution authority. Proposed keystone.

## Design review findings

The first draft proposed another hourly installed job. The existing repository
tick already supplies scheduling and bounded receipts; the draft now extends
that path instead. Directory size is not proof of junk: Jack Heart's direction
makes Etude experiment runs eligible under the disposable-artifact contract;
account/trace history still needs a separate retention decision. Branch absence
and ignored files are insufficient deletion authority even when the disk is full.

Check: filesystem measurements and source inspection only; product tests deferred
to implementation/gate. No cleanup applied.
