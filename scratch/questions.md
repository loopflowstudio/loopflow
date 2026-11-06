# LOO-717 — Decisions and remaining review

Jack Heart approved implementation and review publication. The authoritative
contract is [shell lifetimes](loo-717-shell-lifetimes.md). Task remains feature/demo;
shipping, installation, live-store migration and credentials changes are outside
this request. No product question is open.

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

The requested pursue `04ebc0ed-1382-4328-99e8-15a7d7e3696d` failed before edits:
implement Process `42c298fe-0173-4f43-93cb-514776d208fc` could not write its Claude
Keychain credential (“the item did not take the login”). After Jack's “hmm”,
the existing Codex conversation continued the authorized repair directly. The
failed Graph stays stopped; accounts and credentials remain unchanged.

The repair removes shell holds and indefinite unknown viewing. Dated evidence
on existing Session owners and a foreground SSH heartbeat give disconnects a
bound. The cleanup executor observes owner disappearance even while a Task is
viewed. A quick reconnect/leave between cleanup observations starts a new window.
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

## Proposed Wave memory correction

The current Product memory's LOO-717 description of indefinite unknown SSH holds
and Desktop-shell holds is superseded by Jack's November 5 decisions above.
Replace those claims with Session-only retention, explicit Desktop Quit leave,
and bounded disconnected absence. This direct repair leaves Wave memory read-only;
its next selected memory-maintaining Graph can reconcile that paragraph. No
change to LOO-498's host ownership or LOO-672's conversation-opening responsibility.
