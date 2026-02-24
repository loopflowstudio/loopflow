# Open Questions

- **Wave name ambiguous.** No `<lf:wave>` tag in prompt, branch name `jack-heart.wavemodel.20260223_1611` doesn't follow `<wave>.main` pattern. Evaluated across all waves and picked `security/04-api-surface-gating` based on: explicit "Phase 04 next" in security README, security phase 03 just shipped in recent commits, prerequisites met, and highest urgency (API hardening before remote work progresses).
- **Auth throttle threshold interpretation.** Implemented `auth_failures_per_minute` as "allow up to N failures in-window, return 429 starting on failure N+1".
- **Redirect boundary strictness.** `SafeHttpClient` strips sensitive headers when redirect authority changes (host or port), not host-only.
- **Flaky Rust integration test observed during full suite runs.** `cargo test --all` intermittently fails at `wave_rename_renames_branch` (`rust/loopflow/tests/wave_worktree_tests.rs`) with `fatal: no branch named ...`; reruns pass without code changes.
- **Concerto UI test environment sensitivity.** `xcodebuild test -project swift/LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'` failed in `ScreenshotPipelineTests.testCapture` with "Failed to activate application ... (current state: Running Background)".
