# Implement polish direction D in the native Swift app

Jack: "go ahead and get these visual changes directly implemented in the actual swift code".
Target: direction D in scratch/visual-study/polish (README "Jack's pick" and D section; ?v=d in index.html/polish.css/polish.js).
Sidebar = A, center pane = B, overall mood = C. This is visual POLISH of existing native owners, not new behavior.

Scope (swift/LoopflowMac): WorkspaceNavigator (sidebar), WorkSurfaceView (Wave/Task pages), TaskFlowView (Flow diagram), WorkspaceBreadcrumbBar, SessionsView Session list rows, shared palette/typography. Reuse the existing palette and fonts; add tokens where D needs them rather than one-off colors.
Include:
- One section-heading system; the Wave warning as a compact note with Details disclosure (full text/recovery command inside; nothing hidden permanently); Metrics lighter than Tasks; Chapter history beside Current KRs.
- Sidebar selection tint + 2px burgundy edge, fixed Session count column, clearer header glyph.
- Flow: once/loops/tail rows so the whole Feature including `pr land -c` fits ~1100pt with no clipping; separated return lanes so arrowheads don't collide; each loop labels its own return count from existing FlowReturn.traversals, header shows the per-loop tuple; opaque node base so green/yellow keep hue inside loop tint.
- Task page rhythm and aligned ID/state columns per D.
Do NOT change Rust, DTOs, the tuple runtime correction (owned elsewhere, jack-iteration-tuple.md), control legality, or navigation/terminal ownership. Preserve concurrent dirty edits of other writers; don't blanket-stage or commit.

Proof: build, run the focused Swift suites touching these views (WorkspaceNavigationTests, TaskFlowTests, TaskFlowProofTests, WorkspaceNavigationProofTests/namedSessionDrillDownRetainsTerminal, TaskMonitorProofTests) with `swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel --filter ...`. Update tests only where copy/structure legitimately changed. Produce native captures of Wave, Task(running Flow) and Session via the existing capture env hooks (LOOPFLOW_OUTLINE_CAPTURE_DIR / LOOPFLOW_FLOW_CAPTURE_DIR) into scratch/visual-study/polish/native/, inspect them against ?v=d, and fix mismatches. Write scratch/visual-study/polish/native/README.md: changed files, commands and results, capture comparison, remaining gaps.
