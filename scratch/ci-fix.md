Watched PR #1392 at 13ca987c4c17fa2972d35a5746a029fcf7a49368: SwiftPM and Xcode compilation failed on merged references to removed SessionKind.ask. Removed those branches and migrated participation/navigation fixtures to Flow Sessions. Review found the no-actions recovery fixture should retain current Flow membership to prove action availability independently; corrected it.

`scripts/test_desktop.sh --filter 'TaskFlowTests|WorkspaceNavigationTests'` — passed, 36 tests; Desktop and tests compiled headlessly. Hosted CI owns the Xcode build on the repaired head.
