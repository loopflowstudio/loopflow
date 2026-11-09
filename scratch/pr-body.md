Saved conversations could fail to open without a Task, disappear after an inventory update, or fail location reads when Desktop ran outside a checkout. Exact Session links now reach their saved conversation, and Refresh reconnects a stale pane while keeping its history and running provider.

## What changes

- Preserve exact Session selectors on repository and Wave links. Desktop prepares that conversation without scanning unrelated history; Rex can open it without a surviving caller viewer or stale startup placement.
- Keep an opening request through saved-navigation startup and merge its record into the current inventory. Unpaged reads request every Session instead of inheriting the 100-row limit; paged reads remain bounded.
- Resolve execution location using the selected repository, without requiring the process cwd to be a checkout.
- Classify Codex revoked-refresh responses as requiring login, retaining recovery from a working copy of the same account.
- Keep **Refresh session** visible for a single pane and expose **Refresh** and **Take back** as addressed Desktop actions. Let an exited relay receive its dismissal key; distinguish Reset's icon and document its saved-history behavior.

## Recovery observations and limits

On November 9 at 09:15 local, the scheduled updater replaced the locally repaired app with main `237a10726`, restoring the location and inventory failures. The matching repaired app and CLI were restored. `com.loopflow.refresh` is temporarily disabled pending a release containing these fixes; re-enable it after that release is installed.

LOO-593 reopened on the same provider thread after its idle native client was restarted. API insertion and deletion passed both before and after pane reopening, with the goal still paused. The physical screen was unavailable, so this does **not** establish physical typing or repaint. Four taskless conversations also opened their original threads in earlier installed-app checks. These are recorded recovery observations, not operations performed by this gate.

**Physical invisible typing and automatic interactive quota handoff are not fixed. The incident remains open, and this PR does not complete the Task.** Nineteen previously observed missing-owner clients remain unverified. Physical close/reopen input and repaint still need a native-screen investigation; a passing API or viewport check cannot substitute for it. Private raw captures remain local and are not included here.

## Checks

The affected-suite gate passed on November 9 against `a6f254931`: 289 selected Rust tests; the headless Swift app build and suite reporting 780 tests across 94 suites; architecture coverage; Rust test inventory; `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`; Swift platform-boundary checks; and `git diff --check`. The runner completed in 722 seconds with no timeout or reused result.

Optional display diagnostics were skipped. CI still owns the full Linux matrix and candidate Xcode build; physical close/reopen typing remains unverified. This gate performed no installation, Session restart, or Desktop operation.

Fixes #1864 (the v0.15.2 repository-location regression).
