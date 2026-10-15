# Terminal feedback review — October 15

Both queued findings are repaired in the implementation pass before final artifact publication:

- Copy notices carry the originating surface's existing incarnation UUID. The
  receiving pane checks its window's retained pool; another window with the same
  TerminalIdentity and a replacement surface reject the notice and late dismissal.
  Headless `TerminalFeedbackTests.copyNoticeOwnership` passes. That regression
  also exposed lazy Find-bar construction during terminal deallocation; teardown
  now closes only an existing bar.
- Renderer `blockStyle` preserves failure before selected/hover appearance. The
  failed block retains its red background while Swift keeps the selected gutter
  neutral. Zig `terminal contrast block failure stays distinct from neutral
  selection` and the existing Swift gutter checks pass.

Final review also repaired reduced-motion expiry: feedback wakeups recompute the
absolute deadline after every draw, so custom shaders cannot postpone clearing.
The focused `terminal feedback redraws cannot postpone static cue expiry` passes.

Check: focused Zig contrast/feedback tests and universal ReleaseFast build PASS;
headless Swift focused suites against the public lf5 artifact PASS (12 tests, one optional native diagnostic
skipped), including 324 production Metal color cases and background-edge rendering.
Native visual judgment remains with demo/review.
