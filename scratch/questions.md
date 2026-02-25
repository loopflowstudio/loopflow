# Open questions / follow-ups

- Stage 01 done-when specifies iPhone 16 + iPad Pro 11" (M4) simulators, but this environment only has iPhone 17 + iPad Pro 11" (M5). Validation was run on the available runtimes.
- RepoState and store classes are still located under `swift/Concerto/State/` in this draft. Stage 01 design calls for moving shared state into `swift/LoopflowCore/State/`; this extraction remains as follow-up refactor work.
- `cargo test --all` currently fails two docker startup tests when `/var/run/docker.sock` is unavailable in the environment (`docker_startup_lost_agent_does_not_flip_terminal_run_wave_status`, `docker_startup_rehydrates_running_agents_and_cleans_orphans`).
- `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'` requires code-signing overrides in this environment and still fails at `ConcertoUITests-Runner` bootstrap (early unexpected exit); unit suites pass.
