# v0.12.25

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.25 keeps Task review close to the work and removes repeated preparation from delivery. The Mac app adds file editing beside Task conversations, while consolidated workflows reconcile plans, code, and memory before validation. Landing preserves unchanged commits, published descriptions, and merge-queue progress; verification reuses matching builds and evidence with clearer delivery measurements.

## Review and edit without leaving the Task

The Mac app now places scratch notes, changed files, and an editable file view beside the Task’s retained terminals. Files and terminals follow the selected Task’s checkout.

- **Show Files** provides recursive scratch navigation, changed files, and a PR link. Compare against the recorded PR base (**Parent**) or **HEAD**; **Diff** includes unsaved edits without writing them to disk.
- Drafts, selection, and Undo survive switching files or Tasks within a window. Autosave runs after two quiet seconds; disable **Autosave Task Files** to save explicitly with ⌘S.
- External changes arrive live. Disjoint local edits survive; disk changes win overlaps. Revision-checked saves retain submitted drafts, displaced files, and recovery receipts.
- `lf task changes`, `diff`, and `file` gain comparison and file support; `lf task save` adds saving through recorded local placement without planning sync or starting a Run.

## Reconcile work before delivery

`realign` becomes the builtin owner for updating plans, code, and Wave memory. Consolidated repair and operating skills reduce overlapping entry points and make delivery stopping points explicit.

- `realign` replaces `review-slice`, `refresh-plan`, `update-wave`, and `record-learnings`, preserving accepted requirements, unresolved evidence, and remaining work in existing artifacts.
- `refresh` runs rebase → realign. Queue preparation runs compress → refresh → gate, so reconciliation happens before validation. Pursue passes refresh before convergence and publication; build and slice also make publication explicit.
- `lf pr open` creates a draft and preserves existing readiness. `lf pr publish` promotes drafts; a failed promotion retains draft copy for retry.
- Use `unbreak` to restore broken workflows and `debug` to investigate and fix code failures. `incident` composes unbreak → 5whys → launch-plan.
- `lf vsm-operate` adds a manual repository pass through delivery, coordination, capacity, adaptation, and identity using independently callable S1–S5 skills. `wave/operate` applies the same questions within one Wave while preserving independent Task progress.

## Keep useful CI work through landing

Final preparation and landing now preserve work that still applies to the current head. Merge-queue waiting, GitHub observations, and CI reuse are coordinated without treating stale or missing evidence as a pass.

- Resuming a clean, armed or queued standalone PR retains its head and CI. Final preparation also preserves a single authored commit when its source and base are unchanged; necessary rebases and multi-commit normalization remain.
- When local and remote heads match, final preparation retains GitHub’s title and body. Explicit copy and valid gate output take precedence; changed local heads generate fresh copy.
- PRs waiting to enter the merge queue can remain behind main without an unnecessary rebase. Real conflicts and failed checks still receive recovery before entry; queued work waits for GitHub’s integrated result.
- Landing and release read merge state and queue membership together, avoiding a false missing-request failure when a PR merges between reads. Required gates and repair details come from one check set for the observed head, with pagination and moving-head handling.
- Main can reuse a successful merge-group CI run for the exact pushed SHA. Missing, failed, mismatched, or unreadable proof runs the full matrix. PRs and merge groups retain full checks; main still fills missing shared build caches.
- Rust, SwiftPM, and Xcode caches account for their relevant profiles, source inputs, and toolchains. CI reuses already-built CLI and app binaries; native captures run two at a time while retaining all eight images and comparisons.

## Measure delivery from current Run evidence

Scheduled telemetry now reads the current Run ledger instead of the removed `agent_turns` table. Reports distinguish measured intervals from missing evidence and avoid assigning performance passes to unbudgeted samples.

- The scorecard reports ended-Run duration, direct usage, and recorded Task PR creation, publication, and landing intervals.
- **Recorded agent attempt → merge** joins provider attempts to exact managed Task PRs and reports measured/eligible coverage, including earlier and unfinished Runs. It is an observed lower bound: missing history or standalone work can hide an earlier start. Unstarted prepared Sessions remain unknown; Task PR creation → merge remains the durable baseline.
- Date- and ownership-scoped Run queries filter manifests before parsing unrelated event histories, retaining usage, ordering, and evidence-gap behavior.
- Expensive CI commands log CPU time, memory accounting, faults, and context switches alongside elapsed time. These peaks do not measure simultaneous host memory.

## Operational notes

- **Custom workflows and installed skills:** retired names have no compatibility aliases. Update custom Flow references and sync installed skill exports separately. Captured Flows retain pinned instructions. Manual S1–S5 operation adds no scheduling or channel delivery; its full evidence handoff and the incident handoff remain unproven end to end in the supplied record.
- **Task file limits:** editing supports complete UTF-8 files up to 1 MB. Unsaved drafts do not survive window closure. Recovery storage has no automatic cleanup and disappears with the worktree; it does not guarantee exclusion of concurrent writers or power-loss durability. Extremely long lines can remain slow.
- **Saved Sessions:** returned `open_argv` carries the original executable, Home, and database across installation changes. Retained launches require valid receipts and unchanged artifacts; listing still uses the selected installation. `lf session open <id> --json` prepares the handoff; execute its returned argv to open the conversation. Returned launch failures remain journaled after retry. The complete installation-switch journey and real Ghostty open/resume remain unverified.
- **Disk headroom:** fresh verification reclaims eligible inactive build caches unchanged for 24 hours toward 64 GiB free and stops below 32 GiB. Active and recent builds remain protected. Busy uv caches fail pruning immediately so other cleanup can proceed; recent gate output no longer consumes cleanup capacity without an eligible removal.
- **Performance evidence:** build caches need an initial cold fill, and hosted warm reuse remains unverified for the Swift/Xcode changes. Local native capture comparisons fell from about 28 seconds to 14 seconds with identical images. These measurements and the recorded reader/build improvements do not establish an overall time-to-merge reduction. Added Swift failure logs improve diagnosis; the reported hosted hang is not claimed fixed.

## Small changes

- Store validation embeds the canonical schema reference at build time, reuses constructed references for other migration paths, and reduces checksum bookkeeping while still checking each actual database. Shipped migration SQL and receipt bytes are unchanged.
- Development and test builds optimize bundled SQLite. Cargo avoids rebuilds caused by absent packed refs or unrelated loose-branch commits while retaining provenance updates for the current checkout and tags.
- Native Task proof waits stop when their condition succeeds. Wave selection tests no longer depend on creation order, candidate-release tests avoid unrelated merge polling, and doctor tests fetch isolated local remotes instead of mutating a shared checkout.