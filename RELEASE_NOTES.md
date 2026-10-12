# v0.13.12

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.12 makes saved work easier to carry between Machines and unattended work easier to direct, follow and resume. Shared planning can travel through Git or Linear, while each Session keeps its own record of delivered direction. Claude answers can move Flows forward, dead agents stop appearing available, and background Claude and OpenCode conversations gain a supported stop-and-resume path.

## Carry saved work between Machines

Waves, Projects, Tasks and comments now share one local model with repository-selected personal Git, shared Git or Linear synchronization. Offline edits remain local until reconnection; foreground CLI, Session and Desktop connections synchronize without replaying agent turns or Flows.

- CLI and Desktop expose Git acquisition, publication, conflicts and retained alternatives. Conflicts preserve rejected changes without blocking unrelated records.
- Remote Task commands resolve through the target's configured planning and separately validate pushed code. Repeated dispatch reuses Task identity and preserves existing checkout contents.
- Only shared planning travels. Checkouts, Workflow positions, Sessions, Processes, controls and delivery attempts remain local; this change does not establish exclusive execution ownership across Machines.
- Follow-up work uses ordinary Tasks and linking comments. Completion no longer requires a separate follow-through disposition declaration; historical filing attempts and uncertain effects are retained.

## Keep direction with every conversation

Task direction can now reach every associated Session, including conversations outside a running Flow. Each Session retains its own delivery history, so an idle conversation can receive pending direction when it ordinarily continues without being started just to deliver a steer.

- Task comments and definition revisions produce immutable steers. Repository, Wave and direct Session commands also supply direction; repository and Wave scope follows existing work associations.
- Supported provider boundaries deliver at most eight steers within 10,000 bytes of combined context, with complete sources available when excerpts are used. Resume and compaction preserve delivery history.
- `lf session steers` inspects delivery, reads sources and explicitly retries uncertain attempts without starting a turn.
- Session receipts replace Flow injection and per-step cursors. Recorded emission does not prove provider acceptance or that the model followed the direction; no current adapter confirms acceptance.

## Choose an agent and let its answers advance the Flow

A global `-a` / `--agent` carries one explicit provider and optional model through Tasks, nested Flows and child commands. Headless Claude now records completed answers through the shared harness, allowing deciding steps to advance or request a correction in the same conversation.

- Use `lf -a claude:opus debug` or `lf debug -a claude:opus`. The inherited choice wins over saved defaults and conflicting child selections, and forwards through remote commands. Independent Session shells clear it.
- Replay can select an agent without changing its source capture. Native resume permits model changes within the saved provider; switching providers requires a new Session.
- Flow decisions and final answers require correlated, successfully completed native turns. Native JSON schemas carry through launch, correction and replay; retained transcript prose cannot substitute for a final answer.
- Headless output prints assistant text without the former tool summaries or cost/time footer. Historical captures remain intact.

## See what is running and continue it interactively

CLI and Desktop status now follows the recorded agent process and its exact operating-system identity. Stale client files, attachment events and sockets no longer keep a dead agent looking active or Waiting.

- Waiting requires a live agent. Work-watch checks operating-system state every two seconds as well as refreshing on process changes, so death can appear without a database write.
- Older Sessions without recorded process identity remain Unknown. Failed connections and uncertain status do not authorize a replacement agent or checkout cleanup.
- `lf session connect SESSION` stops a headless Claude or OpenCode process group and opens the same saved native conversation. Missing native identity refuses before stopping useful work.
- Interrupted turns remain unfinished. Shutdown requires the current attachment and accounts for surviving helpers; Codex retains its live connection behavior and protections for unrelated conversations.

## Operational notes

Storage conversion preserves Work identities, authored content, provider mappings, pending receipts and local execution history. Historical steer events migrate into Session delivery history while Linear comment receipts remain separate.

- Personal Git keys require explicit recovery on another Machine. Switching transports retains history but does not automatically migrate unmapped Git-imported Work into Linear. Linear's unconditional-update race remains; exactly-once delivery is not claimed.
- Git publication can remain pending or unconfirmed without warning a short-command caller, although delivery evidence is retained.
- Repository and Wave steers remain Machine-local. Already-open providers without hook declarations require a provider start.
- Scheduled commands move to one repository cron list. Conversion preserves installed job identities, history and execution locks; installation remains explicit.
- Installation checks reuse cached images. Refresh mutable tags with `docker pull`. Recognized registry throttling retries after 15, 30 and 60 seconds within one five-minute download budget; other pull errors fail immediately. The checks themselves are not retried.

Included verification covers provider stand-ins, migration and delivery fixtures, focused CLI checks and headless Swift tests. Configured-provider acceptance, two-Machine SSH, live Linear, installed scheduling and real cold-runner throttling recovery remain unverified. The supplied evidence does not establish a passing full matrix for the combined release.

## Small changes

- `lf commit` saves selected `scratch/` files in ordinary Task worktrees while preserving unrelated staged content and working edits. Persistent workspaces continue to exclude scratch and explain where to save durable notes.
- Empty commit selections explain what needs to change. Git failures retain stderr and stdout, or report the exit status when neither contains diagnostics.
- OpenCode receives permission rules when a new session is created; uncertain writes are no longer automatically retried.
- Planning-only Tasks without worktrees no longer break checkout pruning.