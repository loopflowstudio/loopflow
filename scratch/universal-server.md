# LOO-819: Cheap orchestration through the Machine server

## Explore: three possible worlds

Jack: "ok, launch a new task to explore C and open it in my rex".
Native Rex ownership exploration now belongs to separate Infrastructure Task
[LOO-820](loopflow://loopflow/task/LOO-820). Its brief carries the host
probe, exit/capture receipts, reconnect and execution-owner interface questions.
LOO-819 can finish shared orchestration with existing terminal ownership;
neither outcome depends on the other's completion. This does not select the
extent of incremental computation for A/B.

November 10, 2026. Jack: "$explore". These are proposals; the kickoff architecture
below remains a candidate, not a recorded choice. Review [explore.html](explore.html).

**A — One shared observer.** Keep SQLite projections and real execution owners.
The Machine service shares each complete reading, retained store leases and
dependency watches across CLI, Desktop and native integrations. Changes invalidate
affected readings; the service recomputes each demanded answer once. This is the
kickoff proposal. It removes repeated work without maintaining a second Work
representation. Its limit: many distinct demands can still trigger many full
projections when a shared dependency changes. Checkpoint generations, bounded
admission and independent SQL capacity are essential. Lowest architectural
uncertainty of these three, but still a substantial cross-caller migration.

**B — A live Work graph.** The same Machine service maintains bounded, derived
Task/Session/Graph indexes for demanded scopes. A transactional changed-key feed
identifies affected entities and membership changes; the service reads coherent
committed rows and updates indexes before rendering subscribed views. SQLite stays
authoritative. Restart reconstructs the indexes; missing change history triggers
a fresh snapshot. Checkpoints wait for a committed revision plus refreshed external
facts. Common Task fields can be computed once across distinct views, not merely
identical queries. This is the radical performance ceiling: work follows changed
entities rather than the number of query shapes. It also creates a second derived
representation whose semantics must match SQL through moves, deletes, errors and
recovery. Demand eviction, rebuild memory and catch-up are first-class costs.
Adopt this only if distinct-demand measurements justify that machinery. Existing
execution owners and restart-independent providers remain unchanged.

**C — Native terminal, shared brain.** Use A's shared computation, but put the
provider directly on a capable terminal host's PTY. Rex would own terminal lifetime;
LF would own Session identity and orchestration. Proposed experience: the host can
identify the provider directly, with no LF attach relay in its foreground. Actual
Rex process/icon behavior is unverified. Before this can be selected for delivery,
an isolated host probe must demonstrate reliable exit/capture receipts, current cwd,
provider continuity and the available reconnect behavior. If the host cannot supply
these, a universal server cannot manufacture them. LF-owned PTYs remain necessary
for other hosts unless equivalent capability exists. This expands the kickoff
scope and risks host-specific behavior; it does not by itself remove observer CPU.
C can combine with B later; it is a separate execution-ownership choice.

Common to all: extend the existing installed Machine service; preserve real process
identity, independent cancellation and unknown outcomes; keep optional config errors
from preventing a valid Session launch. No new daemon or change of provider/account.
The page's scenario controls illustrate intended contracts, not measured outcomes.

Recommendation for discussion: A as the implementation candidate, with B kept as
the alternative if full-path measurements show distinct projection work dominates.
C merits a bounded host experiment if native terminal behavior is part of this
outcome. No implementation or Graph continuation follows from this exploration.

Open questions (short answers refer to these numbers):
1. Which world should lead: **1a** shared observer, **1b** live Work graph,
   **1c** native terminal plus shared observer, or a combination?
2. Native Rex work: **2a** retain today's PTY ownership for this delivery;
   **2b** run the isolated receipt/reconnect/icon probe before fixing its scope?

## Outcome and demo

CLI commands, nested agent commands, hooks, native observations and Desktop share
the existing Machine service. Opening more views of the same work adds delivery
cost, not another observer, runtime or database. Quiet Sessions leave CPU to the
agent. Task progress stays prompt. Replacing the service leaves conversations,
terminal input, Graph execution and their real process owners running.

Jack: "lf should not need to really consume almost any cpu -- that should go to the agent and the tools it runs".
Jack: "repeated subscription setup/teardown and the cost of the whole observer loop, not just its Task-update reader".
Jack: "having out of date or incorrect config shouldnt block you from opening your repo session i dont think".

Demo: open a repo Session containing a valid provider choice plus `crons: []`,
then open its Task in Desktop and subscribe from a terminal with
`lf task show <issue> --subscribe`. The conversation opens with a config
diagnostic. A Task change appears in both views and reaches the native Session.
Close either viewer; the other and the agent continue. Restart the isolated
Machine server; the same provider PID and conversation continue, readings
recover, and the next Task change arrives once. `lf server status` shows CLI,
Desktop and native integration clients using that service.

Headless equivalent: a new `scripts/benchmarks/orchestration.py` fixture runs
these interactions through the candidate CLI, real private server socket,
provider stand-ins and Desktop's protocol fixtures. It owns an isolated home,
repository, socket and process groups, and joins every child on exit. It never
installs or restarts the signed service. This script is acceptance work still
to build, not an existing passing command.

## Findings that choose the architecture

Current source: `9c3403a885356276c52c345b426dc9d7b4714be7`, including #1868 and
#1871. Existing server protocol revision 4 already has typed reads/writes,
request traces, admission/byte budgets, independent cancellation, recovery
tokens and shared complete subscription readings. Extend it; `lfd` is a working
name, not another binary, listener or installation.

| Finding | Decision |
| --- | --- |
| `server/subscriptions.rs::Key` includes serialized read/selection, database and directory. Exact demand shares one producer and independent checkpoint generations. | Keep the registry and checkpoint contract. Normalize invocation context before keying; never share across different settings, account routes or caller-dependent answers. |
| `lf/commands/subscribe.rs::Reader` owns a runtime and store per distinct demand. `Read::read`, `screens`, `waves` and engine helpers also construct runtimes. | Remove hot-path runtime construction and store opening across the call tree, not just the Reader field. |
| `Reader::step` checks every store domain except usage; `observe::Cache` polls local facts each second and Git every three seconds, with three 250 ms scanning threads. Unreferenced facts can keep refreshing for five minutes. | Move to dependency subscriptions and deadlines; cached answers may survive briefly, scheduled work must stop at last demand. |
| `ops/task/workspace.rs::watch_task_files` already uses notify 8, handles rescan notifications and filters access events, but owns watchers per companion. | Integrate this watcher with `observe`; one checkout watch plus shared Git metadata/config watches. Keep Task-files framing. |
| `task_updates::watch` has a retained store and thread lock but still checks liveness and provider context every second. `codex_connection::spawn_observer` launches a detached LF process/runtime per client. Pi launches an LF Task-update stream. | Server-owned observation, provider-owned execution. Retain delivery fences and receipts; remove independent observer processes and clocks. |
| `ReadSnapshot` spans multiple SQLite operations. Nested `run_sqlite` uses blocking execution. The prior experiment deadlocked when outer blocking workers consumed all capacity. | Exclusive connection lease per reading; projection workers cannot occupy the SQL execution pool. |
| `agent/session.rs::resolve_session_config` calls the whole-layer validator. On November 10, an isolated installed-build `lf config settings --json` with `agent: codex` and `crons: []` exited 1: `unknown or ineligible setting crons`. | Resolve launch requirements independently of optional workflow/config validation. No `unwrap_or_default()` recovery that changes provider/account. |
| `native_exit::NativeExit` hands capture settlement to the existing terminal execution owner before the launcher execs the provider. Rex sees the outer LF relay's PTY. | Keep that owner and PTY model for this Task. A universal server alone cannot change Rex's program/icon. |

November 10 installed-build pilot, exact server executable SHA-256
`217274235619c9694e0a143b1775c63faab495014179be93fed01c1ba338978f`:

| Distinct Task subscriptions, one connection | Initial readings | Server CPU / ~10 quiet seconds | Git children during quiet window |
| --- | ---: | ---: | ---: |
| 1 | 53 ms | below the probe's 10 ms resolution | 4 `git status --porcelain` |
| 16 | 1,087 ms | 80 ms | 59 `git status --porcelain` |

These are single pilots on freshly created Tasks with distinct worktrees and no
Graph/Session history. CPU excludes children. They establish that recurrent Git
work remains; they do not establish a speedup or explain the older workload.
The original seeder was rejected by the current build for mismatched draft
migration history; no database was rewritten to bypass that check. The passing
pilot used Tasks created by the installed CLI. Source, logs, traces and results
are in `universal-server-probes.zip`.

Historical comparison, separate workload: five alternating pairs at 16 identical
Session subscribers reduced median paired active server CPU 97.98% and complete
lifecycle CPU 97.04%; ten writes needed 10 projections instead of 160
(`17c304389` versus `9c0b7c10b`). Distinct Tasks used about 7.33 server-plus-child
CPU seconds per quiet ten-second window with periodic full projections. #1871
subsequently removed that periodic projection policy. Quiet differences in those
five pairs were inconclusive. Keep the raw receipts; never treat 7.33 seconds as
this Task's current baseline.

Source-only competitor findings are in
`git show e65ddaed78:scratch/coordination-competitor-performance.md`: shared computation before fan-out,
one trailing refresh, canceled queued setup doing no work, and charges retained
through blocked sends. They are test inspiration, not competitor CPU results.

The [notify 8 documentation](https://docs.rs/notify/8.2.0/notify/) documents
missing filesystem events on some filesystems and editor replacement behavior.
Watch parent directories, handle rescan/error events, and keep bounded
reconciliation. Filesystem events are invalidation hints, not authoritative
change records. No new watcher dependency is needed.

## Ownership and APIs

Alternatives considered: routing every current polling loop through RPC removes
neither duplicated projections nor runtime/store churn. A new daemon duplicates
installation and observation lifetime. Making the daemon own every provider
couples restart to execution and makes its PID useless for individual job
liveness. A single shared SQLite connection serializes slow readers and mixes
snapshot ownership. Choose the existing service with bounded, exclusively leased
read connections and independent execution owners. Native Rex ownership remains
blocked on host exit receipts and reconnect semantics, not on daemon plumbing.

```text
CLI / hooks / Desktop / provider extensions
                   │ typed operations and observations
                   ▼
       existing Machine server (lf server run)
       shared reads · stores · fact watches · native observers
                   │ launch/control existing execution owners
                   ▼
     real Graph/terminal LfProcess → AgentProcess → provider
                   │
                   └── existing owner PTY / direct Codex app-server
```

SQLite remains authoritative for Work, LfSession, AgentProcess, attachments,
provider-thread selection, Task-update receipts and creation receipts. Each
LfProcess still represents an actual OS process, including Graph/step and
terminal owners. A served request has a TraceId, never a fake process row or
the daemon PID as a job's liveness. PID plus birth identity decides process
evidence; missing completion remains unknown until evidence resolves it.

The service owns reconstructible observation and request resources. Existing
workers own execution, wait status, interruption and NativeExit capture
settlement. A disconnect cancels reads/delivery, never an admitted mutation or
an agent. Mutations with unknown outcomes are inspected through existing
receipts/resource state, never automatically replayed. Fresh attachment tokens
remain separate from connection IDs, request IDs and observation generations.

Use `Read`, `Write`, `ServerOperation` and `CommandIo` as the common API. Turn
CLI handlers into argument decoding and rendering around these operations;
extend the typed operations where a hot-path caller has no equivalent. A
server handler calls the engine directly. It must not spawn an ordinary CLI
that routes the same operation back to itself. Execution launch uses the
existing detached-owner entry points, which bypass client routing.

Every request supplies its directory, selection, typed invocation settings and
necessary caller provenance. Environment-derived preferences are resolved at
the CLI edge; the server never adopts another request's cwd, environment or
stdio. Do not serialize the whole environment. Provider credentials stay with
their account/owner; transient socket capabilities are private protocol data,
excluded from logs, cache keys and diagnostics. Plain text and JSON output,
stdin-consuming commands, exit status and SSH Machine routing keep their
existing behavior. Remote commands reach that Machine's service through the
existing SSH carrier, not an exposed daemon socket.

One connection per long-lived caller, multiplexing its operations. Short CLI
commands still create a small client process; they do not construct stores or
observers. Local help, schema output, installation/readiness and terminal byte
relay remain local. Terminal/Graph execution owners still need direct store
access for fenced exit/recording when the service is down; this is the same
recording implementation, not another orchestration service.

Reuse installation readiness and the selected service's protocol negotiation.
Do not re-register it per command or enable disabled Background Items. If no
compatible service answers, shared commands return a precise unavailable or
incompatible error. Session bootstrap may invoke the same local launch engine
only before any request was transmitted, to retain offline launch. Once a
launch may have reached the service, inspect its receipt instead. Hooks log a
RecordingGap and return the provider-required success response on failure;
they never stall a turn waiting for service repair. This narrow launch/recording
survival path is intentional and permanent; a general RPC-to-local retry is
forbidden.

## Shared computation and bounded work

1. **Retain execution resources.** One service async runtime, four dedicated
   projection workers, and independent SQL blocking capacity. Each active
   projection leases one of at most four retained read connections for the
   database, from snapshot start through rollback. No other projection may use
   that connection until release. Writes use a separate bounded connection
   pool and existing transaction boundaries; network waits never hold a writer
   transaction. Reuse prepared statements for hot revision, fence, receipt and
   projection queries. Make async engine reads accept the retained Store;
   remove nested runtime construction. Keep schema/busy errors visible and
   retryable, never convert them to empty Work.

2. **Keep complete readings shared.** Extend the existing demand registry to
   serve one-shot reads as finite members as well as subscriptions. A new join
   or explicit checkpoint requires a reading admitted after its requested
   generation. One in-flight projection and at most one trailing invalidation
   per key; distinct keys receive fair admission. Selections remain part of
   the key initially. Sharing formatted subsets across selections adds no
   necessary product behavior and is outside this cut.

3. **Schedule dependencies, not consumers.** Register read dependencies on
   store domains/entities/collections and external facts. Add one additive
   migration for scoped revision counters updated by triggers in the writer's
   transaction; keys cover entity and collection membership, including old
   and new scopes on moves/deletes. These counters derive from existing rows,
   not a second Work model. Keep domain revisions for genuinely whole-domain
   reads. Task, Session, graph and board hot reads must record the narrow keys
   they actually use, with tests for insert, move and deletion. One database
   observer checks revisions on notifications, with a one-second missed-wakeup
   check while demanded. An unrelated entity write must not rebuild a Task
   projection. Failed reads retain dependencies needed to notice recovery.

4. **Integrate filesystem and process observation.** Extend `observe::Cache`
   with reference-counted watch registrations and targeted wakes. Share
   checkout watches and common Git metadata separately; include linked
   worktree gitdirs, common refs/config, global Git includes, directory
   replacement and missing-parent recovery. Invalidate relevant Git answers
   on changes; a requested checkpoint refreshes its external facts before its
   projection. Use the existing 100 ms quiet/250 ms burst coalescing. Thirty
   seconds is the maximum reconciliation interval for quiet filesystem facts;
   failed/unavailable watchers use shared three-second checks until repaired.
   Provider/PR network refresh keeps its independent sixty-second policy and
   cannot block local delivery. A missing external fact suspends the demand
   outside its connection lease; source workers resolve it and reschedule the
   reading. Never hold a projection worker or SQLite snapshot waiting for a
   GitHub request. Process evidence uses platform exit
   notification where possible, otherwise one shared one-second sweep, never
   one timer per Session. Deadlines use SystemTime. Final release removes
   watches, timers and queued work immediately; retaining cached bytes for
   reuse confers no permission to keep refreshing them.

5. **Bound admission before allocation.** Preserve outbox limits: 32 MiB per
   message, 64 MiB per client, 256 MiB total retained and 128 MiB shared retained.
   Include in-flight serialization and blocked sends. Add a service-wide limit
   of 256 queued distinct projection keys and four running projections; keep
   the existing bounded mutation admission separate. Raise advertised client
   slots from 64 to 128 so 64 subscribed clients leave room for hooks/commands.
   Slots and queues return to baseline after cancellation. A slow consumer
   gets the current complete reading or a recoverable capacity error; it
   cannot retain unlimited Codex messages or stall a healthy peer. Replace
   Codex's unbounded message queue with bounded retention and reconnect/history
   catch-up on overflow, recording the gap rather than blocking the provider.

Initial limits are internal constants and measured acceptance assumptions, not
new configuration knobs. Global limits apply across connections; the fact cache
also charges retained bytes and watches against service-owned budgets. Reclaim
unused facts before refusing new demand. A canceled queued job never opens a
store or starts a projection. An admitted blocking read may finish, but drops
its result and lease without resurrecting the canceled demand.

## Native integrations and restart

Add a typed `session.observe` operation with a provider-tagged registration:
Session, AgentProcess, provider conversation, current attachment/generation,
owner endpoint and the provider-specific observation endpoint/capability.
Registration acknowledgement means observation was admitted, not that the
provider was launched or an update consumed. Wire fixtures cover each variant.

Move Claude/Pi TaskUpdates and Codex Recorder scheduling into a server-owned
registry keyed by AgentProcess and provider conversation. Attachment changes
update fences independently; two registrations of the same generation join
the same observation. Different provider conversations never share delivery
receipts. Keep one provider connection where its API requires it, while sharing
LF runtime, store leases, liveness and Task dependency observation.

Registration is a renewable description, not an execution owner. Existing
terminal or execution owners retain each managed provider's observation description in
memory and re-register on server incarnation change. Claude's hook forwards
its transient messaging capability to that owner over the existing private
control channel as well as registering with the service. Pi's extension keeps
its registration/reconnect state and receives updates directly over a Unix
socket, removing its long-lived LF update child. Codex's existing endpoint and
thread selection plus the owner registration rebuild its side observer. The
service discovers live recorded owners on startup and requests their current
descriptions; it does not depend on another human turn to recover an idle
Session. Capabilities are not written into SQLite or scratch.

Re-register before replaying observations, recheck exact process and current
provider/attachment identity, and use existing receipts to deduplicate progress.
Claude's native inbox has no consumption acknowledgement: a dispatched receipt
remains uncertain after a crash and is never replayed as if unsent. Codex
resumes/catches up through its private app-server. Losing a socket is not proof
the provider died. Owner exit without a recorded outcome is detected through
OS evidence, and late completion from an old attachment cannot close a new one.

The recovery fixture must kill only the service after registration, while
native Sessions are idle and an execution worker remains active, then change a
Task after replacement. Same provider PID/birth/thread, fresh transport IDs,
no duplicate update, successful capture settlement at actual provider exit.

## Config isolation and Rex decision

Split config decoding from operation validation using the existing catalog's
typed leaves and applicability. Session bootstrap resolves provider/account,
model and launch inputs without compiling unrelated Graphs or validating
planning/release/schedule settings. Valid leaves retain their current precedence
and source information. Unknown legacy keys such as `crons`, bad optional
values and invalid unused definitions become diagnostics in the opened Session
and its captured history; dependent operations still report their errors.

Malformed YAML makes that layer unavailable, not empty. A resumed Session uses
its saved provider/account; a fresh Session uses an independently resolvable
explicit or saved route. If that route cannot be established, return the
specific missing/invalid launch setting and an actionable flag, without
inventing a provider or switching accounts. This is a required launch input,
not a reason to reject a valid route because optional configuration is bad.
Repairing the file invalidates the config facts and removes the diagnostic.
Direct launch, served launch and context construction must use this same
resolver; changing only `open` would leave the second launch failure intact.

Keep LF-owned PTYs and NativeExit in this Task. A native Rex PTY could expose a
provider process, but replaces the execution/attachment boundary: reliable
wait-status receipt would have to come from Rex's owner or a surviving child
supervisor; the universal daemon cannot `waitpid` an unrelated host child.
Process disappearance alone cannot recover its exit code. It would also lose
LF's host-independent reattachment unless Rex supplied equivalent facilities.
Existing headless Rex tests already show its login wrapper can hide foreground
cwd; launcher-written cwd and OSC 7 are independent of process/icon reporting.
No actual native-provider icon result was established here. Native Rex launch
is excluded pending that receipt/reconnect mechanism and an isolated real-host
probe; no icon improvement is part of this Task's completion claim.

## Delete and cut over

| Remove or replace in the same internal cut | Behavior/tests that survive |
| --- | --- |
| `subscribe.rs::reader_runtime`, `Reader.runtime`, per-reader store opening, polling `Reader::step`, `follow_store` follower scans and `observe` clocks | Snapshot consistency, EOF/parent-exit retirement, retries, selection and checkpoint freshness; move tests to the service scheduler. Keep stdin framing as the CLI protocol client. |
| `Read`/screen/engine hot-path `Runtime::new` and duplicate CLI engine dispatch | Existing typed CLI/server equivalence, human renderers, settings/caller isolation, stdin and remote routing. No local and served implementation of the same operation. |
| `observe::Cache` three scanning loops, unconditional Git intervals, refreshing unused five-minute entries; per-companion Task-file watch ownership | Existing fact capture/inheritance and Task-files frames; share watchers, retain missed-event reconciliation. |
| `codex_connection::spawn_observer`, `run_observer`, hidden `__codex-observe` command; `task_updates::watch` loop/lockfile ownership; Pi `watchUpdates` LF child; Claude detached observer launch | Recorder/TaskUpdates logic, native identity fences, side connection, receipt uncertainty, tool/turn nonblocking behavior and actual owner lifelines. Rewrite old process-count fixtures around one service registry. |
| Whole-config validation as a prerequisite of Session launch | Strict config inspection and validation of operations that consume the bad setting. No new list of tolerated legacy keys. |

Do not delete real Graph/terminal workers, NativeExit, provider transport or
direct best-effort owner recording. Do not repair obsolete observer loops in
parallel with their replacement. New protocol fields go into Rust/Swift DTOs,
schemas and fixtures together, with an appropriate revision bump; optional
keys remain present and nullable. There is no schema-default compatibility
shim and no second daemon. Existing installation compatibility handling owns
version errors.

Internal slices form one complete architectural change, not separate shipped
architectures:

1. **This slice — shared read execution and CLI cutover.** Remove per-reader
   resources, introduce leased read execution, route CLI reads/subscriptions
   through the existing registry, and remove nested read runtimes. Keep
   independent SQL capacity. Focused check:
   `cargo test -p loopflow --lib server::subscriptions::tests::` plus a new
   CLI/server test in that suite showing identical mixed-client demand and
   a queued cancellation. The existing saturated-outer-pool regression must
   still pass. No claim of universal-server completion at this point.
2. Replace polling ownership with dependency scheduling, scoped revision
   triggers and shared Task-file watches; migrate hot writes/hooks to typed
   service operations with retained store access. Exercise unrelated writes,
   external config/Git edits and missed events.
3. Cut native registrations over together with owner-driven restart recovery;
   delete detached observers and bound Codex retention. Exercise idle restart,
   attachment replacement, unknown dispatch, owner death and store outage.
4. Integrate tolerant Session launch/config diagnostics across all launch
   consumers. Complete protocol/Swift consumers, docs, quantitative comparison
   and the end-to-end demo. These slices remain unfinished until their old
   paths are deleted and the combined acceptance scenario passes.

## Gate and done when

Build the new fixture and register `universal_server_tests` in `integration.rs`.
It provides deterministic counters for projections, statements/VM work, store
opens, runtimes, observer workers, watches, queue length and retained bytes.
Exercise 1/16/64 physical clients and multiplexed equivalents, duplicate and
distinct Tasks/Sessions, quiet/unrelated/relevant writes and 32/128 churn cycles.

Required assertions after warm-up:

- Identical demand has one producer; quiet/no-deadline and unrelated writes
  cause zero projections, store opens, runtime builds or Git children. A
  reconciliation deadline may check facts once per unique dependency, never
  once per client. No history-length amplification in revision/fence work.
- A relevant burst has one in-flight read and at most one trailing read per
  affected key. A checkpoint during an in-flight read cannot acknowledge that
  earlier snapshot. Mutation and unrelated read progress continue when all
  projection workers are occupied; no snapshot mixing or nested-pool deadlock.
- Queued cancellation starts no work; completed churn restores owned
  resources and scheduled work to baseline. Slow consumers respect existing
  byte budgets, release charges after sends/reads really finish, and never
  stop a provider or healthy sibling. Also test the 65th command beside 64
  subscribed clients and overload beyond advertised limits.
- Server loss leaves actual owners and provider input/output alive, recovers
  idle observations, retains provider/account/thread identity, and settles
  exit/capture once. Test death without completion, stale tokens, schema/store
  failure, ambiguous write response and launch before service availability.
- Legacy key, bad optional value, malformed optional layer, invalid unused
  Graph and repaired config all traverse CLI and served Session launch.
  Valid launch choices remain unchanged; genuinely invalid launch choices
  are named. Desktop decodes readings/diagnostics and recovers subscriptions.

Headless gate, once per final tree:

```sh
uv run python scripts/test.py --reuse-passing --rust-filter 'test(server::) | test(observe::) | test(lf::commands::subscribe::) | test(lf::commands::write::) | test(universal_server_tests::) | test(settings_launch_tests::) | test(context_launch_tests::) | test(session_cli_tests::) | test(harness::) | test(process::runtime::) | binary(served_reads) | binary(process_ownership_tests) | binary(session_launch_contract) | binary(session_lifecycle_tests) | binary(transport_tests) | binary(lf)'
uv run python scripts/benchmarks/orchestration.py --lf target/release/lf --scenario acceptance --output /tmp/loo819-acceptance
```

The first command includes formatting/static analysis and affected Swift builds
and tests through changed-path selection. Extend its filter for scoped-revision,
Task-file and direct caller modules as implementation identifies them; use the
runner's inventory to ensure named tests actually execute. Swift server DTO,
reconnect and Session diagnostic view tests are headless. Real Rex icon/UI
judgment is not a required headless gate or a shipped claim.

Freeze an optimized current baseline and candidate with identical fixture code,
normal runtime defaults and exact executable hashes. Run five alternating
pairs; count service plus orchestration clients/owners and Git children, with
setup, quiet 60-second windows, unrelated writes, relevant delivery,
checkpoints, cancellation and both teardown tails. Keep providers and fixture
writers separate. Record 1/16/64, duplicate/distinct demand and growing history.
Provisional completion targets: at most 1% of one core for 16 quiet Sessions,
2% for 64 across the whole orchestration tree; quiet reconciliation is included.
Local relevant-write delivery p95 under one second and native progress p95
under two seconds in matched runs. These are targets, not measured results;
deterministic CI checks use work budgets and controllable scheduling, not tight
wall-time assertions. Report regressions/ranges, not only favorable medians.

Success is opening and continuing conversations despite optional config
errors, watching Task progress from every surface without duplicated work,
and replacing the service without disturbing execution. Failure would be
cheap counters hiding per-client background work, stale views after missed
events, or a daemon restart losing a conversation. The acceptance checks cover
those failures through the real carriers, not only TaskUpdates::pending.

## Check receipts

- `uv run python /tmp/loo819-probe.py` — installed 1/16 distinct-Task pilot completed; all server groups stopped. Archived source/results in `universal-server-probes.zip`; gate replaces this small probe with the matched full-path fixture.
- Isolated installed `lf config settings --json` — legacy `crons` rejection reproduced, exit 1; gate tests actual Session opening after the repair.
- Implementation and gate commands above — pending; kickoff changed planning files only.
- `lf context` — scratch below both limits after reconciliation; final accounting accompanies this plan in the run output.
