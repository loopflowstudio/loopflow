# LOO-717 — Decisions and remaining review

Jack Heart approved implementation and review publication. The authoritative
contract is [shell lifetimes](loo-717-shell-lifetimes.md). Task remains feature/demo;
shipping, installation, live-store migration and credentials changes are outside
this request. No product question is open.

## November 6 follow-up — accepted scheduled cleanup and generic launch

Jack Heart accepted one minute-based OS cleanup sweep per Machine, with the
existing retirement rows as its durable queue, replacing the waiting process per
Task. Eligibility normally begins in 15–16 minutes while awake; existing viewer,
work and Git safeguards remain. Each invocation exits; later invocations recover
interrupted work and retry failures with backoff. This includes the previously
deferred retry finding. Reuse installation's launchd/cron approach without combining
the updater and cleanup jobs. The development run must not register a live schedule.

Jack also accepted a generic shell-launch boundary with Desktop explicitly
supplying Ghostty integration. Reuse existing scripts and terminal conventions;
remove hard-coded Ghostty bootstrap selection from shared Task shell launching.
No new shell integration implementation or host registry. Preserve remote startup,
command blocks, LF availability and identity; reconnect never replays bootstrap.

The authoritative contract contains the deletion list and verification cases.
The follow-up is implemented at `d14b32ada0`; focused cleanup, installation,
local/loopback-SSH shell and headless Desktop checks passed. Subsequent concept
review found a cancellation gap described below, now repaired at `6d42ec8ce`.
Publication verification and test repairs passed across the original run and
focused reruns at `3c57ec3bf`; the design records counts and the initially failed
aggregate run. Publication precedes demo review.
No ship, installation or live-store migration
approval is implied.

Implementation choices, November 6: retry delay starts at sixty seconds, doubles
per cleanup failure and caps at one hour. A returning viewer clears retry state;
absence timing stays separate. The account's normal installation provisions the
schedule; alternate HOME or LF_HOME skips account-wide registration. Desktop sends
a generic JSON startup script plus relative UTF-8 resources, materialized under
a content-addressed destination cache. Reattachment ignores creation inputs.

## November 6 — one viewer protocol

Jack Heart selected option 1 after reviewing alternatives: remove the added
`lf session remote` terminal wrapper and extend the existing viewer protocol
across SSH, with heartbeat expiry on that connection. A byte-forwarding bridge
may remain; another PTY or heartbeat protocol may not. The current design contains
the complete implementation and deletion contract. This changes transport,
not the accepted cleanup timings, Session-only retention or Rex ownership.

Implementation choice, November 6: the byte bridge keeps at most 4 MiB queued
in each direction, disconnects a full queue like the owner does, and allows two
seconds to drain after the owner closes. This bounds a stranded helper when SSH
stops reading; it supplies no heartbeat and
owns no Session retention deadline. Remote saved history uses the existing
snapshot frame before attachment, with no input or retention claim. Command-pipe
attachment allows sixty seconds for SSH setup and destination resolution; the
existing five-second bound still applies to a local owner socket.
The existing `Controlled` query supplies the ten-second liveness pulse on an
attached connection; unattached control probes refresh no viewer. This keeps
one protocol and adds no message an already-running owner cannot decode.

The later scheduled-cleanup decision above includes the cleanup-error retry repair.
The historical PR walkthrough at
`scratch/pr-review.html` describes the prior published head
`a1067c99cc`, not this replacement.

## November 5 corrections

Jack clarified that only open LF viewers of Task Sessions retain the checkout.
Desktop shells already accompany those Sessions and add no hold. Rex owns its
shells and arrangements. A native tab remaining after its LF viewer exits adds
no retention, and LF keeps no external-host occurrence registry.

Jack accepted explicit viewer release on orderly Desktop Quit and the same
fifteen-minute absence window after detected network loss. Reconnect cancels
cleanup; active requested work, drafts and pending requests remain protected.
Expiry does not prove a remote client died. There is no extra fifteen-minute
window after expiry, and no automatic deleted-checkout restoration.

The repair removes shell holds and indefinite unknown viewing. Dated evidence
on existing Session owners and the shared viewer heartbeat bound disconnects.
The earlier cleanup executor observes owner disappearance even while a Task is
viewed; the accepted sweep must preserve that behavior. A quick reconnect/leave
between cleanup observations starts a new window.
Desktop Quit delivers the existing leave input and waits briefly for relay exit.
Tests/builds and native proof limits are recorded in the design.

## Code review findings

The simulated ownership/failure review found concrete repairs: a brief reconnect
must revoke already queued cleanup, initial terminal dimensions must be applied
even at 24×80, and a child closing its PTY must not block heartbeat expiry while
it keeps running. The implementation and focused regressions cover these paths.
Interrupt cleanup also restores borrowed standard-descriptor flags. No additional
host state, configuration option or provider-launch path was introduced.

## Review and evidence still needed

Concept review, November 6: design refinement, no open product choice. Returning
Task Session viewing must revoke queued retirement closes before the next sweep.
At `d14b32ada0` the viewer wrote only Process evidence, while a separate shell owner
can still consume `close_requested=2` against the stale `closing` phase. Record
return and cancellation together and recheck current Task retention when an owner
consumes automatic closure. Keep explicit Close, Session-only holds, ordinary
conversation pass-through and the finite scheduled sweep. The working design
contains the source counterexample and the required no-intervening-sweep test.
The repair at `6d42ec8ce` records return and cancellation atomically, including a
brief return/leave coalesced between recorder polls. One dated observation now
records both the viewer flag and checkout hold, replacing separate writes/retries.
Owners reuse the sweep's
viewer judgment before automatic closure; explicit Close remains effective.
The old Desktop README promise that Undo keeps a closed shell running is corrected.
Focused regressions passed, including real shell/job survival without an
intervening sweep. The nullable fixture assertion found during rollback testing
was corrected and passed. The design links the recorded commands/results; gate and native
demo remain separate.

Native Desktop and Rex demonstration remains at feature/demo. Framing tests are
real local PTY/process tests, not a physical remote-Machine demonstration. A
separately opened external SSH session still relies on its host's disconnect
detection; LF's heartbeat bound applies to LF-owned remote routing.

The private demo app/provider has not launched. Preserve its durable history and
runtime roots; see loo-717-demo.md. Earlier review notes are retained at
`500425244c:scratch/questions.md`; uncommitted pre-realignment evidence is at
`/Users/jack/.lf/demos/loo717-500425244c/durable/questions-before-repair-realignment.md`.
The superseded external-host occurrence design remains at
`b81df60579:scratch/questions.md`.

## November 6 reconciliation

Product memory now also reflects the scheduled sweep and Desktop-supplied shell
integration, alongside Session-only retention and the single viewer protocol.
No Product child memories exist in this checkout. Earlier
credential failures and the proposed memory correction remain at
`d5cab9ba60:scratch/questions.md`; they add no remaining implementation work.
Source inspection confirms the committed cancellation repair and README correction;
no further bounded mismatch was found. Existing focused passes remain applicable.
The plan separates those passes from the older publication checks, broader gate
and native demonstration still owed.

Concept-review recheck at `6d42ec8ce`: no further design change or open product
choice. The working design and `concept-review.html` now distinguish the repaired
between-sweep cancellation from its historical counterexample. Gate and native
demonstration remain unverified by this source-only review.
