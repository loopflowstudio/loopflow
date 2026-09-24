# Iteration 10 — configured launch and foreground boundary

The mounted Session-row/nested-workspace proof is complete in the preceding
[review](review-slice-iteration10.md): one test passes with four real PTYs.
The Session workspace and native test hashes match that receipt. This pass does not rerun
it or claim its authorship. No production correction was established.

This runner differs from the review's permission failure. Its read-only check
reported Accessibility trusted. One fresh configured trial reached the actual
Task conversation; a second stopped at foreground ownership. No permission
change is indicated by these observations.

## Executed configured trials

Used the signed disposable `/tmp/loo291-iteration8/Loopflow Proof.app`, executable
SHA-256 `75fdef9e4d35fed6deeb01c3995bddcd5c7e0c6e5b7f88ce733aff01c4ae1abe`.
Signature verification passed. The app used the actual repository and explicitly
aligned installed development Home, with no fixture/capture mode.

The first trial had AX trust, one accessible window, and exact system AX focus
on owned app PID 15245. New conversation created the exact Task Session
`run_eb31ad60301e4b0cb2c006224dd38b70`. Its shared terminal ID was
`DA2DAE6C-7556-4BBE-95EF-508E7AD1311E`; provider PID 21448, birth
`2026-09-24T04:26:57.344916Z`, descended from that app and produced an initial
response. This supplies fresh configured launch/attachment evidence only.

The probe then requested **Show work list**, although the list was already
visible and the app correctly offered **Hide work list**. It stopped before
Session-row selection. This is a probe assumption, not a production defect.
After the owned app exited, the exact Session was closed; it was completed
through the shared CLI. That cleanup is not UI-completion proof. This identity
is retired and must not be replayed.

Corrected the probe to accept an already-visible list and allocated fresh
receipts. The second app, PID 45385, reported AX trusted and one window, but
failed the exact system AX focus check before any UI action or provider launch.
Its cached active flag was true. This probe did not print the focused PID, so
the recipient and cause are unknown. No repeated activation attempt followed.

Launch-to-observed-Task was 7,001.2 ms; New conversation-to-initial-response was
8,329.3 ms in the first trial. Both include setup and traversal. Neither measures
pixel paint or verified terminal readiness, nor qualifies as a budget/trial series.

[First log](configured-ui-evidence/iteration10/implement/provider.log),
[corrected log](configured-ui-evidence/iteration10/implement/corrected/provider.log),
[cleanup](configured-ui-evidence/iteration10/implement/cleanup.log),
[final receipt](configured-ui-evidence/iteration10/implement/receipt.json).
Both owned apps and the recorded provider are absent. LOO-291 remains incomplete.
Other Session membership changed during this interval; no unchanged-population
claim is made. No user-owned Session was selected, moved or resolved.

## Concrete manual continuation

Open the disposable **Loopflow Proof.app** above and bring its window to the
foreground. This runner already has Accessibility permission; the failed check
does not justify requesting another permission. Use the Loopflow repository and
a fresh disposable conversation, not a Session from these archived probes.

1. Choose **All work → New terminal** to retain a main-checkout shell.
2. Select LOO-291 and choose **New conversation · LOO-291**. Wait for its new
   Session row. Record that exact Run and provider-client identity.
3. Add a companion with **New terminal**, then click the new Session row. It
   must focus its original provider shell without **Move here** or a new client.
4. Choose **Split worktrees right** and select the retained main checkout.
   Add a companion there. Selecting the Task Session must focus its existing
   checkout slot while both groups retain their inner panes.
5. Leave an unfinished provider draft, inspect Work details, and return through
   its exact row. Submit and compare the exact provider-history text. Complete
   only this disposable Session; its original shell and companions must still
   respond, and the Task must remain incomplete.

Record configured identities and responses for this procedure. The existing
native test cannot substitute for them. Preserve prior configured draft,
completion and viewport receipts; do not repeat those trials as a replacement
for the missing row/nested interaction.

Review found one probe assumption to remove and no new production owner, policy,
or source defect. This pass changed no executable source and ran no tests or
broader gate. A final status check found concurrent edits moving out-of-plan
diagnostics into Wave details, plus their test/README and human-demo/design notes.
Those edits are preserved and belong to their existing writer. The configured
receipts identify the preceding signed binary; they do not validate that new
working tree. No further app interaction was attempted after observing the
human-demo notes. Full LOO-291 still requires the shared LOO-284 contract, directive
editing, other scopes/destinations, human-selected external trials and measured
budgets. No publication, landing or Task completion occurred in this pass.
