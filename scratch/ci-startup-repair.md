# LOO-415 startup CI repair

Jack Heart requested continuing authorized landing of PR #1581, without weakening the startup row bound or suppressing hooks. LOO-503 and its PID 57749 remain independent and untouched.

Failed published head: 510899fc243424d7d01aa2b4665ed0160dad80f8, run 39045073824. The bare startup check read 25,126 SQLite rows; the prior repair worker reproduced 25,083 locally. That worker stopped during Session recovery before publication. Its temporary instrumentation is preserved in /tmp/loo415-startup-investigation.patch; it has been removed from production and test code.

The trace at /var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/.tmpLdmjsb/bare/stderr showed repeated journal connection/schema setup. Each additional transport process repeated it on start and completion. Retain the first successfully opened ledger connection in the Process context; leave failed opens uncached so later receipts can retry. Existing early observation retains its noninitializing connection. The performance measurement and its limits are unchanged.

Review: reuse has Process lifetime and shared context clones, no cross-process cache or new schema authority. Keep interruption/contended-write settlement checks and startup integration evidence before publication. Prior PTY EOF and ancestry fixes remain intact.

Validation: cargo fmt --all --check and cargo clippy --all-targets -- -D warnings passed; network-isolated agent_startup_tests passed (bare 21,852 rows/1,618 statements, reconnect 18,509 rows/1,340 statements; original limits unchanged); all 20 journal::tests passed. Logs: /tmp/loo415-startup-checks.log and /tmp/loo415-startup-tests.log. Initial relative network-wrapper invocations failed before test execution; absolute-wrapper runs above passed. Hosted Linux CI remains to verify on the new head.
