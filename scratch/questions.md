# Open questions / assumptions

- `detect_manifests()` currently auto-selects known manifests at each detected root (for default target this can include both `Cargo.toml` and `package.json` if present). Assumed this is acceptable for first draft; may need tighter heuristics for "fewest files" in mixed-language repos.
- `has_release_workflow()` currently detects workflows by checking for `tags:` plus `{prefix}v*` string in workflow YAML. Assumed this heuristic is sufficient for bootstrap gating; may need structured YAML parsing if false positives appear.
