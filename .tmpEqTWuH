# v0.12.13

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.13 makes Loopflow's operational surfaces describe current truth, then gives agents safer places to act on it. Status, roadmap, prompts, and the Mac app now share fresh liveness and reviewed metric evidence; Task execution and review survive several recovery and concurrency failures; and default and Wave agents leave canonical main clean. Upgrade when long-lived, multi-session work needs trustworthy health reporting without sacrificing working context.

## Know what is happening now

Status no longer lets historical receipts impersonate live state. Project-owned metrics add reviewed evidence for KR judgment, while unknown or unavailable evidence remains explicit instead of being guessed.

- `lf status` reports Work as `working`, `stalled`, `stopped`, or `unobservable` from fresh Home-owned evidence. Earlier failures remain separately labeled as `last failure`, and invocations without verified liveness appear as `unverified`.
- Reviewed metric contracts define meaning, targets, windows, and freshness. One Rust-derived portfolio supplies `lf status`, roadmap, agent context, and the Mac Wave detail view with `Met`, `Missed`, `Unknown`, or `Unavailable` readings.
- Metrics inform KR decisions but do not complete KRs automatically.
- Status and roadmap remain readable for released Tasks that predate reviewer-facing PR copy, while retired Waves, unavailable Projects, and orphaned Tasks remain visible instead of disappearing.

## Keep Task work moving through failures and review gates

The Task lifecycle now carries the right runtime and reviewer context from User control turns through Ask settlement and PR review. Recovery converges on an actionable state instead of repeatedly launching the same doomed work.

- Tasks and Projects launched from User control enter repository runtime before their Work body runs, restoring journal trace capture without changing their existing failure policies.
- Human flow gates run the actual skill harness with its system prompt, context, configuration, and writable Task authority. User-targeted Task Asks preserve Human reviewer mode and still require explicit resolution, decline, or release.
- Machine-authored Task PR copy identifies the owning Task, feature or fix cycle, PR sequence, and merge disposition; publish and land transitions refresh that block idempotently.
- Trace persistence now waits for concurrent SQLite WAL writers. A remaining capture failure is finalized as partial and reported as actionable and non-resumable while preserving the Task worktree and PR.
- A Task whose persisted lifecycle names a missing flow records one resumable failure and parks before reserving a Run, rotating a PR, or launching a provider. Later supervision ticks stay quiet until the flow is valid again.

## Work in parallel without dirtying shared main

Default terminal agents and long-running Wave residents now run from sibling worktrees, keeping canonical main as a stable control plane. The Mac app's new repo-scoped Sessions surface keeps concurrent human attention in the same boundary.

- Starting bare `lf` from canonical main carries local commits, tracked edits, and untracked files into an author-scoped agent worktree, then restores main to the fetched default branch. Deliberate non-default checkouts and existing linked worktrees remain unchanged.
- Each Wave resident uses a deterministic sibling worktree. Resident writes are limited to that Wave's goal and memory and leave through a CI-gated PR; relocation waits while changes are dirty, unpublished, or under review.
- Sessions is now the default repository surface in the Mac app. Its native Ghostty multiplexer supports split panes, spatial focus, zoom, close and undo, and one pane per Ask or shell session.
- The attention queue groups sessions by Wave, Project, and Task, opens them serially, removes settled sessions, and prevents preparation or presentation from crossing repository boundaries.

## Operational notes

- The status DTOs replace previous body and process fields with required current-state and liveness fields. Rust and Swift ship together; external JSON consumers must migrate to the new shape.
- The metric-storage migration remains a release draft in source builds. Until it is materialized, reviewed contracts are visible but their readings remain `Unknown`.
- Launching from canonical main now relocates agent activity to a sibling worktree. Scripts that assumed the agent would continue running in the canonical checkout should use the reported worktree path.

## Small changes

- Updated Rust dependencies `base64` from 0.22.1 to 0.23.1 and `fancy-regex` from 0.18.0 to 0.19.0.
- Updated the Python development dependency Ruff from 0.16.0 to 0.16.3.