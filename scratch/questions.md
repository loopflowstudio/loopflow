# Choices

2026-11-09: Use demand-populated polling for external facts, including checkout
Git state, rather than retaining the separate macOS checkout watcher. This gives
the same observation path on every platform. Git polls every three seconds,
local files and process identity every second, GitHub every minute.
