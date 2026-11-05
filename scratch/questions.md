# LOO-593 decisions and open questions

Jack Heart selected direct server writes in [server-write-direction.md](server-write-direction.md).
This supersedes the full-CLI subprocess design. Accepted delivery remains the
existing feature Graph and PR #1813 through publication and interactive demo;
no competing Graph, merge, Auto-Ship or Task completion. Direct execution is being verified in the working diff; merge `59287ba3ed` resolved the earlier main conflicts locally.

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

## Continue after capacity failure

Jack Heart requested continuation after Graphs `5547bd41`, `562387e1` and
`3130799e` stopped on Codex usage-limit errors. Fresh Task/Process reads show no
live Graph. Newer cached Codex capacity observations report headroom; this is new
retry evidence, not a credential or account-routing change. Resume implement in
the existing feature graph and stop at demo after publication. Preserve the whole
working direct-write diff, the single Task migration, FFI deletion, request logs
and native proof. Inspect pending test results before rerunning them. The accepted
`server-write-direction.md` supersedes all subprocess-era acceptance and requires
fresh matched measurements; `direct-write-demo.md` records the newer UI proof
and its limitations. A fresh failure with unchanged capacity must be reported,
not retried in a loop.
