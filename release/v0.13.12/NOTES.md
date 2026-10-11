# v0.13.12

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.12 makes unattended work easier to follow and resume: Flows can read Claude’s completed answers, dead agents stop appearing available, and headless Claude and OpenCode conversations gain a supported stop-and-resume path. The release connects conversation history to process status without treating an interrupted turn as successful work. It also fixes selected scratch commits and makes installation checks more resilient to registry throttling.

## Let Claude answers move a Flow forward

Headless Claude now records answers through the shared harness, making them available to the Flow runner. Deciding steps can advance from a valid answer or request a correction in the same conversation while preserving skill input, context and native resume identity.

- Flow decisions and final answers come from correlated, successfully completed native turns. Retained transcript prose remains readable but cannot substitute for a final answer.
- Native JSON schemas carry through launch, correction turns and replay. Claude failure messages remain available for retry classification.
- Claude, Codex and OpenCode use their harnesses; unknown providers return an error. Obsolete batch launch and answer-recovery paths are removed.
- Headless output prints assistant text without the former tool summaries or cost/time footer. Historical captures remain intact.

## See whether an agent is still available

CLI and Desktop Session status now follows the recorded agent process and its exact operating-system identity. Stale client files, attachment events and saved sockets can no longer keep a dead agent looking active or Waiting.

- Waiting requires a live agent, checked before pagination. Work-watch refreshes on process changes and a two-second operating-system check, so agent death can appear without a database write.
- Saved connections are offered only for live agents. A failed connection does not authorize launching a replacement.
- Older Sessions without recorded process identity remain Unknown. Uncertain evidence never ages into permission to replace an agent or clean up its checkout.
- Terminal placement belongs to the attached Loopflow process; terminal updates and Move reject stale attachments. Captured-input completion follows the latest turn’s process while retaining provider outcomes separately from process death.

## Continue background conversations interactively

Run `lf session connect SESSION` to stop a headless Claude or OpenCode process group and open the same saved native conversation. Session identity and history survive the transition.

- Missing native conversation identity refuses the connection before stopping useful work.
- Shutdown requires the current attachment, excludes foreground providers and waits for surviving helpers before settling the process group.
- Interrupted turns remain unfinished. This is stop-and-resume; Codex retains its live connection behavior and protections for servers hosting unrelated conversations.
- OpenCode receives permission rules when a new session is created, and uncertain writes are no longer automatically retried.

## Operational notes

Installation verification now separates image-download failures from the checks themselves, making transient registry throttling recoverable and exhausted acquisition visible as an infrastructure failure.

- The harness reuses cached images, including explicit `--image` selections. Refresh mutable tags with an explicit `docker pull`.
- Recognized throttling retries after 15, 30 and 60 seconds within one five-minute download budget, separate from container startup. Other pull errors fail immediately; installation checks themselves are not retried.

Recorded checks cover Flow decisions and corrections, process-death updates, stop-before-resume and unfinished history using provider stand-ins. Configured-provider and installed acceptance remain unverified, including a real Task reaching publication through Claude’s decision. Cached-image installation paths passed locally; recovery from actual throttling on a cold hosted runner remains unverified. The included evidence does not establish a passing full release matrix.

## Small changes

- `lf commit` can commit selected `scratch/` files in ordinary Task worktrees while preserving unrelated staged content and working edits. Persistent workspaces continue to exclude scratch and explain how to save durable notes elsewhere.
- Empty commit selections explain what needs to change. Failed Git commands retain stderr and stdout, or report the exit status when neither contains diagnostics.