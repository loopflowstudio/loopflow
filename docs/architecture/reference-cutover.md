# Cutover status

Part of the [Architecture Reference](../architecture-reference.md).

**AgentProcess cutover is incomplete (LOO-443).** Provider identity and attachment
now live on tagged process rows; recorded-agent views and scheduled settlement
share that inventory. Captured Claude/native replacement advances the capture's
snapshot without refreshing stale operations. Owned native launches record before
exec while preserving terminal groups; runtime verification remains open. Optional
admission types and foreground orphan cleanup still need conversion.
Headless launch refuses a missing attachment at runtime.
Invocation-owned account failover now settles the old process and preserves its
history before replacement. Top and Task gates share unfinished-row selection
and identity judgment; unknown agents and receipt-less lf rows remain visible.
Sampling failure retains Unknown rows, not an empty inventory. Public Task-status
and scheduled-entry agreement remain unproved. Native launch retains its frozen
provider attachment under a detached terminal
owner. Replacing an outer viewer changes only its input token. Whole-path stand-ins
cover creator exit, zero viewers and the original pending Codex request/channel;
on October 17, 2026, Jack Heart also confirmed that public Session attachment
restored an unanswered native Claude question after closing its terminal. Answering
it completed the original interaction with the same Claude PID and AgentProcess.
That demo used the branch binary and a disposable Machine home with the installed
provider account; installed-store integration and SSH remain unproved.
The LfProcess/LfSession rename is integrated from #1516. The
[execution guide](execution.md#record-actual-processes) records this
boundary; record fixtures do not establish installed acceptance.

**Implemented model; installed acceptance remains open.** LfProcess is one actual lf process;
LfSession is a Loopflow-owned continuable interactive or headless conversation; a Graph is
one lf process and the step processes it starts. Run has no
separate product lifetime. The SQLite owners, shared Graph runner, CLI readers and
coordinated Rust/Swift wire types implement this contract.

The implementation has `processes` and `agent_sessions`, with
captured input and provider outcomes beneath their Session owners. Jack Heart's
2026-09-30 compression decision removes historical import, intermediate draft
schemas and old format readers. The cutover retains current Work and links,
accounts/routes and resumable conversations. Configured acceptance is separate.

Current CLI examples use supported spellings. `lf resume` selects a Session ID,
Task or Wave. Retained native terminals attach without replacing their provider.
A Run's id, running or finished, is refused with a pointer to its Task's Session;
only a Graph continues its own step's Run (structured-output correction).
A Graph whose process died leaves its
Processes as history; nothing resumes it. `session list` supports
`--interactive false`, `--history`, `--task` and `--search`; `--all` means all
repositories. `--page --json` uses stable ID pages; Desktop retains earlier
observations until enumeration succeeds. `lf graph list/show --processes` reads
Graphs from their Graph process and step processes. Historical `lf history show` and `lf history replay` remain command spellings.
History exposes Session event and provider evidence; `RunSnapshot` and Session
`run_id` are removed across Rust, Swift and fixtures. Retaining these names in a command or
wire reference does not make Run a target owner. Coordinate migration with
external consumers before changing those references.

### Implementation and acceptance evidence

| Contract | Current evidence and next dependency |
| --- | --- |
| One Process per actual lf process | Every lf process is listed while it runs; at exit its row stays only when an agent issued it, a running parent may read it, or another record references it. Help, rejected arguments and installation entry use an existing compatible ledger without initialization; unavailable storage remains an explicit gap. Public discovery and Rust/Swift wire fixtures cover the current shape; installed startup acceptance remains open. |
| Stable LfSession with separate attachment and AgentProcess | Admission/publication and Session readers use LfSession directly; ordinary automatic retry retains both outcomes with zero Run rows in the native fixture. Public Codex connect/client replacement preserves the active turn and sibling in the synthetic-Responses proof. Complete configured account/native Machine, stale-client exclusion and shared-AgentProcess preservation across every provider path. |
| Graph reads each step's result from its Process | A step's result is its process exit; a deciding or routing step answers through the Session turn its Process captured. Structured native results close the earlier Codex decision-retry transport failure in the real-Codex/synthetic-Responses fixture; configured-provider acceptance remains unproven. OpenCode batch and native terminal launches share an authenticated private server and API-confirmed identity. Ordered native messages attribute assistant steps to admitted initiating inputs across steers; an idle marker plus a successful nonempty answer settles that input. Interrupted execution is never automatically continued. The separate Task provider launcher and retired Run owners are deleted; configured recovery across providers remains open. |
| One started Graph is one lf process and its step processes | Jack Heart's 2026-10-05 decision: a Graph adds nothing on top of running `lf graph ...` in a Task worktree, and its record is the lf binary's ordinary logging. Jack's later 2026-10-05 statements put that logging in the Graph process: an append-only GraphProcess row with the Graph's name and compiled graph, and one row per step it starts. The Graph process holds one cursor and its return counters in memory; steps are plain commands that know nothing of their Graph. Configured acceptance remains open. |
| Complete recovery | Six standalone native fixtures use real Codex with synthetic Responses/private Machines, including attachment/AgentProcess loss and automatic retry. Managed dispatch has synthetic successor-history proof; configured managed provider/account continuity remains unproven. |
| Current-state cutover | Three direct migration groups retain current database state. Release conversion must also recover resumable native identities from a frozen filesystem snapshot. Finished history and intermediate branch schemas are discarded. |
| Indexed discovery and usage | Runs/usage/telemetry/activity and landing conclusions select LfSession input history before payload decoding, preserving per-input attribution and windows. Native receipts without a captured input/start remain discoverable with unknown ownership and partial usage coverage. Debug measurements at `6d76f74926` used 20,000 Sessions, 5,000 Graphs and 100,000 Processes: bounded warm CLI reads were 291–313 ms against a 300 ms empty-store baseline. The broad Process search miss was 362 ms, with a separate 64 ms SQL control. These are uncontrolled-cache local measurements, not release or Desktop latency. History uses captured event sequences and exact native references. |
| Desktop and wire agreement | Off-roadmap ancestry and pane/draft retention pass unit and mounted native-terminal fixtures. Graph and membership wire use captured numeric IDs in Rust/Swift. History and usage use the coordinated Session wire types. Configured Desktop continuity remains unproven. |
| Status-owned Chapters | Focused fixtures cover rotation/workflows, populated current-state adoption, partial/competing-plan preservation and second-Machine convergence. Configured Linear rotation remains unproven. No Chapter table or packet belongs in the model. |
| Integrated acceptance | Affected checks, configured provider/Desktop, backed-up current-state conversion and final consistency remain required. Branch fixture passes do not establish installed acceptance or promotion readiness. |

Admission preserves saved executable/Machine/database handoff and failed-command
diagnostics. Process owns command outcome and error; Session events own provider
outcomes. Exact PID/start receipts establish process identity. Passive readers
acquire no process or Graph authority. Configured-provider and Desktop acceptance
remain open; fixture results establish only their exercised boundaries.
