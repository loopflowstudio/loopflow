# Explore step: open choices and the defaults pursue builds on

Jack launched pursue on 2026-10-24 with these unresolved. Each default is an
implementation choice to revisit in demo, not Jack's acceptance. Details are in
`scratch/explore-step.md` under "Open tensions."

1. **Before or after kickoff.** Default: after (`feature.yaml` as drafted);
   kickoff unchanged.
2. **review-design.** Default: delete it per the design's "Delete — do not
   maintain" list (explore takes its only Workflow slot).
3. **Who re-plans.** Default: explore records settled direction in the design
   itself; the `explore → explore` edge still reruns task-design.
4. **Default Workflow.** Default: no change; explore rides wherever `feature`
   is selected.
5. **Page guidance placement.** Default: A, inline in each presenting skill
   (explore, demo, research, concept-review). B (node launch hook) is unverified.

Assumptions made during implement (2026-10-24):

6. ~~WebKit without raising the target~~: superseded 2026-10-24. Jack: "Let's
   do macOS 26 webView, and stop supporting macOS15 if possible." The Desktop
   app now requires macOS 26 (Package.swift, project.yml, Info.plist), renders
   HTML through SwiftUI `WebView`/`WebPage`, and `swift-test`/`loopflow-ui-test`
   run on `macos-26`. The CLI still builds and is tested on macOS 15
   (`macos-process-test`, `native-packages`); dropping it there is a separate call.
7. **Read mode for other files.** Read renders `.md` and `.html` anywhere in
   the checkout; other files in Read show the existing text editor. Changed-file
   rows still open in Diff.
8. **"While you've been in the conversation"** is approximated: Files untouched
   for two minutes counts as stale regardless of where you were. Pane focus no
   longer keeps the view (demo, 2026-10-25). A Files pane that has never been touched is
   stale.
9. **Node rename.** The feature node is now `explore` (was `design`). Tasks
   that already took up `feature` keep their captured graph, so a Task stored
   at `design` stays there.
