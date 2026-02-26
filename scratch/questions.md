# Open questions / assumptions

- `detect_manifests()` currently auto-selects known manifests at each detected root (for default target this can include both `Cargo.toml` and `package.json` if present). Assumed this is acceptable for first draft; may need tighter heuristics for "fewest files" in mixed-language repos.
- `has_release_workflow()` currently detects workflows by checking for `tags:` plus `{prefix}v*` string in workflow YAML. Assumed this heuristic is sufficient for bootstrap gating; may need structured YAML parsing if false positives appear.
- Local gate run hit a flaky UI test failure twice in `xcodebuild test -scheme Concerto` (`ConcertoUITests-Runner ... hung before establishing connection`). Unit/package Swift tests passed; deciding whether to quarantine/fix that UITest harness is still open.
