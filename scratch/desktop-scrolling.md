# Desktop scrolling

Reported November 14, 2026:

- “my machine has been kinda loaded, but im noticing that scrolling sessions in my Loopflow Desktop is way slower than scrolling in rex”
- “not ok”
- “Conversation / terminal contents”
- “Can’t reproduce it right now”
- “i quit the loopflow app bc it sucked so bad”

Installed v0.15.3, revision f2a581a2a, PID 11516. Its log recorded main-thread
hangs of 11,384 ms at 02:27:12Z and 15,368 ms at 02:28:46Z November 15.
The 02:29:25Z three-second sample caught 1,338/2,328 main-thread samples in
AppKit's menu-bar application-name notification waiting synchronously on Launch
Services. A later ten-second sample caught 500/8,073 samples deallocating
FSEvents messages on the main queue. Neither captured scroll input.
Raw samples: `/tmp/loopflow-scroll-sample.txt`,
`/tmp/loopflow-scroll-active-sample.txt`, `/tmp/rex-scroll-sample.txt`.

Pinned Ghostty a60e9e2 source fetched November 14:
`Surface.draw` calls `renderer.drawFrame(true)` synchronously; the renderer
already owns rendering and a native display link. `occlusionCallback` changes
renderer visibility and QoS and rebuilds the viewport on becoming visible.
Loopflow continuously forces draws from a main-thread CADisplayLink and never
calls `ghostty_surface_set_occlusion`.

Repair: leave normal drawing to Ghostty; retain a forced draw only for the
existing pending key-to-draw measurement. Forward actual surface visibility.
Do not restart the installed app.

Checks: `swift build --package-path swift --jobs 2` passed. The native
`GhosttyTerminalInputTests.retainedTerminalVisibility` passed (0.199 s), observing
an invisible surface through mode 2033 and continued child input. The first
fixture's show-window check could not run here: AppKit never reported the
test window visible; the regression now exercises off-window retention.
`scripts/test_desktop.sh --skip-build --filter 'GhosttyShellBlockTests|TerminalFeedbackTests|TerminalPolishTests|GhosttyTerminalInputTests'`
passed 30 headless tests (0.335 s); display-only tests skipped.

Remaining: alternate Xcode build; compare scroll latency and
hidden-surface CPU under concurrent builds in a display session. Main-thread
Launch Services and FSEvents stalls remain separate observed concerns; their
relationship to the reported scrolling is unconfirmed.
