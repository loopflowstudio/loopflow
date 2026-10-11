# LOO-450 shutdown repair

Jack Heart requested another CI repair. Failed SHA `18a7f81695`, run
38119214480, Rust job 114412869035. Installation passed on retry. Linux reproduction
confirms `launch_nonzero_exit` fails because the launch-owned lifeline watchdog
survives its naturally exited provider.

Design: retain the spawn-owned lifeline writer with its group PID. Under the exact
attachment fence, natural-exit shutdown can close that owned writer and wait for
its existing watchdog to settle the group. No owned writer means no new signaling
authority; unknown identity and foreign survivors still refuse. Do not infer group
death from leader death. FIFO holders from other attachments remain independent.

Record interactive/headless mode from the actual launcher, not the Session's
historical interactive flag, so a headless resume receives headless group cleanup.
Preserve native terminal groups, stale-attachment refusal and history. No schema
change. Verify the four reported failures in isolated Linux plus lifeline/ownership
proofs and static checks before arming with completion preserved.

Checks: four reported integration cases pass in a network-disconnected Linux container; network-isolated macOS `cargo test --offline -p loopflow --lib os_process::lifeline::tests -- --test-threads=1` (16) and `harness::agent_process::tests` (9), `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings` pass. Full materialized matrix remains CI-owned.

Review: the launch-owned writer is the authority to release its watchdog, not leader death; absent writer and unknown identity still refuse. Launch mode updates only the reserved AgentProcess, preserving Session history. Main-memory merge preserved both sides; duplicate migration/gate reminders were removed to fit the context budget (mechanics remain in MIGRATIONS.md and TESTING.md).
