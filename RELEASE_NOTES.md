# v0.13.2

v0.13.2 makes returning to work easier: resume the last conversation in a worktree, reopen Desktop on saved workspace content, and keep repository and Wave plans in persistent workspaces. Recovery preserves conversations, review history, and release candidates through interruptions, while new timing reports make remaining waits visible. These notes cover the cycle since v0.13.0, including improvements already shipped in v0.13.1.

## Pick up the conversation where you left it

Claude and Codex conversations share their native homes by default, and `lf resume` removes the need to find a conversation ID. Desktop keeps unfinished interactive work visible and provides a dedicated entry point for capturing the next idea.

- Run `lf resume` from a worktree or its subdirectories to reopen its recorded interactive Session with the latest user message. Background work and later assistant output do not change the choice; missing input timestamps fall back to the Session's last interactive opening, then creation time.
- Use `lf resume <ID>` for a specific Loopflow, Claude, or Codex conversation, including one in another worktree. Completed conversations can reopen when saved native history is available. Flow progression remains `lf flow resume`.
- Choose shared or isolated provider homes with `--shared`, `--isolate`, or the `isolate` configuration default. Resuming preserves the conversation's original mode, including existing account-pinned conversations.
- Ordinary Desktop and CLI Session lists show unfinished interactive conversations and current authored reviews, with matching counts. Filtering preserves selection, drafts, prepared commands, and retained native surfaces. Enable Show headless Sessions or use `lf session list --interactive all --history` for full inspection.
- Choose a repository-local skill beside New Session. The default is `capture-tasks`, remembered per repository; choosing a skill does not launch it. Task capture explores ownership and files Tasks without starting workers or binding the capture conversation, while preserving existing Task selection and terminal layouts.

## Return to a useful workspace sooner

Desktop restores saved navigation content before waiting for fresh planning and Session reads. The sidebar now builds the workspace outline once per render, eliminating repeated work for each row's menu.

- Returning launches restore saved rows and the selected Wave or Task. Windows share a Home-scoped cache, and failed refreshes retain the last available content under one workspace status.
- Saved execution state is cleared and controls wait for fresh data. Open conversations are not automatically restored; launches without saved content still wait for reads.
- In 20 saved-workspace launches of each release build against a private copy of Jack Heart's large Home, usable workspace time fell from 735 ms median / 934 ms p95 to 506 ms / 540 ms. A returning launch with planning and Session reads failing remained usable at 590 ms.
- These measurements ran under differing concurrent host loads. First frame measures a render-server commit, not on-screen presentation, and still misses the 400 ms target. Launches without saved content took 10–12 seconds to become usable and 17–22 seconds to become fresh in the recorded runs.
- Desktop keeps bounded local launch, cache, read, and refresh timings under `<Home>/desktop-cache/timings/`, omitting command arguments and output. Contributors can report them with `uv run python scripts/benchmarks/desktop-performance/timings.py`, adding `--json` for structured output.
- `lf wt list` performs fewer repeated Git and remote reads. A recorded 51-worktree benchmark reduced median text listing time from 12.83 seconds to 1.92 seconds; the one-second online p95 target remains unmet. Use `lf wt timing [--json]` to inspect counts, phase timings, failures, and latency.

## Keep plans close to the work

Repository and Wave primary conversations now reuse dedicated workspaces across conversation replacement. Documents can be published while local plans and unrelated edits stay available.

- Create a persistent workspace with `lf wt create planning --resident`. Resident worktrees are protected from automatic pruning; moved checkouts can be reused and missing checkouts can recover committed state.
- Commit selected documents with `lf commit -m "Record decisions" <document-path>` while preserving unrelated staged and unstaged edits. Resident publication sends the committed range without automatically committing later local edits.
- Keep resident `scratch/` on disk while excluding it from the published tree. Non-resident landing still clears scratch. Sync preserves both original notes and resolver-created files when untracked filenames collide.

## Continue through stalled services and interrupted work

Existing Tasks can continue through a Linear outage, and replacing a waiting review retires its owned execution without inventing a successful review. Operators can also end stopped Flows while preserving their failure history.

- Restart and continue existing Tasks using cached planning when Linear is unreachable. Known invalidation, removal, terminal state, and ownership mismatches still block; new advice must publish before replacement.
- Restart a Task at a waiting review without leaving the old review service and provider alive. Replacement waits for owned execution to exit and preserves review history, Task identity, PR, and checkout. Independent reviews and unknown process ownership remain blockers.
- Run `lf flow end FLOW_ID` to retire a stopped Flow without running its remaining steps. It appears as `replaced`, retaining its failure, Sessions, and Execs; its reviews no longer block Task completion.
- Recovery uses machine boot time to recognize exited local execution when completion evidence is missing. Live or unresolved execution since boot still blocks. Completed Sessions with confirmed-dead providers can also release Task gates.
- Native dispatch releases SQLite before provider I/O. Session-scoped exclusion preserves stale-driver rejection, while independent Codex deadlines and history writes off runtime workers address dispatch deadlocks.
- Linear creation, edits, and completion confirm the exact issue and its Project association, avoiding a full-Wave refresh as a prerequisite for local preparation and retaining duplicate-safe retries.
- Pending PR deliveries survive retirement of obsolete Home supervision. Detached workers start in their requested checkout even when the tmux server was left in a deleted directory.

## Know whether a scheduled release reached users

Release history connects scheduled opportunities to verified outcomes, retaining unfinished candidates and failures through retries. Cleanup now also allows time for repair children to release inherited checkout locks.

- Inspect verified publication, verified no-change, deferred work, and failures with `lf release history`; record repair ownership through `lf cron disposition`.
- Catch up missed release opportunities with one execution per wake, preserving original timing and unfinished candidates. Multiple missed opportunities covered by one execution do not become multiple successful releases.
- Recover interrupted checks using process and lock evidence. Publication settlement requires the exact candidate, public artifact read-back, and installer smoke checks; retained artifacts support retries of missing publication stages.
- Retry transient artifact downloads and individual check-page reads up to three times, with bounded cleanup and a fresh directory for each download attempt.
- Release cleanup and repair re-entry wait up to five seconds for an independent checkout lease. A timeout preserves the existing owner's lock and checkout.

## Operational notes

Shared account switching with `lf account <provider> use <email>` also changes the login used by plain Claude or Codex. Running Codex engines retain their original login until restarted; an in-flight turn may fail once, with automatic recovery for headless runs. Shared Codex switching requires file-based credential storage. Forwarded Codex login uses an experimental protocol and requires Loopflow's engine.

Newly started `pursue` Flows run one implementation loop and publish when ready. `code` adds a human PR walkthrough; `feature` retains design review, a separate demo, gate, and landing. Existing captured Flows retain their steps. Builtin instructions remove repeated permission conditions for ordinary work while retaining scope, review feedback, preservation, and evidence requirements; runtime access controls are unchanged.

The `pr`, `wt`, `sync`, and `commit` commands move out of the Task hierarchy while retaining automatic shortcuts. Saved Flow commands migrate without changing identity or progress. Persistent workspace maintenance remains explicit through `lf sync`; removing tracked scratch does not erase earlier committed copies.

Release locks and checkout leases extend through Git, PR, hook, CI-repair, and publisher children. Schedule replacements on the same Home retain unfinished candidates; changing Homes leaves unresolved work available for repair without transferring execution authority. Upgrade running binaries to receive dispatch fixes, and pair Desktop with a CLI containing the new skill-launch support.

Recorded installation evidence includes published v0.13.0 installing at a real login after a missed release, launchd calendar refresh firings, receipt/app/signature checks, containerized fresh install and repair, and checkout sync preserving unpublished commits and uncommitted bytes in a sandbox. This does not establish acceptance of v0.13.2: Monday 09:00 and sleep-coalesced wake firings, interactive app acceptance, and repeated automatic release publication remain unverified. Native skill-picker interaction and live cross-repository Task capture also remain follow-up validation. Rendered Desktop launch measurements are now available, with the limits described above.

## Small changes

- Passing Dependabot updates enter the required merge queue through a dedicated token, with retries after successful CI and required checks preserved.
- Test execution denies external networking while retaining loopback, separating dependency/build setup from isolated suites.
- Desktop benchmark Home copies use SQLite backup and appear only after completion, preserving data still in the write-ahead log and avoiding empty snapshots after failure.
- Update Ruff, thiserror, tiktoken-rs, tokio-tungstenite, and dirs.
- Generated PR reviews focus on behavior, data models, and APIs.
