# LOO-977 verify (2026-11-18)

Outcome: **Verified.** PR #1980 merged as `e55f40bde0`; this checkout's tree equals main's (`08b66358ed`), so `target/debug/lf` built here is main.

Real Codex login was still out of quota. Disposable repo, private `LF_HOME`, one-step Graph `ping` (`- step: say`):

- `agents: [codex, claude:haiku]`, `lf -b graph ping` → exit 0, printed
  `codex: provider usage limit; continuing on claude:haiku` then `pong`.
  `agent_limit_recoveries.reason`: `provider_rate_limited: You’ve hit your usage limit. … try again at 3:36 PM.; continuing on claude:haiku`.
  `agent_sessions.provider, model`: `claude|haiku`.
- `agents: [codex:gpt-6.1-astra, claude:haiku]` → exit 0, printed
  `codex:gpt-6.1-astra: model unavailable; continuing on claude:haiku` then `pong`.
- `cargo test -p loopflow-execution --lib -- agent:: harness::codex_decode::` → 53 passed.
- `cargo test -p loopflow --test agent_tests -- codex_usage_limit model_unavailable_result` → 2 passed.

Not checked here: `unauthorized` and `serverOverloaded` against a real provider (decode test only); the `lf task status` "went on under" line (QA saw it; shows only while a step runs).

Post-merge CI on `e55f40bde0`: merge_group CI success; push runs "CI" and "macOS on main" success; "Packages (main)" still in progress.
