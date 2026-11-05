# LOO-593 decisions and open questions

Jack Heart selected direct server writes in [server-write-direction.md](server-write-direction.md).
This supersedes the full-CLI subprocess design. Accepted delivery remains the
existing feature Graph and PR #1813 through publication and interactive demo;
no merge, Auto-Ship or Task completion. Direct execution is committed at
`2d9c295d8a`; adoption/binding repairs are checkpointed at `b11d49d32f` and
main through #1822 at `30b4394a57`. The working design tracks remaining proof.

The [working design](mobile-backend.md) owns the deletion list, invariants and
remaining proof. LOO-718 owns remote admission, pairing and iOS; do not edit its
checkout. Installed-state migration keeps the old plist and stable socket/lock,
not a second Work implementation.

The [direct-write demo](direct-write-demo.md) records native Task creation,
conversation, rename and save; [matched measurements](server-performance/results.md)
record the direct-write comparison. Both predate the final repairs and main
merge. Merged-build validation and automated composed installed-helper replacement
passed as recorded in the working design. Interactive demo/review owns rendered
acceptance and released-version upgrade proof. Earlier
subprocess evidence cannot substitute. No matched embedded RSS sample supports
a memory-savings claim.

GUI-less distribution, before-login hosting, removal during active work and the
phone takeover gesture remain separate product decisions. No new assumption
about them is needed for the accepted local cutover. Session-to-Task adoption now
shares creation dispatch; ordinary create-and-run retains its execution owners.

## Continuation decision — November 4, 2026

Jack Heart authorized continuation after three usage-limit failures and newer
capacity evidence. The existing feature Graph continued through implementation
and sync; the former absence of a live Graph is historical. No credential or
account-routing change was selected. Original retry evidence and stopped Graph
IDs remain at `0e597c17a1:scratch/questions.md`. No additional product decision
is required for the local cutover; final acceptance is still outstanding.

Settings-merge assumption (November 4): file settings retain access before Work
is compatible. The additive status/configuration operations preserve existing
bootstrap fields and route the incoming FFI consumers through the shared server.
This is an implementation choice within Jack Heart's complete FFI deletion.
