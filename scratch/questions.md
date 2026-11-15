# Open choices

- Use two disposable source roots for the initial cache comparison and one
  sequential late-root fixture for refresh; one build at a time. Admission/job budgets and shared-cache
  quiescent refresh stay outside this probe. No Machine-wide restart belongs to this probe.
- The model cut keeps application module paths as re-exports; internal callers
  import the owning crate. Public enums retain their non-exhaustive contract.
- November 15 direction supersedes the source-debugging stop. Cross-root hits
  may retain A's debug paths; use `scripts/dev-lf --isolated-build` for local
  debugging and cache diagnosis. No source-retention system is selected.
- LOO-908 is done with no PR; no live root-refresh integration is available.
  The private probe tests refresh; a shared-daemon owner remains unselected.
- The isolated build option is a leading `--isolated-build` argument to
  `scripts/dev-lf`; ordinary lf arguments pass through after it. A fixed
  checkout-local `target/isolated` is the simpler reusable debugging target.
