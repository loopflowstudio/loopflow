# LOO-811 validation choices

2026-11-08: Use the existing `tests/e2e/claude_shared_home.py` with installed lf,
real Claude, synthetic logins, a temporary provider home and local inference
endpoint for reversible switch/hold checks. Shared production logins have open
conversations, so do not switch, refresh or restart them for a probe. These
checks establish native client/Keychain behavior, not provider token rotation.
