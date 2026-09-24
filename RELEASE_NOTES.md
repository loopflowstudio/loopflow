# v0.12.20

v0.12.20 makes Task progress and direction recoverable across worker exits. Tasks advance through saved Flow positions, corrections live in Linear comments, and Run conclusions remain available for later review. Checkout refresh and Linear credential recovery also improve, reducing the manual work needed to keep unattended execution moving.

## Resume Tasks from saved progress

Each Task now runs one persisted Flow invocation. A worker advances one saved boundary and launches its successor when autonomous work remains, so progress no longer depends on a resident controller's conversation history.

- Interrupted work resumes at its saved position; stale workers and human decisions cannot advance a replacement invocation.
- Active invocations retain their selected Flow and Skill definitions when those definitions are edited later.
- Human approval, iteration, and resumable sessions remain tied to the invocation they concern. Independent Task-bound Runs provide context without advancing the Flow.
- Projects operate through finite Runs and recommend one Task Flow. Use `lf task run --flow` to override that recommendation.

## Keep Task corrections in one place

Direction entered on a Linear issue or through `lf task steer` now shares one authored comment history. Only the worker advancing that Task consumes it, keeping corrections scoped to the intended execution.

- Posting direction leaves an idle Task idle and does not inject steering into independent Task-bound Runs.
- Workers refresh comments before Skill execution and while running. Polling and webhooks reconcile comment edits without duplicate delivery.
- Run traces distinguish direction included in the starting prompt from direction accepted by a live provider; neither proves that the model followed it.
- The Mac Wave composer offers **Send** for text and **Interrupt** for an active turn when the composer is empty.

## Catch up without losing local work

`lf rebase` on stale main now updates HEAD after fetching upstream. Rebase from a sibling refreshes canonical main before integrating the caller, using the same preserving updater as install and ordinary worktree refresh.

- Main updates retain unpublished commit identities and staged, working, and untracked edits. Creating the next sibling also preserves main's unpublished commits.
- `lf install` updates main, required tools, the Python environment, and the published release. Complete, current installations skip downloads; missing or damaged Mac app bundles still reach the installer.
- Linear authentication refresh serializes credential exchanges and persists complete rotated pairs without overwriting a newer reconnect or deletion. Temporary failures allow one bounded retry; unrecoverable failures distinguish retry from reconnect.

## Recover the evidence behind a review

Chapter reviews now preserve the accepted planning boundary and reconcile KR verdicts against recorded evidence. Missing evidence stays explicit, and later reviews use the newest merged chapter-start record.

- `lf runs --parent <run-id> --json` discovers direct child Runs without the usual history cap.
- `lf runs <run-id> --final` retrieves durable conclusions. Older Runs fall back to labeled streamed prose, which may include commentary.
- The archived baseline and accepted plan preserve decisions and receipts; they do not establish that planned improvements or KRs have been achieved.

## Operational notes

- Migrate automation using `--first`, `--loop`, `--finally`, or removed Project process controls to the persisted Flow model. Legacy positions that cannot reconstruct a complete invocation require explicit restart; available Work and human-session facts are preserved.
- Generic Work and Project steering, `lf chat --steer`, and Wave live steering are removed. Pass Project and Wave guidance as instructions to their operate skills. Task direction publication and comment refresh before a Skill require Linear availability; bare Task interrupts remain local.
- Finishing a Flow does not complete its Task or select another Flow. Delivery and Task completion remain explicit. `ci-fix` now owns rebase, verification, publication, and auto-merge; the landing watcher blocks on unchanged or unarmed repairs.
- After upgrading, run `lf install schedule` to enable login/hourly macOS refresh. Scheduler validation used simulated plist/reload checks; live scheduled firing and wake recovery remain unverified in the included evidence.
- OAuth expiry and rejection recovery were exercised against simulated HTTPS in a development Home. Live candidate recovery against Linear and sustained operational reliability remain unverified.

## Small changes

- Codex final-answer receipts avoid repeating streamed text in chat rendering.
- Architecture checks exclude frozen chapter evidence while continuing to check live instructions.
