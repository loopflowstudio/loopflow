Sync onto main at `28b0af568d120b56113c0dfb0eff9439d0c014d5` completed locally with `lf sync --continue` on 2026-11-09. Provider transitions travel through the live terminal status socket alongside native readiness. The waiting sync caller owns the push.

`scripts/test_desktop.sh --filter 'ServerConnectionTests|ProviderTransitionTests'` — passed: 7 tests; Rust CLI and Desktop compiled. Full gate/static checks remain with the publishing run. Log: `/tmp/loo-810-sync-tests.log`.
