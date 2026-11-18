# LOO-963 verify (2026-11-17)

- PR #1970 merged as 8abce99e8d on main; merge_group CI success.
- Push CI on 8abce99: "CI" success; "macOS on main" still pending — not reported as passed.
- main: `CALLER_SESSION_ENV` + `remove_caller_session_env` in agent/mod.rs; called from harness/mod.rs `configure_agent_env`, harness/codex.rs app-server, native_session.rs, and the launch env map in agent/mod.rs. Named list (CLAUDE_CODE_CHILD_SESSION, CODEX_SANDBOX, ...), `CLAUDE_CONFIG_DIR`/`CODEX_HOME` not in it.
- Server/held-terminal env left unchanged by design (every provider start scrubs; held lf is the caller continuing) — recorded in concept review.
- `cargo test -p loopflow-execution --lib harness::launch_matrix::` on detached origin/main: 3 passed (every_harness_runs_all_four_launch_kinds asserts no inherited marker reaches any harness in any launch kind, CLAUDE_CONFIG_DIR survives).
- Desktop `ProcessEnvironment.swift` keeps a second, prefix-based scrub (`CLAUDE_CODE_`, `CODEX_`) but keeps `CODEX_HOME`/tokens via `nativeProviderKeys`, and `CLAUDE_CONFIG_DIR` matches no prefix, so the review's "drops CLAUDE_CONFIG_DIR" worry does not hold. Duplication only; no follow-up filed.
