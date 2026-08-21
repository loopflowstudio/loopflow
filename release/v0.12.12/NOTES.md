# v0.12.12

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.12 makes Task workflow correctness part of the lifecycle itself instead of metadata that each skill must declare. Default feature work now has explicit design-review and configured-path demo gates, while Linear Task identity remains intact throughout PR operations. The result is a more reliable path from design through shipping, especially when one Task spans serial PRs.

## Task workflows enforce their own lifecycle

Lifecycle validity is now structural, removing skill capability frontmatter as a second source of truth. The default feature path also makes its proof points explicit before work advances.

- Task lifecycle validation no longer depends on capability declarations in skill frontmatter.
- Default feature work is gated at design review and at a demo through the configured user path.
- The built-in Task implementation and incident-prevention flows follow the structural lifecycle model.

## Task identity survives PR operations

PR mechanics now preserve the owning Linear Task consistently, so publishing and landing do not sever orchestration state or lose continuity between serial changes.

- Task identity is retained when a PR is published, refreshed, or landed.
- Starting the next serial PR keeps the same Task identity instead of requiring it to be reconstructed.