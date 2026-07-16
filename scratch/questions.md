# Open questions / assumptions — W2-177 PR #978

## CI `loopflow-ui-test` failure on `643424a92` — not reproducible locally

The pushed commit `643424a92` failed the `loopflow-ui-test` CI check (conclusion
FAILURE). That job is `xcodebuild build-for-testing -scheme LoopflowMac` — a
compile check, not a hosted run.

Reproduced locally with the exact CI invocation (`-disableAutomaticPackageResolution`,
manual signing, fresh `DerivedDataRepro`):

- at `643424a92` (second-round stashed): `** TEST BUILD SUCCEEDED **`, exit 0, no `error:` lines.
- on the second-round tree: `** TEST BUILD SUCCEEDED **`, exit 0.

The CI job ran 45 min (started 05:53:01Z, completed 06:38:01Z) against a 12-min
step `timeout-minutes`, and the job log has expired (404 on fetch). Main CI is
green for the same check on recent commits.

Assumption: the failure was environmental (cache miss / slow runner / transient),
not a code defect. Pushing the second-round review fixes re-runs CI; if
`loopflow-ui-test` fails again, the fresh log will be chased. The second-round
tree is otherwise fully verified: `swift build` clean, full `swift test` 204/204,
`swift test --filter "HandoffSurface|ActiveSessions"` 28/28, and the Swift
multiplatform boundary check passes.
