# LOO-811 validation choices

2026-11-08: Use the existing `tests/e2e/claude_shared_home.py` with installed lf,
real Claude, synthetic logins, a temporary provider home and local inference
endpoint for reversible switch/hold checks. Shared production logins have open
conversations, so do not switch, refresh or restart them for a probe. These
checks establish native client/Keychain behavior, not provider token rotation.

2026-11-08 iteration direction retains USER plus one legacy item after every
switch; the earlier concept-review alternatives do not supersede it. Reversible
choice for services with more than two items: select the lexically first non-USER
account, independent of native lookup order. Update at most two items; third-item
behavior stays separate from the two-item guarantee. Existing unsafe account-name
filtering in the stdin writer remains unchanged in this focused repair.

2026-11-08 QA: exercise the repaired CLI with a temporary scoped Keychain service
and a read-only reload marker. Use synthetic accounts for supplemental native
checks; seeded partial state does not establish a rejected native write or live
login acceptance. Keep the installed pair and production conversations untouched.

2026-11-09 current direction: server restarts are allowed; see the exact words and
native findings in interactive-limit-recovery.md.
Continue implementation using the existing account broker, with native refresh
callback and quota-retry checks recorded in the design. The former credential-ownership choice is
no longer a reason to stop all work. No production processes are probe targets.

Security follow-up: the preceding assessment reported Session capture keys exposed
in process-list output. They need rotation; their values were not repeated or
inspected in this run. Production capture state was not changed.

2026-11-09 guarded submission: native prompt hooks are the simpler demonstrated
admission path for both clients; server replacement is unnecessary for the tested
cases. The probe guards classify only recovery envelopes, so ordinary input does
not depend on the recovery gate. Claim an attempt once: Claude requeues a cancelled
async wake, and Codex can acknowledge queued input with the existing active turn
ID. Native transcript validation occurs inside the gate. These choices do not
license killing a broker worker while it owns a rotated refresh grant; bounded
activation and cancellation remain part of integration, with unsupported cases
waiting or uncertain.
