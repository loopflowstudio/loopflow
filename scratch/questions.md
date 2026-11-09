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

2026-11-09 open technical questions are in interactive-limit-recovery.md:
Codex drain rejection must retain the client and input, or a different demonstrated
managed-auth mechanism must avoid replacement. Pass 3's native account reads
retained the old login after a cross-account file change; same-account rotation
reloaded successfully. Both tested Codex paths fail the design. Claude inbox UUID
joins through transcript `promptId` to Stop `prompt_id`; the two IDs are distinct.
No narrower outcome, credential-ownership change or installed-provider
modification is assumed. The dependent implementation needs a revised Codex
mechanism; repeated probes of drain or `account/read` cannot settle that choice.

Security follow-up: the preceding assessment reported Session capture keys exposed
in process-list output. They need rotation; their values were not repeated or
inspected in this run. Production capture state was not changed.
