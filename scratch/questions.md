# Open after ci-fix of 2feb438201

CI's earlier "green" runs on this branch (b69572de3d, c41dabf1f7) skipped
`rust-test`; these integration failures were present from 5d0a9a4f17 on.

Nine integration tests still expect a direct `lf -i --task/--wave …` launch
to run managed (detached or held, Codex behind its app-server, the provider's
startup failure as the exit status). The plan says a direct launch is a chat
or batch "including with `--task`, `--wave` or `LF_AS`", so they need rewriting
to reach a work session (a Task/primary reopen) or to assert chat behavior:

- context_launch_tests::codex_startup_failures_preserve_input_and_allow_reconnect
- context_launch_tests::codex_terminal_delivers_large_input_and_approvals_then_reconnects_without_replay
  (as a chat, the skill body and instructions ride Codex argv: E2BIG on Linux
  past 128 KiB per argument. That is a real limit for large-skill chats.)
- context_launch_tests::terminal_context_refreshes_without_replacing_instructions_or_losing_failed_sessions
- land_tests::waited_task_landing_reports_a_repair_run_that_opened_no_session
- session_launch_contract::another_agent_resumes_a_failed_codex_bootstrap_on_its_own_input
- session_launch_contract::live_taskless_provider_switch_preserves_identity_and_accepts_terminal_input
- session_lifecycle_tests::personal_task_launch_resume_and_skill_graph_keep_native_identity
- session_lifecycle_tests::repository_upgrade_and_move_resume_the_same_native_conversation
- session_lifecycle_tests::ensured_conversation_recovers_native_exit_before_history_is_published

Open question: should a direct Task/Wave `-i` with no terminal stay a chat,
or should the design admit a work session there? The tests assumed the latter.
