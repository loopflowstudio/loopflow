# Database lock recovery — 2026-11-10

**Status after step 4:** mitigated, not recovered. Delivery and controlled recovery
are sequenced below for the parent incident workflow. Both reproduced causes have
tested local repairs; neither is deployed. The latest process/lock observations
are step 2's saved samples, not a fresh live check. See the causal review below.

Jack: "can we relaunch all the LF processes while preserving the llm sessions"
and "i guess kill that write presure first".

The ongoing incident blocks responsive LF operation. Restore idle LF owners on
the installed build without losing provider conversations, active turns or
unsent drafts. The caller Session must not be resumed or interrupted. LOO-819's
live design Graph in its own checkout must stay running. Desktop must remain
closed until its repeated `session.resume intent=start` loop for manabot
infrastructure (name/UUID alternation) is controlled.

## Original observation and current mitigation

Installed build: `0.15.2+9c3403a88`, including performance PR #1868
(`c7094eb9736a8c66a789558019d6cc9b7b476b84`). Machine server PID 109 reports
ready, revision 4. Original WAL write-byte sample: held 296/425 samples (69.6%)
over ten seconds. Old `__codex-observe` helpers 90401 and 49149 accounted for
84 and 87 samples. Their stacks showed FactWriter → record_session_event →
SQLite INSERT preparation → deeply expanded trigger compilation.

Before this step, both helpers were SIGSTOPPED while a separate connection
held BEGIN IMMEDIATE; that connection then rolled back. Neither helper was
frozen holding the write lock. They remain stopped, with exact birth receipts
and the SIGCONT undo recorded in `database-lock-evidence/lf-pressure-pause-20261110.json`.
Provider PIDs 87436 and 38767 and LF owners 87183 and 38507 remain running.

Fresh ten-second read-only F_GETLK sample after the pause: 49/421 samples
held (11.6%); 372 free. PID 4994 held 40, 6167 held 5, 38507 held 3, 37350
held 1. Desktop executable was absent. Server readiness and the Session rename
write succeeded. This is immediate lock relief, not a cause repair.

The refreshed supported rollover preview selected 19 idle owners (earlier
preview selected 21), retained 63 for native input and one for a running
Graph, and left 113 other processes until exit. One Graph runner uses the
selected build at its next step. The caller was not eligible. Provider thread
references and owner identities were saved before the rollover.

## Recovery result

`lf process rollover --json` exited 1: 19 owners stopped, 17 Sessions reported
resumed, two failed. The full result is `database-lock-evidence/rollover-result.json`.
All 19 recorded provider-thread fields were unchanged. At the final OS check,
16 resumed owner/provider pairs were live. Session
`e2a4b3267d404a61885a763832bfbea5` initially resumed (owner 26535, provider
29467), then its owner recorded `interrupted` 125 seconds after starting and
both processes disappeared. Quiet detached-owner retirement is a possible
explanation, but no matching log receipt confirmed it; do not count it live.

The two failed resumes:

- `ea5bcb72bd404139a2704faa083edd40` (Fantasia repository operations):
  `lf session resume` exposed `invalid graph: unknown or ineligible setting pm`
  in `/Users/jack/src/manabot.repo/.lf/config.yaml`. Migrated `pm.provider` to
  `planning.provider` and `pm.linear_team` to `linear.team`, keeping the same
  team. Original config is `database-lock-evidence/manabot-config-before.yaml`.
  Retrying `lf session resume ... --json < /dev/null` got past config loading
  and failed in Claude: "No deferred tool marker found in the resumed session.
  Either the session was not deferred, the marker is stale (tool already ran),
  or it exceeds the tail-scan window. Provide a prompt to continue the conversation."
  No prompt was invented or sent. Provider thread remains
  `51e1e15e-8aa8-4aa6-b416-6513399df47f`; retry processes exited.
- `b3ede790bbb442febb9a72d74f599cff` (EXP-28 Combined eval database):
  `its checkout /Users/jack/src/etude.eb70f8-keep-each-eval no longer exists`.
  Needs the intended checkout recovered or a deliberate location change.
  No empty replacement directory or Session move was fabricated.

Rollover also returned `turn_cut_off: true` for `ea5bcb72…`, `a17891fe…`,
`b239e550…`, and `e76c4e99…`. The reported flag uses TURN_RUNNING, which counts
old program status; eligibility uses TURN_LIVE, which also checks attachment,
provider lifetime and viewing. The latter three activity rows have timestamp
zero, no start/completion events, and old `working` program status. The first
has activity timestamp 1793215496 and completed turn receipts at that timestamp,
plus old `working` program status. These flags do not demonstrate current-turn
loss; preservation of all active turns cannot be independently proved from
these stale records. No input was replayed to settle the flags.

Fresh post-rollover preview: zero eligible owners; 63 held by native input,
one held by a running Graph, 103 other processes stay until exit; one Graph
runner moves at its next step. No broad signals, Desktop reopen, caller resume
or new Graph launch. LOO-819 status now shows its earlier Graph completed at
18:26:59Z, Task step `explore`, and its primary interactive Session active. Its
execution progressed independently; no run or steer was issued here.

## Final checks and next debug step

- Read-only F_GETLK, ten seconds after rollover: 46/427 samples held (10.8%),
  381 free. PID 4994 held 39; other owners held seven total.
- PID 4994 is `lf session serve`, Session `bf503b9d40bf4d6480f16c6e1ae8af6d`
  (Repository conversation), started during this incident at 10:24:39 local.
  It is the largest remaining sampled source; it was not signaled.
- `lf server status`: ready/listening, same instance, revision 4, no error.
- Real `lf session card` reads confirm one resumed Claude Session and one
  resumed Codex Session live/detached. No test prompt was sent. Desktop's visual
  workflow remains unverified because its resume loop is still uncontrolled.
- Final process sample: Desktop absent; 90401 and 49149 still `Ts`; exact
  provider PIDs 87436 and 38767 and owners 87183 and 38507 retain their original
  birth times. Caller provider thread, agent process and owner are unchanged.
- No code/build tests needed: operational recovery and one config migration.
  All finite diagnostics and rollover commands finished; no diagnostic server
  or watcher was launched. Restored Session owners are the requested service.

Next: control Desktop's name/UUID resume loop before reopening it; establish a
safe observer-only replacement using the original attachment/client receipt
and `input: null`; inspect PID 4994's remaining write load; resolve the two
failed Sessions and the late exit above. The trigger-compilation cause is not
repaired by this recovery. Raw samples, receipts, Session identity checks and
command results are under `database-lock-evidence/`.

## Observation replacement boundary

The supported rollover cannot move owners 87183 and 38507: native input holds
both conversations. The observer launch takes a private stdin receipt with
database, Session, attachment, endpoint, provider thread, account, cwd, exact
client PID/birth and optional first input. No supported observer-only replace
command was found. Reconstructing that receipt incorrectly can replay input or
make attachment mismatch handling stop a live client. Do not force the owners
through rollover, change attachments, or SIGCONT the old helpers without
accounting for renewed write pressure. Their observation remains suspended.

## Debug step 2 — targeted repairs, not yet installed

Synced this checkout to `9c3403a88` with `lf sync --manual`, then resolved the
two subscription conflicts using main's versions. Those versions already
contain this branch's viewer-retention/final-delivery repairs plus the newer
typed events. Sync commit: `82cd3b9b4`. No other Task or Graph was launched.

### Trigger compilation

A schema-only copy of the installed store reproduces the native event INSERT's
compilation expansion. Removing only `canonical_session_events_insert` takes
its EXPLAIN program from 41,743 instructions to 336; removing any other event
trigger does not. The canonical checks inline nested CTE CASE expressions at
every reference. `record_session_event` prepares this statement after
`BEGIN IMMEDIATE`, so compilation monopolizes the writer reservation.

`bounded_identity_triggers.sql` replaces the 55 canonical identity triggers,
materializing `decoded` and `uuids`. It keeps every validation expression,
trigger event and column list unchanged; no rows, tables or identities change.
The same nested expressions exist on process/activity writes, so the migration
repairs the shared cause rather than just the sampled event trigger.

SQLite 3.50.4 isolated WAL experiment, 25 cold preparations: 41,743 → 687
instructions; median 24.174 → 0.378 ms. An existing connection with an already
compiled INSERT successfully wrote after another connection applied the repair
(3.696 ms), then used the smaller program. EXPLAIN alone does not execute
SQLite's schema-version check: an initial EXPLAIN-only experiment continued to
display the stale program; the actual INSERT invalidated it. All trigger
validation expressions compared equal after removing MATERIALIZED annotations.
Results: `database-lock-evidence/trigger-repair-experiment.json`.

The new Rust regression bounds the actual production INSERT's compiled
program below 2,000 instructions, records/deduplicates 100 native outputs, and
checks that canonical validation still rejects a mixed-case structured flow
ID while retaining opaque provider text. Existing usage-update and turn
lifecycle regressions also pass.

### Desktop loop

The store contains Wave `8c4a50d4-a51e-4ceb-8944-446f9175c6fd`, named
`infrastructure`, and Wave `c212f7b1-758f-434a-a743-f381943482de`, named
`8c4a50d4-a51e-4ceb-8944-446f9175c6fd`. Both name-based preparations resolve to
the first Wave, whose primary is `09a40b4c78b2411ab30bfbc3203d7304`. Successful
preparations move its pane between two warm workspaces; the displaced workspace
immediately prepares it again. The saved requests all finish with code 0.
The raw Wave rows and a bounded log excerpt are saved in
`desktop-wave-collision.json` and `desktop-resume-loop.log`.

Desktop now prepares each Wave by `WorkWorkspaceKey.id`, removing the separate
name field and name-ready gate. The regression supplies the same name/ID
collision, resolves IDs before names like the backend, and requires both
primaries to stay in their own panes after three refreshes with no extra
preparations. No Wave row was deleted or renamed.

### Operational boundary

The targeted CLI was built by the migration integration fixture. Read-only
`target/debug/lf self install preflight --json` exited 1: `ahead_pending`,
unmerged `bounded_identity_triggers`; verdict rejects promotion because the
shared store applies only merged migrations. No promotion, direct live SQL,
Desktop reopening, owner rollover or observer signaling was attempted. The
parent incident Graph still has 5whys and launch-plan ahead; repair delivery
and installation remain outstanding.

After a merged build installs this schema, recovery can first try the original
observers individually: the isolated existing-connection experiment supports
SQLite recompilation, avoiding a reconstructed private receipt or replayed
input. This is not yet verified on the live helpers. Recheck exact PID/birth,
provider and attachment identities, and sample writer pressure around each
continuation; do not continue both blindly or pause a helper holding a lock.
No supported observer-only replacement command was added or invoked.

Final ten-second read-only F_GETLK sample: 52/424 held (12.3%), 372 free;
PID 4994 held 40, 38507 five, 89178 four, 6167 three. Both paused helpers remain
`Ts`; providers 87436/38767 and owners 87183/38507 remain alive with the original
birth times. Desktop's app executable remains absent. No resume, input, attachment change or signal was directed at
the caller Session or LOO-819. The failed saved Sessions and late exit from
unbreak remain unresolved.

PID 4994's one-second stack sample points to planning-peer exchange and
`export_peer_planning` → `route_repository` executing its route INSERT, not
the observer's event-trigger preparation. Its remaining workload is not
repaired here; `writer-4994-debug.txt` retains the sample.

### Checks

- Native Session event suite: 20 passed (99.26 s).
- Canonical identity/migration suite: 27 passed (105.06 s), including upgraded Work/native-history CLI reads.
- WorkCache, WorkspaceRetention and SessionsStore: 81 passed; repeated with WindowServer access denied, 81 passed (3.25 s).
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`: passed. Test linking emitted the existing macOS large `__eh_frame` warning.
- Migration namespace/history, architecture, Rust test inventory and Swift platform checks: passed.
- Codex observer regressions: 6 passed (26.80 s), including unavailable-store continuation and exact-client attachment retirement.
- Subscription merge-conflict regressions: 14 passed (12.74 s).

Check logs and final process/lock samples are under `database-lock-evidence/`.
All finite diagnostics, builds and tests finished. No server or watcher was
started. Changes remain in this checkout for the next incident Graph steps.

## Step 3 — causal review

LF operations sharing the Machine store stalled; Desktop repeatedly prepared
one manabot conversation for two Wave workspaces. Pausing two observers relieved
writer pressure but suspended their observation. Rollover restored 17/19 saved
Sessions initially, with the failures and later exit recorded above. Local
repairs do not yet make those workflows recovered.

### Why the writer was occupied

The saved stacks place both heavy observers in FactWriter's SQLite preparation,
not provider computation. Native event recording acquires `BEGIN IMMEDIATE`
before preparing its INSERT. Canonical identity triggers then inline nested
`decoded`/`uuids` CASE expressions repeatedly: even a small receipt incurs a
large compilation while other writers wait. The isolated trigger removal and
materialization experiments above identify this mechanism independently of
history size: 41,743 → 687 instructions and 24.174 → 0.378 ms cold preparation.
The drop from 69.6% sampled lock occupancy to 11.6% after mitigation supports
the operational importance; it is not a controlled attribution of every saved
millisecond to these two helpers.

The earlier performance work addressed real reader amplification and a costly
viewer-retention query. Its checks could pass with this writer defect:

- `performance/observer_measurement.rs` instruments connections opened after
  measurement starts; fixture/writer connections open beforehand. Its VM-step
  counters measure execution, not SQL compilation. Process CPU can include
  compilation, but does not isolate time holding the writer reservation.
- `harness/observer_measurement.rs` injects four unrelated receipts to exercise
  polling. `server/subscription_measurement.rs` separately records writer CPU
  for capture/activity writes. Neither bounds cold native-event preparation or
  competing-writer delay. The subscription benchmark checks delivery and
  cleanup; reduced reader CPU is compatible with an expensive writer.
- The viewer-retention regression bounds executed VM steps for that query.
  It does not bound the trigger program compiled for a different INSERT.

These are gaps in the measured workload and acceptance conditions, not evidence
that the earlier reader improvements failed. The new production-INSERT program
bound closes the reproduced compilation gap without a machine-speed threshold.

### Why successful Desktop requests never settled

The saved Wave rows contain a name equal to another Wave's UUID. Desktop
already keyed workspaces by stable ID, but converted that identity to a name
for preparation. The backend resolves IDs before names, so both selectors
returned the first Wave's primary. `revealSession` keeps one native occurrence
by removing that Session from other workspaces. The displaced warm workspace
had no pane and no error; preparation completion immediately schedules the
next warm opening. Each successful request therefore recreated the condition
for another request. The saved alternating log entries all return code 0.

The old `WorkCacheTests.Source` resolved names only, unlike the backend, and
its normal fixture had no name/ID collision. Thus successful preparation
appeared to imply a settled workspace. The two-preparation concurrency limit
bounds simultaneous work, not this continuing sequence; error retry handling
cannot catch success. Stable-ID preparation removes the ambiguity. The new
collision fixture checks distinct retained primaries and no extra preparations
after refresh, covering the resulting behavior rather than just request success.
How the UUID-shaped name arose is unknown; preventing this loop does not require
renaming that Wave or forbidding such names.

### Prevention and remaining limits

**Already implemented locally:** materialize the shared trigger expressions;
bound the real native INSERT's compiled program while exercising identity
validation and deduplication; prepare Waves by their existing IDs; exercise
the collision with backend-equivalent resolution. Saved logs confirm 67 Rust
and 81 headless Swift tests passed. No test suites were rerun in this analysis.

**Proposed follow-up:** extend the existing measurement harness with native
receipt writes against the complete migrated schema. Report cold preparation,
BEGIN-to-COMMIT time and a second writer's wait separately from reader CPU;
include the first write after schema invalidation. This would expose contention
that VM-step counts and percentage reader improvements cannot establish.
Keep the deterministic program bound as the regression gate; use paired timing
runs to assess lock cost. For Desktop preparation changes, retain the settled
pane/no-new-requests assertion, including successful responses. A generic
retry delay would only slow this particular loop.

**Delivery remains separate:** install preflight correctly rejected the unmerged
migration. Do not bypass it. After merge and supported installation, verify
the applied schema and loaded build, then use the controlled observer-continuation
plan above and check Desktop convergence. The existing-connection experiment
supports recompilation; it does not prove either paused helper will recover.

The latest saved 12.3% occupancy is mostly PID 4994. Its stack and
`planning_peers::export_peer_planning` place route insertion and export inside
another immediate transaction. That identifies the next workload to measure,
not why its cost is high or which repair is warranted. Desktop's contribution
to total lock occupancy was not isolated. The two failed saved Sessions, late
owner exit and stale `turn_cut_off` flags remain separate open recovery issues.
No evidence supports claiming all turns were preserved or all LF processes
were relaunched.

Step 3 changed only this note and the Session's suggested name. Code and raw
diagnostics remain intact; no signal, resume, replay, attachment change, deploy,
Task or Graph launch occurred. Desktop stayed closed; caller execution and
LOO-819 were untouched.

Next: the parent incident's launch-plan should sequence delivery of the two
tested repairs, controlled recovery and post-install measurements; keep the
planning-peer investigation and unresolved saved Sessions distinct. This note
records proposals, not authorization for additional implementation.

## Step 4 — delivery and controlled recovery decision

Keep one repair in this checkout: bounded canonical identity triggers and
Desktop Wave preparation by stable ID, with their existing regressions. No
additional implementation, Task, or Graph is needed. Landing is already
authorized; deployment follows verified merge, never the reverse. The parent
executes this sequence. This planning step deploys nothing.

### 1. Deliver the current repair

- Review the final diff against current main, including the untracked
  `bounded_identity_triggers.sql`. Keep the five changed Rust/Swift files and
  that migration together. Check the branch's existing PR before publishing;
  refresh it if it represents this work. Do not adopt LOO-819's work.
- Retain the saved 67 Rust / 81 headless Swift passes, fmt, clippy and structural
  results. No rerun for this prose change. At delivery, reuse results only for
  unchanged tested content and the same command plan; run any missing affected
  gate checks once. A conflicting sync or code repair requires its affected
  checks, not a gratuitous full local matrix. CI still owns its full matrix.
- Before scratch cleanup, retain this existing note and raw diagnostics outside
  the checkout's `scratch/` cleanup scope, with private file permissions. Do not
  publish raw Session/process records. Put the repair's intent, constraints and
  done conditions in the PR body so they survive cleanup: native receipt writes
  compile a bounded trigger program while retaining validation/deduplication;
  colliding Wave names retain distinct primary panes without repeated successful
  preparations. Deployment/recovery results belong in a subsequent PR comment.
- Checkpoint only the repair paths, then use the existing delivery commands:

  ```bash
  lf commit -m "Bound identity trigger compilation and prepare Desktop Waves by ID" \
    rust/loopflow/src/store/migrations/bounded_identity_triggers.sql \
    rust/loopflow/src/store/sqlite/session_events.rs \
    swift/LoopflowMac/Views/SessionsView.swift \
    swift/LoopflowMac/WorkModel.swift \
    swift/LoopflowTests/WorkCacheTests.swift \
    swift/LoopflowTests/WorkspaceRetentionTests.swift
  lf land --wait-and-fix
  ```

  Use the delivery skill in that step. Confirm the PR actually merged and record
  its merge commit; a successful auto-merge request alone is insufficient.

### 2. Install the merged repair with Desktop closed

Wait for a successful `main-packages.yml` run whose commit includes the merged
repair (`gh run list --workflow main-packages.yml --branch main`). Then use
`lf install --dev`. This selects and records the dev stream; it must install
both the signed Desktop and its matching CLI. `scripts/install.py local` only
stages a development bundle and is not this deployment path. Never promote the
existing pre-merge debug binary or apply SQL directly to the live store.

Immediately before installation, refresh the protected process identities and
confirm the Desktop executable is absent. The installer resumes Desktop only
if it was running; keep it absent throughout installation. Let the supported
installer handle migration and Machine-server refresh. Do not add a broad kill,
owner rollover, caller resume, or LOO-819 run to that operation.

After installation, run:

```bash
lf --version
lf self install preflight --json
lf server status --json
```

Check the selected CLI and installed app's source revision against the merged
repair, the install's settled state, and the actual applied migration frontier.
Read `sqlite_schema` to confirm the 55 replacement canonical triggers contain
the intended materialization; an installed version string alone does not show
that the live schema changed. Preflight must now report a compatible, applied
frontier rather than `ahead_pending`/unmerged. Confirm server readiness and
Desktop absence. If the latest green package predates the merge, or installation
fails, leave both observers paused and Desktop closed; report the incomplete
deployment without bypassing the installer.

### 3. Continue original observers individually

Take a fresh ten-second read-only F_GETLK writer-lock sample before continuation.
Compare subsequent samples with this baseline and retain per-PID counts; the
saved 12.3% is historical, not a current threshold or an acceptance target.

For each observer, immediately before signaling, match OS PID, birth time,
executable and parent chain to the saved pause/process records, and resolve its
Session, provider thread, attachment, exact client PID/birth and LF owner from
the original launch/attachment records and current store. Verify the current
attachment still names that same live client and thread. The pause JSON alone
contains only second-resolution process identity, not the complete attachment
receipt; obtain the missing fields read-only, never reconstruct private stdin.
If identity cannot be established or has changed, do not signal that PID.

| Observer | Expected provider | Expected owner | Saved observer start (local) |
| --- | --- | --- | --- |
| 90401 | 87436 | 87183 | Nov 9, 08:19:01 |
| 49149 | 38767 | 38507 | Nov 9, 12:06:38 |

Proceed with 90401 first. Only after those comparisons, send `kill -CONT 90401`.
Take an immediate ten-second lock sample, then another after at least 30 seconds
of observation. Check normal observer recording of already pending or naturally
arriving events, no attachment retirement/store errors, and unchanged protected
provider/owner/thread identities. Do not send a synthetic prompt to make a check
pass. A live idle helper without an event is not verified recording recovery.

If pressure returns persistently, identify the writer before acting. To re-pause
that exact helper, first obtain a separate bounded `BEGIN IMMEDIATE` reservation,
recheck identity, signal STOP while the reservation is held, then roll back.
This ensures the helper is not stopped holding SQLite's writer lock. If the
reservation cannot be acquired, do not issue an unsafe STOP or kill; leave the
second helper paused and report the failed recovery. Do not signal providers
87436/38767 or owners 87183/38507.

Only after the first continuation is healthy, repeat identity checks and the
same sample/recording sequence for `kill -CONT 49149`. If the first is idle with
no recording to observe, report that limit rather than declaring recovery and
continuing both blindly. Existing connections should recompile after the schema
change, but only this live check can establish recovery. No observer replacement,
attachment rewrite, private receipt replay or owner restart is part of this plan.

### 4. Reopen the corrected Desktop and check convergence

Once installation and observer recovery checks above pass, open the installed
app once with `open /Applications/Loopflow.app`. Confirm the running executable
belongs to the corrected installed bundle. Observe the two colliding manabot
Waves by their IDs from the saved collision record. Both must retain distinct
primary Sessions/panes through at least three refreshes; after initial
preparations complete, observe at least 60 seconds with no renewed alternating
successful preparations for these two workspaces. Check actual retained panes
as well as code-0 responses, then take another ten-second lock sample.

If the loop returns, close only Desktop using its normal quit path and inspect
the logs; keep provider processes and owners alive. If a display is unavailable,
leave this live convergence check outstanding; headless test passes do not
establish it. Never resume the caller Session or steer/restart LOO-819. A read of
LOO-819 in this step still showed Task step `explore`, a completed kickoff Graph,
and its active interactive Session; no execution command was issued.

### Completion and separate follow-ups

This recovery is complete only when the merged repair is installed, the live
schema is repaired, both original observers record successfully without renewed
observer-driven contention, protected conversation/attachment identities remain
unchanged, and Desktop preparations settle. Record measured lock occupancy and
remaining writers rather than promising a total percentage reduction. The old
observer executables may remain loaded: schema recovery does not relaunch them.

Keep these unresolved or proposed outcomes separate; none is allocated or
launched here:

- PID 4994's planning export/route-write cost: investigate before choosing a fix.
- Saved Session `ea5bcb72…`: Claude needs real continuation input; invent none.
- Saved Session `b3ede790…`: recover its intended checkout or obtain a deliberate
  location decision; fabricate neither.
- Session `e2a4b326…`'s later owner exit and stale `turn_cut_off` flags: unresolved;
  no claim that every original turn survived or every LF process relaunched.
- Full native-writer transaction benchmark: proposed cold preparation,
  BEGIN-to-COMMIT and competing-writer wait measurements, including first write
  after schema invalidation. It does not delay this reproduced-cause repair.

Step 4 changed only this existing note and the suggested Session name. No code
tests reran; no deployment, signals, Desktop launch, Task/Graph launch or saved
Session resume occurred. No diagnostic process remains running.
