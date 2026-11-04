# LOO-672 — Ready Task conversations

Re-evaluated November 4, 2026 against merged main `ac8e088f5f`, LOO-717 / #1811
and LOO-498 / #1814. Jack Heart approved proceeding after this review on November 4:
“ok, proceed.” Implement the accepted experience through interactive demo and
review publication; this does not approve shipping or installation. Routine
implementation choices are delegated to the build. The old plan is archived
at `research/loo-672-pre-infrastructure-plan.md` and is superseded here.

## Accepted direction

Jack Heart wants automatic conversation opening, fast switching between Tasks,
and management that saves more machine resources than it consumes. Return lands
at the bottom; exact old scroll position is not required.

On November 4 Jack selected **keeping recent unfinished Tasks warm**, accepting
more memory for instant input. Completed Tasks still release on departure under
LOO-717. This changes that branch's current Desktop implementation, which releases
unfinished Tasks on departure too; update that owner rather than add another hold.

Jack initially selected last-screen + Take back and Fork, considered shared
input, then chose takeover without Fork. Keep the explicit **Take back** click
from the selected interaction. Selecting alone does not transfer input from Rex
or another window. No Fork button or simultaneous-input feature in this delivery.
The CLI fork remains independently available: it creates another conversation
on the same Task and checkout from the first conversation's log, not another Task
or isolated files.

Viewing never starts a Graph, submits a turn, creates a provider thread, or
replays interrupted input. Explicit Task launch retains its existing authority;
navigation observes its resulting conversation.

## Proposed user guide copy

Select a Task to see its conversation. Recent unfinished Tasks stay ready for
typing when you switch back. Older conversations return automatically through
their saved screen and provider history, at the bottom, without an Open click.

If Rex or another window controls the conversation, Desktop shows its last
available screen with **Take back**. Click to continue here; the previous terminal
reports the move. With no saved screen, show the external-holder explanation.
Display Rust's prepared command alongside the action.

Close removes the conversation from Desktop's warm set. Refresh does not reopen
it; deliberately reopening/selecting that Task later can resume it. Requested
work, drafts and pending questions keep their shared lifecycle protections.
Closing a view is not Task completion.

After restarting Desktop, the selected Task opens first. The bounded saved warm
set of unfinished conversations reconnects in the background where safe. Other
saved views remain cheap until selected. Saved screens are history, not proof
of readiness: show a small connecting indication until input works and an honest
error with Retry if continuation fails. Do not promise instant cold input.

Completed Tasks remain available while selected. Departure releases their
Desktop hold. Under LOO-717, eligible idle agents end after two minutes and
eligible shells/checkouts retire at fifteen minutes total; returning cancels
retirement. History remains afterward. Automatic deleted-checkout reconstruction
is outside scope.

This is proposed guide copy, not a claim about the installed app.

## Delivered foundations and remaining integration

| Work | Observed state | What 672 reuses |
| --- | --- | --- |
| 708 / #1802 | Done, merged | Thin viewer, poll-based owner, Claude launcher removal. Codex/OpenCode still retain launchers. |
| 709 / #1804 | Done, merged | Two-minute idle policy and protections for turns, watches, drafts and pending requests. |
| 710 / #1801 | Done, merged | libghostty state, persisted `.ghostsnp`, VT repaint while resuming. Replay has bounded history and does not repaint images. |
| 712 / #1800 | Done, merged | Binary transport, byte-counted reconnect, separate appearance ownership and input control. |
| 720 / #1809 | Done, merged | Provider-owned status, status replay on attach and repaired Desktop association. |
| 717 / #1811 | Open PR at demo | Shared host retention and coordinated retirement. Latest confirmed-host-exit refinement is still pending. |
| 498 / #1814 | Open PR with delivery gaps | Rex native workspace adapter, occurrences and release evidence; not implemented again here. |

720's accepted candidate Claude/Rex/Desktop demonstration and CI passed. Its
machine-wide installation and live Codex/OpenCode behavior were not established.
719's installed macOS idle-preservation proof remains unstarted. Keep those limits
visible in combined acceptance; do not duplicate the infrastructure.

## One concrete interaction

1. Select A: reuse its existing primary conversation automatically. No resumable
   history means the existing explicit Start/New action, not an implicit first turn.
2. Select B, then A: A stays warm, preserves provider/editor identity and accepts
   input immediately; return scrolls to the bottom.
3. Visit enough Tasks to evict A: release Desktop's view and hold through 717.
   Never kill providers to meet a cache count. Work/drafts/questions/other hosts
   can keep A's owner alive.
4. Return to cold A: attach the surviving owner or paint the existing saved screen
   while resuming the same provider thread. No turn replay.
5. Resume A in Rex: Desktop loses input and shows Take back over its last screen.
   Polling/navigation cannot take it back; the explicit click can.
6. Complete A and leave: remove it from the warm set and let 717 own retirement.
   A cached screen cannot renew retention or recreate the deleted checkout.

## Implementation proposal

### One bounded Desktop warm set

Reuse the retained repository workspace and terminal pool. Track recent explicit
Task visits, not status polls or passive arrivals. Start with a target of **eight
recent unfinished Task conversations**, including selected conversations. This
is a provisional engineering value, not Jack's chosen budget. Actually visible
panes remain retained; report any explicit multi-pane exception instead of
evicting something visible. Never share an NSView across windows.

Warm surfaces and host holds must agree. Eviction, Close, completion-and-departure
and app exit release through the existing lifecycle. A hidden attached viewer
must not defeat the idle clock after release. Ordinary shells keep their own
lifetime; no shell LRU or provider termination manager is added.

Save warm intent beside workspace state. Restore selected first, then the bounded
eligible saved set with limited concurrency (initially two preparations). Recheck
completion, closure, location and external control. Saved layout is not a live
host hold. An unavailable remote Machine never becomes a local replacement.

### Make automatic opening safe at execution time

Current `SessionsStore.adoptPreparation(.attach)` accepts only a detached record;
`prepareSession` returns a takeover command. This prevents auto-resuming ended
conversations and leaves a race between checking availability and attaching.
Rust's `open_waiting` can also initiate captured input with no provider history.
A Swift-only `opens_turn` check cannot establish the required behavior.

Extend the existing preparation/launch path with typed **reveal existing** intent.
Rust resolves: attach a free existing owner, resume existing native history,
report external control, require explicit initial start, or report unavailable.
Recheck under current launch/claim boundaries, carrying non-replacing intent into
the actual attach (`replace: false` already exists in the protocol). Another
client winning the race produces Take back, not takeover. Unknown liveness does
not authorize an automatic provider stop.

Explicit Take back uses ordinary resume. Keep the public CLI's simple semantics;
reuse its machinery rather than build another resolver. Exact wire shape is an
engineering choice, with strict Rust/Swift fixtures updated together. Rust owns
holder labels and executable actions; Swift does not invent either.

### Reuse screen and status delivery

Use the existing VT relay for terminal repaint. The elsewhere overlay retains a
bounded last frame/excerpt; missing preview is an honest text state. Do not add
another durable screen format or GhosttyKit snapshot-decoder project. Current
Working/Blocked/Done comes from 720's projection; a saved badge is not live evidence.

Use actual native readiness for input and preserve failures with Retry. Do not
buffer surprise input for another owner. Screens cannot restore native editors
or pending requests; 709 protects their running owners.

### Delete replaced behavior

Replace the Task exclusion in `revealPrimaryConversation`, click-only recovery
for resumable history and blank elsewhere placeholder. Update existing tests.
Remove the old plan's detached-owner quota, second idle timer, blanket reboot
sweep, Claude-only Fork work, Fork button and `opens_turn`-only guard. Change
717's unfinished departure policy at its existing owner; keep completed release,
explicit first-turn/Graph actions, Close and failures distinct.

## Delivery and proof

The original infrastructure landing prerequisite is satisfied. The revised
product direction is specific enough to implement after this review. Integrate
after 717 lands or explicitly stack on its PR; do not recreate its unmerged
retention contract. 498 need not finish every workspace feature first, but the
cross-host demo must name the adapter being exercised. Jack's subsequent proceed
instruction authorizes implementation through demo, not shipping or installation.

Focused checks must cover:

- Selected existing Session opens once; late preparation cannot reopen an evicted
  or closed view; passive arrivals do not warm unseen Tasks.
- A→B→A preserves warm surface/provider/draft identity; eviction leaves protected
  work running; completed Tasks lose background warmth; Close defeats refresh.
- Another host claims between prepare and attach: auto-open does not displace it,
  duplicate a provider or start a turn; explicit Take back transfers once.
- Cold native history resumes; missing history/checkout/auth/location stays an
  explicit action or failure, without implicit thread creation or local fallback.
- Restored warm intent is bounded, selected-first, and cannot reclaim externally
  controlled or explicitly closed conversations in the background.
- 717's return cancellation, other-host retention and final-work protections win
  over cache bookkeeping. Completion changes state independently of provider exit.
- 720's current status survives switch/reconnect/takeover; silence is not idle.

Demonstrate real Desktop with Claude and Codex: warm switching and typing, cold
return, takeover from Rex, app relaunch and stale-owner/reboot recovery in a
disposable environment. Measure first paint and input-ready separately, plus
Desktop + lf + provider private footprint with twenty visited Tasks. Record
protected owners and visible-pane exceptions. Eight is a starting value, not a
claimed budget or result.

Review checks: source/design reconciliation only; no runtime acceptance claimed.
