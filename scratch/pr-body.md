Repositories with existing worktree layouts, nonstandard integration remotes or valuable scratch files should retain those conventions when using lf. Authored Wave definitions now have one canonical location under `.lf/wave/`.

- Infer worktree placement from existing Git registrations, reuse existing branches locally, follow Git tracking for integration, and retain checkout failures with actionable setup output. Rejected normal pushes no longer trigger force pushes.
- Import the scratch implementation from `levels-of-lf`: delivery and `lf scratch clear` remove committed additions relative to the Task/base while preserving existing files, edits, renames and stacked parent content. Delete the superseded opt-in, persistent scratch rewriting and global exemption for a required check named `scratch-clear`. The repository owns its CI candidate policy.
- Add explicit reviewer requests and provider review reads, required-check selection and pending-check handling. Respect repository-allowed merge methods and merge queue authority; submitting leaves reviewer/assignee choices intact.
- Move repository Wave definitions, Rust and Swift consumers, context, metrics, docs and fixtures together to `.lf/wave/`. Preserve identities and historical revision links. Existing installations must explicitly move authored definitions; there is no fallback to arbitrary top-level `wave/` content.
- Skip symlinks during automatic scratch and repository-memory inclusion.

Review fixes include preserving explicit stacked parent bases, using Git tracking for scratch comparison, avoiding a nested checkout lock on explicit upstream sync, and retaining explicitly requested Wave documents through context filtering and deduplication.

Validation: the Rust library run passed 1,947 tests and exposed eight failures, all repaired and passed in focused reruns (10 opt-in tests ignored). All 236 affected integration tests passed, plus golden/remote fixtures and post-repair context checks. Python passed 410 tests initially; the three failures were repaired and the 40 affected checks passed (one skip). All 75 website checks passed. Formatting, Clippy, architecture coverage, the four-layout CLI demo, Swift syntax and Swift boundary checks pass.

Gate also repaired fixture assumptions about ignored Wave paths and refresh timestamps; no production clock behavior changed.

Limits: Desktop build/tests are deferred to CI because ViewInspector and GhosttyKit are unavailable locally. Placement fixtures do not establish integration with third-party apps. GitHub reviewer/merge behavior is covered by provider fixtures, not live reviewer mutations. Full fork publication, sparse/submodule/LFS setup, inline review remediation and a second host remain outside this change. Scratch additions are a Git-diff proxy for Flow notes, not an authorship registry.
