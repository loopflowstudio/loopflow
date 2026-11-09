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

2026-11-09 implementation assumption: keep Codex's native terminal alive while
replacing only its private app-server; the disposable probe retains and submits
a draft across that replacement. Do not reuse `resume --account` for automatic
recovery: it ends the native client. Pending requests and uncertain dispatch wait.
The provider-generation/exit-record handoff and turn-arrival race remain a design
conflict to resolve before automatic recovery can be implemented or published.
An observer lock cannot prevent the native client from submitting directly to
Codex. Provider-side replacement admission and Claude retry/turn correlation
remain technical questions; no narrower recovery outcome has been selected.

2026-11-09 native drain finding: the replacement assumption above is invalid for
Codex 0.167.0 when input races shutdown. Provider-side SIGHUP admission exists,
but the native TUI exits 1 on `turn/steer`'s `serverShuttingDown` rejection.
Pending approvals remain answerable and a non-racing client survives replacement.
The design now requires native rejection handling or demonstrated managed-auth
reload before Loopflow can integrate automatic recovery. No narrower outcome,
credential-ownership change or installed-provider modification is assumed.

Security follow-up: the preceding assessment reported Session capture keys exposed
in process-list output. They need rotation; their values were not repeated or
inspected in this run. Production capture state was not changed.
