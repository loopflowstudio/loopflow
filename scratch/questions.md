
## Intermittent: forwarded_secrets_reach_a_remote_launch_only_by_name
CI run 45568405396 (job 139049897611, head 144f6d4dd3) failed it with
`execution failed: invalid control authority: headless AgentProcess identity is unavailable`
(`harness/agent_process.rs` `stop`, when `close_session_agent_process` finds no
attachment, agent row or recorded pid). Passed on the prior CI run and locally
(40s). Code untouched by this branch; not fixed in the ci-fix commit.
