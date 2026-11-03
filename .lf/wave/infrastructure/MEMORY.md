# infrastructure wave memory

Pre-curation detail for the Session continuity and upgrade sections below:
`eda48a4e80f033e821ea08756e865214cec06f42:.lf/wave/infrastructure/MEMORY.md`,
under the same headings. Archives authorize no work or acceptance.

## Unpublished checkout preservation (LOO-666, 2026-10-27)

Jack Heart authorized repair through merge after installed 0.14.19 cleanup
deleted MTG-193's unpushed calibration checkout. Remote absence and inactivity
prove no delivery. Source now requires merged/closed PR evidence at the exact
local head, retains existing work protections, and rechecks branch/head/files
under the removal lease. Explicit disposal stays separate. Ignored artifacts in
finished checkouts are not protected by non-force Git removal.

Release's entry-point lesson applies: the real CLI/script fixture proves source
behavior, not installed acceptance. Infrastructure owns patch/install and
disposable verification before restoring the existing hourly cleanup entry.
Preserved refs/bundle establish committed-history recovery only; no pre-deletion
inventory establishes whether uncommitted/ignored data existed. Recovery must
preserve MTG-193's active checkout and Task. LOO-543 remains separate.

## Pi harness boundaries (LOO-507, 2026-10-17)

Jack Heart requested Pi through existing Session, Task and Flow paths with exact
continuity. Adapter choices await demo review. Pi owns models/credentials;
an underlying model proves no managed account identity. An execution descriptor
names writable roots, not an OS sandbox.

Pi 1.3.0 accepts a missing session path as a new conversation. Retain the absolute
session file and fail missing-file continuation explicitly. `agent_end` may precede
retry; `agent_settled` ends the Loopflow turn. Cumulative usage must retain unknown
fields and rejoin its capture counter on recovery. Shared attachment fences and
stable CLI dispatch own control and callbacks.

Pi owns internal retries; final failure needs explicit later input. Only a nonblank
terminal answer completes headless work. Automatic compaction can skip
before_agent_start, so context projects at the model-request boundary. Native
assistant checkpoints retain partial usage without completing the turn. Public
steers use shared callback receipts; queued or emitted is not model incorporation.

Real-Pi/local-model probes and Task/Flow fixtures prove behavior, not
authenticated execution or visuals; direct Pi account access passed separately and
grants no product restriction. Probe list: `acd37dde71:.lf/wave/infrastructure/MEMORY.md`,
this heading. Remaining gate/demo boundaries stay in the Task plan.

## Opening work conversations (LOO-499 → LOO-618, 2026-10-28)

Jack Heart decided (2026-10-26) that `lf resume` always gets you back: it takes
over from whatever holds the conversation, `--fork` leaves the holder running,
`lf session new [-a]` restarts on a fresh thread seeded from the conversation log.
Public `connect`, `attach` and the open-mode flags are gone; internal transport
entries remain. On 2026-10-29 he narrowed it:
resume takes only Sessions (a Run's id points to `lf resume --task`), and a Task is
assigned or not (`task claim/unassign`, no deferred state). Scope still comes from physical checkout or an explicit target, never
recency; preparation launches no provider. LOO-499 history:
`acd37dde71:.lf/wave/infrastructure/MEMORY.md`, its heading.

## Idle owners end (LOO-709, reconciled 2026-11-03)

Jack Heart requires idle conversations to cost nothing. Detached settled owners
end after `session.end_detached_after` (default 2m, configurable/off). Jack selected
editable unsent drafts and the same answerable pending requests on November 3,
removed bulk resume, and authorized delivery after implementation and checks.
Single-Session resume never replays. Workspace restoration belongs to LOO-498
(Rex first, Herder second); open Rex tabs retain viewers even when unselected.

Finished Graphs, awake-time clocks and stale unviewed status made the old policy
ineffective; store errors killed its watcher. Use wall time and retry judgments;
preserve running Graphs, live/silent turns and Runs. Claude's fenced callbacks
record activity before context delivery. Stop alone cannot settle background
watches or scheduled wakeups: retain complete task/cron evidence. Same-provider
handoff carries activity to its new attachment fence; replacement cannot inherit it.

Preserving a native editor requires retaining its owner, not repainting its screen.
The owner records input before delivery; exact plain Claude submission text plus
matching prompt-ID settlement can release it. Enter, submission hooks (which may
reject or restore text), and Stop alone cannot. Edits/paste, later or ambiguous
input and missing IDs retain the owner until native exit; other providers lack
this acknowledgement. This deliberately limits idle-ending coverage. Recheck
queued endings against input and live viewers; viewers veto idle ending, not
executable rollover. LOO-705 owns rollover through the same owner judgment.

LOO-710's integrated `screens/<session>.ghostsnp` writer/reader replaces `.vt`;
save at policy and command exit. Jack selected no DTO field, expiry or deletion.
Real Claude 2.1.317 with local synthetic model responses preserved an editable
draft and the same AskUserQuestion across grace/executable removal, and ended
a settled plain submission. This proves native interaction, not configured-account
or installed acceptance. Earlier twenty-owner teardown reopened only three;
full return acceptance remains open. Release's entry-point lesson applies:
source/fixture success and publication do not establish installed behavior.

## Session viewer wire (LOO-712, 2026-11-02)

Jack Heart asked (2026-11-02) that the owner and its viewers adopt the parts of
Rex's protocol worth copying; the design and evidence live in the LOO-712 plan
and `transport/README.md`. Decisions: one binary frame type (`tag · len · payload`)
replaces JSON lines; a stream opens with the owner's snapshot in whatever format
the owner has, so LOO-710's format change touches no framing. Events ride the
output stream as APC sequences tagged with the viewer's nonce and a gapless
`seq`; replies carry the `seq` they were computed at, in `--watch`'s id/answers
shape. Resume is byte-counted against a 1 MiB per-viewer spool kept 60 s after
EOF; exhaustion answers `reconnect_required` and the viewer repaints.
Ownership arbitrates size and colors only and is separate from input control,
which keeps LOO-618's token/replace/Busy semantics.

Concept-review correction (2026-11-02): reports belong to each viewer, not a
serve-wide theme cache. Retain queued reports through reconnect grace; only the
settled owner's known theme reaches broadcasts and fresh observers. Resume alone
does not restore appearance; takeover or queued promotion does. The ownership
and byte-resume fixtures prove this with sequence/reply ordering intact.
Unknown colors supply no override: this remains an implementation assumption,
not a palette/reset decision attributed to Jack Heart. Desktop acceptance stays
unproved.

Deviations from Rex, with reasons: input control and ownership release at EOF,
not after the grace window, because LOO-618's automatic attach must not see Busy
for a disconnected viewer. EOF proves connection loss, not process death. Events ride stream 1 in pipe
mode too (the viewer strips them, piped stdout stays byte-exact); a second
control channel was rejected. Theme is relayed opaque; Desktop (LOO-672) owns
colors. Lesson: `#[serde(flatten)]` of a struct with its own `id` onto the
request envelope's `id` silently broke every status report, and the owner drops
a connection whose frame fails to parse; a `wire.rs` unit test pins the key names.

Land order per Jack Heart's comment: LOO-705 (successor on the Moved message)
first, then LOO-708/LOO-709, then LOO-712 re-homes `successor` on the `control`
event. The successor is the full `[entry, "session", "resume", session_id]`
command, not an entry path or replay of viewer argv: LOO-708's thin viewer argv
would reattach to the ended owner. LOO-712 now carries that command end to end;
focused fixtures prove terminal restoration before following, exact argv and
successor exit/error propagation. LOO-705's superseded-install detection and
retry policy still require dependency integration. Release's entry-point lesson
applies: shell fixtures prove the wire and handoff, not installed rollover or
the requested live Claude walkthrough. Desktop consumption, measured SSH
reconnect and the 1 MB wire bound outside the fixture remain unproved.

## Waited landing (LOO-503, 2026-10-17)

Jack Heart requires no elapsed-time termination or synthetic reruns in
wait-and-fix supervision/repair; source removes outer, operation, admission and
check-page cutoffs and pending-CI timeouts. Preserve real provider
failures, explicit cancellation, exact-head intent and incident reservations.
A repair can expose its new head before exiting; landing-wide worker liveness
prevents duplicate admission and a false checkout-busy exit.

Required failures outrank pending required checks; optional failures alone do
not override a pending required aggregate. LOO-415's later failure snapshot
cannot establish what was observable when its waiter exited.
One paused-clock waiter now proves long-pending → repair → merge → requested
Task completion with provider doubles. CLI/Flow fixtures separately publish a
real Git head, retain review and complete at `end` before any status command.
Merge cannot invent completion intent. Release's entry-point lesson still applies:
configured GitHub delivery and installed upgrade remain unproved. Native repair
startup also waits without a cutoff; viewer and release publication policies stay separate.

## Native terminal diagnostics (LOO-501/508/516, 2026-10-18)

Jack Heart authorized repair preserving editing, diagnostics and history; accounts
stay out of scope. Merged #1595 (`e204ce6072`) scopes parent output to announced
private Process logs; transport keeps native descriptors, OpenCode its stderr log.
Installed repair, macOS pending-request and title/reconnect startup remain unproved.

LOO-516: Jack Heart requested durable private relay-failure evidence, not
attribution of an unobserved cause. Retain bounded observed Session events with
the original AgentProcess, operation and direction, independent of a started
native turn. Unexpected renderer resets stay evidence; explicit close stays quiet.
Observations claim no attachment and settle no turns; store failure must not end
reconnect. Gate, CI and delivery remain open. Detail and probe evidence:
`acd37dde71:.lf/wave/infrastructure/MEMORY.md`, this heading.

## OpenCode protocol evidence (LOO-504, 2026-10-17)

Jack Heart selected OpenCode 2 repair/delivery; automatic crash continuation is
deferred. Candidate deterministic-native tool/context/Flow and configured-account
launch/tool/resume passed; PR #1590 records verification and remaining acceptance.

No parent IDs or replayable events; steer/queue share idle.
Readmission can emit empty success; managed recovery injects synthetic guidance
with unaccepted at-least-once semantics. Native stability proves neither
attribution nor deduplication. Preserve frozen capture/Process/Work,
independent steers, separate queued results and LOO-415's owner. Release's lesson:
candidate proof cannot bypass supported installation. Gate, broader native
acceptance and delivery remain open.

## Test maintenance reduction (lf-6d3ff35, 2026-10-13)

Jack Heart authorized selective cleanup without retries or a five-minute CI claim.
#1545 merged; installed savings remain unproved. Evidence:
`72dbe0344f:.lf/wave/infrastructure/MEMORY.md`, this heading.

## Global agent override (LOO-440, curated 2026-10-11)

Jack Heart selected global `-a`/`--agent` over Task, Flow, step, skill and child
choices; omission preserves defaults. October 10 approval authorizes landing and
completion after verified merge. No logins, credential reads or real-provider tests.
Six fixture Sessions prove Claude across child overrides; replay/native history
retain their provider. Conflicting resume explains starting anew.

October 11 gate passed override checks; release-lock failure's cause remains
unknown. Remote/live/installed acceptance, landing and completion remain unproved.
Evidence: `6935bfbadb:wave/infrastructure/MEMORY.md`, this heading.

## Plan ownership (LOO-406, revised 2026-10-08)

Jack Heart selected identical local plan records/APIs with or without repository-wide
Linear synchronization. Delete personal/shared namespaces,
`personal_plans`, personal-only workflows and `project_authority_on`; provider
mappings are links, not planning authorities. During an outage, local edits succeed
and show pending sync. Preserve uncertain effects and concurrent edits; no resident
or automatic turn/Flow retry.

Jack Heart requires inbound completion/membership/comments during pending writes,
immediate local saves and active CLI/Desktop propagation without another turn.
Retain mutation identities, both conflicting values and reconnect deduplication;
Linear wins observed planning conflicts, while unseen remote edits remain a race.
LOO-412's accepted ordering uses causality before clock/mutation-ID ties and keeps
losing edits recoverable. User-keyed Git is the default; shared planning is opt-in, and joining
cannot silently publish existing plans. Ref separation supplies no privacy.
Sessions, Processes, captured Workflow positions, reservations and checkouts stay
local. Shared completion grants no execution or cleanup authority.
The superseded prototype/delivery narrative and exact October 8 comments remain at
`616d73b4dcd21dd78c62f3f3420c5e4f9b0d8f21:.lf/wave/infrastructure/MEMORY.md`,
Plan ownership; LOO-406 owns common writers/Linear and LOO-412 transport.

Foreground acquisition and delivery are independent; effect locks block neither.
Captured receipts retain losing values; only complete-list observations settle
Project ordering. Every create returns a distinct ID, even for identical titles
(Jack Heart's October 8 comments removed public creation tokens); LOO-412's
October 10 redesign owns machine-local execution linked by Task ID, and shared
completion does not move or stop the local Flow. Observed Linear reopening keeps
both values and retires delivery, leaving execution; unseen reopening can be
overwritten. Comments preserve authorship, losing edits and pending direction;
cancellation resolves the Team's lowest-position state; deletion requires
trash/acknowledgment, never absence. Earlier checks, writer proofs and schema
evidence: `72dbe0344f:.lf/wave/infrastructure/MEMORY.md` and
`078a6642e:wave/infrastructure/MEMORY.md`, Plan ownership; peer and installed
acceptance remain unproved, full CI open.

Jack Heart's LOO-449 decision makes checkout files own goal, memory and ancestor
Markdown; edit directly, without a document cache or CLI. Canonical identity
cannot redirect bytes. SQLite retains identity/execution; `wave_workflows` retains
Project selection and Task capture. Registry relocation preserves files in place;
authoring moves them.
Jack's October 10 decision makes Linear sync pin one committed local default-branch
revision, never fetch or fall back; local views stay checkout-based. Mocked sync,
checkout and stand-in launch proofs preserve LOO-444; installed acceptance remains
unproved. Jack approved #1521 and landing with the default-branch fix (`05b04fa8`).
Plan: `scratch/read-wave-goals-and-memory.md`.

Export saves UUIDs before acquisition and retains uncertain creates/links;
acceptance remains unproved. Jack selected unambiguous Task prefixes (4+ hex
characters), preserving full IDs, receipts and placement; no landing/install.
Writer/prefix evidence:
`72dbe0344f73722bd90a4f392c3e801cd976b80f:.lf/wave/infrastructure/MEMORY.md`,
Plan ownership. Backup/export remains unproved.

## On-demand Linear acquisition (lf-6f396c0, 2026-10-11)

Jack Heart's `2d140be4` replaces per-agent polling with team deltas at turn, Flow,
planning-read and write boundaries; only watchers retain a slow clock. No daemon,
webhook or pub/sub. Shared attempt freshness (10 s while Linear's budget is healthy,
longer as it falls: LOO-649) differs from the successful watermark: failed/interrupted ingestion retains the latter. Exclude terminal Tasks
and teams without live Tasks; pagination may require multiple requests. Successful
mutations settle captured receipts directly; uncertain effects retain identity.
Home-wide reset cooldown handles HTTP 400 limits as well as rate-limit headers.

#1491's Git exchange remains intact. One foreground connection selects Git exchange
or watcher-only Linear checks; removing Linear timers must not remove Git lifetime.
Release's operation-entry lesson applies: generated Stop-hook child processes,
Flow launches, watcher backoff/shutdown and populated migration fixtures prove their
boundaries, not installed behavior. Idle providers issue no timed Linear requests.

October 12 source checks repaired stale acquisition timestamps, slow bulk
fixture seeding and Linux reconnect peers. Network-isolated Linux proofs cover
concurrent hooks, no idle-agent requests, watcher backoff, lost replies and
rate-limit recovery; formatting, Clippy and architecture checks pass. A scheduled
release child signal did not recur; cause unknown. Hosted candidate CI, live
request counts and installed acceptance remain unproved. `rust/loopflow/tests/README.md` retains
Linux HTTPS reconnect as CI's obligation. Exact failures, commands and counts:
`6935bfbadbc2cecca1321dda4bd6668b3d42a062:wave/infrastructure/MEMORY.md`,
On-demand Linear acquisition.

Jack authorized landing after checks. His `6d169dc1` permits publication before live
measurement while Linear is paused. Live request counts, rejected-request quota
accounting and installed pending-write recovery remain unproved. After installing,
manual restoration of `linear.team` with `planning.provider: linear` in loopflow,
etude, kata and fantasia precedes measurement; rollout notes belong in the PR.

## Session titles (LOO-439, 2026-10-08)

Jack Heart requested shipping after review; complete after merge.
`283fa7b5-3fdd-4457-9170-0de821534803` selects supported future lf/plain,
interactive/headless, Task/taskless starts. Existing conversations and provider
changes are out. Shared LOOPFLOW.md teaches self-renaming.
Fake-API Claude TUI/headless and shared Codex TUI pass; embedded Codex/exec lack
the endpoint. Live cmux workspace/tab rename passed with a stand-in
(`/tmp/lf-cmux-demo-owapn43t/`). Docker promotion/retry preserves hooks, settings
and symlinks; rendered host/shim/herdr and Jack's installation remain unproved.
Plan: `scratch/every-lf-session-in-cmux.md`.

October 11 hook repair: Jack Heart's comment `d6ad11df-fd8b-4b4a-861b-ac7ab785cb26`
requests landing PR #1530 after sync. A held WAL writer reproduced a callback
over six seconds before naming; Process admission permits a 15-second receipt
wait. Optional prompt hooks bound observation and ownership reads separately
(lock waits vs history scans), reuse the bounded connection for parent reads and
never retry failed early observation at exit. Unavailable ownership leaves names
unchanged. Lock fixtures prove source behavior only; installed concurrent-Flow
acceptance and the slow status/ensure cause remain unproved.
Plan: `scratch/stop-the-session-title-hook.md`.

Jack Heart's October 11 Kata report (`d4991d61`): prompt timeouts recurred outside
Loopflow; the Kata lock holder/cause remain unknown. Evidence:
`4fc8754e7d:.lf/wave/infrastructure/MEMORY.md`, Session titles.

## Program Status direction (LOO-398/415, reconciled 2026-10-16)

Jack Heart selected every-pane Desktop reading, unchanged native passthrough and
lf writing only for pipe-driven work/Flow position. Reports override inference
in Rust's single Waiting judgment; they grant no execution authority.
Jack Heart assigned emission to LOO-415 on October 7, conditional on lf and
Flows reporting equally well to external hosts; LOO-398 owns reading. LOO-415's
brief requires one terminal writer for Flow steps as well as conversations.
A PTY accepted only 62 of 74 OSC bytes: a parent mutex cannot serialize independent
step writers. Provider reports pass unchanged, with no reports in JSON/log output.
LOO-384 retains non-reporters; LOO-402 owns broader presentation. Source proofs
do not establish installed/native adoption; viewer absence is not provider death.
Earlier patch evidence and constraints:
`72dbe0344f:.lf/wave/infrastructure/MEMORY.md`, Program Status direction.

## Session continuity (LOO-416/710, reconciled 2026-11-02)

Jack Heart accepted per-Session owners (per-shell PTYs), Work-keyed layout intent
and uniform repository/Wave/Task lifecycle including nested CLI. No daemon,
replay or eager-start policy. LOO-415 owns transport, LOO-618 resume, LOO-497
layout APIs, LOO-489 integration. Known scenes render before lookup; fresh
resolution owns new links. App death loses runtime state; repaint restores
neither input nor cross-installation layout. Registries/twelve-layout fixtures
prove no provider continuity.

LOO-710 snapshots name their Ghostty revision: v1 has no cross-revision promise;
GhosttyKit has no decoder (LOO-672). One preview/live cleanup owner restores
modes/colors on launch failure or pre-ack cancellation. Even blank live state
supersedes preview; freshness means no output. Jack's fidelity requirement:
paint history/primary before alternate, inspect a disposable copy, preserve
1049's saved cursor, and use the color override mask so untouched colors follow
the viewer theme. Stand-ins prove return to primary, round trips, colors and
public resume, not configured Claude, Desktop or four-target packaging.

Filing/admission commits uncaptured primary Sessions; execution reserves input
and claims Tasks. Nested CLI stand-ins preserve pending input, provider/Session
identity and provenance after caller/viewer death, not readiness or warming.
Desktop cancellation reaches the worker, drains both directions and bounds
inherited pipes after exit. Kill only the exact query child; timed-out mutations
stay unknown. Public reopening, native remount and provider survival need separate
proof; LOO-416 saved-state integration/measured continuity remain open.

## Machines (LOO-394 / LOO-411, 2026-10-07)

Jack Heart accepted #1494's rename/shipping and discarded version-1 Desktop caches.
A Machine is one OS user/data directory; Home/provider paths, cron, selection keys
and released JSON retain their bytes. LOO-401 owns the command map, with no aliases;
GUI-browser capture remains prohibited and independent Desktop snapshots remain.
Gate/lock relocation is unproved. Superseded command/rename details:
`888509b3ff71f1afbeea3bf951bf1bd588bc11eb:.lf/wave/infrastructure/MEMORY.md`,
this heading, and `06e88761d:wave/infrastructure/MEMORY.md`.

Jack rejected stable cross-version APIs, negotiation and old-peer support. Existing
version/identity commands suffice: retain versions when identity fails; report
differences and the update command without a version gate. Named connections live
on Machine rows. Removal clears label/repo and releases destination uniqueness;
identity and route survive so a replacement can be added at that
destination without deleting history or touching remote work. Only explicitly
added machines may receive foreground credentials.
OpenSSH owns destination syntax. Exact-frontier fixtures seed released
SQL. Local source proofs establish no installed migration or configured remote
continuity. Release schedules retain original ownership and activation.
Test retired options beneath valid owners; root words may resolve to skills.
Flow normalization must not restore SSH ownership. Disposable Linux accounts
prove renamed installation entry only.

Jack Heart's PR #1489 review permits private OpenSSH sharing, a default-yes
first-install offer and recovery hints. Existing lf is never replaced; status,
batch and JSON never install. Personal masters stay separate; agent forwarding
has its own socket scope. Jack selected global `--machine <label>`, removing
`lf ssh` without an alias; LOO-411 supplies the selector cutover. Loopback proofs
establish reuse, not installation or account continuity.
The target parses the command in its saved repository; Task, worktree and Wave
selectors resolve there. `--machine Y machine add X` edits Y's registry. No SSH
alias or per-call repository override remains.
LOO-411 / PR #1489 merged (`35e759aaf`).

## Resident machine logins (LOO-413, 2026-10-08)

Jack Heart accepted the real Codex demo and requested `ship` for #1493,
superseding publication-only delivery. Only added machines receive stdin logins.
Retain interrupted registrations, selected identity, atomic/private keys and
target-owned resolution. Keychain read failure permits a new key only without
encrypted tokens. Batch never starts missing logins, even with a terminal.

Browser-approved Codex transfer to MINI (`mini-heart`), authenticated reads on
both machines, selected execution and headless reconnect passed in isolated
source deployments; standard installs remain unchanged. Later refresh independence,
other providers, locked-Keychain reboot and installed acceptance remain unproved.
Demo evidence: `353cd661e:scratch/machine-sign-in-demo.md`.

Gate repaired build-version comparison; OAuth's output-handle warning did not
reproduce, without a known cause or repair. CI retains Rust and
macOS sandbox Python checks. Earlier proofs: `178abab13:wave/infrastructure/MEMORY.md`;
current checks: `scratch/work-on-another-machine-name.md`.

## Task decisions and delivered work (LOO-408, 2026-10-07)

Jack Heart authorized autonomous repair, verification and landing, with verified
merges completing Tasks by default. Explicit remaining work carries its outcome,
evidence condition and next check; overdue calls for a decision, never invented
success. Task state changes must preserve Session turns, reservations, process
outcomes, ancestry and live controls. Historical uncertainty can retain a checkout,
but cannot veto an authorized completion or cancellation. Per-Exec acceptance is superseded; decisions remain history.

Installed 0.13.9 reproduces LOO-353's five pending turns, one reserved input and
two unknown processes despite its merged PR's completion intent. Source fixtures
prove preservation. PR #1488 merged as `cead4c952`, including PR #1483's
Process vocabulary and migration without another draft. Installed acceptance
awaits the first published release containing #1488 and preservation readback.
LOO-285's unattended settlement and LOO-390's storage-prevention evidence remain
distinct from merge. Legacy keep-open requests need scope reconciliation.

Jack Heart's comment `cf9e2775-154a-4f37-86b1-79a42cd5cf49` retires arbitrary
numeric performance targets, soak requirements and deeper optimization while the
product surface changes. LOO-304 closed successfully. Add supported settlement
of LOO-371/376/375 to LOO-408's installed acceptance, retaining respectively
Session `session_ccab7b1eafea4370a16b799dd1705737`'s unresolved turn, read-only
Process `ef54b06d-9896-467f-a920-f8d4648f9d8b`, and interrupted research Process
`6fbaec2a-3031-4c26-aae0-7aa1231005ca`. Their briefs retire performance acceptance;
closure is not yet observed. LOO-378 is explicitly paused with substantial
unpublished code retained, without deletion or delivery authorization.

## AgentProcess (LOO-443/447, reconciled 2026-10-16)

Jack Heart accepted #1519 (`71741bd4`) and #1524 landing (`55bc3fcd`).
AgentProcess owns fenced attachment and exact stop; client death differs from
provider death, and unknown descendants grant no signaling. Stand-ins prove
A → B → A rejection, not configured relay or installed acceptance.

LOO-415 supersedes the stop/resume design. Jack Heart's October 10 decisions,
#1524 approval and source proofs: `72dbe0344f:.lf/wave/infrastructure/MEMORY.md`,
this heading. Explicit recovery retains exact stop, native history and uncertainty;
deleted replay/relay writers stay deleted.

LOO-450: Jack selected ClaudeHarness, native schemas and predecessor deletion,
approving landing with quieter output. Fenced native answers, replay/schema and
fake-Flow proofs pass, not configured Task-to-publication or installation.
Harness errors retain failed-result text for retries. CI owns the full matrix.

## Live conversation ownership (LOO-415, curated 2026-10-17)

Jack Heart accepted per-conversation owners for nested CLI, headless, terminal,
shell and Flow work (#1581, `28dc141671`). LOO-415 owned transport; LOO-712
owns its viewer wire, and LOO-416 lifecycle/readiness. No shared server or automatic replay. Explicit resume
preserves history/input; viewer loss starts only LOO-709's grace. Native-only
Flows remain unselected. Owners retain execution/capture/client; passive viewers
claim nothing. Viewer timeout/cancellation changes neither provider attachment
nor execution authority.

Jack's October 17 disposable-Home Claude walkthrough kept provider PID and
AgentProcess. Installed-store/Desktop, SSH, owner-crash/startup and twelve-Session
acceptance remain unproved; detail: `28dc141671:.lf/wave/infrastructure/MEMORY.md`,
this heading. Reattachment retains hooks. LOO-502 later removed automatic restart
and selected stable entries.

LOO-708 (Jack Heart, November 2): exec removes Claude launcher/viewer heap;
only the owner's direct child hands off settlement. Graph steps still wait.
Blocking child wait wakes descriptors without taking runtimes' SIGCHLD.
Pre-compression stand-ins: owner/relay 20/2.5 MB; no installed acceptance.
LOO-705 supplies the Session successor; replaying relay argv targets a dead owner.
LOO-712 owns encoding. Every resume/new-thread entry must carry its resulting
Session across exec without replay. Owner/returning-launcher settlement share
result/capture semantics; native status cannot redefine failure/attachment loss.

## Session steers (lf-9c03797, 2026-10-11)

Jack Heart accepted Session-only delivery, repository/Work wrappers, downward
propagation, independent receipts, bounded launch seeding and idle staying idle;
he renewed pursuit after the provider gaps. Replace old paths together in one PR.
Jack Heart then selected the vocabulary “steer” and “delivery” and authorized
the naming update followed by landing PR #1535. `Steer`/`SteerId` replace the
duplicate message projections; JSON uses `steer`, and the single draft stores
`steers` with `steer_id` references. Released migration history remains unchanged.
Source-level delivery inspection and agent acknowledgement were review proposals,
not selected work. Keep explicit retry and honest unconfirmed delivery; no new
acknowledgement lifecycle or follow-up Task is implied by vocabulary reduction.
Provider-bound capability survives Codex handoff; OpenCode uses local invocation
IDs, never fabricated provider request IDs. Child callbacks and replaced providers
retain no authority; emission is not confirmation.

Claude 2.1.296's exclusive startup-hook/input cycle wrote only 512 of 131,133
bytes in two seconds. Shared dispatch plus exclusive takeover resolves it.
Old injection paths are deleted. Membership reconciles on reads/callbacks;
cancelled mixed batches retain unaffected direction. Reserved capabilities are not
installed ones; explicit diagnostics do not drive workspace refreshes. Saved
steers now trigger planning revisions. Historical fixture counts, branch cuts and
the shared-dispatch counterexample remain at
`28dc1416712c59203ac89d0d4128eb0e7f79b085:.lf/wave/infrastructure/MEMORY.md`,
this heading. These prove neither configured model acceptance nor OpenCode 2
compatibility. Current semantics: `docs/architecture-reference.md`, Durable
communication.

## Process-owned liveness (LOO-451, 2026-10-10)

Jack Heart removed client-file compatibility (`5228ab69`); launches record an
AgentProcess before exec. Unknown grants no Waiting or cleanup. Terminals follow
attachment; completion outranks sockets. Inputs follow the latest turn's LfProcess,
not an ended creator; native outcomes remain history. Jack approved shipping #1523
(`5df94b2c`). Proofs include Work-watch detecting OS death without writes, not
configured kill/reconnect or installed acceptance.

## Execution vocabulary (LOO-441/442/446, 2026-10-10)

Jack Heart selected LfSession/LfProcess (#1516) and AgentSession (#1517), both
landed; AgentSessionId has no table. LOO-446 (PR #1520) is the cleanup he
requested; after #1519 merged he asked for it to land (`dc87102b`). LfProcess
identity is `id`, references `<role>_lf_process_id`; AgentProcess references
`agent_process_id`. The attached lf and Flow process retain distinct roles;
FlowRunner executes ordinary command steps. `processes` holds both kinds.
Superseded cutover and proof details: `72dbe0344f:.lf/wave/infrastructure/MEMORY.md`,
this heading. Five Python gate cases await CI; installation remains unproved.

## Terminal conversation recovery (curated 2026-10-18)

LOO-409/lf-8fed729 evidence: `28dc141671:.lf/wave/infrastructure/MEMORY.md`,
this heading. LOO-499 supersedes ensure/replace.
LOO-512: fenced captured-input receipts and drained startup replies prevent replay.
Candidate, 0.14.5 and 0.14.6 PTY faults preserve exact drafts/history, proving no
original-reset repair. Live/unknown providers grant no duplicate launch or signaling.

## Storage footprint (LOO-390, 2026-10-07)

Jack Heart authorized investigation, cleanup and delivery.
`session_events` was 92% of a 2.6 GiB store: each streamed token was two rows,
also kept in `runs/*/events.jsonl`. PR #1474 (v0.13.9, installed October 7 and
measured): new captures keep 28% of rows and 72% of bytes, preflight validates
an exact store in place (1.0 s, no copy), WAL holds 64 MiB, two fingerprinted
migration backups remain (13.6 → 5.9 GiB). Old rows are not rewritten.
PR #1501 merged as `812d8cc55` (installation unproved): promotion appended every
release to `retained_published_sets`; a settled install now retains its fallback and the
prior one, then removes unnamed content-addressed binaries/bundles no live
process executes.
Store growth stays unbounded, about 1.7 MB per capture. Next condition is Jack's
retention choice: system of record (SQLite or `events.jsonl`) and capture expiry;
then sizes in `lf home doctor`. Left intact: 23 GiB of
unread legacy `traces`, `backups`, `lfd.db*`, `logs`; LOO-304's 40 GiB of
`/private/tmp` fixtures. Unknown ownership is not permission to delete. Test
`session_record` with `LF_*` cleared.

Earlier cleanup: `66da3fdf7:wave/infrastructure/MEMORY.md`, “Retained capture
storage and autonomous cleanup”; current direction: Capture cutover.

## Project configuration and review direction (2026-10-05)

LOO-366 remains unfinished (Optional chapters below). LOO-367's retry stopped on
boot witness 819671; preserve its conversation/Flow. Prior review:
`906576f39:wave/infrastructure/MEMORY.md`, this heading.

## Release follow-through (reconciled 2026-10-05)

Jack Heart assigned Infrastructure verified publication and installed acceptance.
[Release memory](release/MEMORY.md) owns execution, recovery and proof limits;
prior account: `86d0e5e6a2:wave/infrastructure/MEMORY.md`.

## Capture cutover (LOO-370, reconciled 2026-10-05)

Infrastructure selected one opaque `~/.lf/runs` root under Jack Heart's source-only
delegation (comment `3829b49e-2ade-4bee-8850-2ae2297399a8`); PR #1450 (`1af81fe03`)
is integrated. This supersedes relocation, parallel layouts and offline migration.
Capture keys select history; Session IDs select mutations, and Session/Exec
provenance supplies authority.
Preserve paths, payloads, feedback, native identity and usage; missing payload
cannot erase resumable identity. Checksum-pinned v0.13.3 stub-provider proofs and
#1450's merge establish no installed conversion or interruption authority.

Writable hard-link aliases, a recovery check/write race altering replacement
metadata and unresolved Mac boot custody remain counterevidence, not repaired by
deleting probes. Exact proofs and references:
`5948e490eb7414a059f27ba39f838423a4730465:wave/infrastructure/MEMORY.md`
under this heading.

## Review replacement (2026-10-04)

Jack Heart authorized LOO-377; #1429 merged as `c5dc238b0afb`. Installed
acceptance is unproved; LOO-373 owns landing-placement reconciliation.

Save service, attached lf and provider identities before stop removes receipts.
Lock launch before attachment fencing; settle spawn, then reobserve the Flow.
Retain native history without inventing review success; protect unknown owners
and independent reviews. Evidence:
`c28b7d0197:wave/infrastructure/MEMORY.md`, this heading.

## Transient recovery (LOO-326, curated 2026-10-09)

Jack Heart approved cached planning regardless of age for existing Task restart,
continuation and worker launch. Invalidation, removal, terminal state and ownership
mismatch still block; retain observation age. New advice needs Linear publication
before worker replacement. Retry failed provider reads only, never publisher writes.
Bound pipe collection and exit: descendants can retain stdout. Artifact attempts
retain candidate identity in separate temporary directories.

LOO-326 completed October 6 after v0.13.5; Jack accepted its read-only Exec's
unknown outcome, retaining the checkout. Session closure and
missing receipts grant no control or cleanup authority. Receipts survive terminal
write failure; pruning requires exact death and matching persisted outcome.
Cleanup independently reacquires its lease: terminal receipts can precede
descriptor closure. This proves neither the loss cause nor recovered evidence.
Release owns re-entry leases, transfer deadlines and pending-version lessons.

#1459's exact-reflog repair shipped in v0.13.5; history:
`cad03fe6d:wave/infrastructure/MEMORY.md`. October 11 lf-cb9012a supersedes
its ancestry veto in source. Jack Heart subsequently authorized landing #1533
after resolving main conflicts (`7689d642`); this gate does not establish delivery.
Current Git ancestry owns PR comparison; saved placement cannot
classify commits as foreign. Preserve every commit. Stack forks still own squash-parent sync;
refreshing them during publication can restore files main deleted. Disposable
CLI proofs pass; installed Etude and hosted acceptance remain unproved.

## Scheduled release accounting (LOO-285, source reconciliation October 2)

Jack Heart retained one execution per wake for frozen missed dues. Completion
still requires two adjacent original configured dues, two automatic executions,
at least one artifact publication, required verification and no manual repair.
Collapsed misses supply accounting coverage, never additional settlements.

Publication uncertainty blocks candidate replacement; retain rejected candidates,
original dues, physical cron exits, publisher stages, settlement proofs and repair
ownership independently. Same-Home continuation does not transfer old-Home authority.
Child-held mutation locks and checkout leases protect separate scopes; cleanup
reacquires after descriptor closure. Parent death, elapsed time and reset/stash
helpers cannot confer cleanup rights or replace a held lock inode.

October 5 evidence: installed jobs repaired at unchanged 09:00/10:00 schedules;
three opportunities have unknown timezone provenance, zero executions and no
qualifying pair. Schedule repair cannot backfill settlement; September 24's 36
telemetry failures remain contrary evidence. Reproduce a current failure before
repeating analytics repair. Recovery fixtures are not Scheduled evidence. No
installed pass or accepted Intelligence handoff is established.

LOO-357/PR #1441 removed the UI-host prerequisite; Jack Heart waived the
interactive demo and authorized #1419 landing October 4. Installed acceptance and
two automatic settlements remain unproved; no local reconciliation authorizes
installation, schedule/Home changes or review completion. Reporting proves no
operation success; activation proves neither accounting installation nor
qualifying settlement. Dated proofs and authorizations:
`6935bfbadbc2cecca1321dda4bd6668b3d42a062:wave/infrastructure/MEMORY.md`,
Scheduled release accounting.

## Operator acceptance and account direction (2026-10-04)

Jack Heart closed LOO-295 and LOO-342 on machine usage and declined retired Ask
work; acceptance artifacts: `35e759aaf4:wave/infrastructure/MEMORY.md`, this
heading. LOO-324: cross-binary history discovers native Session records across
folders, never duplicating transcripts or replaying context. A usage threshold
redirects future launches only; actual exhaustion triggers shared failover;
isolated agents keep their own account/fallback. The Linear brief owns scope.
[LOO-298 decisions](#data-model-and-performance-decisions-curated-2026-10-17)
supersede older Run-owner, historical-import, pinned-Home and demo-before-landing
directions.

## Optional chapters and Task workflows (curated 2026-11-03)

Jack Heart's October 2 direction makes chapters optional. LOO-366 owns Project
ensure/resets; LOO-367 owns admission/completion. Neither ordinary work nor Tasks
require a managed Graph. Reads provision nothing and grant no KR acceptance.

October 4–6 decisions preserve exact Project identity, atomic ingestion, unfinished
work and unreviewed backlog. Rotation freezes membership without losing later
starts/external moves; partial facts cannot retire work. Cancellation retains
write guards through commit; Session reads and cached preparation stay available.
Current architecture owns selection; retired YAML/writers remain retired.

Jack accepted observation naming October 6 without changing wire/cache/selection
identity. Reading validity differs from Work state; workspaces can be non-Git.
That gate stopped at disk reserve after static checks. Public CLI/crash recovery,
configured Intelligence repair and installed acceptance remain unproved; headless
app/view checks remain required, mounted judgment optional. Full decisions and
contrary evidence, without settlement or installation authority:
`39f57361dd597044c0939d0f3ca1aa67d223ea6a:.lf/wave/infrastructure/MEMORY.md`,
this heading.

## Migrations get ids at merge (LOO-656, 2026-10-28)

One slug migration per Task, edited until merge; origin/main's first-parent
order gives ids, and `lf install --dev` alone advances `~/.lf`. Released
SQL stays immutable. Mechanics: [MIGRATIONS.md](../../../rust/loopflow/src/store/MIGRATIONS.md).
The retired draft/copy model (LOO-344): `986be7988:wave/infrastructure/MEMORY.md`.

Jack Heart decided on 2026-11-02 (LOO-693) that a build may migrate `~/.lf`
under running readers: one lf per Mac on every stream (LOO-669/688), additive
migrations never break an older reader, and no process rollover at install.
The 2026-10-30 outage came from 0.15.0 readers predating slug ids, not from
additive tails. Lesson: store skew must surface as an error naming the skew and
`lf install`, never read as an empty registry — agents take "nothing here" as
fact. `open_existing_store` is the one opener (`Ok(None)` only when no file).
A refusal names one fix: callers prefix who refused and the path, then pass the
store's error through verbatim; appending `lf doctor` after it sent operators to
a command that installs nothing. Limit: a step's own long-lived connection that
meets a `-- breaks:` migration mid-write still fails with raw SQLite text.

## One main Home (LOO-342, curated 2026-10-10)

Jack Heart approved installed `lf` and `~/.lf` for ordinary commands, Task
workers and Flow steps. Explicit `LF_HOME` experiments need a fresh directory
after schema changes.
LOO-342 closed (v0.12.31):
`b1a50a7dc:wave/infrastructure/MEMORY.md`, this heading.

## Worktree listing and fenced dispatch (LOO-375, reconciled 2026-10-07)

PR #1456 shipped in v0.13.6. Installed measurements, batching and limits:
[report](../../../scripts/benchmarks/wt-list/README.md),
`07bbd801e:wave/infrastructure/MEMORY.md` under this heading.

- Store opens validate ledger/schema, leaving full integrity scans to migration,
  doctor and install. Keep the Session fence outside runtime waits; OpenCode's
  fenced post remains bounded at 10 s. Exec observation waits 15 s, then warns
  and runs unrecorded. Exact measurements and counterexamples remain in the
  `6448e3c9e7:wave/infrastructure/MEMORY.md` archive; current timings live under
  `<Home>/perf/`.
- **Install preflight/promote read the OS account's Home whatever `LF_HOME`
  says.** Tests running them are container-only installation proofs.
- Task decisions no longer depend on historical execution; checkout cleanup
  still retains live or unknown providers. LOO-408 owns the installed proof.

## Environment variables (LOO-341, reconciled 2026-10-07)

Jack Heart's audit lives in [Environment](../../../docs/architecture/environment.md).
`LF_HOME` selects the machine's data directory and `loopflow.db`; shared Exec
names drive shell/tmux clearing. Earlier branch evidence remains at
`c31279995a4ea0eec09c053e39c2f71a81d26034:wave/infrastructure/MEMORY.md`.

## Task worktree membership (LOO-358)

Jack Heart selected checkout membership plus explicit binds. Rust owns the
reader; membership grants no process or Flow authority. Architecture
supersedes the managed-Flow distinction; LOO-408 separates Task decisions from
execution and retains conservative cleanup. Proofs and unresolved Desktop
acceptance remain at
`e50dbd749e3207599f9937ce45653f4c6f33a5cd:wave/infrastructure/MEMORY.md`.

## Tasks across machines (LOO-412, 2026-10-11)

PR #1491 landed October 11 with Jack Heart's approval. Design/checks: `6fabcd0734:scratch/`; [mechanics](../../../docs/architecture/planning.md).

Jack selected Linear, shared or default user-keyed Git per repository. One typed
writer owns local/imported Work; canonical Task/Project fields replace plan/PM
wrappers and duplicate readers. Linear privately owns acquisition, observations
and acceptance. Saves attempt delivery before returning; failures buffer locally
and warn. No fallback transport, publisher election or turn/Flow replay.
Execution and delivery attempts stay local. Shared completion never moves a
Workflow, signals a Process or authorizes checkout cleanup.

Receipts preserve attempts, later edits, nulls, ages, foreign/partial facts and IDs.
Journal routes replace inventories, retaining causal losers and uncertainty.
Released SQL stays immutable; no old-version Git decoder. Key pairing and automatic export of Git-imported,
unmapped Work into Linear remain open; Linear's unconditional updates retain a
remote-edit race. LOO-523 (2026-10-19) replaced first-start admission, placement
and Started with an append-only (person, Machine) `task_ownership` log and a
personal, Machine-local `lf wave machine`.

Ordinary Tasks/comments replace FollowThrough. Private recovery retains filing
IDs/uncertainty, committing Tasks before comments. One config
cron list replaces WaveCronDef/GOAL declarations. Conversion retains labels,
Machine, activation, dues, receipt classifications and child-held locks. Release
remains Infrastructure-owned. Main #1523's terminal draft stays separate.

Status preserves pending-publication uncertainty. Real SSH, live Linear, installation and cron activation
remain unproved. Candidate/run IDs and repairs:
`6935bfbadbc2cecca1321dda4bd6668b3d42a062:wave/infrastructure/MEMORY.md`, Tasks across machines.

## Planning and launch preservation (curated 2026-10-13)

Earlier evidence:
`9e8ed2abcc703e78e07d5c0ea94fe94516643172:.lf/wave/infrastructure/MEMORY.md`,
this heading. Current contracts supersede archived routing/provider authority.

Presence, freshness and eligibility differ. Omission is not deletion; acquisition
is not revision. Partial facts invalidate. Preserve removals, conflicts and frozen
historical membership; unrelated relations share no revision. Readback settles
lost replies, not KRs. Relocation/completion remain unproved.

Preserve failed launches, executable/data ownership, native and Task/PR/checkout
identity, and pending decisions. No timestamp merge; database isolation does not
isolate provider/checkout effects. Inspection proves no continuity or conversation
access. Replacement owns new definitions; archives restore no deleted writers.

## Managed account identity (LOO-339, curated 2026-10-08)

Jack Heart authorized landing and a patch release; shipment remains unproved.
LOO-340 owns account state/routing and LOO-338 naming. Compare email and per-user
subject, not workspace identity; usage proves no login. Unavailable identity is not
rejection. Native refresh, sole browser ownership and installed acceptance remain
unproved by that work. Exact decisions, isolation requirements
and proofs: `32607f1d2:wave/infrastructure/MEMORY.md`, this heading; mechanics:
[subscriptions](../../../docs/subscriptions.md).

Main #1511 (`e69d5103f`): Codex terminals use `--no-daemon` (0.161.0-verified).
Explicit account selection, not launch-time switching, reconciles the shared daemon
with five-minute turn grace. Configured acceptance remains open.

LOO-665 (2026-10-28): Jack Heart authorized the same-login Codex hotfix through
merge; Infrastructure owns installation/acceptance. Mini's stored limits succeed
while native returns 401 despite account/read; rotation order remains unknown.
Jack's 04:13 UTC steer reports isolated same-Session marker recall after upgrading
Codex to 0.165.0, but shared status still unavailable. This proves neither shared
recovery nor a fresh 401 diagnosis. Identity alone proves no usable access.

Recovery needs definite target rejection and alternate acceptance; transient
failure stays unknown. Keep conversation placement independent of credential
selection. Compare-before-rename cannot preserve an uncooperative writer: retain
the displaced inode, including later descriptor writes, and refuse stale evidence.
Save-back must honor the same evidence or it can recreate the failure. General
refresh coordination and Claude changes remain separate. Explicit continuation
of a completed Session retains retirement and prior captures; fresh atomic input/
attachment admission still rejects stale completion snapshots and live owners.
Release's entry-point lesson applies: public status/launch/resume fixtures prove
stand-in continuity, not real-provider inference or installed recovery.

## Account auth consolidation (LOO-320, curated 2026-10-10)

Jack Heart approved delivery, excluding cross-account continuation, native refresh
coordination and headroom ranking. Selected account precedes native history;
usage proves no login or capacity. Browser login, first connection, remembered
Linear profile and live usage windows remain unproved; Claude returned
`invalid_grant`. Branch binaries never migrate the installed Home.
[Mechanics](../../../docs/subscriptions.md); constraints and proof limits: `be4a2b2af:wave/infrastructure/MEMORY.md`, this heading and its
`b2228bce8` archive.

## Task deletion and command ownership (LOO-305, curated 2026-10-10)

Jack Heart selected provider/local deletion and command consolidation; execution
settlement stayed deferred. History:
`58de9907b6:wave/infrastructure/MEMORY.md`, this heading. [Mechanics](../../../docs/lf.md).
Fresh ownership authorizes deletion; acknowledgment or trash confirms it, never
missing membership. Preserve Done outcomes, terminal times, PR/Git evidence and
positive confirmation across retries/stale reads. Allocation failure retains created
identity. Removal is not termination. The demo migrated the installed Home and broke
its CLI; Jack forbade branch-binary access/promotion. Use disposable Homes without
inherited authority.

## Task convergence (LOO-319, curated 2026-10-07)

LOO-319 / PR #1301 own this work. Exact slice/demo evidence and superseded
Flow compositions remain at `cc18c992b:wave/infrastructure/MEMORY.md` under this
heading and its linked prior memory.

Jack Heart selected reconciliation before decision/publication, demo and authored
delivery. Preserve captured definitions; locate policy rather than catalog indices.
Feedback is not a verdict; failed decisions cannot regain authority. Reload Task
agent choice at each launch, including review, despite PM refresh. Executable and
Home must agree. The real Codex fixture used synthetic feedback and predates
generic loop-decide; Claude launch/resume, real five-minute stall and rendered
Desktop agreement remain unverified. Simulated liveness grants no signal authority;
preserve first PID/birth across missing samples and account for active tools.
`rust/loopflow/tests/README.md` and `scripts/README.md` own fixture/disk isolation; no branch binary may migrate main.

## Data model and performance decisions (curated 2026-10-17)

Jack Heart selected one SQLite owner per object, direct Processes and compiled
Flows. Current architecture supersedes historical Exec/FlowSession, conversion
owners and S5 binding. Exact September decisions, migration counterexamples,
draft inventory and linked proofs remain at
`d476a8cfb6b7553bda4a52829fb854f128636dcb:.lf/wave/infrastructure/MEMORY.md`,
this heading; LOO-298's checklist: `ab901f1f1:scratch/remaining-work.md`.

Preserve Work/links, accounts, native IDs, reviews, captures and resumable Sessions.
No fleet compatibility, old-turn import or restored attachment authority is required.
Use disposable stores without inherited authority; released SQL stays immutable.
Conversion needs quiescent writers, consistent database/filesystem and executable
backups, rehearsal and verification before reopening. Live sidecar reads plus
SQLite backup are not atomic; manual Task moves prove no conversion.

Skill steps use ordinary commands. Ancestry preserves proven generations without
Task/Flow authority. Loop passes are positions, not child Flows; retry retains the
pass. Captured inputs belong to Session history. Selected successful turns supply
decisions; Blocked needs a reason. Retired in-turn decision commands stay deleted.
Bind permits done Tasks, preserves earlier usage and never splits a turn.
Validate denormalization or delete it. Explicit Project selection supersedes
status inference and backlog expiry; retain unreviewed work. No Chapter table.
Event-renaming proposals remain unresolved; deleted owners grant no restoration.

LOO-298's full gate, configured continuity and installed conversion remain unproved;
the archive retains its unaudited real-provider fixture escape. Preserve Session
ID ordering, PR stacking writes and Work-bound checkpoints; stubs must contain
conflict-agent launches. Earlier proofs establish no final-frontier continuity.
Current attribution and published-installation policy supersede historical
source-install directions; discarded attribution needs no importer.

LOO-291/300's performance limits are historical (targets retired October 7);
[benchmarks](../../../scripts/benchmarks/desktop-performance) retain them.

## Continuation, delivery and authority lessons (curated 2026-11-03)

September history:
`39f57361dd597044c0939d0f3ca1aa67d223ea6a:.lf/wave/infrastructure/MEMORY.md`,
this heading and its archives. Archival proves no acceptance or deletion authority.

- Saved definitions preserve alternatives and interactive boundaries. Claims fence
  late results; settlement consumes actual outcomes before teardown. Cursor
  settlement proves no exactly-once effect; finished Graphs alone do not complete
  Tasks. Native handoff requires proof across executable resolution.
- Preserve unknown history/order and exact process receipts, including PID-reuse
  rejection. Ancestry, age and missing sockets/displays grant no control. Missing
  observations are not idle capacity: split readers duplicated LOO-293's worker.
  One Work reading serves status/actions without rewriting attribution.
- Preserve authored PR copy, managed metadata and exact-head intent. Publication
  requests no completion; record readiness after provider success. Same-SHA
  progress counts; failed repair surfaces its cause. gh 2.101.0 could report
  dequeue success while queued: reobserve before replacing a head.
- Async cancellation cannot release blocking descendants' locks. An empty lease
  rejects a branch created after absence was read. Rotation retains frozen
  membership/metrics/time, verifies uncertain effects, and preserves started
  identities. LOO-278 integration transferred no checkout ownership. Fixtures
  isolate ambient authority and stop only their own identified children.

Hosted interruption, orphan cleanup, green-unmergeable PRs, Etude clipboard
behavior, LOO-296 restoration and LOO-286/287 configured continuity remain
unproved. Exact obligations also remain at `71bbdb80d2` under this heading.
UI observation schedules no work.

## Installation and command scope (curated 2026-10-11)

Jack Heart closed LOO-292 on machine evidence October 4. History, cadence/app checks:
`243ed91a36:wave/infrastructure/MEMORY.md`, Installation and command scope/closure.
Install owns artifacts; sync owns checkouts. Reinstall cannot heal a truncated gate.
Pinned artifacts activate through promotion; candidate recovery and exact-store
preflight precede authorization. Verify matching macOS bytes and preserve launchd
labels/logs/custom directories without source maintenance. OS-account Home bypasses
HOME/LF_HOME: verify activation failure/recovery in disposable accounts. Simulated
launchctl proves no timing; asynchronous copy caused mismatched bytes/segfault.
Ubuntu 24.04 passed where Debian lacked GLIBC.

LOO-287 retains command-scope reduction and unproved ordinary-folder/provider
completion and target-first resolution. Distinguish non-Git folders from Git errors;
never invent repositories/routes. Setup and receipt redesign remain unselected;
a phase alone proves no advancement.

Jack Heart approved #1529 for landing October 11 (`ff9c03dd`). ECR quota
failure and bounded-retry evidence: `72dbe0344f73722bd90a4f392c3e801cd976b80f:.lf/wave/infrastructure/MEMORY.md`,
Installation and command scope. Cached disposable proofs establish no hosted
recovery; persistent throttling fails as infrastructure. [Release memory](release/MEMORY.md)
retains cold-download/operation-entry evidence; reporting proves no operation success.

## Upgrade continuity (LOO-502, 2026-10-17)

Jack Heart removed executable pinning after #1587 review, accepting natural
failure and retry in the same Session/checkout; delivery stays review-only.
Providers, hooks, prepared commands and Flow children use the stable entry;
execution selects/verifies artifacts. Main dispatch ignores inherited LF_BIN;
private Homes retain selection. Schema checks remain; compatibility probing
and installer-triggered restart are deleted. LOO-705 owns viewer return.

Reattachment cannot update pre-fix environments; same-Session resume after exit
can. Inspect old Flow receipts/effects before choosing remaining work; display
graphs cannot reconstruct cursors or authorize replay. Disposable exit-42 retry
proof and limits: `38dd8abd09:.lf/wave/infrastructure/MEMORY.md`, this heading;
design: `4ca3bbe3d7b30b8a4bb05d466ad55f5eeb368201:scratch/loo-502.md`.
MTG-117's unknown exit is not proven upgrade-caused.

## Moving onto the installed lf (LOO-705, 2026-11-02)

Jack Heart requested one rollover mechanism after 0.15.0 refused 0.15.1 attaches
and rollover moved none. Replacement from durable state, without negotiation,
remains the headless choice pending Jack. `installation::superseded` compares
the owner's build with its launch entry (`resolve_lf_binary`), shared by owner,
resume and inventory. Removed/replaced/superseded owners end when waiting and
send `<entry> session resume <id>`; viewers execute it without replaying argv
(LOO-708 execs viewers before Clap). Resume watches an answering busy owner;
an unanswering one ends with a cut-turn report. Rollover sweeps all superseded
owners; viewed/artifact-set filters are deleted. Graph runners stay; steps use
the entry. Jack calls LOO-709's 2m grace sufficient for "move always."

Cross-build transport adds optional fields to existing Response variants, never
variants (`moved` broke 0.15.0). Compare against the relaunch entry, not the
viewer binary. Unreadable advanced stores keep retrying; mismatched app/CLI
builds restart each other's owners. Fixtures prove neither Mac `lf resume LOO-618`
nor a Graph's next step on a newer build.

## Shipped history

Archive: `c756414c8:wave/infrastructure/MEMORY.md`, this heading.
Cron continuity judges the latest due against exact scheduled receipts; manual
runs prove no firing, and old gaps (LOO-241, `ec126964d`) do not keep later
telemetry red; fresh failures join existing work.

## Gotchas

- **A bundle id reaches any registered copy** (LOO-620, 2026-11-02). Launch
  Services resolved `com.loopflow.mac` to a benchmark staging copy and Desktop showed
  a database mismatch. `lf open` launches the installed app by path (the OS account's
  `~/.local/bin/lf` resolves into it); `LF_DESKTOP` routes by id only to a dev
  Desktop, and fixtures set a test id so a Machine's real app never answers. Source
  and every build carry `com.loopflow.mac.dev`; release packaging stamps the release
  id. Promotion stages in `<Applications>/.Loopflow.app.noindex/` and sweeps earlier
  sibling copies. Same-id copies registered elsewhere stay registered; removing them
  means parsing `lsregister -dump`.
- **Git exclusions can fail after staging work** (LOO-664, 2026-10-27).
  Apple Git 2.50.1 rejects a literal ignored `.lf/tmp` exclusion after staging
  ordinary edits; the rooted glob `[.]lf/tmp/**` avoids it, and a separate
  checkpoint index restores pre-staged telemetry from HEAD. Whitespace-only
  filenames are work: never trim them to detect an empty checkpoint. Installed
  LOO-661 recovery is unproved by fixture.
- **Commit failures need both Git streams** (2026-10-11). Shared diagnostics
  retain stdout and stderr or a silent failure's exit status; scratch exclusion
  belongs only to persistent workspaces.
- **Desktop gates are headless** (rechecked at `bc6d11661`). `--swift` runs app/model/view checks; `--loopflow` builds app/UI runners; `--all` excludes explicit-only `--ui-host`. Runner-hang history: `bc6d11661:wave/infrastructure/MEMORY.md`, Gotchas.
- **Rust compilation does not validate SQLite column names.** Runtime SQL whose shape depends on a released schema must be shared with a behavior test that prepares and executes it against the migration head. Epoch Work ownership is three exclusive foreign keys (`wave_id`, `project_id`, `task_id`); generic kind/id belongs to explicit routes such as synchronous cross-Work questions, not to Epochs.
- **Source history must reconstruct every applied release frontier.** Preserve
  an unknown store frontier and its old binary; match embedded SQL against the
  ledger before ratifying history. Unmerged-migration candidates cannot promote;
  schema-complete exact-frontier repairs need no migration. The July 20 recovery
  and #1123 evidence remain at `ec126964d:wave/infrastructure/MEMORY.md` under Gotchas.
- **Installation tests need OS-account isolation.** Jack Heart’s October 5 steer forbids the three LOO-370 host checks named in `tests/e2e/README.md`: getpwuid bypasses HOME/LF_HOME. PR #1444 supplies disposable-account proofs; until integrated, isolated CI owns them. Ordinary fixtures also scrub inherited LF_* authority.
- **One writer owns a checkout and its Git sequencer.** Concurrent-edit/rebase incident: `6016eca30585:wave/infrastructure/MEMORY.md`, Gotchas.
- **Linear Project UUIDs survive renames; derived slugs do not.** Canonical local
  Project fields own content; optional Linear bindings and observations retain
  integration evidence. No `projects/*.md` cache. Reconcile history by stable IDs.
- **Environment configures a process; it must never decide what the process is.** Keep roles explicit. Inherited-listener incident: `6016eca30585:wave/infrastructure/MEMORY.md`, Gotchas.
- **Provider omission cannot erase canonical Work.** A Project absent from a
  current inventory can still own unfinished Tasks. Read canonical content with
  separate acquisition validity/age; missing or contradictory observations neither
  synthesize ownership nor authorize deletion. Preserve identity and history even
  when a checkout is absent. Earlier provider-hierarchy guidance is superseded by
  LOO-406/412; exact counterexamples: `3d8b939495:wave/infrastructure/MEMORY.md`, Gotchas.
- **Shared disposition and local execution have different authority.** LOO-406's
  accepted Linear conflict policy permits observed reopening, retaining losing
  values. It grants no Workflow movement, process control or checkout cleanup.
  Membership changes preserve historical execution and require current ownership
  evidence before external effects. Earlier blanket refusal of provider reopening
  is superseded, not evidence that configured continuity passed.
- **macOS cron cannot reach the login Keychain** (LOO-682, 2026-10-28): both
  scheduled jobs failed on publisher/Claude Keychain access. The bootstrap now
  installs gui-domain LaunchAgents on macOS; Linux keeps crontab. Real scheduled
  firings remain LOO-645's evidence.
- **Release orchestration, publication and caller edits are separate evidence**
  (August 2026): an unpublished v0.12.14 despite cron/workflow success, and a
  scheduled retry that removed two pre-run memory edits, remain at
  `24db5d4d6:wave/infrastructure/MEMORY.md`, Gotchas. Judge product state; keep
  same-tag recovery singular; LOO-261 owns clean-host candidate validation and
  LOO-266 preserving caller branch, index and working bytes across every release
  exit.

## Durable model lessons (curated 2026-10-02)

[Earlier model and planning records](https://github.com/loopflowstudio/loopflow/blob/d281370191844294ce0ad877752f2aa6a6402282/wave/infrastructure/MEMORY.md#model-design-settled)
preserve their exact decisions and implementation history. Current planning and release decisions supersede archived cadence, daemon-chat
and mandatory Project assumptions.

Self-hosting remains the default; credentials follow the repository Doppler
policy. Products own release substance; avoid a generic deploy platform before
real consumers require it. One writer per checkout is dispatch discipline;
mutation-specific locks do not create a general execution authority. Durable
state is not a message bus, and attributed input cannot manufacture another
controller. Task completion owns its transaction/event; a merged-PR status read
must not complete it or mint a synthetic agent execution.

Performance evidence distinguishes missing receipts/fields from measured zero.
Preserve first accepted usage and conflicting-repeat partiality. Window by the
owning fact's terminal time; publish measured coverage beside percentiles.
Unknown authority cannot be inferred from observer time or trace prose.

Plan ownership above records LOO-406's current direction. KRs are explicit
judgments; preserve unrelated provider associations and diagnose ambiguous moves.
Standing frontier plans need no invented completion date. No fourth planning noun
was selected. Earlier detail: `7fdbe0aea6:wave/infrastructure/MEMORY.md`, this heading.

## Archived pointers

Archives authorize no work; installed worker recovery is under Shipped history.
Prompt reduction boundary (LOO-287's open proof; Intelligence owns assembly) and
earlier follow-ups, incl. Jack Heart's unresolved July 6 “up/down 5ths”
referent: `7b28ab1ca:wave/infrastructure/MEMORY.md`, those headings.

## Direct invocation and large inputs (curated 2026-11-03)

Jack Heart's October 9–10 LOO-444 decisions supersede LOO-429's all-system
assembly: fixed per-surface additions, first-turn skill/request/tagged clipboard,
terminal argv and one bytes/cap refusal. Checkout files own context, not SQLite;
small sources stay whole, marked excerpts name source/size. Excerpt interpretation
and memory shrinking remained review choices. Inlining, constant triggers and
Codex base overrides are deleted; spawn hooks replace the temporary profile.
Budget rendered UTF-8 bytes: Unicode spill preserved markers while losing content.

Jack approved #1518 (`96ee2917`) after branch Codex headless and Claude native
proofs. Interactive Codex, compaction, resume and OpenCode stayed unverified,
nonblocking. Preserve checkpoints/common invocation loading. LOO-422 owns OSC
7501 simplification; rejected-turn recovery and installed homepage capture stay open.

LOO-420: Jack selected native installed-skill invocation, matching discovery,
exact arguments/assets and translated ports, then delivery after verified merge.
Exclude busy-terminal injection, competing writers and generic recovery. One
catalog owns declarations; export preserves third-party files. Native subagent
names select no lf harness. Preserve placement, refused captures and reservations
on failed publication. Codex 0.160.1 needs discovery for typed skills; explicit
Markdown references select originals. Custom prompts retain positions/assignments,
not bundle semantics. Preserve unfamiliar declarations, native/sibling history,
stale-client rejection and lossless JSON. Unbound third-party launches omit
operating guidance; attributed Work/Graphs retain it. PATH discovery replaces
availability probes, never spawn error handling.

Fixture benchmarks prove no live compliance or speedup; hosted CI, delivery and
installation remain unproved. LOO-428/429/420 history:
`39f57361dd597044c0939d0f3ca1aa67d223ea6a:.lf/wave/infrastructure/MEMORY.md`,
this heading and its archives. cmux/pointer judgment stays with demo/review.

## macOS CI evidence (lf-ddf2d53, 2026-10-13)

Jack Heart authorized CI delivery. lf-12be237 retains ten consecutive typical-code
PRs under five minutes and queue/push-to-merged proof.

Miss overlap is implemented; hits use Xcode. Trials and rejected alternatives:
`72dbe0344f:.lf/wave/infrastructure/MEMORY.md`, this heading.

Twelve signed builds passed; `loopflow` compiled once per miss, zero per hit.
Scoped cancellation stops the compiler process groups left alive. Toolchain/SDK/
deployment and source/target must match. Identical re-signing of disposable
copies proved bytes equal; signature removal alone leaves Mach-O padding.

Hosted benefit, cleanup cost and Swift/installation/network proof remain open.
Preserve copy/sign and disposable OS accounts for installation. No paid runners,
test removal, cache relaxation or retries.

## Agent startup profiling (LOO-436, 2026-10-08)

Jack Heart requested landing #1505 October 8, superseding review-only feedback.
The Task stays open for cold-cache verification; Git retains measurements and
rejected patches. Provider-probe removal, absent-Wave resolution and directory reuse
matched evidence; capture-store reuse and ancestor-probe deferral did not and were
removed.
Claude reconnect's native median worsens; the later handoff gain cannot repair it.

Readiness requires raw-mode echo of unsubmitted text; Codex reconnect needs an
untimed seed turn. Fresh Homes are not cold OS caches.
LOO-436 gate owns cold readiness on dedicated macOS with exclusive cache control
and native login, reporting the limitation and reset method. SVG readability awaits review. Neither gap is waived.

Copied databases keep absolute provider-home paths: strip routes before launch and
connect only benchmark-created Sessions. Early excluded probes reached those
paths without a write audit; isolation remains unproved. Fixtures prove no
installed gain or managed-account continuity. The network-isolated CI smoke
bounds launch work and preserves identity, context and history; nested-directory
launch retains the repository root. Hosted success remains unproved.
