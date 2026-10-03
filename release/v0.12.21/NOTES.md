# v0.12.21

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.21 makes it easier to change plans without losing active work or the evidence behind it. Waves gain resumable chapters, Task status separates execution from lifecycle, and PRs explain the specific change before its Task context. Installation now updates the published release from any directory, independently of source-checkout maintenance.

## Replace the plan, preserve the work

Each Wave now has one current chapter, with Tasks and priorities presented directly through Wave → Task navigation. Chapter transitions preserve work already underway and leave dated history available for review.

- Started, unfinished Tasks retain their identity, worktree, PR, and Flow state across chapters. Untouched backlog is abandoned; uncertain evidence stays unresolved.
- Chapter previews and durable transition receipts support recovery from interrupted transfers and lost provider responses.
- Reusable metric instruments belong to the Wave; targets belong to its chapter. Closed chapters preserve readings and evaluation dates, so later Task completion or instrument changes cannot rewrite their history.
- CLI and desktop surfaces replace ordinary Project selection with chapter and Task views, including explicit unavailable or stale evidence.

## See what a Task is doing and what its PR will deliver

A Task's lifecycle label no longer has to stand in for execution evidence. Status and recommendations now share that evidence, while Task PRs keep the author's specific change at the front of the review.

- Task status includes execution state, reason, current step, worker Run, and recent Run history. Active or uncertain execution suppresses duplicate implementation recommendations; worker evidence takes precedence over dirty files.
- Runs, Usage, Activity, and Top resolve Work identity consistently, keeping named workers and internal-ID helpers discoverable after renames.
- PR publication preserves authored titles and opening summaries, followed by one managed Task note describing the merge consequence.
- Refreshing an armed PR at the same head retains its recorded merge consequence. Publishing a changed head reports that no Task settlement is requested.

## Update Loopflow from any directory

`lf install` now selects and installs a published release without requiring a source checkout or refreshing Git state, packages, and the Python environment. Checkout updates remain the responsibility of `lf rebase`.

- Release downloads use a resolved, pinned tag and installer checksum verification. Complete installations skip asset downloads; missing or stale artifacts trigger repair.
- Promotion and recovery handle first installation without a previous installed selection.
- Machine commands and catalog inspection no longer require a repository. Listings retain repository scope inside a checkout and show machine-wide records outside one.
- Older CLIs retain their `scripts/install.py refresh` upgrade entry point as a delegate to the release installer, preserving install-directory overrides and failure reporting.

## Operational notes

- Chapters change CLI commands, JSON shapes, and metric authoring. Update Rust and Swift consumers together. Rotation changes Linear membership, cancels untouched backlog, and archives predecessor Projects; missing or conflicting evidence can leave reconciliation pending.
- Prompt directions are removed from flags, configuration, Skill/Flow parsing, discovery, and prompt injection, without a compatibility path. Migrate instructions that depended on that feature. Historical direction traces remain readable.
- macOS scheduled updates now default to login plus weekly. Rerun `lf install schedule` to adopt the new configuration; positional `daily`, `hourly`, and `5min` options are available. Automatic scheduling remains macOS-only.
- Task execution describes the Task Flow. An idle state does not establish that independent helpers or interactive Sessions have stopped, and this release adds no launch lock. Recent Task Run history is limited to 50 Runs started within seven days.
- The repaired legacy installer completed a real published v0.12.20 CLI, daemon, and Mac app installation. That validates the upgrade entry point, not installation of this candidate; populated historical Home migration remains a separate validation requirement.

## Small changes

- Untargeted metric readings retain their values without pass/fail judgments.
- PR-writing and reporting guidance separates automated evidence from user walkthroughs and uses readable links.