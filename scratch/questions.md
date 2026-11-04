# LOO-593 decisions and open questions

Jack Heart selected direct server writes in [server-write-direction.md](server-write-direction.md).
This supersedes the full-CLI subprocess design. The live feature Graph must
return to implement within its authored loop, finish the shared CLI/server
mutation path and FFI deletion, resolve PR #1813's conflicts, publish, then stop
at interactive demo. No new Graph, merge, Auto-Ship or Task completion.

The [working design](mobile-backend.md) owns the deletion list, invariants and
remaining proof. LOO-718 owns remote admission, pairing and iOS; do not edit its
checkout. Installed-state migration keeps the old plist and stable socket/lock,
not a second Work implementation.

The contributing conversation owns the final native Task creation/conversation/
rename/save demo and matched performance measurements on the completed build.
The older [demo](server-cutover-demo.md) and [samples](server-performance/results.md)
remain dated evidence, not acceptance of direct writes. No matched embedded RSS
sample supports a memory-savings claim.

GUI-less distribution, before-login hosting, removal during active work and the
phone takeover gesture remain separate product decisions. No new assumption
about them is needed for the accepted local cutover.
