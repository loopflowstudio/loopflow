# infrastructure wave memory

## Plan ownership (LOO-406, revised 2026-10-08)

Jack Heart selected identical local plan records/APIs with or without repository-wide
Linear synchronization. Delete personal/shared namespaces,
`personal_plans`, personal-only workflows and `project_authority_on`; provider
mappings are links, not planning authorities. During an outage, local edits succeed
and show pending sync. Preserve uncertain effects and concurrent edits; no resident
or automatic turn/Flow retry.

Jack Heart's October 8 comment `e36cf3ec-5376-4d29-8ab0-a69a64d70919` requires
independent inbound completion, membership and comments during pending writes and
after reconnect. Comment `93690dc2-5526-4ecf-a203-5e248339b0c9` adds immediate local
comment/completion saves and active CLI/Desktop propagation without another turn,
manual refresh or unrelated outbound drain. Acceptance requires stable mutation
identities, retained conflicts and reconnect deduplication. Jack Heart selected
Linear as winner for every planning conflict.
Without Linear, prefer the host where possible, otherwise last-write-wins with
try-not-to-clobber and recoverable losing edits; LOO-412 owns ordering.
`185de5fbf` implements observed Linear conflict adoption and retires losing
intentions. Receipts preserve both values; unchanged baselines preserve saves.
Sessions, Processes, Flow/captured Workflow
position, reservations and checkouts never synchronize. Planning completion grants
no Workflow movement, Process control or cleanup authority.
Jack Heart selected user-keyed Git planning by default, with explicit opt-in to
one shared plan. Sharing a code remote never merges plans implicitly. Selection
uses the same records/APIs, not personal/shared types. LOO-412 owns stable user
keys, remote/ref binding and shared ordering. Ref separation is not privacy;
joining must not silently publish existing local plans. Shared UX needs clear
plan selection, authorship, assignees and sync status; execution stays local.
Jack Heart selected LOO-412’s custom Git planning ref prototype on October 8
(`8b45e82d-6765-490a-a3d6-44b44611cedd`) and accepted the cross-Task boundary
(`b5402659-b825-4c12-b278-5d56ae1aa518`). LOO-406 owns common storage/writers and
Linear sync; LOO-412 owns transport, excluding execution. Isolated transport fixtures
can proceed now; integration consumes a coherent committed writer cut, never dirty
code or duplicated planning. Ownership is independently reviewable. Jack invoked ship-decomposed. No competing Flow, machine
replication here or real plan publication to the public code repo is authorized.

Foreground acquisition and delivery are independent; effect locks block neither
saves nor acquisition. Matching observations settle captured attempts and advance
later saves' unchanged baselines; retired receipts retain losing values.
`b1f175bbf` delivers Project ordering with captured moves; only complete-list
acquisition recognizes partial progress, never detail reads.
`6bb8b935a` supplies receipt-backed CLI/Desktop sync status; `086d3560c` deletes
pending-only projections, retaining baselines. Configuration selects pending display;
creation receipts advance revision. `29777b8cb` scopes sync to repositories;
`b6f34a6f8` separates Linux reconnect and portable abandonment fixtures.
`078a6642e`'s Linux migration, native/public Flow/work-watch reconnect and adoption
passed after fixture/DTO repairs. `ffe986160` removes synthetic settlement;
queue gate's 15-second lock refusal leaves receipt verification open.
Swift/full matrix need CI. After queue, Jack Heart authorized landing and CI checks.
PR #1503's classifier proves no acceptance.

Jack Heart's `8821db44-0480-4493-8609-953207663f1d` removes public creation tokens.
Each create generates/saves/returns a distinct ID, even for identical titles;
internal delivery retains it. His `34c1739a-8831-4578-96c9-797aeaa87743` proposes
complete Task/Project planning with machine-local execution linked by Task ID.
No rewrite is authorized; Linear Done beside a running Flow remains unanswered.

Jack Heart authorized this regression repair. Tests distinguish observed reopening
(adopt Linear, retain both values, retire delivery, preserve execution) from unseen
reopening (unconditional writes can overwrite it). This supersedes frozen test
disposition. Readback settles state, not atomicity; `7d2bc4279f`'s Linear schema
has no expected-revision update. [Rationale](../../docs/architecture/planning.md).
Linux: 54 tests, fmt/Clippy pass; CI remains open.

Comments retain provider authorship and losing edits without echo. Cancellation
resolves the Team's lowest-position state before attempting; failed discovery stays
retryable. Deletion settles only on trash or acknowledgement, never absence.
Newer explicitly active Linear evidence restores visibility and retains the loser;
inventory grants neither restoration nor execution authority. Exact slice proofs:
`078a6642e:wave/infrastructure/MEMORY.md`, this heading; installed acceptance is unproved.

SQLite owns Wave definitions/relocation, preserving IDs, files, execution, PRs,
checkouts and uncertain effects. Personal owners and provider-first deletion stay
removed; Project IDs precede slugs. Common-writer boundary: `84664e661`;
earlier proofs: `bbc6eb8d3:wave/infrastructure/MEMORY.md`, this heading.
`e68f2a423` exports saved UUIDs. Separate creation/link attempts retain uncertainty
without replay; identity attachment precedes inventory and preserves later saves.
Composed/installed export acceptance is unproved.

Jack selected Git-like Task prefixes: four or more hex digits, bare or prefixed;
reject ambiguity, retain full IDs and lengthen display abbreviations as needed.
Identity, receipts, mappings and placement stay distinct. No landing/installation.
Design: `scratch/explore-loopflow-s-own-store.md`.

Superseded split proofs and recovery evidence: `dfe18ab6060901992b55e64842e23c4295673b08:wave/infrastructure/MEMORY.md`.

Disposable workers prove no backup or private export.

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

## Program Status direction (LOO-398, 2026-10-07)

Jack Heart selected every-pane Desktop reading, unchanged native passthrough and
lf writing only for pipe-driven work/Flow position. Reports override inference
in Rust's single Waiting judgment; they grant no execution authority.
Implement from the spec. LOO-384 remains for non-reporters; provider adoption is
unverified. LOO-402 owns broader presentation.
Jack authorized build and publication for review, not landing, and made LOO-394's
absent relay a named follow-up. Its #1484 covers naming only.

Jack approved the embedded patch at `a60e9e2a…`; both earlier patches survive
in immutable lf2, verified by public checksum and SwiftPM
linking. Literal state/kind/message appears in the breadcrumb header and pane
strip; Task-header intent remains for review. SQL/CLI/DTO/Swift proofs establish
separate boundaries, not installed or composed native-pane acceptance. Provider
generation and surface incarnation fence observations; viewer absence is not death.

Live emission remains unfinished: a PTY short-wrote 62 of 74 OSC bytes, and the
existing independent output paths cannot safely finish the escape before normal
text. Flow children inherit terminal descriptors; a parent mutex cannot serialize
them, and piping native children changes terminal behavior. Extending this Task
with PTY transport or deferring emission to LOO-394 remains an unresolved choice,
not Jack's scope decision. Details and acceptance: `scratch/read-and-write-program-status.md`;
original design is preserved at `c62c19f5c:scratch/read-and-write-program-status.md`.

## Machines (LOO-394 / LOO-411, 2026-10-07)

Jack Heart authorized rename PR #1484 and stacked machine-record LOO-411 through
publication and review, later superseded by the landing request below. A machine
is one OS user and Loopflow data directory. `LF_HOME`, provider/account homes,
opaque `home_…` IDs, cron plist and Desktop selection keys retain their meanings/bytes.
Jack chose to discard version-1 Desktop caches. Jack Heart's October 7 LOO-401
steer supersedes `installation` with `self` for install, doctor and skill export;
`config user` reads the display name. Root `open` replaces desktop; screenshot
and its supervisor are removed. The GUI-browser capture prohibition and independent
Desktop snapshot remain. No aliases; short install/doctor/user commands stay.
Jack Heart accepted PR #1494 and advancement through shipping. LOO-401 supplies
LOO-397's command map. Released gates,
receipts and jobs pin `~/.lf-machine/install` and the promotion lock path; no
relocation is proved. Recovery uses `install` for retained binaries. Cron reads
released JSON keys; historical payloads survive.
Rename evidence: `06e88761d:wave/infrastructure/MEMORY.md`.

Jack rejected stable cross-version APIs, negotiation and old-peer support. Existing
version/identity commands suffice: retain versions when identity fails; report
differences and the update command without a version gate. Named connections live
on Machine rows. Removal clears label/repo and releases destination uniqueness;
identity, route and placements survive so a replacement can be added at that
destination without deleting history or touching remote work. Only explicitly
added machines may receive foreground credentials.
OpenSSH owns destination syntax. Exact-frontier fixtures seed released
SQL. Local source proofs establish no installed migration or configured remote
continuity. Release schedules retain original ownership and activation.
Release's operation-entry lesson also applies to command removal: reject the retired
option beneath a valid current owner; an unknown owner can produce a false pass.
Root words may resolve to authored skills; public lookup proves builtin removal.
After #1489, Machine discovery covers id/add/list/status/rename/remove; Flow
normalization must not restore the removed SSH owner path.
Renamed installation entry proofs passed in disposable Linux OS accounts.

Jack Heart's PR #1489 review permits private OpenSSH sharing, a default-yes
first-install offer and recovery hints. Existing lf is never replaced; status,
batch and JSON never install. Personal masters stay separate; agent forwarding
has its own socket scope. Jack selected global `--machine <label>`, removing
`lf ssh` without an alias; LOO-411 supplies the selector cutover. Loopback proofs
establish reuse, not installation or account continuity.
The target parses the command in its saved repository; Task, worktree and Wave
selectors resolve there. `--machine Y machine add X` edits Y's registry. No SSH
alias or per-call repository override remains.
Jack Heart's October 7 steer records LOO-411 / PR #1489 merged; main `35e759aaf`
is integrated. Parent landing grants no later-slice or LOO-413 landing authority.

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
reproduce, without a known cause or repair. CI retains materialized Rust and
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
Process/LFID vocabulary and migration without another draft. Installed acceptance
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

## Execution ownership names (LOO-441, 2026-10-09)

Jack Heart selected LfSession/LfProcess for Loopflow ownership; provider-owned
names remain LOO-442. This pure rename preserves wire/storage/fixture bytes,
ProcessLfid, enum variants and product Session/Process. SQL and profiling
captures retain historical names. `2b183c547` implements Rust/Swift and live
docs; Rust tests await gate, installation unproved.
LFID stays durable; optional PID is reusable. LOO-400's accepted history,
mapping, placement and proofs:
`6130a4eed:wave/infrastructure/MEMORY.md`, “Process vocabulary.”

## Provider conversations (LOO-442, 2026-10-09)

Jack selected AgentSessionId atop LOO-441/#1516; LOO-443 owns engine/driver.
No table; retain history/attribution, SQL/JSON bytes and Swift rawValue ids.
Focused checks pass; stacked publication authorized, not merge.
Installation unproved.

## Terminal conversation recovery (LOO-409, 2026-10-07)

Jack Heart authorized autonomous repair through landing. Installed v0.13.9
revision `6448e3c9e7` recorded reservation and spawn-request receipts for LOO-386's
first input `ca3b3cf9be254e118f314fef934cd2e9`; the later two admission errors
were retries. Its TUI returned successfully without native history. The terminal
path discarded its temporary client receipt without retaining a provider process.
This contradicts the proposed initial-generation mismatch.

Record native provider identity at spawn and observed exit at wait; failed spawn
is positive non-start evidence, not an engine exit. Commit reservation with the
driver claim. Native fallback must use the same ownership checks as first launch;
remote clients cannot stand in for their surviving engine. Fast terminal exit
must preserve successfully published native history between opener probes.
LOO-408 owns completion, LOO-324 native/account discovery, LOO-400 terminology.
CLI/PTY proofs pass; installed reconnect still fails. No migration. The legacy
Session lacks exact process evidence; driver success proves no provider exit.

## Storage footprint (LOO-390, 2026-10-07)

Jack Heart authorized autonomous investigation, cleanup and delivery. Evidence
and proposals: [storage footprint review](../../docs/reviews/storage-footprint.md).
`session_events` was 92% of a 2.6 GiB store: each streamed token was two rows,
also kept in `runs/*/events.jsonl`. PR #1474 (v0.13.9, installed October 7 and
measured): new captures keep 28% of rows and 72% of bytes, preflight validates
an exact store in place (1.0 s, no copy), WAL holds 64 MiB, two fingerprinted
migration backups remain (13.6 → 5.9 GiB). Old rows are not rewritten.
PR #1501 merged as `812d8cc55` (installation unproved): promotion appended every
release to `retained_published_sets`; a settled install now retains its fallback and the
prior one, then removes unnamed content-addressed binaries/bundles no live
process executes. Projected 11.4 GiB here by `du`; APFS clone sharing unmeasured.
Store growth stays unbounded, about 1.7 MB per capture. Next condition is Jack's
retention choice: system of record (SQLite or `events.jsonl`) and capture expiry;
then sizes in `lf home doctor`. 27.9 GiB was reclaimed October 6; Jack's 78.7 GiB
recording and 81-copy leads were gone before measurement. Left intact: 23 GiB of
unread legacy `traces`, `backups`, `lfd.db*`, `logs`; LOO-304's 40 GiB of
`/private/tmp` fixtures. Unknown ownership is not permission to delete. Test
`session_record` with `LF_*` cleared.

Earlier cleanup history: `66da3fdf7:wave/infrastructure/MEMORY.md` under
“Retained capture storage and autonomous cleanup.” Current direction is under Capture cutover.

## Project configuration and review direction (2026-10-05)

LOO-366's October 5 decisions are under Optional chapters and Task workflows;
source and configured acceptance remain unfinished. Infrastructure recommends
`code`; KRs and reviews remain. Earlier evidence, including v0.13.3 review:
`470382987:wave/infrastructure/MEMORY.md` under this heading.
LOO-326 and LOO-370 completed October 6 under the decisions below. LOO-367's
retry recorded boot witness 819671 but stopped on that same boot; preserve its
conversation and saved Flow. A later authorized restart, not unchanged evidence,
can establish old-provider death. No successful continuation is claimed.

## Release follow-through (reconciled 2026-10-05)

Jack Heart assigned Infrastructure responsibility through verified publication and
installed acceptance. Release owns execution/evidence; recover through its supported
operation and report blockers/next actions in the ongoing conversation. Merge or a
queued build is unfinished. Manual publication proves no unattended settlement.
Exact v0.13.5 installation/recovery evidence: [Release memory](release/MEMORY.md);
prior parent account: `86d0e5e6a2:wave/infrastructure/MEMORY.md`, this heading.

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

Jack Heart authorized LOO-377's autonomous repair; PR #1429 merged as c5dc238b0afb
with hosted CI passing. Restart serializes review launch, fences writers and
retains exact stop evidence for retry. Independent reviews remain protected.
LOO-370's replacement Flow finished after recovery; its Task completed October 6
under the opaque-root decision above.
Installed acceptance remains unproved;
LOO-373 owns the retained landing-placement reconciliation error.

A review's service, Session driver and provider are distinct process owners.
Record the service's exact Exec before child launch. Acquire the existing launch
lock before the driver fence: reservation/spawn must settle before collecting
owners, and reobserve the same Flow afterward. Retirement must fence review
writers and retain captures/native history without recording successful review.
Save exact process identities before signaling: native stop removes client receipts,
so an interruption before replacement otherwise destroys retry evidence. The
public restart fixture reproduces that boundary with no Session provider PID;
retry uses saved identities. Unknown ownership and independent reviews still block.

## Transient recovery (LOO-326, curated 2026-10-08)

Jack Heart approved existing Tasks using valid cached planning regardless of age;
known invalidation, removal, terminal state or ownership mismatch still blocks.
Apply this at restart, continuation and managed worker boundaries. New advice
still needs successful Linear publication before worker replacement. Preserve
observation age; a stale response is never fresh evidence.

Retry only the failed provider read. Artifact attempts use separate temporary
directories and retain candidate identity. Bound pipe collection as well as exit:
a descendant can retain stdout. Never retry publisher writes or invent checks.

Jack accepted read-only Exec `5f239ead-89f9-49c4-92c4-4c2f8b97ca94`'s historical
uncertainty. PRs #1413/#1435/#1445/#1455 shipped through v0.13.5. October 6's
supported completion of LOO-326 retained its unknown outcome and checkout; fresh
status confirmed done/current writeback. Acceptance grants no process control or
cleanup authority. Session closure and missing receipts cannot prove child exit.

Process receipts now use Exec identity and survive until a successful terminal
write. Pruning requires exact death and matching persisted outcome; PID reuse,
interruptions and failed writes retain unfinished identity. This prevention does
not reconstruct the missing receipt or establish the original loss cause.
Release's retained-landing incident still separates death evidence from re-entry
leases. Checkout cleanup independently reacquires its lease; #1435's delayed
child fixture proves a terminal receipt can precede descriptor closure.

#1459 repaired #1456's incorrect PR base using exact remote-tracking reflog
evidence, preserving feature history; missing evidence and foreign ancestry
still refuse. It shipped in v0.13.5, and #1456 merged October 6.
Release owns the healthy-transfer deadline and pending-version lessons in
[its memory](release/MEMORY.md). Detailed shipped proofs, acceptance command,
exact commits and contrary evidence remain at
`4e6770cbf550aa5f9d653650295481ced737a640:wave/infrastructure/MEMORY.md`
under this heading.

## Scheduled release accounting (LOO-285, source reconciliation October 2)

Jack Heart retained one execution per wake for frozen missed dues. Completion
still requires two adjacent original configured dues, two automatic executions,
at least one artifact publication, required verification and no manual repair.
Collapsed misses supply accounting coverage, never additional settlements.

At `fe36937fa`, main's Session/Exec/Home/Flow and release recovery are integrated.
Exact saved-candidate inspection permits replacement only with affirmative
unpublished evidence, preserving rejected candidate/proof and original due owner.
Unknown or partial publication blocks replacement. CI-repair children now retain
release target and checkout ownership. Reported focused passes and merged source
are not a full affected gate, configured publication or Task completion.

Separate durable facts: physical cron exit, product settlement, publisher stages,
public artifact proof and dated repair ownership. Assigning a closed opportunity
an owner does not resume it. Same-Home continuation is implemented locally;
old-Home authority is never transferred by attribution. Child-held mutation locks
and exact checkout leases protect different scopes; cleanup must independently
reacquire after dropping the parent's shared handle. Parent death and elapsed
wait grant neither mutation nor deletion authority. Main-reset/stash helpers can
replace a held lock inode; explicit source selection avoids that failure class.

Same-Home coverage, overlap, retry timing and interruption proofs remain at
`c418953634bd101f51878d2be2b40fb3facafabd:wave/infrastructure/MEMORY.md`
and its referenced commits `a60ac0281`, `02d6b3c00`, `95643bd50`, `d60d254ef`.
They retain original ownership, physical failure, frozen coverage, candidate,
caller bytes and child-held locks without republishing or double settlement.
Synthetic proofs do not establish configured automatic settlements.

PR #1457 merged installer isolation: candidate preflight and public installer
smoke use disposable Linux containers, checking selected CLI bytes instead of
the entry gate. Native macOS smoke stays separate. Prior simulated checks prove
neither container/public acceptance nor automatic settlement.

October 6 scheduled receipt cron_a4b8b11b2a534bf99d183e677f2a6871 failed after
recovery cron_5d930c31c7ae4a118f6b93774496991c passed continuity but the scorecard
emitted retired product/task-loop-trust. Accepted chapter commit 42451654e removed
that contract; remove its hardcoded producer output, preserving strict metric
validation and lifecycle rows. Installed CLI with the repaired checkout reports
35 rows; twelve focused tests pass. Original failed receipts and repair ownership
remain unchanged. Jack Heart authorized publication; the operator owns subsequent
release recovery after LOO-382. This manual proof supplies no automatic settlement.

Release's September 28 incident proves entry points need operation-level recovery:
an agent reported failure successfully, producing a misleading green cron receipt.
Jack's later steer records v0.12.24 publication/install and skill-to-Flow activation
at unchanged 10:00. That supersedes the child's dated pending-activation evidence,
without proving this accounting branch is installed or either qualifying outcome.
Release-specific detail remains in [release memory](release/MEMORY.md).
October 5 child evidence records repaired installed jobs using the machine gate
at unchanged 09:00/10:00 schedules. Ownership remains Infrastructure; the child
is not registered. History exposed three unresolved opportunities with unknown
timezone provenance, zero executions and no qualifying pair. Schedule repair
cannot backfill that evidence or count as an unattended settlement.

All 36 telemetry failures, including the original 35, remain dated counterevidence
from September 24. The missing `agent_turns` diagnosis is historical: integrated
scorecard source consumes SessionHistory. The isolated doctor/scorecard Flow
regression proves Recovery cannot replace natural Scheduled evidence. No installed
pass or accepted Intelligence handoff is established; reproduce current failure
before commissioning duplicate analytics repair. Required headless Desktop/public proof and
actual automatic settlement observations remain outstanding. LOO-357 retired
the UI-host prerequisite; PR #1441 removed its scheduled validator requirement. The working plan
owns remaining implementation; no production release, install, schedule change,
Home transfer or review completion is authorized by local reconciliation.


On October 4, Jack Heart waived the interactive demo and authorized landing
PR #1419. This supersedes the earlier review requirement for this delivery;
installed acceptance and the two automatic settlement observations remain
unproved. The branch’s prior memory curation remains in Git history at
`e2eccdd257f0f3e2cf34db7583f7309eb0084394:wave/infrastructure/MEMORY.md`.

## Operator acceptance and account direction (2026-10-04)

Jack Heart closed LOO-295 and LOO-342 on actual machine usage; both read `done`.
LOO-367/285's retained Flow progress and PR #1283 established continuation;
installed routing established the one-Home outcome. Jack declined retired Ask
work. Exact acceptance and preserved viewer artifacts:
`35e759aaf4:wave/infrastructure/MEMORY.md` under this heading. Legacy retirement
below supersedes the then-live process handles.

LOO-324 now follows stop-bundling's shared/isolated model. Jack clarified that
cross-binary history means discovering native Session records across folders,
not duplicating transcripts or reconstructing new conversations from replayed
context. Reuse native history and retain Loopflow Session identity. A usage
threshold such as 90% redirects future launches only; actual exhaustion triggers
automatic shared failover. Isolated agents retain their own account/fallback.
The updated Linear brief owns the current scope; stop-bundling's branch evidence
does not yet establish shipment.

Renamed from `systems` July 8; current schedules supersede history.

The 2026-09-30 [LOO-298 decisions](#data-model-and-performance-decisions-reconciled-2026-09-30)
supersede older Run-owner, historical-import, pinned-development-Home and
demo-before-landing directions for this cutover. Earlier incident observations
remain evidence of their own versions, not instructions to restore those owners.

## Installed worker recovery

LOO-295/292 closed; LOO-373 retains placement repair. Evidence and unapproved
batching: `10ca6bd39:wave/infrastructure/MEMORY.md`, this heading.

## Optional chapters and Task workflows (2026-10-02)

Jack Heart's October 2 direction separates ordinary work from optional chapters:
LOO-366 owns Project ensure and coordinated resets; LOO-367 owns Task admission
and completion. Projects need no chapter or default Flow; Tasks need no managed
Flow or delivery workflow merely to exist and record an outcome. Preserve
unfinished work and unresolved delivery. Reads never provision Projects. This
supersedes mandatory-chapter assumptions, without proving either KR or authorizing
a repository reset.

Jack Heart's October 4 reviewed direction selects an explicit Wave Project
configuration field and exact-ID ensure; no name matching or candidate selection.
Chapter creation requires planned KRs, with new Task admission afterward.
Unreviewed backlog survives rotation until explicit disposition. These decisions
supersede the earlier status-selection and automatic-expiration proposals.

Jack Heart selected one shared local binding across checkouts on October 5
(`f092d63a-a152-4920-af81-d676a576f694`). October 6's SQLite decision below
supersedes its file layout; stale files and settled receipts never select a Project.
Review: `e04c83513573cc09883fb2b92ebdb63e06a22c95:scratch/keep-every-wave-ready-for.md`.

October 5: Jack Heart authorized one historical name-only correction
(`5419b87c-bfec-4f42-8914-021483249895`), preserving evidence and later conflicts.
Empty Flow is permitted. Atomic ingestion retains ages, accepted Initiative
ownership and foreign/unmapped plans; cold detail resolves configured ownership.
Rotation accepts confirmed readbacks; reteam requires exact Team/full issue
readbacks and preserves newer facts. Deleted identity shortcuts and generic
writers stay deleted. Source-only evidence: `d4d77d8e22f4244ad83027ba9c85bb8644a622e6`.

October 5: queued SQLite workers own Wave guards through commit, including after
caller cancellation; one shared writer preserves that lifetime. Multi-Wave
operations lock in stable ID order and reuse held guards. Cold detail re-reads
ownership under the lock; delayed absence compares the queried revision/age.
Inspection follows UUID across renames. Cancellation proofs exclude competing
reteam preview until commit; separate apply fixtures cover cached recovery and
expansion/move/narrowing response loss. These supersede the unrepaired-reteam
finding; the registration and rotation proofs below establish later boundaries,
not installed acceptance.

Exact-ID rotation supersedes the name-selector failure retained at
`42e6c2706b1:scratch/keep-every-wave-ready-for.md`; cross-process recovery remains
unproved. Desktop activation
must preserve #1447's cached plans and independent Session reads, outside polling.
#1446's ongoing procedures retain started-Task follow-through; reads never provision.

October 5 admission, registration and rotation evidence is preserved at
`fc0bb97f7e8e0ad3f9520daf12e7e76b2e4f2ee1:wave/infrastructure/MEMORY.md`
under this heading. LOO-406 deleted the obsolete registration writers; their
historical proofs are not the common-writer acceptance. Retain stored workspace
membership, Task/PR identity, unset Started and issue-reported ownership. Ordered
Wave and checkout locks survive queued writes and caller cancellation; Git leases
remain separate. Unreviewed backlog is never automatically canceled.
CI-repair entry, public crash/retry and configured readiness remain unproved.

Jack Heart's October 5 comment `e4dafef5-2a87-4359-818a-3770356ba850` authorizes
Intelligence repair of Project `999bdbdd-c045-41a6-8ffc-a97c4a40b0b3`, preserving
KRs, Tasks and identity without competing creation or repository rotation. That
exact ID supplies selection; active-Project adoption proves no Backlog activation
or installed repair.

The archived file-binding design is superseded by SQLite selection below.
Its output-handle leak remains unresolved; seeded ensure recovery proves no crash
recovery. Exact evidence remains in the preserved October 5 notes above.

October 5 rotation retains exact-ID input, selected issue membership and settled
history. Whole-input preflight includes accepted facts and legacy conversion;
all pairs reserve before provider writes. Creation intent survives retry; unfinished KRs stay editable.
Before switching, reclassify new work; afterward reconcile only saved selections,
preserving later starts and external moves. Planning updates preserve names,
summaries and unrelated text. Queued membership/conversion writers retain guards.
Operation fixtures cover partial settlement and both sides of the switch;
CLI crashes, Desktop and configured Intelligence acceptance remain open.
Earlier counterexamples: `fc6df439424bd341ec3cd8182c19b13ed45cffd7:wave/infrastructure/MEMORY.md`.

October 6: Jack Heart authorized SQLite selection and LOO-382's stream, deleting
the YAML selector and `ProjectPreparation`. Dependency `e887a21c1` entered through
`66a3daa86`; its dirty work stayed separate. Explicit mutations import YAML once,
retaining bytes; later edits select nothing. Creation selection/settlement commit
together. Accepted facts retain age and partial inventory. Readiness uses those
facts, transitions and the actual activation Exec; unfinished is unknown. Both
surfaces share activation; absent Flow hides its template. Watchers never activate.
Committed frames replace refresh callbacks. Only the live per-Wave command shows
“Preparing Project…”; cached plans survive it. Current transport errors precede
retained errors; Retry clears transport feedback. Public CLI/crash, mounted and
installed Intelligence acceptance remain unproved; source grants no installation
authority. Release's operation-entry lesson still applies. Earlier detail:
`c99baa3baa23d324b013a03ea02cdcbc8953c2aa:wave/infrastructure/MEMORY.md`.

Jack Heart selected Loopflow Desktop and Work-named observation on October 6,
then accepted the reviewed source/naming direction with “lg2m.” Wire/cache bytes,
selection keys and identities stay unchanged; Work remains Wave/Project/Task.
Workspace can be non-Git; TaskWorktreeSnapshot means Git placement. Loading is
WorkReadingStatus, distinct from WorkStatus. Review and the two-reader proof at `457e65d6b` do not establish public CLI
crash recovery or configured Intelligence repair. October 6 gate stopped below
the 32 GiB disk reserve after supported recovery; static checks passed, product
suites did not run. The CLI/crash suite remains absent. Release's entry-point
lesson applies: operation fixtures cannot prove dispatch/recovery. Desktop's
required boundary is headless app/view checks; mounted judgment is optional.


## One migration draft per Task (LOO-344, 2026-10-01)

Jack Heart selected one editable draft per Task and immutable released SQL.
Mechanics: [MIGRATIONS.md](../../rust/loopflow/src/store/MIGRATIONS.md).
Unshipped branch evidence: `986be7988:wave/infrastructure/MEMORY.md`.

## One main Home (LOO-342, curated 2026-10-07)

Jack Heart approved the one-Home cutover: ordinary commands, Task workers and
Flow steps use installed `lf` and `~/.lf`; explicit `LF_HOME` experiments need a
fresh directory after schema changes. PR #1381 merged as `6c73356074c4`; installed
v0.12.31 acceptance passed and LOO-342 is done. Current mechanics:
[CLI docs](../../docs/lf.md#use-one-machine) and
[Machines](../../docs/architecture/machines.md#one-main-machine).

Jack authorized legacy retirement on October 4. All 37 identified processes
exited after SIGTERM; retained databases and exact signal/path receipts remain
under `~/.lf-retired/20261004T161815Z/`, earlier snapshots under
`~/.lf-retired/20261002T191224Z/worktrees/`. Main identity stayed unchanged.
Release recovery, acceptance and retirement details:
`173d649cf:wave/infrastructure/MEMORY.md` under “One main Home.”

## Worktree listing and fenced dispatch (LOO-375, reconciled 2026-10-07)

PR #1456 shipped in v0.13.6. Installed measurements, batching and limits:
[report](../../scripts/benchmarks/wt-list/README.md),
`07bbd801e:wave/infrastructure/MEMORY.md` under this heading.

- Store opens validate ledger/schema, leaving full integrity scans to migration,
  doctor and install. Keep the Session fence outside runtime waits; OpenCode's
  fenced post remains bounded at 10 s. Exec observation waits 15 s, then warns
  and runs unrecorded. Exact measurements and counterexamples remain in the
  `6448e3c9e7:wave/infrastructure/MEMORY.md` archive; current timings live under
  `<Home>/perf/`.
- Jack Heart retired further performance acceptance. Only supported settlement
  preserving the interrupted research Process remains; LOO-408 owns it after installation.
- **Install preflight/promote read the OS account's Home whatever `LF_HOME`
  says.** Tests running them are container-only installation proofs.
- Task decisions no longer depend on historical execution; checkout cleanup
  still retains live or unknown providers. LOO-408 owns the installed proof.

## Environment variables (LOO-341, reconciled 2026-10-07)

Jack Heart's audit lives in [Environment](../../docs/architecture/environment.md).
`LF_HOME` selects the machine's data directory and `loopflow.db`; shared Exec
names drive shell/tmux clearing. Earlier branch evidence remains at
`c31279995a4ea0eec09c053e39c2f71a81d26034:wave/infrastructure/MEMORY.md`.
LOO-370's October 6 completion above supersedes this section's pending-delivery
claim; it establishes no physical capture conversion.

## Task worktree membership (LOO-358)

Jack Heart selected checkout membership plus explicit binds, supplied by Rust
for status and Desktop. Descendants, closed history and missing checkouts retain
membership; usage and control authority do not change. Every Flow naming the
Task is equally its work. LOO-408 separates Task decisions from execution and
retains conservative cleanup. Earlier details:
`6448e3c9e7e519585378feaffe04606bc1b55d3e:wave/infrastructure/MEMORY.md`
Current contracts: architecture reference.

## Synced planning integration (LOO-334, 2026-09-30)

The landed cutover supersedes the bridge; planning preserves Task/PR identity,
not execution progress. Detail: `57ac8b09fbbff3f3b7c82c00ecf9032f0cb792b1:wave/infrastructure/MEMORY.md`.

## Planning and launch preservation (curated 2026-10-02)

The September 28–29 details remain at
`3a219561ad029224aed4692d4ce23d6f17cad350:wave/infrastructure/MEMORY.md`
under this heading and in its linked pre-curation history. Current one-Home,
Session and LOO-406 unified-plan decisions supersede development-store routing,
intermediate migration, Run owners and provider-authority selection. Records prove no configured acceptance. Repository-authored Wave identity and
preservation obligations below remain; no-remote policy/outward definition sync
were unresolved, not authorization to restore old writers.

LOO-334's durable constraints survive: presence, freshness and execution eligibility
are separate; omission is not deletion; acquisition time is not provider revision.
Partial/unknown-revision webhooks invalidate rather than invent complete facts.
Preserve removal evidence across lookup races. Project revisions cannot order
independent Initiative/Team relations; contradictions require explicit repair.
Repository-alias repair moves normalized planning atomically, without combining
conflicting observations. Explicit relocation and configured Linear completion
remain unproved. Historical chapter membership requires frozen evidence, not
reconstruction from current normalized entities. Lost completion responses require
provider readback; archive acknowledgement never proves KRs or transferred Tasks done.
Cached-Task outage admission belongs to LOO-326's current review, not these notes.

The launch-hardening incident, unknown initiator, failed retry and unresolved
configured continuity remain at `b3cd894f3:wave/infrastructure/MEMORY.md` under
this heading, with its original design link. Selected-store absence proves no
deletion. Preserve failure, executable/data ownership, native identity and pending
decisions across retry; prohibit timestamp-based store merging. Prepared identity
and window output prove no conversation access.

LOO-321 / PR #1308’s superseded isolated-store design and unproved configured
continuity remain at `b3cd894f3:wave/infrastructure/MEMORY.md` under this heading.
Database isolation never isolates provider or checkout effects. Preserve saved
Task/PR/checkout identity; explicit replacement owns new definitions. Current
mechanics and ownership replace the archived store-routing and command names.

## Managed account identity (LOO-339, curated 2026-10-08)

Jack Heart authorized landing and a patch release; shipment remains unproved.
LOO-340 owns account state/routing and LOO-338 naming. Compare email and per-user
subject, not workspace identity; usage proves no login. Unavailable identity is not
rejection. Native refresh, sole browser ownership and installed acceptance remain
unproved; no installed repair is authorized. Exact decisions, isolation requirements
and proofs: `32607f1d2:wave/infrastructure/MEMORY.md`, this heading; mechanics:
[subscriptions](../../docs/subscriptions.md).

## Account auth consolidation (LOO-320, curated 2026-10-08)

Jack Heart approved delivery, excluding cross-account continuation, native refresh
coordination and headroom ranking. Native OAuth/callback ownership, side-effect-free
cached inspection and usage-window provenance: [subscriptions](../../docs/subscriptions.md).
Selected account precedes native Session discovery. Missing windows or reset success
prove no capacity; printed URLs and cached login prove no current OAuth success.
Browser login without pasted code, first connection, remembered Linear profile and
live Claude/Codex windows remain unproved; Claude returned `invalid_grant`.
No branch binary may migrate the installed Home; fixtures prove no installed outcome.
Exact constraints and archived evidence: `b2228bce8:wave/infrastructure/MEMORY.md`
under this heading, retaining the October 4 source and original caveat references.

## Task deletion and command ownership (LOO-305, curated 2026-10-08)

Jack Heart selected provider/local deletion, command consolidation and saved-Flow
delivery; execution settlement stayed deferred. Decisions, proofs and the incident:
`423ff2ec2:wave/infrastructure/MEMORY.md` under this heading. Current mechanics:
[CLI](../../docs/lf.md), [planning](../../docs/architecture/planning.md).

Fresh ownership authorizes deletion; acknowledgement or explicit trash confirms it.
Missing membership proves neither. Preserve terminal times, Done outcomes, PRs,
Git and positive confirmation across stale reads. Completion and planning have
separate writers; retry retains merged-PR evidence and original completion time.
Planning-only creation needs no checkout or agent. Allocation failure preserves
created identity; upstream tracking never defines checkout identity.

The deletion demo removed LOO-299–302 but migrated the installed Home and broke
its older CLI. Jack forbade branch-binary access and promotion: use disposable
Homes without inherited authority. Removal is not termination. Exact incident:
`abd039b2a818669c43e7c189f6a37382335639f2:wave/infrastructure/MEMORY.md`.

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
TESTING.md owns fixture/disk isolation; no branch binary may migrate main.

## Data model and performance decisions (reconciled 2026-09-30)

Jack Heart selected one SQLite owner per object, direct Processes and compiled
Flows. September 30's counterexamples, delivery decisions and draft inventory
remain at `72c58cac5d2f4f1700186379e0645446341b1373:wave/infrastructure/MEMORY.md`.
Current contracts live in the architecture and CLI references.

**One client, one main Home.** Jack reported manually moving active Tasks to
`~/.lf` and retiring the pinned development Home; that proves no conversion.
No fleet compatibility, historical import, old formats or intermediate drafts
are required. Preserve current Work/links, account routes and resumable Sessions.
Branch checks use disposable stores with inherited authority removed; no branch
binary writes the installed store. Released SQL remains immutable.

**Merge and conversion have different proof.** LOO-298's checklist remains at
`ab901f1f1:scratch/remaining-work.md`. Fixtures prove no configured provider/Desktop
continuity. Before conversion, quiesce writers and launches, preserve a consistent
SQLite/filesystem backup and matching executable, rehearse the exact candidate,
and verify before reopening. The private-copy converter's live sidecar reads
plus SQLite backup are not atomic. Preserve native IDs, pending reviews, selected
captures and manually transferred Tasks; import neither old turns nor driver authority.

September 29–30's detailed owner/compilation decisions and superseded schema
proofs remain at `fe07245a3614334aea71dc40e802b54b47ccaf17:wave/infrastructure/MEMORY.md`
under this heading. Current vocabulary supersedes Exec/FlowSession names:
one LfProcess, one durable LfSession conversation and one compiled
Flow graph with step Processes. History has no independent lifecycle. Skill steps
use ordinary `lf skill`; operations use their own commands. Parent means Process
ancestry, matched to Session/provider generation and origin, never Task or Flow
authority. Replacement preserves proven historical ancestry. Loop passes are
node/iteration positions, not child Flows; retry keeps the pass and iteration
advances its counters. Captured input belongs to Session history and names its
Process; deleted RunId and intermediate conversion owners stay deleted. Older
proofs establish no final-frontier conversion.

- **Flow decisions are typed results** of the selected successful turn,
  modelled on PydanticAI and Jev: the step declares its output schema. The
  in-turn decide and route commands are removed; Jack said the command "felt
  wrong". On 2026-09-30 Jack selected Blocked with a required reason in that
  structured result; the blocked command is removed. A keyed Ask returns
  feedback to another turn of the same conversation without moving the cursor.
- **Bind** is write-once null to Task, allowed on done Tasks, and sets Started
  once. Jack selected prospective usage attribution for now on 2026-09-30;
  earlier usage retains its recorded owner. Keep this one read-time choice for
  Intelligence to re-evaluate; do not invent a mid-turn token split.
- Every denormalization has a validator or is deleted. Method for any model
  review: derive the user's objects and APIs from the product first, then check
  the infrastructure for hops.
- **Naming remains a choice.** Jack kept `session_events` → `agent_events` and
  the earlier `exec_events` rename proposals open. `run_events` is now deleted;
  Exec results live on `execs`, so there is no current journal table to rename.
  This observation neither closes the proposal nor authorizes a new event owner.
- **Optional chapters, explicit selection.** Jack Heart's October 4–5 direction
  supersedes this cutover's status-based selection and backlog expiration: one
  shared local binding selects each Wave's Project; chapters and Flow are optional.
  Chapter creation requires KRs, and backlog survives until explicit disposition.
  No Chapter table, second deployed client or distributed transaction is implied.

LOO-298's gate and +6,202-line diff:
`c418953634bd101f51878d2be2b40fb3facafabd:wave/infrastructure/MEMORY.md`.
Full gate, configured acceptance and installed conversion remained unproved.
Preserve ID ordering through Session projection, use dedicated PR stacking writes,
and resolve automatic checkpoints through the same Work binding as execution.
Provider stubs must contain conflict-agent launches: one bad fixture launched
real credentials whose effects were not audited. Dense CLI timing and configured
continuity remained unfinished in that dated evidence.

Jack Heart retired numeric performance targets, soak and deeper optimization
on October 7. LOO-291/300 evidence and instrumentation:
`07bbd801e:wave/infrastructure/MEMORY.md` under this heading and in
[scripts/benchmarks/desktop-performance](../../scripts/benchmarks/desktop-performance).
Different recording windows prove no causal improvement; PTY echo and next draw
are not glyph presentation. Those historical gaps are not open acceptance requirements.

The earlier S5 active-PR resolver left landed branches unbound. That is historical
failure evidence, not binding policy: current Session rows own attribution and
write-once bind permits done/landed Tasks. Preserve current ownership through the
one-machine conversion; discarded historical attribution needs no importer.

The superseded source-install staging incident remains at
`1452f58d3:wave/infrastructure/MEMORY.md` under “Data model and performance
decisions”; current published-installation policy governs verification.

## Continuation, delivery and authority lessons (curated 2026-10-02)

The detailed September 23–25 observations, historical schemas, test counts and
counterexamples remain in [the pre-curation memory](https://github.com/loopflowstudio/loopflow/blob/d281370191844294ce0ad877752f2aa6a6402282/wave/infrastructure/MEMORY.md#continuation-and-recovery-lessons-curated-2026-09-25).
The September 30 model/cutover decisions above supersede Run, Ask, Project-cursor
and private-store mechanisms there. Archiving detail does not establish remaining
acceptance or authorize deleting execution history.

- LOO-295's October 4 operator acceptance above supersedes this section's
  unresolved continuation claim. LOO-296 retains broader restoration; historical
  manual Flow/native-Home corrections and ambient-account/lease gaps remain in
  the pre-curation memory, not proof of automatic recovery.
- Capture all alternatives and human boundaries before execution; recover from
  saved definitions after sources disappear. Invocation identity fences late
  results independently of reusable versions/generations. Consume actual
  successful completion evidence under the exact claim. Cursor settlement does
  not prove external effects happened once. Serialize feedback/reopen writers,
  persist completion before teardown and preserve published native identity.
  A finished Flow does not complete its Task. Prove driver and native handoff,
  not only reducers, including another executable first on PATH.
- Unknown custom/queued/nested history stays unresolved with bytes/order intact.
  Inspection does not recompile or launch it. Causal ancestry, age, a missing
  listener, finished launcher or absent display cannot grant process authority
  or prove provider death. Preserve original dispositions and exact process
  receipts, including hashed executable names and PID-reuse rejection.
- One Task execution snapshot feeds status/actions/roadmap. Current execution
  evidence wins over dirtiness or next-launch configuration errors. Missing
  observation must not become idle capacity or a duplicate worker; the earlier
  LOO-293 duplication came from split readers and that inference. Catalog joins
  use stable identities and minimal schemas; current hierarchy filters do not
  rewrite historical attribution. Fixtures isolate all ambient authority and
  stop only their own identified provider children.
- Preserve authored PR copy plus the managed Task block; publication alone does
  not request settlement. Draft opening preserves readiness; promotion records
  state only after provider success. Exact copy must survive release re-arming.
  Same-SHA pending/passing/merged changes are progress without another commit.
  Failed repair must surface its real blocker rather than loop indefinitely.
- Queue cancellation is distinct from disabling auto-merge: gh 2.101.0 could
  return success without dequeueing. Reobserve authoritative queue state before
  replacing a head. Canceling an async waiter does not stop its blocking child;
  keep the actual checkout lock and effect ownership until descendants finish.
  Local landing tests did not establish hosted interruption/permissions or
  orphan cleanup; green-unmergeable PRs and the old Etude -c discrepancy remained
  unresolved in those records. PR #1289 proved deleted remote-branch recreation
  only. An empty lease must still reject a branch created after absence was read.
- Rotation preserves started identities and freezes the boundary's membership,
  metrics and evaluation time. Failed fresh reads cannot justify retirement;
  retries consume the frozen boundary and verify uncertain writes at the provider.
  Deduplicate stable Work IDs; renamed PM labels do not rewrite captured history.
  LOO-278's integration did not transfer its registered checkout/PR ownership.
  Historical archives remain outside live architecture discovery.

LOO-286/LOO-287's broader configured continuity obligations are retained where
not superseded by the accepted cutover: concurrent starts, real provider death
and same-cursor replacement, late-result rejection, helper non-authority, and
exact Desktop close/reopen continuity. Their seeded stores, missing executables,
sleeping children and isolated tests did not prove those outcomes. Old migration
counterexamples dropped controller-only Task/Project positions; the newer
current-state cutover's explicitly accepted scope governs disposition, not a
retroactive claim that those proofs passed. Finite Project Runs and generic
Wave/Project execution symmetry are superseded, not future implementation mandates.

The deleted lease/liveness stacks (a7044e2b5, 5f7f66833) and repeated prompt-fallback
add/delete/restore cycle show why failure must be repaired in the operation.
The full operational cause of run_liveness removal remains unknown. UI projects
shared evidence and triggers operations; it is not another scheduler or synthetic
controller. Historical automatic-trigger/default-wait proposals need current
selection, not automatic resurrection. Current delivery mechanics live in
[delivery documentation](../../docs/architecture/delivery.md); TESTING.md owns
populated migration and clean-PATH proofs. Refresh exported skills after changes;
Wave learning stays with its identified owner, never miscellaneous .lf notes.

## Prompt reduction boundary (2026-09-24)

`719226ef4:wave/infrastructure/MEMORY.md` retains the evidence and LOO-287's open
proof. Intelligence owns prompt assembly; identity, eligibility and consumed launch
evidence remain separate.

## Installation and command scope (curated 2026-10-02)

LOO-292's installation acceptance closed below; LOO-287 retains command-scope
reduction. Earlier installation evidence:
`24db5d4d68b1e4a5efe11781712a321b564e01ca:wave/infrastructure/MEMORY.md`, this heading.

Pinned artifacts activate only through promotion. Candidate-owned recovery and
exact-store preflight precede startup authorization. Verify matching macOS app bytes;
preserve launchd labels/logs/custom directories without source WorkingDirectory,
source maintenance or recursive Python refresh.
OS-account Home bypasses HOME/LF_HOME: use disposable accounts/containers and verify
first-activation failure/recovery. Simulated launchctl proves no timing; asynchronous
copy caused mismatched bytes/segfault. Ubuntu 24.04 passed where Debian lacked GLIBC.

Distinguish non-Git folders from Git errors; never invent repositories or change
routes. LOO-287's ordinary-folder/provider-completion and target-first resolution
remain unproved across consumers. Repository setup and receipt redesign remain
unselected; a phase alone is not durable advancement evidence.

## Installation and checkout closure (LOO-292, 2026-10-04)

Jack Heart closed LOO-292 on machine evidence; proofs and unresolved cadence/app
checks: `86d0e5e6a2:wave/infrastructure/MEMORY.md`, this heading. Install owns
artifacts, sync owns checkouts. Reload stopped unexplained redundant downloads;
reinstall does not heal a truncated entry gate.

## Shipped history

Historical delivery records:
`4b4ad05186e85ba8a48924a6294ba9ec2e6df65f:wave/infrastructure/MEMORY.md`,
this heading and its archive link. Current contracts supersede history.
Cron continuity judges the latest due against exact scheduled receipts; manual
runs prove no firing, and old gaps do not keep later telemetry red.

## Gotchas

- **Desktop gates are headless** (rechecked at `bc6d11661`). `--swift` runs app/model/view checks; `--loopflow` builds app/UI runners. `scripts/test.py --all` excludes the explicit-only `--ui-host` diagnostic. The historical runner hang remains at `bc6d11661:wave/infrastructure/MEMORY.md` under Gotchas; changing the gate proves no repair of that hang. Compile-only and skipped checks prove no interaction behavior.
- **Run `cargo test` to completion before trusting a green-looking suite.** A failing lib target makes cargo skip every later target, so lib failures mask bin failures.
- **Rust compilation does not validate SQLite column names.** Runtime SQL whose shape depends on a released schema must be shared with a behavior test that prepares and executes it against the materialized migration head. Epoch Work ownership is three exclusive foreign keys (`wave_id`, `project_id`, `task_id`); generic kind/id belongs to explicit routes such as synchronous cross-Work questions, not to Epochs.
- **Source history must reconstruct every applied release frontier.** Preserve
  an unknown store frontier and its old binary; match embedded SQL against the
  ledger before ratifying history. Draft-bearing candidates cannot promote;
  schema-complete exact-frontier repairs need no migration. The July 20 recovery
  and #1123 evidence remain at `ec126964d:wave/infrastructure/MEMORY.md` under Gotchas.
- **Tests must survive draft migration materialization** (learned 2026-07-21).
  Release-equivalent Rust tests delete ordinal-free drafts and compile the
  generated canonical batch. Test fixtures resolve migration SQL by its draft
  marker through `migration_sql_for_test`; an `include_str!` pointing directly
  at `migrations/drafts/` passes locally and fails the release tree at compile
  time.
- **Installation tests need OS-account isolation.** Jack Heart’s October 5 steer forbids the three LOO-370 host checks named in TESTING.md: getpwuid bypasses HOME/LF_HOME. PR #1444 supplies disposable-account proofs; until integrated, isolated CI owns them. Ordinary fixtures also scrub inherited LF_* authority.
- **Concurrent editing corrupts a file; concurrent rebasing corrupts history.** Two drivers sharing one worktree shared its `rebase-merge` state dir: conflicts resolved themselves between commands and `done` advanced 6→22 with no `--continue` from the losing session. Check for a live agent before working — or rebasing — a wave worktree; the driver that owns the worktree owns its `.git` sequencer.
- **Linear Project UUIDs survive renames; derived slugs do not.** Project content
  lives in Linear and the local SQLite snapshot, with no `projects/*.md` cache.
  Use stable IDs when reconciling current names with captured historical plans.
- **Environment configures a process; it must never decide what the process is.** An earlier runtime chose between booting a listener and being a resident from inherited environment, so a promoted wave could attach to its parent's listener with the parent's token. The current `lf wave` surface keeps that role explicit.
- **Current PM truth and durable Work history have different lifetimes** (learned 2026-07-21). A terminal Project omitted from the current PM snapshot can still own non-terminal historical Task Work. Wave reads must render the current PM hierarchy and classify the stranded Project/Task separately as Wave-owned degraded evidence; they must not fail the whole join, delete history, or synthesize a PM Project. Preserve stable identity when the historical checkout is absent. LOO-305 removes the old local-only `work abandon` recovery command; inspect retained facts with explicit `lf task status <task-id>`. Missing Project evidence alone never authorizes provider deletion.
- **Terminal Task state and current PM routing are authorization boundaries**
  (learned 2026-07-21). An open Linear issue, inherited direction, or sibling
  completion is evidence, never permission to reopen `Done` or `Abandoned`
  Work. Recovery from abandonment requires explicit User authority. When
  Linear moves an issue, historical Task Runs retain their evidence but lose
  automated PR and completion authority; fail closed before side effects and
  preserve the full Work, Run, Steer, and PR history for remediation.
- **Historical continuity blocked daily telemetry** on August 23. LOO-241's
  original eight-gap failure remains at `ec126964d:wave/infrastructure/MEMORY.md`
  under Gotchas; current continuity judges the latest due interval, as recorded
  under Shipped history. Fresh failures belong to existing work, not duplicate Tasks.
- **Release orchestration and publication are separate evidence.** August 23's
  unpublished v0.12.14 despite cron/workflow success remains at
  `24db5d4d6:wave/infrastructure/MEMORY.md`, under Gotchas. Judge product state;
  keep same-tag recovery singular. LOO-261 owns clean-host candidate validation.
- **Incomplete release synchronization still consumes caller edits**
  (reproduced 2026-08-23). The scheduled retry left `main` clean after removing
  two pre-run Infrastructure memory edits. LOO-266 owns preserving the caller
  branch, index, and working bytes across every release exit; do not file a
  second repair Task for later instances of the same failure.

## Durable model lessons (curated 2026-10-02)

[Earlier model and planning records](https://github.com/loopflowstudio/loopflow/blob/d281370191844294ce0ad877752f2aa6a6402282/wave/infrastructure/MEMORY.md#model-design-settled)
preserve their exact decisions and implementation history. Current objective,
accepted optional-workflow direction and authored release schedule supersede old
nightly/weekly cadence, daemon-chat and mandatory Project assumptions.

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

LOO-406's October 8 direction selects local Wave/Project/Task ownership with optional
provider mappings and explicit ingestion of Git-authored definitions; that cutover
remains unfinished. Historical label migration
and old command names are archived, not current instructions. KRs are explicit
judgments, never inferred completion. Preserve unrelated provider associations;
ambiguous moves remain diagnoses. Standing frontier plans need not acquire an
invented completion date. No fourth user-facing planning noun was selected.

## Earlier follow-ups (reselect through the accepted chapter)

Historical suggestions remain at
`d281370191844294ce0ad877752f2aa6a6402282:wave/infrastructure/MEMORY.md#earlier-follow-ups-reselect-through-the-accepted-chapter`;
they authorize no current work or retired owners. PR #818 resolved rebase-efficiency
follow-ups. Measure command drift, avoidable rebases and post-land repairs before
tuning policy; the synthetic harness remains unbuilt. Jack Heart's July 6
“up/down 5ths” referent remains unresolved and deferred.

## Direct invocation and large inputs (curated 2026-10-08)

Jack Heart's LOO-429 decision supersedes 428's argv/fixed-system constraints:
all assembled context in one system/instructions file, short user trigger, no
split, fallback or lf-side refusal wording. PR #1498 owns transport/oversized
acceptance. Claude readback proves neither plan mode, cmux nor the combined
candidate; shared transport proves no native resume. Jack authorized 429 landing
despite gaps, then 428 queue/landing October 8.

Jack approved setup-free Codex coexistence: temporary native capture profile,
bounded wrapper-trust parser probe, unchanged saved config/trust. Normal exit
removes the profile; interruption may leave it unselected. Codex 0.160.1 composes
profile/CLI hooks; two CLI tables replace each other; config bypass is ignored.
Native PTYs prove capture, launch hooks and reconnect. Fresh reconnect hooks
are absent even without lf; earlier assertions reread launch receipts. Actual
cmux/installation remain unproved; preserve live Sessions. Launch receipts prove
neither fresh reconnect hooks nor host tracking.

Flow output prints position/name and messages; verbose adds accounting/INFO,
never prompts. Started commits with Flow registration. Jack removed `--tui`
October 8: terminal defaults interactive, `-i` forces it, `-b` stays headless;
`default`'s override and #1500's stale forwarding are removed. Dated gate/PTY,
website and static proofs: `9ab5b6a52:wave/infrastructure/MEMORY.md`, this heading;
CI owns the full materialized matrix. LOO-422 owns OSC 7501 simplification after
adoption. Preserve caller checkpoints/common invocation loading. Codex rejected-turn
recovery remains unresolved; homepage capture follows installation.

LOO-420: Jack Heart's October 8 comments `5f149330-5f5d-4a71-bb05-289b13fe2d94`
and `76407cd0-b271-45e7-8953-62abe7f9df2b` correct the overexpanded scope:
ordinary installed-skill invocation through `lf audit` / `lf audit -a codex`,
matching help/list, native same-harness execution, translated ports, exact arguments,
assets and declarations. Builtins stay inline; `--agent/-a` remains. Busy-terminal
injection, competing writers, queues and generic engine recovery are not
prerequisites. Jack requested radical compression through review. His October 8 comment
`c4741d38-a84a-4cfc-b3a1-b72ea685ab59` then authorized gate and landing with
completion after verified merge, superseding the review-only boundary.

One engine catalog replaces external/npx/rams and Flow resolvers. Retain source,
declarations and arguments together; export preserves third-party files. Claude
subagent names do not select lf's harness. Independent fixes preserve saved Session
placement, active captures on refused continuation and unpublished reservations on
publication failure. Launcher success proves no provider exit.

Compression history: `1e4ae02a5`, `754efacb2`, `31e0640eb`, `dd2cdba82`;
source audit: `48d145b78`. Native/sibling history and stale-client rejection remain
required. Native terminal snapshots retain declarations, exact arguments and
lossless JSON user context; escape native argument/preprocessing syntax.
Rejected hooks changed context authority; shell preprocessing missed first-request
context. Exact counterexamples: `4624224bb`, this section.

Codex 0.160.1 ignores typed skill input outside its discovered catalog, but an
explicit Markdown skill reference selects the original path on both surfaces.
No catalog mount or second resolver is required. Single-file custom prompts keep
one-based positions and named assignments; valid metadata does not make them skill
bundles. Unfamiliar native declarations are reported, not silently discarded.

Ordinary unbound third-party launches omit operating/conversation guidance;
attributed Work and captured Flows retain it. Budget checks remain; notices accompany
managed context or excerpts. Captured input identifies a Flow; ordinary selection
also retains an in-memory invocation. PATH discovery replaces availability's
`--version` subprocess/cache without changing actual launch failure handling.

Fixtures prove native model/argument/context fidelity, collisions, out-of-catalog
Codex expansion, custom-prompt translation and fake-API asset reads. Comparisons:
[scripts/benchmarks/skill-invocation](../../scripts/benchmarks/skill-invocation/README.md).
PR-base startup shows no added second; base's post-provider Git failure limits
that evidence. Plain-native overhead remains; production speedup, live compliance
and terminal UI are unproved. Prior costs: `d505007b1:scratch/run-any-claude-or-codex.md`.

October 8 gate repaired Codex terminal's missing `--` before translated YAML.
After main's system-file merge, the resume fixture reads that file,
retaining workspace/provenance assertions. Eight native/ported surface cases,
mapping proofs and affected checks pass after repair.
Native home continuity and reconnect passed earlier. Hosted CI, merge and Task
completion remain unproved.

## Agent startup profiling (LOO-436, 2026-10-08)

Jack Heart requested landing #1505 on October 8, superseding review-only Flow feedback.
Keep the Task open for outstanding cold-cache verification.
[The report](../../scripts/benchmarks/agent-startup/README.md) owns numbers,
release wall-stack SVGs and rejected patches. Provider-probe removal, absent-Wave
resolution and directory reuse have matched evidence; capture-store reuse and
ancestor-probe deferral did not and were removed. The last rejection meets
the stopping condition, not exhaustion.
Claude reconnect's native median worsens; the later handoff gain cannot repair it.

Readiness requires unsubmitted text echoed by the raw-mode application. Codex
reconnect needs a seed turn outside timing. Fresh Homes are not cold OS caches.
LOO-436 gate owns cold readiness on dedicated macOS with exclusive cache
control/native login; the report records the limitation and reset method. SVG readability awaits review. Neither gap is waived.
Wall stacks include shutdown, not CPU accounting.

Copied databases retain absolute provider-home paths. Strip routes before launch;
connect only benchmark-created Sessions. Early excluded probes reached those
paths without a write audit; isolation remains unproved. Fixtures prove no
installed gain or managed-account continuity. The network-isolated CI smoke
bounds launch work and preserves identity, context and history; nested-directory
launch retains the repository root. Hosted success remains unproved.
