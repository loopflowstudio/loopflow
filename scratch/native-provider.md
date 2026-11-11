# LOO-820: Direct provider execution in Rex

## Direction, November 11, 2026 (supersedes the Rex-owner framing below)

Jack Heart: "i dont think this task really needs anything rex specfiic, its
about just taking pty ownership away from lf for raw skills"

This Task is `docs/dev/server-first.md`'s "Bare launches go direct" row: a
bare `lf <skill>` compiles prompt and context, starts the agent as a
foreground child of the calling terminal, waits, and reads the transcript at
exit. No owner, no relay, no LF PTY, whatever the terminal is. Managed
launches are LOO-824's holder ("managed should = held"); turn hooks of bare
interactive launches go with the owner (LOO-821, "dont really care about
intermediate states as long as we remove the owner eventually"); exit-time
usage/completion rows come from LOO-822's transcript read; "with bare lf,
noting holds its at all" (LOO-823). Jack Heart: "repo and wave sessions
should be managed" (sent to LOO-824 as option (a)), so bare-direct covers
only launches that are not Task, Wave or repository conversations. The Rex probe below stays as one
host's receipts, not as the design: Rex-specific receipt ingestion,
host-owned exit contracts and cross-host control are out of scope.

## Shipping docs (write-docs, November 11)

Intended behavior for bare-direct, written in place:

- `docs/sessions.md`: new "Run one in your terminal"; resume table row for a
  bare Session live elsewhere (second agent, same thread); lifetime paragraph
  (bare ends with its terminal; `session.end_detached_after` only for
  conversations still on `lf session serve`); "Runs in" table rows.
- `docs/troubleshooting.md`: `lf session serve` processes now mean
  pre-holder conversations and Desktop Task shells only.
- `docs/glossary.md`: AgentProcess.

Open question the docs expose: with turn hooks gone, a steer to a bare
Session (including `lf task steer`, which targets every Session on a Task,
and so bare subagents run in its checkout) arrives only at its next start or
resume, or at compaction through the context hook. The docs say "arrive when
it is next started or resumed, not mid-turn". Should `lf session steer` /
`lf task steer` say at send time that a bare target won't see it until then,
or skip bare Sessions entirely?

## Outcome and stopping point (kickoff, Rex-framed)

Determine whether Rex can own the provider's terminal while LF coordinates
beside it, with truthful exit/capture results and useful reconnect behavior.
The deliverable is an experimental comparison and a recommended design in
`scratch/explore.html`. Production cutover, publication and merge are outside
this exploration. LOO-819's shared Machine service can ship with today's owner;
neither Task depends on the other.

Jack, recorded in LOO-819's `scratch/universal-server.md` on November 10:
“ok, launch a new task to explore C and open it in my rex”. The same source
records: “lf should not need to really consume almost any cpu -- that should
go to the agent and the tools it runs”. These select an exploration, not a
weaker recovery contract or a measured CPU improvement.

Intended experience: Rex identifies the native provider, opens it in the Task
checkout, and retains its editor, pending questions and conversation when a
viewer or optional LF service disconnects. Reopening a pane shows whether it is
the same execution or recovered provider history. Missing exit observations
remain unknown. The Infrastructure chapter's plumbing and unattended-work KRs
apply; there is no numeric performance target for this Task.

## Choice for the experiment

Investigate Rex itself as the execution owner first. It already owns the PTY,
waits for its child and exposes a retained exit result. A second LF daemon is
unnecessary for that ordinary case. Prototype a small observation/receipt
contract, with today's LF owner as the other implementation. Keep this contract
out of production dispatch until the exploration is reviewed.

The first probe refutes transparent execution survival across a Rex **server**
restart. A surviving waiter could retain an exit receipt but cannot restore a
dead PTY master. Do not disguise that limitation as reconnect. Whether history
recovery after server death is sufficient for a later native cutover remains a
product choice; it does not prevent this comparison.

Success would make LF recording/service replacement invisible to ongoing input,
with native identity in Rex and fewer terminal owners. Failure would couple
every conversation to a host restart, lose drafts, or call a restored pane the
original process. These are the decisive checks, ahead of CPU measurements.

## Findings, November 10, 2026

Source baseline: `454f768944fb4b4aa4ee572beb1ca5418b18df6d`.
Reproducible pilot: `uv run python scratch/probe_rex.py`.
Raw responses, executable hash, launcher PID/cwd/time receipts and cleanup
result: [rex-probe-receipts.json](rex-probe-receipts.json).

The pilot uses a bounded Python stand-in on a private server, never a Rex app
or authenticated provider. Rex identifies itself as
`0.0.1793377904-ge6592f`, commit
`e6592f74969959db77f1f84261901ef599a025cf`, built October 30, API revision 8.
It does not exercise LF Session recording, Machine-service replacement, UI
icons, provider/account/thread continuity, or native app Split.

| Question | Observed answer | Consequence |
| --- | --- | --- |
| Can Rex see a command directly? | With `--shell=none`, `process.child` and `process.foreground` identified the Python stand-in; launcher cwd and OSC 7 matched the private directory. | Direct process reporting is feasible. Real provider and visible icon behavior still need separate observations. |
| Must an observer be present at exit? | Two fresh `rex wait --json` calls after exit both returned 42; `process.last_exit` also retained 42. | Reconnecting observation can recover an exit while the block/server retain it. Event-only recording is unnecessary. |
| What does a crash look like? | Self-SIGKILL produced `exit_code: -1`, `error: "signal: killed"`; CLI status was 1. Both later waits recovered it. | CLI status 1 is not the provider's exit code. Rex supplies no typed signal number in this schema; retain the native payload without manufacturing a POSIX wait status. |
| Does the block identify an execution after server restart? | The same block ID changed from the stand-in to a new `zsh` PID with `last_exit: null`; the stand-in launch log had one entry. | Restored placement is neither execution survival nor replay of that stand-in. Bind receipts to a launch, not only a block. |
| Does a pre-restart exit remain queryable? | A fresh wait on the earlier normal-exit block timed out instead of returning its previous 42. | Host restart cannot be relied upon as a durable receipt store. Missing results stay unknown; retain received receipts independently. |

The first pilot stopped after restart because its script assumed `rex new`
returned a top-level `block_id`; it actually returns `initial_windows[].block_ids`.
Cleanup ran. The corrected probe obtains block IDs from `session.list_blocks`.
This was a probe error, not a Rex defect.

Official [CLI reference](https://www.superlogical.com/rex/docs/reference/cli)
documents direct launch and terminal waits. The
[event reference](https://www.superlogical.com/rex/docs/reference/events)
documents `terminal.child_exited` and `terminal.process_changed`.
These support trying retained reads plus event invalidation; they promise no
survival of a server restart. The installed `process` schema is retained with
the receipts and is the exact-build reference for this experiment.

Adjacent findings remain distinct. LOO-785 reports that command-line `rex split`
and raw `session.new_split` did not inherit cwd; the app's `pane.split` was not
observed. LOO-779 still owns native/remote shell lifecycle acceptance. Neither
proves this direct-provider path. On November 10, PR #1861 was open at
`07a21c59560aeebbf254344762a03db2561c247e`, and #1812 at
`f3ff3041d0718ddc16dad51bc9a384c44ab2fe9a`; both overlap transport/wire.
No production transport edits are part of this plan.

## Current and proposed ownership

Today:

```text
Rex PTY → LF attach relay → LF execution owner → LF PTY → launcher execs provider
                                               └────── waits and settles capture
Machine service / hooks / native observers record beside execution
```

`open/rex.rs::Rex::terminal` creates a `shell: none` block; `Rex::block` adds
an exec wrapper that emits OSC 7. The command inside is still an LF viewer.
`lf/commands/util.rs` hands `NativeExit` to `transport::hand_exit_record` before
`agent_process::exec_native` becomes the provider. `transport.rs` accepts that
handoff only for its direct child, keeps the `Child::wait` result, and invokes
`native_exit::settle`. Graph steps retain their Graph-owned wait. No unrelated
service process can replace this wait merely by observing that a PID vanished.

Candidate:

```text
Rex PTY → prepared launcher execs provider/client
Rex child wait → retained host result → receipt ingestion → existing LF settlement
Machine service / hooks / native observers remain beside execution
```

Rex owns terminal bytes, dimensions, editor lifetime, native status and screen.
LF owns the existing Session, Work placement, AgentProcess, attachment fence,
capture and provider/account/thread records. OS PID plus birth identifies the
execution; Rex server incarnation/session/block locates it. A block label,
provider display name or shared service PID never establishes that identity.
For Codex, distinguish the TUI client from its private app-server/provider:
client exit alone must not settle a surviving provider. Keep direct app-server
I/O and the side observer.

### Small execution-owner contract to validate

Use one closed owner enum (LF terminal owner or Rex location) in the prototype,
not a generic plugin framework. The service consumes these operations:

| Operation | Information and rule |
| --- | --- |
| Register observation | Existing Session/attachment/AgentProcess/capture identities, owner locator, provider-or-client role and exact PID/birth. Register before launch handoff; acknowledge the same registration on retry. It grants no new control token. |
| Read execution | Exact native identity, live/dead/unknown observation and source/time; terminal placement separate from identity. Missing socket, timeout or denied process read is unknown. |
| Read/subscribe to receipts | Initial retained snapshot followed by notifications; resnapshot after a gap or observer replacement. Receipt names registration and launch identity, never just block ID. Repeated receipt ingestion does not settle twice. Conflicting results stay visible. |
| Read settlement | Host outcome and LF capture settlement are separate: received, pending recording, settled, or unknown. An unsuccessful LF write cannot replace a native exit result. |

Retain normal exit code, termination without a numeric code, and unknown as
distinct outcomes. Do not turn Rex's `-1` or CLI status into
`ExitStatus::from_raw`. Compare the existing `SessionCommandOutcome` meanings
(interrupted, moved/lost attachment, provider failure) against host observations
in test-only code before proposing a production signature change.

The launch receipt must carry a unique execution identity across server restart
and block reuse. Subscribe, obtain a snapshot, and reconcile by that identity;
events alone have a registration/exit race. A failed create response cannot
trigger another launch until the original outcome is resolved. Prototype
independent receipt retention before ingestion so a temporarily refused store
can recover; crash before any receipt is retained remains unknown. Reuse
capture ownership and settlement records rather than inventing a second
Session/event history. Filesystem failure must not stop provider I/O.

This contract is observation and result ingestion. Existing fenced admission,
resume and exact stop still own control. A small interface cannot make Rex's
unfenced `write`/`close` calls equivalent to LF's input takeover tokens. The
experiment must expose that gap, especially for simultaneous Desktop/mobile
and Rex access, rather than silently adding an LF byte relay back in.

## Alternatives and portability

| Mechanism | What it buys | Where it stops |
| --- | --- | --- |
| Current LF owner | Existing host-independent attachment, drafts, screen restoration, fenced input and actual child wait. | Rex sees the foreground LF relay; retain as the comparison baseline. |
| Rex child + beside-it observer (chosen probe) | Native foreground process, one PTY, retained ordinary child exit. | Server death loses the tested execution; receipts need retention outside Rex; cross-host input/control is unresolved. |
| Tiny per-execution parent using Rex's slave PTY | Parent can wait for provider crash and retain an exact status while service restarts; no second PTY. | Must test foreground process-group/job control and wrapper/icon reporting. It still loses terminal usability when Rex's master dies; parent death can lose the result. Try only if exact signal/receipt coverage defeats host results. |
| Shared service launches/waits for everything | Central place to collect exits. | Couples executions to service lifecycle and still cannot wait on unrelated Rex children. Rejected as an ownership shortcut. |
| Surviving master-PTY owner with a relay | Host-independent live reconnect. | Recreates today's ownership and byte path; blocked as a route to the direct-Rex goal. |

Other hosts continue using their actual execution owner. No cmux, tmux, SSH,
Desktop or mobile migration is implied. A later host-specific cutover must
explain how `lf resume` elsewhere works: native host attachment, history-only
recovery, or a separately commissioned control design. Missing host capability
must not silently start another provider. LOO-819 needs no Rex support to finish.

## Delete — do not maintain

This Task deletes no production owner or transport code. A future Rex cutover
would remove its use of the outer attach relay, LF PTY allocation, Ghostty
snapshot/spool and LF input/resize loop. It would replace Rex's use of the
`Request::ExitRecord`/`DropExitRecord` handoff in `transport/wire.rs` and
`util.rs` with host receipt registration. Those implementations remain live for
other hosts and headless/Graph execution; deleting their files now would break
supported behavior. `NativeExit` settlement semantics survive, even if its
input type changes later.

Preserve tests for `a_terminal_launcher_becomes_its_provider_and_the_owner_records_its_exit`,
`native_exit_preserves_returning_launcher_results`, failed native exec,
pending requests, drafts, byte reconnect and service replacement in
`transport_tests.rs`. They protect the existing path. No exclusive obsolete
test or fixture was found: add a separate native-host probe rather than weaken
their assertions. Keep hooks, `RecordingGap`, account pinning, thread history,
Graph waits and exact stop. Do not repair or clone the relay to make C appear
native; do not add dual settlement writers, fake LfProcess rows for RPCs, or
success inferred from disappearance.

## Remaining work and acceptance

1. **This slice — complete the isolated ownership probe.** Extend
   `probe_rex.py` with machine-readable pass/fail/unsupported results and
   compare the actual LF owner to direct Rex using the same stand-in. Cover
   exit 0/42, signal, failed exec, exit before registration, disconnect before
   exit, block removal, observer replacement, Rex server SIGTERM/SIGKILL and
   lost launch acknowledgement. Record precise OS birth, host incarnation,
   cwd, OSC 7, launch counts and input markers. Never repeat uncertain input.
   The focused command is the pilot command above; it must clean up all owned
   processes and report missing capabilities without relabeling them passes.
2. **Test receipt-to-capture semantics.** Add test-only fixtures beside
   `harness/native_exit.rs`, reusing ephemeral stores and capture handoffs.
   Feed captured Rex payloads through the proposed interpretation and compare
   the actual settlement result with current owner behavior. Replace an
   isolated observer/Machine service while a provider stand-in edits and
   answers; retained result ingestion after recording recovers must settle
   the matching capture once. Duplicate/stale/conflicting receipts, PID reuse,
   client-only exit, store lock/refusal and missing receipts are counterexamples.
   No production launch dispatch, schema migration or installed-service change.
3. **Provider and experience comparison.** Repeat with isolated native
   provider configuration and available credentials, preserving provider,
   selected account and thread identity before/after reconnect. Obtain secrets
   only through Doppler; do not copy credential dotfiles or switch the shared
   Machine login. A missing isolated credential route leaves this check
   unobserved. Exercise a pending request and an editable draft; explicitly
   reopen saved history after execution loss and distinguish it from survival.
   Record real UI program/icon observations separately when available; never
   start a second Rex app. Native Split/resize stays coordinated with LOO-785.
4. **Compare and stop at exploration.** Produce `scratch/explore.html` with
   current/direct/supervised ownership diagrams, the failure matrix, linked
   receipts and observed versus proposed labels. The supervisor option needs
   a small working counterexample only if the host receipts cannot meet the
   selected exit semantics. Recommend a route or state which requirement it
   cannot meet. No automatic production continuation follows from this report.

Demo: run `uv run python scratch/probe_rex.py`, then open
`scratch/explore.html`. The page shows the native process and its launch receipt,
the recoverable 42/crash result, and the same block containing a different
process after server restart. Later panels show real-provider and LF capture
checks only when those have run. The headless command never opens the app.

| Failure | Required interpretation/check |
| --- | --- |
| Viewer/control observer disconnect | Same execution if PID/birth matches; retained draft and next input marker demonstrate continuity. No new launch. App viewer loss is separate from CLI observer loss. |
| Machine service replacement | Provider/PTY survives, observation re-registers, existing capture identity continues. Service PID is not execution liveness. |
| Provider exits or crashes | Match a host receipt to its launch, retain native result, settle capture once; client-only exit is separate. |
| Rex server ends | Earlier receipt remains available only if independently retained. Original execution is dead or unknown based on OS facts; no invented exit code. Restored block must not adopt the old attachment. |
| Store/hook unavailable | Input and tool boundaries continue; record/recover a gap. Receipt persistence failure is visible, never provider failure. |
| Unsupported host/missing history | State the unavailable operation. No duplicate launch, fabricated thread, empty-success result or local-shell fallback for a remote provider. |

Gate runs the isolated probe once for the final tree and compares raw receipts
with the page. For the planned test-only settlement fixtures, the headless
command is `cargo test -p loopflow --lib harness::native_exit::` with inherited
`LF_*` removed and stdin closed. Expected: ordinary owner semantics and the
new host-outcome counterexamples pass without opening a provider or display.
If Rust files change, run formatting/clippy and the affected gate suites once;
no build is needed for this kickoff's scratch-only changes. Real rendered
icons, unavailable credentials and unsupported platforms remain clearly marked
unobserved, not required interactive gates.

Done for exploration when the comparison identifies exactly which owner can
return each result, what survives each failure, what must remain in LF, and
what would be removed for Rex. Every claimed improvement has a dated receipt;
remaining host/control/provider limits are visible. CPU savings and icon
improvements require measurements/observations and are not current claims.

## Checks

- `lf context` — initial memory 15,965/16,000 tokens and 70,948/131,072 bytes; scratch empty; kickoff.
- `uv run python scratch/probe_rex.py` — final pilot exited 0: repeated reads recovered 42 and crash; server restart replaced the execution; one launch per stand-in and no surviving tracked children; kickoff. Gate reruns the completed matrix.
- `lf context` — final scratch 4,033/12,000 tokens and 19,271/98,304 bytes before this check-line update; memory unchanged and within both limits; kickoff.
- `git diff --check` — passed; kickoff.
- Rust/app tests — not run; production code unchanged. Gate runs the test-only settlement checks when implemented.
