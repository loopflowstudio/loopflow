# Session ownership · LOO-812

Worktree: /Users/jack/src/loopflow.session-ownership
Branch: jack-heart/session-ownership
Draft PR: https://github.com/loopflowstudio/loopflow/pull/1858

Landing is authorized once branch work and checks are finished. Do not install
or touch personal live incident Sessions; LOO-813 owns that recovery.

## Direction recorded

Jack: "Make sure to leave the codebase simpler  than you found it. Do not add YAGNIs. Make it so that future work on e.g. mobile is *easier*."
Jack: "thios is what makes me expect it tobe 1:1 with lf processes, at least when theyre skills"
Jack: "ok, go for it"
Jack: "update docs"
Jack: "I started this PR by telling the agent to just improve the code quality as much as possible. I was hoping to see more things in that realm. Can we build on what we've done here and try to get more user experience wins in the areas that I've described?"
Jack: "The cursor can be in the wrong place for a few seconds or there can be a flash of \"reconnecting,\" then another flash of \"building starting conversation.\" Finally we actually get to just rendering the session as it's supposed to be"
Jack: "ive also noticed that even non-agent lf commands will end up clearing or jumping the terminal, which should not happen"
Jack: "Show the last screen immediately, clearly pause input, then enable it in place (recommended)"
Jack: "1a"
Jack: "can you also create a report on all the ways that session recovery or transport-related UX are polished in T3code, paradev or paseo in a way that it is not for loopflow"
Jack: "launch subagetns to look through each codebase's commit logs and test code to see details that we may want to to test or polish in our own codebase"
Jack: "it doesnt have to be just recovery, just general things that makes launching new sessions, seeing their progress, moving between machines or desktop/mobile, etc"
Jack: "ok, lets do as much as makes sense in this session. then lets look at what existing tasks should get new instructions vs what still needs design input vs what should just be a straightforward foollow up task"

## Current implementation and review

LfProcessRuntime owns live invocation resources; SessionRecorder records history.
AgentExecution is deleted. The same LfProcess spans successive provider attempts.
A viewer is temporary; no new execution ID, persisted lifecycle or manager.

The pane shows a readable last screen and one Opening label until live attachment
and native readiness enable input in place. Reconnecting requires a prior attached
connection. Status does not change terminal dimensions. Resize precedes the first
snapshot; cursor save/reset/restore prevents cleanup from homing the shell cursor.

Delete — do not maintain: status JSON file, Retry/Move action files and file watcher;
independent prepared-state Connecting overlay; retry-derived opening labels.
One private per-surface Unix stream carries current readings and deliberate actions.
EOF revokes Desktop input permission. Reporting failure only closes that stream,
logs a RecordingGap and retries with current state. Never replay Retry/Move actions.
Retain owner/claim generation fencing, native readiness, and pane-close cancellation.

The additional silent-return-path fix uses the existing 30-second heartbeat bound.
Read available data before checking elapsed wall time after sleep. A server viewer
reconnects to the same owner; other viewers report unknown and end. No provider
restart or input replay. Quiet owners answer without producing terminal output.
A new regression restores default terminal modes into a dirty viewer.

Review: `scratch/concept-review.html`. Source-backed comparison and proposed Task
instructions: `scratch/session-experience-comparison.html`. Three source reviews
are embedded there and retained as `.txt` beside it; they inspected upstream tests
and commit history, but did not run upstream apps/suites. Pins/release distinctions
are in the report. The source baseline was 6394e4e900 before the latest changes.

## Remaining

- Review the updated draft f4275a36e, pushed after sync with main 4848816f73cc.
  Ownership, presentation, heartbeat and repaint fixes are included.
- Resolve or classify the five optional native-input acceptance failures below
  before claiming complete native acceptance; retain the installed/physical checks.
- Review proposed Task instructions in the report. No new Task/comment was sent.
  LOO-796 owns reading/focus; LOO-810 creation recovery; LOO-718/799 mobile journey;
  LOO-795/798 progress/read state; LOO-753 remote Desktop; LOO-774 bridge endings;
  LOO-719/779 installed/native checks. Do not duplicate those implementations.
- Design input: app-owned draft lifetime/conflicts, cross-device read markers,
  remote attachment staging/retention, and successful execution with missing receipt.
  One narrow follow-up candidate: complete mounted terminal restoration/resize
  acceptance using existing fixtures, distinct from installed/remote checks.

## Observed checks and limitations · 2026-11-09

1a commit c564092ca; clean sync b2c3ff08c with main 4848816f73cc on November 9.
The preceding pushed revision 6394e4e900 had fully green CI when read.

Before sync: 13 focused transport regressions passed (40.467s); final 39 Swift
checks across ServerConnection, TerminalPolish, WorkCache and mounted
SessionConnectionProof passed (13.711s). Swift build, formatting, Clippy with
warnings denied, architecture/test inventory/platform boundary checks passed.
The mounted proof uses native key events into Ghostty, a real source-built
relay/server and private Machine with a provider stand-in. It checks same
surface/provider, stable frame, working input before/after restart and discarded
paused typing. It did not install the app or test physical hardware input.

Earlier ownership tests: 118 focused tests passed; two real Claude subscription
checks ignored. Receipt-failure regression previously left provider life Live;
now cleanup runs before receipt publication, with retry/replacement tests.
An earlier CI background-watch case failed with missing idle-settled and empty
logs; focused local rerun and later CI passed. Its original cause is unknown.

Isolated PTY probes of `lf --version`, `lf help` and `lf process list --json`
returned 0 with no cursor/screen controls. The reported ordinary-command jump is
not reproduced. The exact command and terminal are still needed to establish it.
A soft-reset experiment retained margins/origin mode in pinned Ghostty and was
discarded; cursor save/reset/restore kept position and reset those modes.

No physical invisible-input recovery or automatic quota handoff was performed.
LOO-813 keeps those incident concerns. Successful work plus failed receipt remains
a separate product choice: report known completion and recording gap separately;
only wait when a Graph needs a missing artifact. Unknown outcomes stay unknown.

The dirty-viewer test failed before repair: a default-mode owner inherited enabled
bracketed paste, application keys, focus and mouse reporting from the viewer.
Ghostty’s formatter emits enabled modes without clearing every disabled mode.
Repaint now exits the old alternate screen and applies existing mode/drawing
resets before restoring the owner; untouched owners still leave shell output alone.

The first mode repair used 1049-reset and failed existing scrollback tests because
that sequence restores an old cursor even on a primary-screen viewer. Replacing
it with 1047/47 exit preserved scrollback; dirty-viewer and all 13 terminal unit
checks now pass. The broader 62-test transport unit run subsequently passed.

Final repaint revision: all 62 transport unit tests passed (78.05s), including
13 terminal tests, output snapshots, preview handoffs, input and lifetime checks.

Final checks: seven repaint/attachment integration tests passed (10.784s);
Desktop build passed (64.16s); Clippy --all-targets -- -D warnings passed (51.26s).
Expanded Swift run: 69 tests / seven suites, five failing tests / 11 issues in
GhosttyTerminalInputTests. The six other suites, including the final mounted
SessionConnectionProof, passed. Command:
`env <LF_* unset> LOOPFLOW_NATIVE_TESTS=1 uv run python scripts/swift_test_limit.py python scripts/test_network.py --desktop swift test --skip-build --disable-sandbox --package-path swift --no-parallel --filter 'ServerConnectionTests|TerminalPolishTests|WorkCacheTests|SessionConnectionProofTests|GhosttyTerminalInputTests|WorkspacePanesTests'`.
Log: /tmp/loo-812-swift-post-sync-tests.log. Failures: shellSessionAttachment
(missing bundled helper); commandBlockClickAndCopy (drag copied “comman”, later
block/resize selection differed); releaseSurfaceIsWindowLocal (immediate release
assertion and synthetic Session echo); typingAfterKeyboardRestore (screen still
read live bottom after scroll_to_top); pasteShortcutFollowsFirstResponder (synthetic
Session half rejected paste). No baseline runtime comparison; do not call these
new regressions or claim the optional native suite passed. Existing synthetic
Session /bin/cat fixtures provide neither old mailbox nor new stream readiness.
The report keeps exact failure observations and proposed next actions.

Jack: "i dont think we need cross-remote drafts. explain the other neeeds design stuff. Throw the straightforward followup into appropraite existing or new ttasks"
Jack: "make sure to update the other tasks for everything lksited in the doc"
Jack: "Opening the task for now...didnt even know we had Read state"
Jack: "seems like we should put it into /tmp on that machine or soemthign similar"
Jack: "use scp?"
Jack: "other than all that, you are aproved to land one the work is ifnished in this branch"

Cross-device draft synchronization is out of scope. Opening the Task is the read
marker trigger; background refresh/reconnect never acknowledges unseen changes.
Remote Desktop file paste will reuse Machine SSH with scp into private remote
temporary storage, inserting the path only after success. Follow-up instructions
are being applied to existing Tasks; LOO-779 owns native acceptance fixture repair,
LOO-796 owns resulting reading/selection/focus defects. LOO-775 is done in #1865.
CI at f4275a36e passed every required job when reread before landing preparation.
