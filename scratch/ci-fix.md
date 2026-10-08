# LOO-428 CI repair — 2026-10-08

Jack Heart's queue-and-land authorization remains in effect. PR #1502 at
`4dac21a6001d6990531f081e68c696f4ae9fb13f` failed only the Rust bare-conversation
regression (run 37776351087). Removing the skill-name mode override exposed that
the bare entry substituted `default` before mode resolution. Bare dispatch now
selects interactive execution unless `-b` is explicit, including bound launches;
named skills and Flow steps retain their resolved mode.

The provider fixture captures argv, the native system file and headless stdin,
and verifies conversational context and both bare launch modes. It uses isolated
homes without accounts. TESTING.md includes this suite for mode-selection changes.

Checks: original failure reproduced; repaired `default_conversation_tests`, focused
`flow_output_shows_steps_and_agent_messages_with_opt_in_diagnostics`, fmt, Clippy
and diff check PASS. Hosted CI is pending the repaired head; actual cmux tracking
and installed acceptance remain unverified. LOO-422 owns the OSC 7501 follow-up.
