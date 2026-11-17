# Desktop bug batch

One PR from this branch (UI, demoable); infra-only work runs as its own Task.

## In this branch
1. Deleted Task restored from desktop-cache/workspace.json (task_ed311b47…, "No saved Task") kept
   the selection: "Selected Work is unavailable" + "Layout not saved: not found".
   Fix: `clearSelectionIfOutsideScope` no longer lets an empty retained workspace hold a Task
   planning doesn't know. Server failure log now names the request `kind`.
2. "Conversation change unconfirmed · Inspect" never cleared (session.new receipt stuck at
   `stopping` after "dispatch is still busy" on 2026-11-10). Fix: Dismiss button.
   Open: Rust `new_thread` saves the receipt before stopping the agent (human_session.rs ~1588);
   stopping first would leave no receipt on a refused stop.
3. "Reading workflow…": Task frames dropped when they beat planning are now replayed after
   planning applies; graph catalog / view refresh no longer block the frame loop.
4. Sidebar "No working Tasks in this list" while loading → "Loading Tasks…".
5. Filter Tasks field in the sidebar (replaces the hidden ⌘F sidebar sheet).
6. Backlog Task clicked from the repository board opened a second Task UI; unstarted Tasks
   now open the terminal workspace whose Session slot offers Start.

## Running as Tasks
- LOO-859 server load (session summary N+1 JSON scan, wave.sync write lock, planning frames ~2s)
- LOO-860 Session resumes on the provider of its latest input (claude row, codex thread)
- LOO-839 SQLite audit

## Checks
- `swift test --package-path swift --filter "WorkNavigationTests|WorkspacePanesTests"`: 56 pass
- `cargo check -p loopflow`: ok
