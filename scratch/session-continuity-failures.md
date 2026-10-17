# Continuity failures and intentional restart

October 16, 2026. Jack Heart accepted the per-Session owner direction and requested
an audit of recurring loading, stalled selection and forced-restart bugs. Source
baseline `70f6a628e2`; historical commits are evidence of failure classes, not proof
that every old implementation or incident still applies. No production changes.

## Historical evidence

The [full audit](evidence/session-continuity-failures-full.txt) retains commit-by-commit
findings and limits. Principal cases:

- `8990250f8f`: reconnect scanned 234,483 receipts; exact lookup reduced controlled
  warm-host prompt readiness from ~54s to ~1.8s. Source: startup benchmark dated
  20261016. Not a measured current Desktop incident.
- `cb4dd53c94`: provider I/O held SQLite locks and blocked timeout execution.
  Short transactions and independent transport progress remain necessary.
- `15595e7672`: optional title-hook store waits stalled prompts. Optional decoration
  must remain outside readiness.
- `e0e7a74d41`, `def8881d82`, `b82876138c`: missing launch/death evidence stranded
  conversations. Preserve exact identity and startup acknowledgement, not old tmux paths.
- `81910e6e4d`, `ebca186949`: stale activity/elsewhere readings misrepresented health.
  Execution and viewer state need separate evidence.
- `8181b61c14`: a lingering spinner overwrote native input. Separate output ownership.
- `27a07643fb`: missing native context binding caused hook failures. Preserve thread,
  account, context and request continuity.
- `8f0895e62c`, `dd78ac59dc`: repeated refresh/redraw delayed retained Work. Render
  cached layout independently of fresh inventory, without trusting cached liveness.

`3e1e6245c2` stopped orphan providers when driving lf processes died: move the
lifeline to the owner, never disable supervision. `9961412553` preserved Desktop
clients during CLI-only install, but Desktop still pins its bundled helper;
installed CLI improvement does not prove Desktop adoption or upgrade continuity.

## Current paths that still need explicit treatment

`SessionsStore.select` already has a retained-surface fast path. Preserve it;
selecting A → B → A should do no preparation at all. Its recovery path awaits
`prepareSession`, which invokes `lf session connect --prepare --json --try`.
`MacLocalWaveAgentLauncher.run` drains pipes concurrently, but waits for child exit
and pipe EOF without a deadline or cancellation path. A stalled command can leave
`.opening` pending indefinitely. This is source evidence of exposure, not a new
reproduction of Jack's past hang.

`SessionItem.State` currently carries pending/elsewhere/opening/prepared/live/failed/
ended. `onSurfaceCreated` calls `recordPaneLive`: that measures surface construction,
not editable provider readiness. `noteSurfaceClosed` maps child termination to
elsewhere or ended, while the provider may have a distinct lifetime. The accepted
owner design makes these distinctions unavoidable.

Use existing shared records plus transient viewer state to answer three questions,
without persisting a second Session liveness enum:

- Is the owner/provider alive, ended, or unverified?
- Is this viewer connecting, attached, disconnected, or lacking input control?
- Is the provider accepting input, working, or awaiting an answer/approval?

Warm switch: retain the pane and focus immediately. Reattach: keep known content
visible with connection progress; don't replace it with an empty loading screen.
An attachment deadline ends that attempt with a retryable connection error, not a
provider stop. Cancellation abandons only the viewer operation; late responses
must not attach to a newly selected pane or submit input. Preserve process and
request evidence when acknowledgement is uncertain.

Measure selection → surface mounted, attachment acknowledged, and input usable
separately. Existing surface-created timing cannot stand in for all three. Log the
Session, owner Process, viewer attempt and stage so a stuck operation is attributable
without logging private prompt contents.

## Restart as a chosen experience

Jack wants restarting to be safe and pleasurable: a deliberate way to reduce
context or begin fresh, after ordinary continuity is reliable. Continuity must
not make a long conversation compulsory. Keep these operations distinct:

| Intent | Preserve and change |
|---|---|
| Reconnect a viewer | Preserve the running provider, native thread, turn and requests. Replace only the connection/viewer as needed. |
| Recover execution after verified loss | Preserve saved conversation history; start a new process deliberately. Show lost live state and uncertain delivery; never replay an uncertain prompt automatically. |
| Start fresh with less context | Retain access to the prior conversation and make carried context explicit. A successor or provider-native context operation is a later design choice. |

“Safe restart” cannot mean undoing external actions or guaranteeing an in-flight
turn survived. The eventual interaction must make any running work and pending
approval visible, then distinguish leaving that work running from interrupting it.
No destructive reset of history, automatic compression, silent cancellation or
successor creation is selected by this research. A distinct fresh-context product
Task can follow once reliability is established; none was filed by this audit.

## Required proof

The [full audit](evidence/session-continuity-failures-full.txt) retains the detailed
scenario list. Integrate with the implementation plan and [readiness audit](session-readiness.md):
twelve retained Sessions without preparation; viewer death during work/approval;
provider death separately; stalled transport/store without cross-Session blocking;
startup acknowledgement failures and stale claims; context hooks and actual
bundled helper/owner upgrade behavior. Terminal-state restoration remains unproved.

Review finding: a healthy owner removes viewer-induced execution loss, but does
not remove unbounded waits, transport deadlocks or false readiness. Require all
four properties: independent lifetime, bounded attachment work, truthful derived
state, and explicit recovery. Do not add another supervisor/state store to patch
missing evidence.

Checks: local source/history reviewed; documentation-only changes, no runtime
regression suite or new performance measurements claimed.
