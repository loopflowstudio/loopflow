# Scheduled disk cleanup (machine opt-in)

```bash
scripts/disk-cleanup.sh --dry-run    # inspect candidates; no deletion or process signals
scripts/disk-cleanup.sh install     # explicitly enable hourly cleanup at minute 17
scripts/disk-cleanup.sh             # run the enabled cleaner now
scripts/disk-cleanup.sh uninstall   # disable cleanup and remove this schedule
```

This repository utility is not shipped in Python wheels, source distributions or
DMGs, and product installation never enables it. `install` copies this same runner
and its dependencies into `~/.lf-machine/maintenance/disk-cleanup/versions/`, so a
Task checkout can disappear without breaking scheduling. Reinstall explicitly to
update that copy. It preserves a running job, restores the prior job/configuration
on bootstrap failure, and removes only the predecessor's marked cron entry.
Launchd uses a clean environment, macOS's system Python and the Standard resource
policy. Background throttling can starve process inspection under build load.
The schedule and private configuration belong to this machine.

Ignored, untracked `target`, `.build`, `DerivedData`, `node_modules` and `.venv`
roots anywhere under `~/src` (`LOOPFLOW_SRC` overrides it) are candidates. Nested
worktrees use their own Git rules. Git lists registered worktrees inside ignored
containers. Git enumeration and traversal skip generated contents, ignored data
trees and `.lf` state, without reading tracked file contents or running content
filters. Deletion still requires the checkout's actual ignore rules.
Global Xcode Build/Index and shared generated caches are included; SourcePackages
and embedded Git checkouts survive. Cargo profiles can be reclaimed separately.
Discovery excludes symlinks and stops at
two minutes or 100,000 entries, reporting incomplete coverage.

Idle builds need two hours without writes, one hour below 64 GiB free, or
fifteen minutes below 32 GiB. Largest candidates go first; recovering space
restores normal retention. Held generated output needs **six hours** before owner teardown.
In-use `.venv` and `node_modules` roots always stay; their users are not stopped.
The cleaner identifies actual owners through cwd, executable mappings and open
files, sends TERM to verified owners and their descendants, waits five seconds,
then KILLs verified survivors. It rechecks ownership before removal. PID/start
identity changes, unknown owners, surviving owners and interruption preserve the
output and report a failure. An unrelated builder in the same checkout (a
compiler, package manager or interpreter whose cwd is there) retains output
without being terminated. Agents and their helpers don't count as builders:
a process descending from `lf session serve`, `codex` or `claude` holds output
only by having files open under it, so a resident conversation or a finished
Task's idle agent doesn't keep a checkout's builds. Chrome processes and the cleaner's ancestors
are never terminated. Owners using `project_owned` state are also preserved.
Direct new builds can still race inspection; run them after
cleanup when reusing an otherwise idle output root.

Disposable test evidence retains **six hours after success**, **two days after
failure or an unknown outcome**. This covers pytest runs/garbage, gate artifacts
under `.lf/tmp/gate/run-*`, and `lf-worlds-`, `ghostty-shaders-`, `swift-test-`
fixture directories. Pytest and the gate write `.cleanup-evidence.json` with the
observed `outcome` (`passed` or `failed`). `.cleanup-keep` anywhere in an evidence
tree, or `"saved": true` in its root outcome record, preserves it.

Other demos are observed unless explicitly marked under
`~/.lf-machine/demos/<name>/` with `.disposable-coding-demo` containing
`disposable runtime; evidence stored elsewhere`. That marker authorizes teardown
and removal of the entire runtime after the evidence retention window; an unsettled
draft held by its owner ends with that owner. A demo keeps two roots: this runtime
root (copied app or CLI, generated output) and a durable root at
`~/.lf-machine/demo-history/<name>` (private Machine, provider history, fixture
repository), which is never a candidate. The demo writes the marker at release,
never at creation: writing it restarts the retention window, so a long live review
cannot age out. Demos write no `.cleanup-evidence.json`, so a released root has an
unknown outcome and waits two days.
Ordinary Home/account history, databases, backups, experiment data, source
worktrees, simulator devices, general app caches and Trash are never candidates.

Native cache maintenance runs even with plentiful free space. Private
`config.json` accepts `cache_caps` in bytes: `uv_bytes` defaults to **8 GiB per
cache**, `sccache_bytes` to **10 GiB**, and `docker_build_bytes` to **8 GiB**.
UV prunes unused global and repository `.uv-cache` entries; over the cap it drops
unused downloads with `prune --ci`, then uses native `cache clean` if still over.
Native reader locks preserve active installers, without force. The cleaner
checks the running sccache server's native eviction limit and reports an unknown
or oversized limit; configure sccache itself before starting its server.

Docker maintenance requires a local Unix engine. Its engine-owned BuildKit cache
keeps recently used (24h) and in-use records, pruning older unused records toward
the cap. Only images older than 24h with registry digests and no container
references are removed, without force. Local unpublished images, all containers,
volumes and remote/custom builders stay intact. Images without a creation timestamp
stay with an unknown-age reason; other eligible images can still be reclaimed.
Age, activity and explicit reservations can keep caches above their caps; native
busy locks are retained for the next run. Other native failures remain nonzero
in the receipt.

`install --chrome-copies` separately opts this machine into removing unused Chrome
code-signing copies older than 24 hours from Darwin's per-user temporary tree.
It always keeps the newest and in-use copies, verifies their bundle-only shape,
and never stops Chrome. This option does not authorize other non-coding deletion.

Read `~/Library/Logs/loopflow-disk-cleanup.json` for candidate decisions, failures,
stopped PIDs, removed allocation and measured free-space delta. Check `dry_run`,
timestamps and exit status before attributing a receipt to an invocation. A
missing or older receipt is not proof of success. Positive `target_shortfall_bytes`
means space remains below target even when cleanup exits zero.

Private `~/.lf-machine/maintenance/disk-cleanup/usage.json` retains at most 168
observations and 4 MiB. Each run measures source directories, tooling, temporary
roots and observation-only app data, caches, Downloads and Trash. Sampling is
limited to 256 roots and two minutes. Unreadable or timed-out measurements have
null bytes and a visible error; they never masquerade as zero. Least recently inspected roots go first so later repositories do not starve.
Deltas compare the last known measurement of the same root, with its timestamp;
missing/unknown measurements have no delta. Add private `usage_roots` entries to `config.json` with `path`, `retention`
(`keep_forever`, `emergency_reducible`, `mixed`, `unclassified`, `project_owned`) and `content_type`
(`code/development`, `media`, `entertainment`, `desktop apps`, `system/other`).
These axes describe accounting independently. They never authorize deletion;
`keep_forever` and `project_owned` also veto deletion of that path or a containing
candidate. Project-owned/deferred means another project owns its retention; it
does not mean permanent retention or emergency deletion approval. Unclassified is the default,
not inferred permission. Physical reconciliation gaps remain null, never an
estimate of reclaimable space. Roots can overlap, and APFS clones and concurrent writes mean allocation,
growth and free-space change must not be equated or summed as exclusive usage.

The shared nonblocking recovery lock releases on exit or death. Candidate
inspection takes at most thirty seconds; commands, including process and open-file
inspection, take at most two minutes, capped by the remaining run budget.
Each run has thirty minutes (`--budget-seconds` overrides it). Failed cleanup steps exit nonzero;
partial removals get no allocation credit. Activity inspection failure stops
deletion. This cleaner's own text/removal logs rotate at 10 MiB; durable product
history is untouched. All inventories and receipts stay local, never in the PR.

