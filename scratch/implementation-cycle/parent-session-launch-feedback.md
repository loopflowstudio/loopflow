# New-session feedback follows its Task

2026-09-25. Fixed the audit's delayed-feedback bug. WorkSurfaceView's single
starting/error state previously belonged to whichever Task was currently visible.
The existing repository navigation now retains pending Task IDs and per-Task
errors. The click captures that navigation before awaiting preparation; success
or failure settles there, even after Task/repository switching. Another Task's
New session stays usable. Returning to the original Task reveals its failure;
retry clears only that error. No launch, planning or terminal owner changed.

Focused proof: `swift test --package-path swift -Xswiftc -gnone --jobs 4
--no-parallel --filter WorkspaceNavigationTests/newSessionFailureKeepsItsTask`
passes one test. It taps the actual view action, holds preparation, changes Task
and repository, releases a rejection, returns to both Tasks and retries. The
preparation callback is simulated; this proves presentation targeting, not a
configured provider or the real subprocess preparation path. Log and exact source
hashes are in the adjacent receipt. No broader test or publication occurred.

Review: state survives repository navigation with its existing owner, rather
than suppressing late errors or showing them on the new selection. Other Task
controls keep their existing ownership. The outstanding actual app/CLI delayed
preparation and terminal-retention proof remains in the audit.
