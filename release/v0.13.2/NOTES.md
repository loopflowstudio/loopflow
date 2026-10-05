# v0.13.2

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.2 makes returning to work easier: resume the last conversation in a worktree, reopen Desktop on saved workspace content, and keep repository and Wave plans in persistent workspaces. Task restarts and delivery recovery preserve more of the work and evidence needed to continue after interruptions. These notes cover the cycle since v0.13.0, including improvements already shipped in v0.13.1.

## Pick up the conversation where you left it

Claude and Codex conversations share their native homes by default, and `lf resume` removes the need to find a conversation ID. Desktop keeps unfinished interactive work visible and provides a dedicated way to capture the next idea without moving existing work.

- Run `lf resume` from a worktree or its subdirectories to reopen its recorded interactive Session with the latest user message. Background work and later assistant output do not change the choice; missing input timestamps fall back to the Session's last interactive opening, then creation time.
- Use `lf resume <ID>` for a specific Loopflow, Claude, or Codex conversation, including one in another worktree. Completed conversations can reopen when saved native history is available. Flow progression remains `lf flow resume`.
- Choose shared or isolated provider homes with `--shared`, `--isolate`, or the `isolate` configuration default. Resuming preserves the conversation's original mode, including existing account-pinned conversations.
- Ordinary Desktop and CLI Session lists show unfinished interactive conversations and current authored reviews, with matching counts. Filtering preserves selection, drafts, prepared commands, and retained native surfaces. Use Show headless Sessions or `lf session list --interactive all --history` for full inspection.
- Choose a repository-local skill beside New Session. The remembered default is `capture-tasks`; choosing a skill does not launch it. Task capture explores ownership and files Tasks without starting workers or binding the capture conversation, while preserving existing Task selection and terminal layouts.

## Keep plans and workspace context close

Repository and Wave primary conversations now reuse dedicated workspaces across conversation replacement. Desktop similarly restores saved navigation content before waiting for fresh planning and Session reads.

- Create a persistent workspace with `lf wt create planning --resident`. Resident worktrees are protected from automatic pruning; moved checkouts can be reused and missing checkouts can recover committed state.
- Commit selected documents with `lf commit -m "Record decisions" <document-path>` while preserving unrelated staged and unstaged edits. Resident publication sends the committed range without automatically committing later local edits.
- Keep resident `scratch/` on disk while excluding it from the published tree. Non-resident landing still clears scratch. Sync preserves both original notes and resolver-created files when untracked filenames collide.
- Returning Desktop launches restore saved workspace rows and the selected Wave or Task while fresh data loads. Windows share a Home-scoped cache; failed refreshes retain the last available content under one workspace status.
- Saved execution state is cleared, and controls wait for fresh data. First launches still wait for reads, and open conversations are not automatically restored.

## Continue through stalled services and interrupted work

Recovery now distinguishes retained work from authority to run it. Existing Tasks can continue through a Linear outage, and replacing a waiting review retires its owned execution without inventing a successful review.

- Restart and continue existing Tasks using cached planning when Linear is unreachable. Known invalidation, removal, terminal state, and ownership mismatches still block; new advice must publish before replacement.
- Restart a Task at a waiting review without leaving the old review service and provider alive. Replacement waits for owned execution to exit and preserves review history, Task identity, PR, and checkout. Independent reviews and unknown process ownership remain blockers.
- Keep stalled native dispatch from holding SQLite across provider I/O. Session-scoped exclusion preserves stale-driver rejection; independent deadlines and off-worker history writes address further dispatch deadlocks.
- Confirm Linear creation, edits, and completion against the exact issue and its Project association, avoiding a full-Wave refresh as a prerequisite for local preparation and retaining duplicate-safe retries.
- Preserve pending PR deliveries when retiring obsolete Home supervision. Detached workers start in their requested checkout even when the tmux server was left in a deleted directory.
- Completed Sessions with confirmed-dead providers can release Task gates. Live, unknown, and unfinished Sessions still block.

## See where time and release attempts went

Release history now records whether scheduled work reached users, while local timing reports help explain slow navigation. Both retain failures instead of treating a completed process as proof of a successful outcome.

- Inspect verified publication, verified no-change, deferred work, and failures with `lf release history`; record repair ownership through `lf cron disposition`.
- Catch up missed release opportunities with one execution per wake, preserving original timing and unfinished candidates. Multiple missed opportunities covered by one execution do not become multiple successful releases.
- Recover interrupted checks using process and lock evidence. Publication settlement requires the exact candidate, public artifact read-back, and installer smoke checks; retained artifacts support retries of missing publication stages.
- Retry transient artifact downloads and individual check-page reads up to three times, with bounded cleanup and a fresh directory for each download attempt.
- List worktrees with fewer repeated Git and remote reads, and inspect invocation counts, phase timings, failures, and median/p95 latency through `lf wt timing [--json]`. A recorded 51-worktree benchmark reduced median text listing time from 12.83 seconds to 1.92 seconds; the one-second online p95 target remains unmet.
- Desktop records bounded local launch, cache, read, and refresh timings under `<Home>/desktop-cache/timings/`, omitting command arguments and output. Contributors can report them with `uv run python scripts/benchmarks/desktop-performance/timings.py`, adding `--json` for structured output. These records do not yet establish a production startup speed improvement.

## Operational notes

Shared account switching with `lf account <provider> use <email>` also changes the login used by plain Claude or Codex. Running Codex engines retain their original login until restarted; an in-flight turn may fail once, with automatic recovery for headless runs. Shared Codex switching requires file-based credential storage. Forwarded Codex login uses an experimental protocol and requires Loopflow's engine.

Newly started `pursue` Flows run one implementation loop and publish when ready. `code` adds a human PR walkthrough; `feature` retains design review, a separate demo, gate, and landing. Existing captured Flows retain their steps. Builtin instructions remove repeated permission conditions for ordinary work while retaining scope, review feedback, preservation, and evidence requirements; this instruction change does not alter runtime access controls.

The `pr`, `wt`, `sync`, and `commit` commands move out of the Task hierarchy while retaining automatic shortcuts. Saved Flow commands migrate without changing identity or progress. Persistent workspace maintenance remains explicit through `lf sync`; removing tracked scratch does not erase earlier committed copies.

Release locks and checkout leases extend through Git, PR, hook, CI-repair, and publisher children. Schedule replacements on the same Home retain unfinished candidates; changing Homes leaves unresolved work available for repair without transferring execution authority. Upgrade running binaries to receive dispatch fixes, and pair Desktop with a CLI containing the new skill-launch support.

Recorded installation evidence now includes published v0.13.0 installing at a real login after a missed release, launchd calendar refresh firings, receipt/app/signature checks, containerized fresh install and repair, and checkout sync preserving unpublished commits and uncommitted bytes in a sandbox. It does not establish acceptance of this release: Monday 09:00 and sleep-coalesced wake firings, interactive app acceptance, and repeated automatic release publication remain unverified. Rendered Desktop startup, native skill-picker interaction, and live cross-repository Task capture also remain follow-up validation.

## Small changes

- Passing Dependabot updates enter the required merge queue through a dedicated token, with retries after successful CI and required checks preserved.
- Test execution denies external networking while retaining loopback, separating dependency/build setup from isolated suites.
- Update Ruff, thiserror, tiktoken-rs, tokio-tungstenite, and dirs.
- Generated PR reviews focus on behavior, data models, and APIs.