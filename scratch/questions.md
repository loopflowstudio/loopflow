# Open choices

- Use two disposable source roots for the initial cache comparison and one
  sequential late-root fixture for refresh; one build at a time. LOO-908 owns admission/job budgets and shared-cache
  quiescent refresh. No Machine-wide restart belongs to this probe.
- The model cut keeps application module paths as re-exports; internal callers
  import the owning crate. Public enums retain their non-exhaustive contract.
- Cross-root hits retain A's absolute debug-source paths in B's rlib. Runtime
  identity is correct, but debugger source lookup can outlive a deleted worktree.
  Decide the canonical debug-source mapping before enabling automatic reuse;
  do not hide this with a blanket source remap or claim production acceptance.
- LOO-908 is done with no PR; no live root-refresh integration is available.
  The private probe tests refresh; a shared-daemon owner remains unselected.
