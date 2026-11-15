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

2026-11-09 runtime admission: use one ephemeral Unix socket per recovery attempt,
held by the existing owner, instead of a second credential store or service.
Expire admission after 40 seconds; the recovery-only shell watchdog blocks before
native timeout. Claude's cache expires inside admission before taking the shared
credential lock. Clients without the guard stay waiting; cancellation or partial
installation retains the attempt as uncertain. Task fixtures use separate linked
checkouts under their disposable root.

2026-11-14 holder adaptation: keep the recovery button on Waiting for native
conversation. It checks the other affected conversations too; the reason and
sessions guide explain that an older client needs manual continuation. No client
is restarted to install its guard. Recovery belongs in the Machine server,
without restoring the deleted Task-update hooks or per-Session observer processes.

2026-11-14 concept review: the recovery row's states are named for what the
person does. `waiting_native` splits: its short waits join `waiting`; a rejected
turn the client cannot take becomes `manual_retry` ("Send it again yourself"),
decided at the rejection instead of after capacity returns. The button stays on
both, per the choice above. Labels and reasons were agent-written; nobody chose
them. Direction: interactive-limit-recovery.md, "Concept review — November 14".

2026-11-14 state implementation: a failed Codex `hooks/list` read stays `waiting`;
only a successful read showing no recovery hook establishes `manual_retry`.
The failure is a temporary inability to check, so it must not claim that the
conversation cannot take a resend. Both states retain bulk rechecks.

2026-11-14 concept review, second pass: the recovery row ends at the person's
next message in that conversation; newer input no longer turns a row into
"Review conversation". Agent-written, reversible: the alternative is a row that
stays until the next limit. One reason string, "No eligible login is ready for
this model", for both providers. Direction: interactive-limit-recovery.md,
"Concept review — November 14, second pass".

2026-11-14 QA repair: the recovery reader no longer filters on
`processes.agent_interactive`. Every Codex AgentProcess is an app-server
launched through the headless spawn path, so that column is 0 for interactive
Codex conversations too; `agent_sessions.interactive=1` is the remaining
interactive filter. Reversible alternative: set `agent_interactive` from the
Session when the interactive open starts the app-server. Each live interactive
Codex conversation now keeps one server side connection, because Codex runs no
hook for a provider-rejected turn.

2026-11-14 draft probe: use pyte to check the rendered PTY contents and native
raw/bracketed-paste readiness. Keep the synthetic rejection held until each
expected draft is visible; fail without re-pasting if readiness or visibility
is absent. A dedicated reader drains PTYs during blocking fixture commands.
This repairs the probe’s unverified timing; it does not attribute the earlier
missing input to a product defect.

2026-11-14 compress: keep completed strategy experiments in Git (source
reference in the design), and keep the surviving runtime probe in this
checkout. Future native validation should exercise that runtime.

2026-11-14 QA after main sync: an answered Codex refresh logs at info, which
the server's file filter (`warn,loopflow::server=info`) drops; only failures
reach `logs/lf.log`. Reversible alternative: log answers at warn or widen the
filter to `loopflow::harness=info`.
