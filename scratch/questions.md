# Decisions and assumptions

2026-11-04: Jack Heart accepted 4h/1h/15m idle-output retention and separately
required a 24h minimum for owner-terminating cleanup. The newer laptop-wide,
coding-only instruction supersedes prior general cache/Trash/history pruning.

2026-11-04: For reversible ambiguity, default to observing unknown temp/demo
folders. Known pytest/synthetic screenshot fixtures are disposable; other demos
must explicitly mark their disposable runtime root. Durable conversation/history
roots never become candidates merely because they are old or contain SQLite.

2026-11-04: The single runner will install into a private machine-local directory
and require local opt-in. Chrome cleanup is a separate explicit installation
option, authorized for this machine only. Other users receive no job or opt-in
through package installation. Measurement totals overlap and APFS allocation is
not exclusive physical usage; report comparable growth without claiming causality.

2026-11-04: Real scheduled discovery reached its 30-second bound even after
breadth-first traversal. Use Git's ignored-directory decisions to avoid scanning
large data trees, and Git's worktree registry to retain coverage of registered
checkouts under ignored containers. Unknown unregistered repositories hidden in
ignored containers are not recursively discovered; explicit source placement or
registration supplies coverage. This reduces repeated scanning, not source
preservation authority. The scheduled receipt must retain its actual partial
failure; the corrected successor requires another installed invocation.

2026-11-04: Read LOO-725's current demo integration notes. Its shipped skill retains
private provider history in unmarked demo roots; it deliberately does not depend
on this laptop-only cleaner. Keep that history intact and report unmarked roots
with an explicit retained reason. Marked disposable roots exercise teardown and
reclamation; generic product demos do not auto-enroll. Splitting their durable
evidence from disposable runtime remains a cross-Task review choice, not inferred
permission to erase retained history or tie the shipped skill to this machine job.

2026-11-04: Jack Heart's later accepted retention rules supersede the generic
24h temp policy. Unknown historical outcomes use the seven-day failed window;
new repository pytest/gate runs record their observed outcome. Size caps govern
native reproducible caches, not reserved evidence, volumes or live output. Native
locks and recent/in-use records can defer reaching a cap; receipts expose this.

2026-11-04: The unused-dependency approval keeps held `.venv`/`node_modules`
roots intact regardless of age. Owner teardown remains for generated build output
and disposable runtime/evidence; dependency users are preserved.

2026-11-04: Foreground discovery took seven seconds; launchd background discovery
still exceeded 30s under concurrent load. Reuse each repository's worktree
registry across its checkouts and give background discovery 120s, within the
existing 30-minute total and 100,000-directory bounds. Partial receipts stay
failed; the corrected installed invocation must prove the behavior.

2026-11-04: Retention proven by root metadata needs no complete tree walk.
Unknown allocation stays null. A concurrently removed root is retained in the
receipt as changed during inspection and earns no reclaimed-byte credit. Fair
usage sampling rotates through least recently attempted roots; deltas use the
last known value and timestamp, never treating unknown measurements as zero.
