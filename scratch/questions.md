# LOO-717 — Decisions and remaining review

The authoritative contract is [shell lifetimes](loo-717-shell-lifetimes.md).
No product question is open. Jack Heart: “ok, lg2m, ship it” (November 6, comment
`62c4fa16-c780-4492-af33-9f01cc16876d`). Shipping LOO-717 / PR #1811 is now
authorized, superseding the earlier restriction; LOO-498 is excluded. This run
finishes publication and returns to demo. The primary Task operator invokes
`lf ship`, then verifies merge and completion. Installation, live-store/scheduler
migration and credential changes remain outside scope.

## Accepted decisions

- November 6, Jack Heart: “yeah, lets remove that API and put it in the desktop.”
  Remove `lf task shell` without an alias. Desktop owns receipts, startup,
  create/attach/reconnect and Close through a hidden cross-platform runtime in
  the existing `desktop` namespace. It works on Linux without GUI control.
  LF supplies shared PTY/Machine transport and scheduled checkout retirement;
  retirement delegates Desktop shell settlement. Rex owns its ordinary shells.
- November 6, Jack selected option 1: carry the existing viewer protocol across
  SSH, deleting `lf session remote` and its additional terminal layer. The bridge
  forwards bytes, owns no PTY or heartbeat and records no host identity.
- November 6, Jack accepted one minute-based OS sweep per Machine over SQLite
  retirement rows, replacing per-Task sleeping workers. Normal installation
  provisions distinct cleanup and updater jobs; development does not register them.
  Each pass exits, recovers interrupted work and retries eligible failures with
  backoff. Fifteen-minute absence normally reaches cleanup in 15–16 awake minutes.
- November 6, Jack accepted caller-supplied integration: Desktop sends its existing
  Ghostty startup script and resource contents; shared transport selects none.
  Creation keeps login startup, literal initial argv, LF availability and command
  blocks. Reattachment never replays startup. No new integration or host registry.
- November 5, Jack clarified Session-only retention: passive, hidden and unfocused
  LF viewers retain a Task checkout; ordinary shells and surviving native tabs do
  not. LF closes only Desktop-owned shells, never Rex shells. Host layouts and
  restoration stay native; the existing LF database owns Desktop layouts only.
- Jack accepted orderly Desktop Quit releasing viewers and detected network loss
  starting the same fifteen-minute absence window with “sounds good.” Expiry
  does not prove client death or add another grace window. Return cancels cleanup.
  Requested work, drafts, pending requests and running Graphs remain protected;
  finishing work does not restart the clock. Eligible idle agents use two minutes.
- Desktop hide/switch/quit retain shell execution until eligible cleanup. Close
  ends its ordinary jobs; Undo restores identity/history. New Shell is deliberate.
  Remote failure never opens a local prompt. Cleanup may discard uncommitted files
  but retains existing Git safeguards. Deleted checkouts are not reconstructed.

## Implementation choices

- Hidden `desktop shell` dispatch precedes repository/UI binding. `desktop::shells`
  owns policy and receipt storage; one unreleased migration renames the receipt
  table to `desktop_shells`. No compatibility table or public listing surface.
- A content-addressed destination cache materializes relative UTF-8 resources.
  Reattachment ignores creation inputs. Swift derives launch and Close argv from
  the saved shell identity; file observation remains under Task workspace.
- Existing `Controlled` queries pulse each attached connection every ten seconds;
  the owner expires silence at thirty. Unattached probes add no retention.
  The bridge bounds each queue at 4 MiB and drains for at most two seconds after
  owner closure. Command pipes allow sixty seconds for SSH/destination preparation;
  local sockets allow five. Saved-history snapshots grant neither input nor a hold.
- Cleanup retry starts at sixty seconds, doubles to one hour and stays separate
  from absence. Return clears retry state. Alternate HOME/LF_HOME skips account
  schedules. Held rows are reconciled so a dead owner cannot retain forever.
- Viewer return and cancellation of queued automatic shell closes commit together,
  including a brief return/leave between recorder polls. Owners recheck current
  Task retention before consuming closure, independently of sweep cadence.
  Explicit Close remains effective; unreadable cleanup state retains execution.
  The design records the `d14b32ada0` counterexample and no-intervening-sweep tests.

## Verification and remaining work

The Desktop ownership cut is committed at `e8385db949`. Focused local/loopback-SSH
lifecycle, migration/cancellation and headless Swift checks passed. Earlier
publication checks at `3c57ec3bf` had an initially failed aggregate followed by
passing repairs; they do not validate this later cut. The design retains commands,
counts, warnings and logs, including initial failures.

November 6 concept review found the hidden runtime incorrectly advertised a GUI
result schema. The repair committed at `57e0eb180` returns no schema, excludes
the runtime from GUI action validation and documents terminal bytes/Close status.
Three focused tests passed, covering exact inspection, public-list exclusion and ordinary GUI
schemas. Reconciliation confirmed the source, README and saved logs agree.
Concept review at `57e0eb180` found nothing further to change and no open product choice.
Compression passed 23 selected checks and Clippy; the design records
the nextest leak warning and clean isolated rerun. Publication checks now passed
471 selected Rust checks, 74 boundary checks, three real SSH checks, 687 headless
Swift tests, 72 website tests, Clippy and the disposable Linux upgrade. The
aggregate initially failed on a duplicate architecture-map owner; its one-line
documentation correction passed the architecture recheck. There was no fresh
all-green aggregate rerun. The design records commands and logs for PR #1811.
Native Desktop/Rex and physical remote-Machine proof remain at interactive demo.
LF-owned SSH heartbeat tests do not establish external hosts' loss detection.

The subsequent gate found Xcode fallback app/test-runner compilation missing from
that record. Two attempts stopped before compilation because the shared uv-cache
inventory exceeded 15 seconds. A diagnostic read completed in 18.41 seconds;
145.3 GiB was free. Xcode is deferred to macOS CI's `loopflow-ui-test`, not passed.
No executable code changed; the lifetime module comment now includes retirement.
The design's gate continuation records commands, logs and carried results.

The private demo app/provider has not launched. Rebuild the old candidate and keep
its history/runtime roots and input-release contract in [the demo record](loo-717-demo.md).
`lf sync` completed cleanly against `c8d7a7f4a8` during publication and
checkpointed the reconciliation notes at `eaf88994c`. An earlier
`lf task status LOO-717 --json` produced no result before it was stopped.
PR #1811 remains this publication target; the primary Task operator owns the
subsequent authorized shipping and completion checks.
Product memory includes Desktop ownership and the runtime/GUI metadata distinction;
no child Product memories were found.

History: `e8385db949:scratch/questions.md` retains earlier implementation choices;
`500425244c:scratch/questions.md` retains prior reviews;
`b81df60579:scratch/questions.md` retains the superseded occurrence design;
`d5cab9ba60:scratch/questions.md` retains earlier credential failures.
The prior published walkthrough is `a1067c99cc:scratch/pr-review.html`.
Uncommitted notes before this compression are in
`.lf/tmp/loo717-compress/questions-before.md`; the pre-realignment evidence remains
at `/Users/jack/.lf/demos/loo717-500425244c/durable/questions-before-repair-realignment.md`.
The design and [Rex coordination](rex-coordination.md) own remaining work and handoff limits.
