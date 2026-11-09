# Desktop session recovery

## Recurrence, 2026-11-09 10:04–10:17 local

Jack Heart:

> still having issues wit my open Loopflow
> Typing stays invisible or does nothing
> e.g loo-593
> also Loopflow esktop seying Error: select a repository before reading execution location

- The scheduled `com.loopflow.refresh` job replaced the repaired release at
  09:15 with base `237a10726`. Its refresh log and installation receipt
  `switch-37d546ca08074ffb84a693a0ca074ebf` record the change from
  `7e90dafb1-dirty`. This brought back the repository-location failure and
  100-row subscription truncation. The matching signed repaired release bundle
  and CLI were restored through promotion; no migration. Both repository
  windows subsequently reported current readings and LOO-593's opening usable.
- Temporarily disabled and unloaded **only** `com.loopflow.refresh`; its plist
  remains. Re-enable after the installed stream contains these repairs:
  `launchctl enable gui/$(id -u)/com.loopflow.refresh`, then
  `launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.loopflow.refresh.plist`.
  Bootstrap runs the installer immediately because the job has RunAtLoad.
  Rental renewal was not changed. Local unpublished fixes remain vulnerable to
  an explicit installation of the older stream.
- LOO-593's old native client PID 38770 ignored Desktop Escape, Refresh and
  diagnostic input. The temporary CLI raw-Escape observation was initially
  mistaken for dismissal: resizing displaced the modal in the passive parser;
  the full stream and later Desktop capture still contained it. That claim was
  corrected in conversation. These probes establish no encoding-specific cause.
- The native transcript's latest turn completed at 2026-11-08T13:43:22.660Z;
  its visible composer was empty beneath the paused-goal dialog. Verified 38770
  was not an ancestor of this conversation, then terminated only that client.
  The first resume raced attachment cleanup and reported `Session attachment
  changed`; subsequent Desktop Refresh reopened successfully. Provider thread
  remains `01a1b986-3324-7913-949d-c8054da335b0`. Engineering login freshly
  verified accepted (2% weekly used). No login, reset-credit use or model turn.
- After restart, Desktop Escape dismissed the goal dialog. API insertion of
  `x` and separate Backspace passed before and after closing/refocusing its
  Session pane. Final capture has an empty composer and paused goal. A first
  capture during reopening had no surface; the later mounted surface passed.
  Historical quota output remains in conversation history.
- **Physical keyboard repaint remains unverified.** CUA lost access after the
  app replacement and repeatedly returned `noWindowsAvailable`, including after
  rebinding by path, bundle ID and resetting its runtime. The question about
  whether the window is visible remains unanswered. API editing is not the
  physical close/reopen acceptance check. No speculative input patch was made.

Artifacts: `/tmp/loo813-debug/promote-recurrence.log`, `593-native.sample`,
`593-*.json`, `593-*.vt`, `engineering-recurrence.json`. No remaining diagnostic
characters or temporary CLI viewers; parent native PID 88257 was untouched.

## 5whys disposition, 2026-11-09

**The incident remains open.** The 80-Session Loopflow/manabot inventory and
29 same-thread Codex account recoveries are complete; four taskless exact opens
are repaired and infrastructure's composer is recovered. Physical typing after
reopen is still unresolved, demonstrated on MTG-269 after account recovery and
Refresh. Nineteen clients with missing owner paths remain alive and unverified.
This analysis changed only this note; no live Session was probed or replaced.

Jack Heart's report was:

> every session that i close and reopen (maybe only codex, i only have codx right now bc im out of claude credit) isnt receiving keystrokes anymore

### Why recovery did not establish working typing

1. **Visible silence sometimes concealed accepted input.** On Nov 9, LOO-515
   accepted two physical `x` keys after reopen without displaying them; literal
   insertion of `y` revealed `xxy`. MTG-269 retained `sssjs`, then paste revealed
   `sssjsxy` after a physical probe. Thus those keys reached the native draft.
   This does not locate the delay: native redraw, relay delivery, terminal
   viewport and screen presentation were not observed together during failure.
2. **Account recovery addressed a separate failure.** Fresh verification
   rejected three logins and accepted engineering; 29 conversations reopened on
   their original provider threads. MTG-269 still failed physical repaint after
   that move, Refresh and focus-away/back. Authentication recovery therefore
   cannot explain or close this symptom. Capacity readings establish account
   access, not visible typing or a successful model response; no model request
   or goal resume was sent.
3. **The earlier checks exercised a narrower path.** Desktop literal insertion
   calls `insertTerminalText` → `ghostty_surface_text`; it bypasses physical
   `performKeyEquivalent`/`keyDown` dispatch. Addressed special keys also bypass
   AppKit dispatch. `readText(.screen)` reads Ghostty's viewport text under its
   renderer lock, not the pixels on screen. A successful API receipt alone
   establishes neither native consumption nor display. In this incident the
   literal probe itself revealed hidden text, changing the condition being
   investigated before the capture. Those checks were useful for drafts and
   routing, but could not establish the reported physical workflow.
4. **The remaining experiments did not reproduce that workflow.** CLI raw and
   Kitty probes produced redraws and `sssjsx`; separate deletion restored
   `sssjs`. The first synthetic test expected literal `x` but received valid
   `CSI 120u`, so its failure was not dropped input. The corrected flags 5/7
   viewport test passed. Pinned Ghostty queues rendering and, when configured,
   scrolls after encoded non-modifier keys; missing `key.text` alone is not the
   cause shown here. Final CUA attempts returned `cgWindowNotFound`; the test
   environment also reported CoreVideo -6661 and timer rendering. Neither a
   passing synthetic event nor unavailable screen capture closes the live bug.

The verification gap is a mismatch between the reported action and the check,
not proof of a particular renderer defect. Earlier notes already qualified
physical input as unverified; retain that qualification through planning.

### Proven adjacent causes and repairs

- **Misleading account readiness:** the Nov 8 `account/read` rejection used
  JSON-RPC -32603 with revoked-refresh wording. Subscription polling treated it
  as unavailable and retained connected state because its classifier was
  narrower than the runner's. Reusing the existing classifier fixes that
  discrepancy; the recorded rejection/working-copy regression passed. It does
  not implement automatic interactive quota handoff, which remains unfinished.
- **Opening depended on unrelated context:** Task location required cwd even
  with a named repository; saved navigation cleared an opening request; an
  unpaged subscription silently inherited the 100-row limit. Separate branch
  regressions cover each repair. Taskless exact links additionally lost or
  rejected their Session selector, and Rex refused a missing current viewer.
  Those restrictions are removed; exact taskless preparation avoids a history
  scan that stalled locally. Installed receipts, selected panes and composer
  captures confirm all four original threads. In `exact-results.json`, three
  receipts still say `opening`; selection and capture, not that status alone,
  support recovery. None of these checks proves physical input.

### Contradictions and remaining experiment

Refresh repaired LOO-515 in one later check but not MTG-269; focus alone and raw
focus reporting did not reliably reveal text. The earlier 08:00 LOO-515 probe
revealed only `y`, and its long draft differed after reopen. Do not generalize
the later hidden-key finding into “all input and drafts survived.” The cause of
that draft change, infrastructure's original blank pane, and the unlinked
owner sockets remains unknown. Six of the nineteen PIDs are app-server
companions, not terminal clients. A live PID does not establish a saved draft.
The parent has a pending locked-screen question; `cgWindowNotFound` alone does
not answer it or explain the earlier visible failure.

**Proposals for launch-plan, not additional fixes:**

- Make the physical close/reopen sequence the acceptance check for this bug.
  Keep account acceptance, exact opening, API editing and physical repaint as
  separate results. A missing display leaves that check unverified.
- When a native screen is available, identify MTG-269's exact surface and saved
  `sssjs` draft. Capture owner output, Ghostty viewport text and screen together
  before and after one physical key, **before any paste**; remove that key in a
  separate operation and repeat across close/reopen. No Enter or goal resume.
  Missing owner redraw points upstream; owner redraw without viewport change
  points toward relay/terminal ingestion; changed viewport without changed
  pixels points toward presentation. Record actual key encoding and surface
  identity so a different viewer or protocol cannot explain the comparison.
- Once that trace reproduces the failure, add the smallest regression at the
  failing stage, retaining a display check for the full sequence. No speculative
  focus, reset or renderer patch is justified by the current results. Investigate
  socket unlinking separately before considering replacement of those clients.

Read-only review rechecked the full incident note, branch diff, input paths,
pinned Ghostty callback, saved CLI probes, four opening results and the log with
131 passing Swift tests. No builds or tests were rerun for this note-only step.
Next: return to launch-plan with the synchronized screen experiment still needed;
do not mark the incident resolved.

## Debug disposition, 2026-11-09 08:57 local

- **Invisible physical typing remains unresolved.** No input/render production
  change was justified by the experiments. Corrected Kitty flags 5/7 regression
  passes against the viewport; native raw/encoded key probes redraw and leave
  the original MTG-269 draft `sssjs`. Final passive snapshot after both installs
  still reads `sssjs`. Physical keys and close/reopen in the installed app could
  not be verified: CUA repeatedly returned `cgWindowNotFound`, including the
  final release installation. API captures do not establish visible typing.
- **Four taskless exact opens are repaired.** Desktop rejected repository/Wave
  Session selectors and had no receiver for them; Rex also refused agents whose
  current viewer was gone. Exact links now retain their Session ID, prepare that
  exact existing conversation, and use the existing pane placement. Rex opens
  on its local server when the caller has no current viewer, ignoring inherited
  stale placement. The first implementation's history scan stalled on this
  machine; the final code uses exact preparation without enumerating history.
  All four original IDs selected their own pane and returned their native
  composer through addressed Desktop text reads; every provider thread matched
  the saved pre-install ID. Results: `/tmp/loo813-debug/exact-results.json`,
  with per-Session state, capture and opening receipts beside it. No message sent.
- **Infrastructure is recovered, original blank-pane cause unknown.** Final
  installed exact opening selected `09a40b4c78b2411ab30bfbc3203d7304`, reported
  usable/current, and returned its composer with fresh 12% capacity. Final
  capture: `/tmp/loo813-debug/infra-final-capture.json`. Refresh earlier restored
  the saved conversation after its owner socket had disappeared; this is not a
  demonstrated root-cause fix for blank rendering.
- **Nineteen missing-owner clients remain untouched.** All nineteen original
  PIDs were alive at the final check. Six are Codex app-server companions, thirteen
  Claude clients; older owners hold sockets whose pathname is absent. No safe
  draft observation or in-place rebind was available. Neither socket deletion's
  cause nor these clients' draft state was established. No blanket restart.
- Matching signed **release** app and CLI installed through normal promotion at
  `/Applications/Loopflow.app` and `~/.local/bin/lf`; preflight found current
  `server_request_stops`, compatible references, and no migration to apply.
  Shared server reports ready on the selected build. Dev app started for this
  investigation was stopped; no temporary viewers, test runners or build jobs
  remain. Parent PID 88257 and this run's native PID 27873 remained alive and
  received no input or signals. No goal resumed, account login, publication or
  merge performed. Prior changes and notes remain.

Final checks: focused Swift suites **131 passed** in
`/tmp/loo813-debug/swift-direct-session-final.log`; the preceding direct-session
run failed because its fixture expected `--reveal` instead of
`--reveal-existing`, and the corrected rerun passes. Final Xcode
`build-for-testing` passed (`xcode-direct-session.log`); release Swift build,
release Rust build, signature verification and normal promotion passed.
Earlier unchanged-code checks: Rust opening regressions 34 passed/2 opt-in
skipped; Work links 5 passed; Kitty viewport diagnostic both cases passed;
`cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed. Final Swift
platform-boundary check and `git diff --check` passed.

Return for incident 5whys and launch-plan with the physical-rendering failure
still open. Next useful experiment needs an available native screen: capture
owner output and Desktop viewport together during one physical key, then repeat
close/reopen, preserving `sssjs`. Do not treat the repaired route or encoded-key
diagnostic as closure of that failure.

## Debug continuation, 2026-11-09 08:27 onward

- Protected parent `session_55fdf896fde94f92aec807061df59417` still has native
  PID 88257; this debug run is `session_87fbf5e08fd2475592b61e35d0e498ff`,
  native PID 27873. Neither was signaled or sent input.
- MTG-269 still had draft `sssjs`. A temporary CLI viewer tested raw `x`,
  Kitty `CSI 120u`, press followed by `CSI 120;1:3u`, repeat, and shifted
  encoding. Every case produced native redraw bytes and an owner snapshot with
  the appended character; separate Backspace restored `sssjs` each time.
  Temporary viewers detached with Ctrl+]. No message or goal resume was sent.
  Raw per-case output and snapshots: `/tmp/loo813-debug/`.
- The original `typingAfterKeyboardRestore` assertion confused valid Kitty
  encoding with missing input. Its corrected display diagnostic exercises flags
  5 and 7, starts at the top of 80 rows of scrollback, and reads the **viewport**
  through production `readText(.screen)`. Both cases returned to the live
  viewport and displayed the encoded key. This rules out missing `key.text`
  alone preventing auto-scroll in the pinned build. Ghostty revision
  `a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0`, `Surface.zig` keyCallback,
  scrolls after any encoded non-modifier key. No speculative input/render patch.
- CUA requests for both production and Dev repeatedly returned
  `cgWindowNotFound`; the native test reported CoreVideo error -6661 and timer
  rendering. Synthetic NSEvent/viewport checks are not physical-screen checks.
  The invisible-typing cause is still unknown. Next distinguishing check:
  observe owner output and Desktop viewport/active screen during one physical
  key on MTG-269, then close/reopen and repeat once the screen is available.
- Infrastructure's retained surface captured an empty string. At the later
  passive check its owner socket was absent. Refresh opened the saved conversation
  and its composer, with fresh 15% capacity and the old revoked-token message
  in history. No fresh login or model request. This is recovery; the original
  blank surface's cause remains unestablished.
- All nineteen missing-owner PIDs remain alive. Six are **Codex app servers**,
  not terminal clients: MTG-225, 235, 236, 237, 238, 239. The other thirteen
  are Claude processes. Process ancestry and `lsof` show older lf owners still
  holding listening socket descriptors whose paths no longer exist. No owner,
  provider or companion was stopped. Drafts cannot be inferred from an app-server
  PID or from the absence of the socket path. Full read-only process/stdio
  observations: `/tmp/loo813-debug/missing-owner-processes.json`.

Checks: `scripts/test_desktop.sh --filter 'WorkDestinationTests|SessionsStoreTests|DesktopPaneControlTests'`: 131 passed. `LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --filter typingAfterKeyboardRestore --no-parallel`: both viewport cases passed, with the screen limitation above. Rust work-opening suite: 32 passed and two fixture expectations corrected; both corrected cases passed separately (34 total, two opt-in host diagnostics skipped). `cargo test -p loopflow --lib work::link::tests`: five passed. `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, Swift platform-boundary checks and `git diff --check`: passed. Xcode `build-for-testing`: passed before the final shared shell-pane selection adjustment; the final incremental build follows. The affected-suite wrapper was stopped after it queued duplicate Rust work; Xcode was run directly.

The initial Dev request met the previous shared server and did not establish
a working opening. The final release installation and verification are recorded
in the disposition above. No migration is introduced by this change.

## Unbreak sweep, 2026-11-09 08:06 onward

- Scope: 80 active/detached interactive Sessions, 43 Loopflow and 37 manabot.
  Fresh passive snapshots reached 61 owners; 19 returned ENOENT. Raw observations
  are in `/tmp/loo813-unbreak/`; no missing-owner client was replaced.
- Physical CUA `x` and Backspace worked in original LOO-813 after clicking its
  composer. Its draft at this observation was `s`, not the earlier long draft.
- LOO-515: addressed focus accepted physical `x` and Backspace. Close/reopen
  showed “Active in another terminal”; Take back restored its surface. Two
  physical `x` keys then remained invisible, including after clicking the composer.
  Desktop literal `y` revealed `xxy`. All three characters were deleted.
  Refresh then made physical `x` visible immediately; physical Backspace restored
  the empty composer. This establishes stale display after native key acceptance,
  not that the native editor rejected those keys.
- Display diagnostic `/tmp/loo813-keyboard-native-test.log`: failed the literal
  `x` assertion, but captured `ready\n^[[120u`. It emitted the encoded key;
  this result does not establish that `performKeyEquivalent` swallowed it.
- Fresh `lf account codex --json` at 08:08 accepted engineering with 18% weekly
  remaining; jack, jackstah and manabot-eng were rejected with no login.
- New steer authorizes restarting auth-failed clients on the verified account,
  keeping saved conversations and drafts where possible. Process ancestry showed
  this run under 80759 → 88257; original LOO-813 PID 30832 was not an ancestor.
  Explicit-account resume replaced 30832 on the same provider thread, restored
  its saved `s` draft, and passed physical insert/delete after Desktop Refresh.
  Current parent `session_55fdf896fde94f92aec807061df59417` remains untouched.
- Account recovery and complete per-Session dispositions are recorded below.

### Unbreak result, 2026-11-09 08:25 local

**29 Codex conversations reopened on the freshly verified engineering login,
all on the same provider thread.** At the last inventory, 25 were active or
detached and four were idle/closed after temporary terminal detachment. Idle
retirement is not by itself a failed reopen: all four reached the restored
composer and fresh capacity reading before detach. Desktop exact opening refused
those four taskless conversations; Rex opening refused stale viewer placement.
Their saved history and engineering account selection remain available to
`lf session resume <ID>`.

The complete sweep covered 80 Sessions: 43 Loopflow, 37 manabot. Final passive
snapshots reached 57, with the 19 original missing owners plus the four retired
clients absent. Every original Session is listed below. Passive reachability
is not a physical-keyboard check.

- 14 of the 18 retained Desktop panes passed addressed literal insertion and
  deletion after Take back. LOO-811's additional multiline draft contained
  `uasasfaasfasfa`, then `a` and blank lines: the check's simple composer parser
  stopped at that blank line. Its test `x` was subsequently seen and removed;
  `/tmp/loo813-unbreak/811-probe-cleaned.txt` records the retained draft.
  Engine-performance's delayed `x` was likewise seen and removed, leaving an
  empty composer. MTG-269 was not probed again by the final batch. Infrastructure
  had an input-ownership refusal; exact-Work switch succeeded, but its last
  Desktop capture was empty. Do not count its Desktop input as verified.
- LOO-515 physical typing worked after Refresh and after a later close/reopen.
  The earlier close/reopen failure remains reproduced: `x`, `x` invisible,
  literal paste `y` reveals `xxy`. All three probe characters were removed.
- MTG-269 still received physical keys without visible repaint after account
  recovery, Refresh, and an addressed focus-away/back. Paste revealed `sssjsxy`;
  the test suffix `xy` was removed, retaining `sssjs`. Further keyboard testing
  stopped for debug. Refresh is therefore not a reliable general repair.
- On 15 stale native composers, raw `CSI I` did not reveal hidden probe text.
  Paste did. These checks do not establish why focus reporting or repaint fails.
- Revealed drafts restored through account recovery: original LOO-813 `s`,
  LOO-775 `2as`, MTG-241 `isis uusfs\nafasfadsfaasdfasfsdf`, MTG-260
  `what is whsd`, MTG-269 `sssjs`. Probe cleanup receipts and before/after snapshots
  are in `/tmp/loo813-unbreak/`. No diagnostic character remains in these drafts.
- Agent platform craft and LOO-718 reopened at the paused-goal chooser. Escape
  dismissed 718; encoded Escape (`CSI 27u`) dismissed Agent platform craft after
  the first raw Escape was not confirmed. Both final captures show
  `Goal hit usage limits (/goal resume)`. No goal was resumed.
- Fresh account verification accepted engineering (17% weekly remaining at
  08:18). The other logins were rejected; no credentials were read or copied by
  these diagnostics. Replayed old auth errors remain in conversation history.
  No new model request was sent to prove generation access.
- Parent PID 88257 and this run's PID 80759 remain alive. Original PID 30832 was
  replaced only after the new restart direction and an ancestry check. Native
  lifecycle events in the inspected saved Codex transcripts did not change
  outside the protected active parent. No production code or build changed.
  The pre-existing uncommitted Swift hypothesis test is untouched.

Unresolved cases:

| Case | Exact remaining check / next requirement |
|---|---|
| Physical redraw after reopen | MTG-269 still fails visible physical typing despite received keys. LOO-515 reproduced the same symptom earlier. Debug the native input/render path. |
| Infrastructure Desktop pane | Account resume reached a prompt; final exact-Work switch returned success, then capture returned empty. Verify its actual surface and input ownership. |
| Four taskless conversations | Codex session connections, Competitor architecture research, Land resume fixes, and default `session_aedeca…` resumed on engineering, then retired after detach. Desktop says “Desktop opens exact Sessions through their Task”; Rex says “Current conversation viewer is unavailable; refusing stale Rex startup placement”. CLI resume is the confirmed opening path; Desktop path remains unavailable. |
| Remaining passive Sessions | Physical input was not tested on each detached or Claude client. Reachability and historical completion are not a claim that every composer repaints. Claude quota fallback remains outside this recovery. |
| Nineteen original missing owners | Each `connect(owner.sock)` returned ENOENT. Their native processes and uncertain drafts were kept; inspect/recover native state before replacement. Names and IDs follow. |

| Missing owner Session | Native PID |
|---|---|
| Active work status (`f505abc0e07647b58fdf9d33c5017c25`) | 82207 |
| Fantasia repository operations (`ea5bcb72bd404139a2704faa083edd40`) | 50794 |
| LOO-669 Revamp the release process: minors from main, release streams, lessons f (`f090e9dcf80641afb8d1a0f486979871`) | 66000 |
| MTG-212 Benchmark MTG-183's best checkpoint against the Jev decision model over (`636b009eccd244db836492c431b0ba4e`) | 64105 |
| MTG-219 TLA Card Authoring (`d9c467555ed34b169e20acaf67694088`) | 43702 |
| MTG-220 Decklist import (`1fb1694142e3486e9ddad9e40b945733`) | 78926 |
| MTG-221 TLA T2: Casting, costs, zone permissions and private zones (`a17891fe2e26441382555d7332b5c6ad`) | 72522 |
| MTG-222 TLA T3 Characteristics (`session_89ffcdf8d2444e88a9ae3249095c0609`) | 80851 |
| MTG-222 TLA T3: Characteristics, counters, replacements and attachments (`b239e5500dfb4c9190c126484537559c`) | 88006 |
| MTG-223 TLA T4: Triggers, history and stack behavior (`af0e58a294e547df806225111e9b3170`) | 29585 |
| MTG-224 TLA T5: Permanent copying and linked-ability primitives (`e76c4e99e66a4cbcb9935767c661b97b`) | 53356 |
| MTG-225 T6 Engine Foundations (`5d552358ea0940a1beca11c09a0110ab`) | 40286 |
| MTG-233 Cull everything the codebase no longer needs (`fa6178b2144c47339d9db37f19bf1068`) | 47023 |
| MTG-235 Effect Vocabulary (`81c33e7abbfc4484bf7d81f103db4501`) | 41897 |
| MTG-236 Derived state revisions (`e5d9d2b388ee43349d8b7f3e249c107a`) | 38711 |
| MTG-237 Engine budget review (`7f20038b9d4f48c9b3f9952a0ac44756`) | 39371 |
| MTG-238 Catalog scale types (`eac235f5c3ed403eb9b7f3a94d3a3e75`) | 37552 |
| MTG-239 Five Card Completion (`e2a4b3267d404a61885a763832bfbea5`) | 68037 |
| Repository operator (`session_4ff317503562462cb15dd4ce15b59cf0`) | 71643 |

### Complete 2026-11-09 sweep disposition

| Session | Disposition |
|---|---|
| 30df0c Embed fact and effect ids through the card encoder's exact vocabulary ins (`5c46a82e3afb49b0a92e9df5ad26e71e`) | Passive snapshot; native client untouched. |
| Active work status (`f505abc0e07647b58fdf9d33c5017c25`) | Owner socket missing; native client untouched; input unverified. |
| Agent platform craft (`session_21feb1903a7c4c8f87a8ded394421dbe`) | Same thread reopened on engineering. |
| Codex session connections (`session_b68a12875148418f8361de1f2e5b8387`) | Same thread reopened on engineering. Idle after temporary viewer detach; taskless Desktop opening unavailable. |
| Competitor architecture research (`session_bc1dd22044004fd5a5cbbe748e3e5a20`) | Same thread reopened on engineering. Idle after temporary viewer detach; taskless Desktop opening unavailable. |
| Definitions as code (`session_3c7a3f1076944badb32d253c544b07e4`) | Passive snapshot; native client untouched. |
| Engbot Wave (`617d2a8eaa7b40b5880cc610f4ea098b`) | Same thread reopened on engineering. |
| Fantasia repository operations (`ea5bcb72bd404139a2704faa083edd40`) | Owner socket missing; native client untouched; input unverified. |
| HumanLayer study (`session_ff1b9e68cd2c4055b8b8bdf42c0a82d2`) | Passive snapshot; native client untouched. |
| LOO-312 Newcomer setup (`d566a570e17f465cb266110ae85f3a23`) | Passive snapshot; native client untouched. |
| LOO-434 workflow experience (`01f484c818004d53b393cd8d59c2d090`) | Same thread reopened on engineering. Desktop literal insert/delete passed; physical input not implied. |
| LOO-498 Open and restore (`session_d08da070c64442f68dacef019a3c9c86`) | Passive snapshot; native client untouched. |
| LOO-498 Rex workspace (`310fb7b6015843e498316cd921a348b9`) | Passive snapshot; native client untouched. |
| LOO-515 Task prototypes (`1a2185651fce4f078f44b2201f49d660`) | Passive snapshot; native client untouched. Desktop literal insert/delete passed; physical input not implied. Physical typing passed after Refresh; earlier close/reopen failure recorded. |
| LOO-558 stage handover (`eb1f9cfd9a5045a79b16e262de519f7f`) | Passive snapshot; native client untouched. |
| LOO-593 CI incident (`0ad63132a5314ec996bac303c03859bd`) | Passive snapshot; native client untouched. |
| LOO-669 Revamp the release process: minors from main, release streams, lessons f (`f090e9dcf80641afb8d1a0f486979871`) | Owner socket missing; native client untouched; input unverified. |
| LOO-696 Task concepts (`87d0876bb32941b8a97f81e44872c76f`) | Passive snapshot; native client untouched. Desktop literal insert/delete passed; physical input not implied. |
| LOO-718 Mobile architecture (`session_71e26e01ca6f41a1a9a6f1b4469b3813`) | Same thread reopened on engineering. Desktop literal insert/delete passed; physical input not implied. |
| LOO-723 coherent installation (`322bfd3c1877409d9eea5205d3cd1e14`) | Passive snapshot; native client untouched. |
| LOO-732 telemetry explore (`f24f2497f3fe4753a8a6d23cbcf84e99`) | Passive snapshot; native client untouched. |
| LOO-733 review surface (`942a90ad9a5347d897dbe22c0922e8da`) | Passive snapshot; native client untouched. |
| LOO-747 Ship recovery (`03d6e18c7c6146c2a0e3938cd6baba96`) | Same thread reopened on engineering. |
| LOO-747 default (`session_fc498228861845a39262491f0706d78a`) | Same thread reopened on engineering. |
| LOO-772 Task conversation (`session_8ebcd369c5e9479fbfe989fea0f7b65d`) | Passive snapshot; native client untouched. |
| LOO-775 Watch recovery (`6c0210c03a964af1bc8cc31cd6fd8fd6`) | Same thread reopened on engineering. Revealed draft restored. |
| LOO-776 Keep keystrokes a viewer sent just before it disconnected (`99cbee9948054bdd8795f38b184075b1`) | Passive snapshot; native client untouched. |
| LOO-788 Demo: measure a Task conversation launch (`fcb7642a89c34c4292eb03d080cd5ca5`) | Passive snapshot; native client untouched. |
| LOO-793 Reply references (`85c52002fe524d5daeb4ee4a8e7f8102`) | Same thread reopened on engineering. |
| LOO-800 launch checks (`a1f6ee01550d42cea338c291d1231b00`) | Passive snapshot; native client untouched. |
| LOO-809 Attach tickets for remote clients (`aff39379845f44b8a64b682a75705da9`) | Same thread reopened on engineering. Desktop literal insert/delete passed; physical input not implied. |
| LOO-810 creation recovery (`f53a9069597341d497c56a105eab2e3f`) | Same thread reopened on engineering. |
| LOO-811 Recovery review (`e47ba8ae55044365af58d39fba6c2f83`) | Passive snapshot; native client untouched. Multiline draft revealed; probe removed separately. |
| LOO-812 Session ownership (`a4f6654f7c0f43f4b5273601f739549d`) | Same thread reopened on engineering. |
| LOO-813 Desktop recovery (`ce8db5ba4fcb45a59769b7aaacf19d8a`) | Passive snapshot; native client untouched. |
| LOO-813 Desktop recovery (`session_e52c5b3724d6486d901c84e643e09005`) | Same thread reopened on engineering. Revealed draft restored. Desktop literal insert/delete passed; physical input not implied. |
| LOO-813 Reopened input (`session_55fdf896fde94f92aec807061df59417`) | Protected active parent; passive only. |
| Land resume fixes (`session_cd7e1aab97e8425da06951bb1271c65c`) | Same thread reopened on engineering. Idle after temporary viewer detach; taskless Desktop opening unavailable. |
| List Wave (`6cf8740c22eb4236b5f27cbf90ece384`) | Passive snapshot; native client untouched. |
| MTG-175 Live Eval Reports (`f294041cf95f4c62bdb6b64ac8e56d33`) | Passive snapshot; native client untouched. |
| MTG-212 Benchmark MTG-183's best checkpoint against the Jev decision model over (`636b009eccd244db836492c431b0ba4e`) | Owner socket missing; native client untouched; input unverified. |
| MTG-219 TLA Card Authoring (`d9c467555ed34b169e20acaf67694088`) | Owner socket missing; native client untouched; input unverified. |
| MTG-220 Decklist import (`1fb1694142e3486e9ddad9e40b945733`) | Owner socket missing; native client untouched; input unverified. |
| MTG-221 TLA T2: Casting, costs, zone permissions and private zones (`a17891fe2e26441382555d7332b5c6ad`) | Owner socket missing; native client untouched; input unverified. |
| MTG-222 TLA T3 Characteristics (`session_89ffcdf8d2444e88a9ae3249095c0609`) | Owner socket missing; native client untouched; input unverified. |
| MTG-222 TLA T3: Characteristics, counters, replacements and attachments (`b239e5500dfb4c9190c126484537559c`) | Owner socket missing; native client untouched; input unverified. |
| MTG-223 TLA T4: Triggers, history and stack behavior (`af0e58a294e547df806225111e9b3170`) | Owner socket missing; native client untouched; input unverified. |
| MTG-224 TLA T5: Permanent copying and linked-ability primitives (`e76c4e99e66a4cbcb9935767c661b97b`) | Owner socket missing; native client untouched; input unverified. |
| MTG-225 T6 Engine Foundations (`5d552358ea0940a1beca11c09a0110ab`) | Owner socket missing; native client untouched; input unverified. |
| MTG-233 Cull everything the codebase no longer needs (`fa6178b2144c47339d9db37f19bf1068`) | Owner socket missing; native client untouched; input unverified. |
| MTG-235 Effect Vocabulary (`81c33e7abbfc4484bf7d81f103db4501`) | Owner socket missing; native client untouched; input unverified. |
| MTG-236 Derived state revisions (`e5d9d2b388ee43349d8b7f3e249c107a`) | Owner socket missing; native client untouched; input unverified. |
| MTG-237 Engine budget review (`7f20038b9d4f48c9b3f9952a0ac44756`) | Owner socket missing; native client untouched; input unverified. |
| MTG-238 Catalog scale types (`eac235f5c3ed403eb9b7f3a94d3a3e75`) | Owner socket missing; native client untouched; input unverified. |
| MTG-239 Five Card Completion (`e2a4b3267d404a61885a763832bfbea5`) | Owner socket missing; native client untouched; input unverified. |
| MTG-241 Rules execution (`7e0bc5c92dd54c99b67e2cdea689197e`) | Same thread reopened on engineering. Revealed draft restored. |
| MTG-242 Mana Payment Resources (`e30ecfae91b84e97b2049b9d74318439`) | Passive snapshot; native client untouched. |
| MTG-243 Catalog naming choices (`bbac3df212f24f2db8154ac1538f488e`) | Same thread reopened on engineering. |
| MTG-244 Composable Layer Evaluation (`cac2909324d54d76bc1e28099e30245b`) | Same thread reopened on engineering. |
| MTG-245 Growing Pool Protocol (`ebabbc8406ea42d5a524640af9789582`) | Passive snapshot; native client untouched. |
| MTG-249 Representation experiments (`39a4fa719f2e4399880838fa94905c98`) | Passive snapshot; native client untouched. |
| MTG-250 Sparse attention comparison (`312884d8ee3d4eaaa25b8b84719aca81`) | Passive snapshot; native client untouched. Desktop literal insert/delete passed; physical input not implied. |
| MTG-251 Separate learning seats from batch closure; compare closure and lambda (`7bc9684e2cba48e49a703732b9d4212c`) | Passive snapshot; native client untouched. |
| MTG-252 Resume Claude session (`session_99ffa20715cd497fada9abfce92cbaee`) | Same thread reopened on engineering. Desktop literal insert/delete passed; physical input not implied. |
| MTG-260 Elemental comparisons (`8eeadf4c3bae40448241e4b23dfa0335`) | Same thread reopened on engineering. Revealed draft restored. Desktop literal insert/delete passed; physical input not implied. |
| MTG-261 Development speedups (`ee411e2f7d2c415eb83899367441b93a`) | Same thread reopened on engineering. |
| MTG-267 Frontier self-play (`8f977fd226214c7eb489e6fb6ac6a390`) | Same thread reopened on engineering. Desktop literal insert/delete passed; physical input not implied. |
| MTG-268 Batched rollout status (`6c1c2159e220471e97887187d01d49eb`) | Same thread reopened on engineering. Desktop literal insert/delete passed; physical input not implied. |
| MTG-269 Magic Mirror search (`session_c73fe3a8e9e94b56b1ffe915b575a9c0`) | Same thread reopened on engineering. Revealed draft restored. Physical redraw still fails; probe removed. |
| Release hang fix (`session_4386223ba09a4f0e8bdb1df40c445963`) | Passive snapshot; native client untouched. |
| Repository operator (`session_4ff317503562462cb15dd4ce15b59cf0`) | Owner socket missing; native client untouched; input unverified. |
| Spend Wave (`c76153e219f549b5a5aa5fbede5926f0`) | Same thread reopened on engineering. Desktop literal insert/delete passed; physical input not implied. |
| Study status (`f5acdee3953a449ea8529d905bbf2d3e`) | Passive snapshot; native client untouched. Desktop literal insert/delete passed; physical input not implied. |
| b015d5 Training throughput (`881aafe7262040e486a79a1c429dfe16`) | Same thread reopened on engineering. Desktop literal insert/delete passed; physical input not implied. |
| default (`session_136d7b61eafc4c1a8137ddabb2941051`) | Passive snapshot; native client untouched. |
| default (`session_aedeca37725c4897ad28414ed780af58`) | Same thread reopened on engineering. Idle after temporary viewer detach; taskless Desktop opening unavailable. |
| default (`session_e42c8f360cc64026ab0003f998c304dd`) | Passive snapshot; native client untouched. |
| default (`session_f90a21bc15dd44dfb6c4fa67f2d100de`) | Passive snapshot; native client untouched. |
| engine-performance (`5e99643c30514c3b95e78063030848c7`) | Same thread reopened on engineering. Delayed API probe removed separately; empty composer captured. |
| infrastructure (`09a40b4c78b2411ab30bfbc3203d7304`) | Same thread reopened on engineering. Desktop input unresolved; final capture empty. |

## Reopened input report, 2026-11-09 08:00 local

Jack Heart:

> every session that i close and reopen (maybe only codex, i only have codx right now bc im out of claude credit) isnt receiving keystrokes anymore
> why didnt lf task open loo-515 work then in terminal (it opened it but didnt accept intput)
> now 813 isnt receiving input
> can you go through all my loopflow and manabot sessions then
> $incident

This continuation is `session_55fdf896fde94f92aec807061df59417`; keep it and
the original LOO-813 native process PID 30832 alive. No provider restart or
model turn was submitted during this pass.

- LOO-515 initially captured an empty composer. Desktop API insertion of `x`
  revealed a long existing draft; Backspace removed only `x`. A CUA `x` and
  Backspace also worked before closing. After `desktop close` and `desktop
  view` reopened it, CUA `x` had no visible effect; API insertion of `y`
  displayed only `y`. Backspace removed `y`. The native PID remains 95992,
  started Nov 8 at 19:07 local. Do not claim the earlier draft survived this
  close/reopen: the current empty composer differs, with no established cause.
- A temporary `lf session resume 1a2185651fce4f078f44b2201f49d660` PTY
  accepted both raw `x` and CSI `120u`, and produced redraw output. It detached
  with Ctrl+]. Backspace combined in the same write as Ctrl+] was discarded
  by the relay's detach branch; the remaining test `x` was removed separately
  through Desktop after Refresh. No diagnostic text was submitted.
- LOO-813 initially showed an empty composer and Codex's “New activity”
  scrollback view. API Escape did not visibly change it. API `x` revealed an
  existing multiline draft; Backspace removed only that character. Draft ends
  `asdfasfasdfasasdfasfdasdfasdfasdfasdfafasdfasdfaasdfasasfasfsdfdsdfsdfsdsdf`.
- Passive owner snapshots: 515 restores Kitty flags 5, 813 flags 7, on both
  primary and alternate screens. `/tmp/loo813-input-{515,813}.vt` retain raw
  snapshots. Initial Desktop state, Sessions and process rows are in
  `/tmp/loo813-input-{state,sessions,processes}.json`.
- Added an uncommitted diagnostic `typingAfterKeyboardRestore` in
  `GhosttyTerminalInputTests.swift`, investigating `performKeyEquivalent`
  swallowing plain keys with `key.text == nil`. This is a hypothesis, not a
  fix. `swift test --package-path swift --filter typingAfterKeyboardRestore`
  built successfully but SKIPPED the display suite. Enable its documented
  display-test option before interpreting a result. No production code changed.
- CUA works now, including screenshots and native keys. The app binding follows
  the current window, and the person is navigating concurrently; inspect the
  intended window before input. Prefer addressed Desktop API for draft checks.

The incident skill was invoked now; run unbreak, debug, 5whys, launch-plan in
that order. Prior incident commands are historical, not this invocation.

2026-11-08 report, Jack Heart:

> (a) sessions that hit usage limits didnt auto transfer to codex accounts with space, despite recent diff attempting ot help with this
> (b) one session (718)  Is stuck with Resume paused goal? and not accepting input
> (c) some sessions just wont open
> (4) there's no "refresh" button as there should be for sessions to deal with situations like this

Observed here: Desktop's LOO-718 pane shows Codex's paused-goal dialog. Its
provider and transport processes are alive. Sending Escape through Desktop's
pane API returned success but did not dismiss the dialog. No goal was resumed.
MTG-252's screenshot names Claude thread e80bd648-0697-4548-b0a4-4a82710fdae8;
that transcript was absent from both native and stored Claude homes when checked.
Desktop logs repeatedly show `select a repository before reading execution location`.

Further observations on 2026-11-08:
- LOO-718's viewer-only reconnect did not restore input. Escape advanced the
  native Codex tty's byte offset by five bytes without dismissing the dialog.
  Stopping idle Codex PID 21650, reopening the same saved conversation, and
  sending Escape dismissed it. The goal stayed paused. The cause inside that
  Codex client remains unknown; this was recovery, not a cause fix.
- The ended relay showed “Press any key to close the terminal” while Desktop's
  connection-input guard rejected those keys.
- MTG-252 already had Codex Session session_99ffa20715cd497fada9abfce92cbaee.
  Its opening messages included “reusme the claude session that was hapening in
  this workspace” and “we were testing depth 4 attetnion with different critic
  strategies”. Bound it as primary, replacing the missing Claude reference.
  `session resume --prepare` resolves Codex and reports Reconnect; the installed
  Desktop still reports no surface for this Task.
- The recent quota change automates Graph fallback. Native Claude's limit hook
  only switches Claude logins; it has no Codex fallback. Native Codex observes
  usage but does not perform that recovery either.

Work remaining:
- Extend interactive quota recovery to eligible accounts/providers. A pending
  question asks whether switching should continue the interrupted work or wait
  for a new message; neither behavior has been implemented here yet.
- Verify MTG-252 opens in Desktop after the repository-location fix is installed.
- Verify the Refresh button and ended-terminal key handling in a built app.
- Trace the native Codex input hang if it recurs; reattachment did not fix it.

Checks: focused Rust location regression passed; 69 Swift tests across five
suites passed; Swift rebuilt after ended-terminal repair; cargo fmt and
`cargo clippy --all-targets -- -D warnings` passed. Native terminal interaction
still needs the built-app check above.

## All-session recovery, 2026-11-08 evening

Jack Heart:

> 696 is still unresponsive. can you look into all my open sessions and fix thtem all

A steer relayed a screenshot of MTG-261: “Conversation failed to open” with
“Error: select a repository before reading execution location”, Retry,
“Recorded checkout is missing / Restore checkout” and “Reading workflow…”.
This is the relayed screenshot description, not a quotation from Jack.

Initial inventory: two Desktop windows, 13 native Session surfaces (including
retained hidden workspaces); `session list --all --limit 0` returned 530 rows:
15 active, 59 detached, 13 unknown, 443 closed. The current parent
`session_e52c5b3724d6486d901c84e643e09005` and this unbreak run
`session_c599afa6462f4976b2cf7f7d4b34a0af` are protected throughout.
Raw local observations are under `/tmp/loo813-recovery/`; they include the initial
Desktop state, all Session records, process identities, pane captures and
read-only transport snapshots. No diagnostic message was submitted.

Observed before further recovery:
- LOO-696: Desktop capture initially showed an empty prompt. Inserting `x`
  revealed `whats wsasx`; Backspace left `whats wsas`. Desktop focus succeeded.
  The native editor responds, and its existing draft remains. No restart.
- MTG-269 exhibited the same stale empty prompt: insertion revealed
  `gowgows itasfokaasfx`; Backspace left `gowgows itasfokaasf`. No restart.
- LOO-718, engbot, list, spend and study accepted and removed one character
  through the Desktop API. LOO-718's goal remains paused at its usage limit.
- MTG-175 displayed an active Claude turn and draft `asd`; neither was touched.
  MTG-252, MTG-267 and training-throughput b015d5 displayed working turns.
- Infrastructure and engine-performance displayed Codex sign-in screens.
  Both used the `engineering` account, whose cached credential state was
  `missing`. Their original Codex JSONL histories were readable, 17 records each,
  ending in `task_complete`. Reopening with the connected account keeps the same
  provider thread; Desktop reconnection is still being checked.
- MTG-261's recorded checkout `/Users/jack/src/manabot.0253da-find-out-why`
  was absent. Ordinary `lf task checkout MTG-261` failed because remote branch
  `jack-heart/0253da-find-out-why` was gone. GitHub PR 432 records merged head
  `1841412e42dd667ad6860554754a8134294e9010`, merge
  `c59aeee0ede2b5d9c05a8ad3ce61f89134e60287`, merged 2026-11-06T04:09:59Z.
  `lf task checkout MTG-261 --recover-from 1841412e42dd667ad6860554754a8134294e9010`
  restored the existing Task checkout. No new Task was created.

The first read-only transport snapshots of detached clients showed strings such
as `65535u5;1u` and `65535u7;1u` in Claude and Codex prompt rows. No input had
been sent to those clients by this run. The observer sends only `attach` with
`control:false`, `replace:false`, `own:null`, then `leave`; it sends no key or
terminal-response frames. These are reconstructed snapshots using pyte, so a
parser artifact remains possible until checked against Desktop captures.
No existing draft was erased. Preserve this keyboard-protocol clue for debug.

## Recovery result

- No goal resumed, no diagnostic message submitted, no blanket process stop.
  Only the three idle sign-in clients were reopened with an explicitly selected
  connected Codex account after checking their saved native histories. Original
  provider thread IDs stayed the same. The authentication errors visible above
  their restored prompts are saved conversation output; fresh weekly-capacity
  readings appeared. No new model turn was submitted to test authentication.
- MTG-261 now has its original checkout restored and opens through production
  Desktop. Opening provisioned its previously absent interactive primary,
  `ee411e2f7d2c415eb83899367441b93a`; it reported the merged PR and idle workflow.
  Earlier MTG-261 Sessions were all headless. The opening did not run its Graph.
- Built and installed `~/Applications/Loopflow Dev.app` through
  `uv run python scripts/loopflow-dev.py install` and launched with `run`.
  Production `/Applications/Loopflow.app` was not replaced. Dev remains open as
  the temporary working view for LOO-434 and engine-performance. Their production
  panes were not proven recovered; engine-performance still showed an ended relay.
- Added `lf desktop do refresh-session -p session` to the local Desktop action
  registry, using the existing Refresh implementation. This was needed because
  CUA returned `cgWindowNotFound`, and the installed API cannot invoke Refresh.
  Dev's Refresh reopened the existing conversation; its insert/delete checks
  passed. The production end-of-relay key-dismissal fix is still not installed.
- Refresh changes the viewer, not the native client. LOO-696's and MTG-269's
  empty-looking captures were stale relative to their drafts. API input worked;
  physical keyboard input cannot be verified in this headless run.

Checks: `swift test --filter 'SessionsStoreTests|DesktopPaneControlTests'`:
62 passed. The first run failed only because the new action was absent from
`desktop_action.json`; the fixture was updated and the same suites passed.
Swift app build/install passed. No Rust source changed in this pass; the earlier
Rust format/clippy results remain recorded above. Nothing published or merged.

### Remaining failures for debug

1. Production engine-performance still displays an ended relay. Dev opens the
   same thread and accepts input. Production LOO-434's switch receipt succeeded,
   but only Dev's recovered input was verified.
2. Quota-blocked native clients remain on their original account when a native
   draft or pending state cannot be ruled out. LOO-672's production pane showed
   the zero-capacity account and an unsubmitted `/logo` draft on the later check;
   it was not replaced. Automatic interactive account/provider recovery remains
   incomplete. Claude's two configured accounts were cooling down.
3. Older native processes remain alive without an owner socket. The observer's
   exact failed check was `connect(<owner>.sock): ENOENT`; these may contain
   unrecorded drafts or requests. They need native-state inspection before safe
   replacement, not blanket pruning. The per-Session inventory names each one.
4. The last explicit exact-Session openings of LOO-747, MTG-243, MTG-244 and
   MTG-245 returned opening receipts, then `desktop capture` reported “has no
   workspace in this Desktop window”. LOO-793 reported “no native surface”.
   These were bounded checks, not evidence that opening succeeded. Their passive
   native snapshots showed quota history; no input or restart was performed.
5. The relayed keyboard-protocol clue matches `RESET_MODES` in
   `transport/terminal.rs` and keyboard restoration in Ghostty patch 0003.
   This is a hypothesis for debug. Snapshot parser artifacts have not yet been
   ruled out; no draft cleanup was attempted.

All temporary CLI viewers were detached with Ctrl+]. No watcher or build remains
running. The parent Session and this run's current native identities were checked
again at the end; no process belonging to either was stopped.

### Complete initial open-interactive inventory

This is the full 74-row active/detached inventory across all repositories, not
just Desktop's visible panes. “Passive” means a read-only transport snapshot was
read; it does not claim an input or model-response check. Missing sockets are
reported as unverified, not healthy. Closed history was not reopened.

Counts: active-unmodified 6, blocked-account 7, checked-input 7, missing-relay 20, passive 23, quota 8, recovered 3.

| Session | Initial state | Check and disposition |
|---|---|---|
| 30df0c Embed fact and effect ids through the card encoder's exact vocabulary ins (`5c46a82e3afb49b0a92e9df5ad26e71e`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| Active work status (`f505abc0e07647b58fdf9d33c5017c25`) | detached | Unverified: owner socket absent; recorded native PID 82207. No replacement: draft/turn unknown. |
| Agent platform craft (`session_21feb1903a7c4c8f87a8ded394421dbe`) | active | Rex client: read-only snapshot; paused-goal status retained. |
| Codex session connections (`session_b68a12875148418f8361de1f2e5b8387`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| Competitor architecture research (`session_bc1dd22044004fd5a5cbbe748e3e5a20`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| Definitions as code (`session_3c7a3f1076944badb32d253c544b07e4`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| EXP-28 Combined eval database (`b3ede790bbb442febb9a72d74f599cff`) | detached | Unverified: owner socket absent; recorded native PID 30937. No replacement: draft/turn unknown. |
| EXP-40 Checkpoint wall time (`02f4ffa825c1457daf114b93bc60c27f`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| EXP-42 eval live progress (`274056d18b1b4ecbbd8ee1379b65a6a6`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| Engbot Wave (`617d2a8eaa7b40b5880cc610f4ea098b`) | active | Checked: Desktop insert/delete passed. |
| Fantasia repository operations (`ea5bcb72bd404139a2704faa083edd40`) | detached | Unverified: owner socket absent; recorded native PID 50794. No replacement: draft/turn unknown. |
| HumanLayer study (`session_ff1b9e68cd2c4055b8b8bdf42c0a82d2`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| LOO-312 Newcomer setup (`d566a570e17f465cb266110ae85f3a23`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| LOO-434 workflow experience (`01f484c818004d53b393cd8d59c2d090`) | detached | Recovered in Dev: same thread/account move, Refresh, insert/delete passed. Production reconnection not verified. |
| LOO-498 Open and restore (`session_d08da070c64442f68dacef019a3c9c86`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-498 Rex workspace (`310fb7b6015843e498316cd921a348b9`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| LOO-558 stage handover (`eb1f9cfd9a5045a79b16e262de519f7f`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| LOO-669 Revamp the release process: minors from main, release streams, lessons f (`f090e9dcf80641afb8d1a0f486979871`) | detached | Unverified: owner socket absent; recorded native PID 66000. No replacement: draft/turn unknown. |
| LOO-672 ready conversations (`a66598baebd440cd83b6684339920e95`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| LOO-696 Task concepts (`87d0876bb32941b8a97f81e44872c76f`) | active | Checked: Desktop insert/delete passed; draft `whats wsas` retained; no native restart. |
| LOO-718 Mobile architecture (`session_71e26e01ca6f41a1a9a6f1b4469b3813`) | active | Checked: Desktop insert/delete passed; goal still paused. |
| LOO-723 coherent installation (`322bfd3c1877409d9eea5205d3cd1e14`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-732 telemetry explore (`f24f2497f3fe4753a8a6d23cbcf84e99`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-733 review surface (`942a90ad9a5347d897dbe22c0922e8da`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| LOO-747 Ship recovery (`03d6e18c7c6146c2a0e3938cd6baba96`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| LOO-747 default (`session_fc498228861845a39262491f0706d78a`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-772 Task conversation (`session_8ebcd369c5e9479fbfe989fea0f7b65d`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-775 Watch recovery (`6c0210c03a964af1bc8cc31cd6fd8fd6`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-776 Keep keystrokes a viewer sent just before it disconnected (`99cbee9948054bdd8795f38b184075b1`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-788 Demo: measure a Task conversation launch (`fcb7642a89c34c4292eb03d080cd5ca5`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-793 Reply references (`85c52002fe524d5daeb4ee4a8e7f8102`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-810 creation recovery (`f53a9069597341d497c56a105eab2e3f`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-811 Recovery review (`e47ba8ae55044365af58d39fba6c2f83`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| LOO-813 Desktop recovery (`session_e52c5b3724d6486d901c84e643e09005`) | active | Protected parent: native process alive; no input or restart. |
| Land resume fixes (`session_cd7e1aab97e8425da06951bb1271c65c`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| List Wave (`6cf8740c22eb4236b5f27cbf90ece384`) | active | Checked: Desktop insert/delete passed. |
| MTG-175 Live Eval Reports (`f294041cf95f4c62bdb6b64ac8e56d33`) | active | Active/unmodified: Claude working, draft `asd`, account marked blocked. |
| MTG-212 Benchmark MTG-183's best checkpoint against the Jev decision model over (`636b009eccd244db836492c431b0ba4e`) | detached | Unverified: owner socket absent; recorded native PID 64105. No replacement: draft/turn unknown. |
| MTG-219 TLA Card Authoring (`d9c467555ed34b169e20acaf67694088`) | detached | Unverified: owner socket absent; recorded native PID 43702. No replacement: draft/turn unknown. |
| MTG-220 Decklist import (`1fb1694142e3486e9ddad9e40b945733`) | detached | Unverified: owner socket absent; recorded native PID 78926. No replacement: draft/turn unknown. |
| MTG-221 TLA T2: Casting, costs, zone permissions and private zones (`a17891fe2e26441382555d7332b5c6ad`) | detached | Unverified: owner socket absent; recorded native PID 72522. No replacement: draft/turn unknown. |
| MTG-222 TLA T3 Characteristics (`session_89ffcdf8d2444e88a9ae3249095c0609`) | detached | Unverified: owner socket absent; recorded native PID 80851. No replacement: draft/turn unknown. |
| MTG-222 TLA T3: Characteristics, counters, replacements and attachments (`b239e5500dfb4c9190c126484537559c`) | detached | Unverified: owner socket absent; recorded native PID 88006. No replacement: draft/turn unknown. |
| MTG-223 TLA T4: Triggers, history and stack behavior (`af0e58a294e547df806225111e9b3170`) | detached | Unverified: owner socket absent; recorded native PID 29585. No replacement: draft/turn unknown. |
| MTG-224 TLA T5: Permanent copying and linked-ability primitives (`e76c4e99e66a4cbcb9935767c661b97b`) | detached | Unverified: owner socket absent; recorded native PID 53356. No replacement: draft/turn unknown. |
| MTG-225 T6 Engine Foundations (`5d552358ea0940a1beca11c09a0110ab`) | detached | Unverified: owner socket absent; recorded native PID 40286. No replacement: draft/turn unknown. |
| MTG-233 Cull everything the codebase no longer needs (`fa6178b2144c47339d9db37f19bf1068`) | detached | Unverified: owner socket absent; recorded native PID 47023. No replacement: draft/turn unknown. |
| MTG-235 Effect Vocabulary (`81c33e7abbfc4484bf7d81f103db4501`) | detached | Unverified: owner socket absent; recorded native PID 41897. No replacement: draft/turn unknown. |
| MTG-236 Derived state revisions (`e5d9d2b388ee43349d8b7f3e249c107a`) | detached | Unverified: owner socket absent; recorded native PID 38711. No replacement: draft/turn unknown. |
| MTG-237 Engine budget review (`7f20038b9d4f48c9b3f9952a0ac44756`) | detached | Unverified: owner socket absent; recorded native PID 39371. No replacement: draft/turn unknown. |
| MTG-238 Catalog scale types (`eac235f5c3ed403eb9b7f3a94d3a3e75`) | detached | Unverified: owner socket absent; recorded native PID 37552. No replacement: draft/turn unknown. |
| MTG-239 Five Card Completion (`e2a4b3267d404a61885a763832bfbea5`) | detached | Unverified: owner socket absent; recorded native PID 68037. No replacement: draft/turn unknown. |
| MTG-241 Rules execution (`7e0bc5c92dd54c99b67e2cdea689197e`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| MTG-242 Mana Payment Resources (`e30ecfae91b84e97b2049b9d74318439`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| MTG-243 Catalog naming choices (`bbac3df212f24f2db8154ac1538f488e`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| MTG-244 Composable Layer Evaluation (`cac2909324d54d76bc1e28099e30245b`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| MTG-245 Growing Pool Protocol (`ebabbc8406ea42d5a524640af9789582`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| MTG-249 Representation experiments (`39a4fa719f2e4399880838fa94905c98`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| MTG-250 Sparse attention comparison (`312884d8ee3d4eaaa25b8b84719aca81`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| MTG-251 Separate learning seats from batch closure; compare closure and lambda (`7bc9684e2cba48e49a703732b9d4212c`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| MTG-252 Resume Claude session (`session_99ffa20715cd497fada9abfce92cbaee`) | active | Active/unmodified: production Desktop working turn; opens successfully. |
| MTG-267 Frontier self-play (`8f977fd226214c7eb489e6fb6ac6a390`) | active | Active/unmodified: production Desktop working turn. |
| MTG-269 Magic Mirror search (`session_c73fe3a8e9e94b56b1ffe915b575a9c0`) | active | Checked: Desktop insert/delete passed; draft `gowgows itasfokaasf` retained. |
| Release hang fix (`session_4386223ba09a4f0e8bdb1df40c445963`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| Repository operator (`session_4ff317503562462cb15dd4ce15b59cf0`) | detached | Unverified: owner socket absent; recorded native PID 71643. No replacement: draft/turn unknown. |
| Spend Wave (`c76153e219f549b5a5aa5fbede5926f0`) | active | Checked: Desktop insert/delete passed. |
| Study status (`f5acdee3953a449ea8529d905bbf2d3e`) | active | Checked: Desktop insert/delete passed. |
| b015d5 Training throughput (`881aafe7262040e486a79a1c429dfe16`) | active | Active/unmodified: production Desktop working turn. |
| default (`session_136d7b61eafc4c1a8137ddabb2941051`) | detached | Unmodified: passive snapshot read; account marked blocked; native input retained. |
| default (`session_aedeca37725c4897ad28414ed780af58`) | detached | Still blocked: passive snapshot shows Codex quota refusal; uncertain native input retained. |
| default (`session_e42c8f360cc64026ab0003f998c304dd`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| default (`session_f90a21bc15dd44dfb6c4fa67f2d100de`) | detached | Unmodified: passive snapshot read; input not tested; native input retained. |
| engine-performance (`5e99643c30514c3b95e78063030848c7`) | active | Recovered in Dev: same thread/account move, Refresh, insert/delete passed. Production pane remains ended. |
| infrastructure (`09a40b4c78b2411ab30bfbc3203d7304`) | active | Recovered: same thread, connected account; production Desktop input check passed. |

### Historical records without a known active client

The remaining 13 non-closed rows had state `unknown` and no recorded AgentProcess.
They are not established open native clients; none was launched or reset.

- Define durable subwave identity and memory addresses: `task_f8a2b8980fb74eec9c66fad1b590a7bf:08972f20-efde-4113-bfc5-e14b649ea195:task-design:review_kickoff:0`.
- LOO-572 Record one usage row per turn, stop recording internal commands, close d: `6df92ca4d9b249bfa6c379dd98e69735`.
- MTG-10 Define viewer-safe Study artifact and evidence contract: `f2ab8b42b2ce46f5b2c4d5b10fed54dc`.
- MTG-268 Batch local-update rollouts into one network call per step: `6c1c2159e220471e97887187d01d49eb`.
- Make Linear OAuth recovery survive unattended Project runs: `task_81bb835a6bd04748bf99e906aa9f4236:migrated-task-task_81bb835a6bd04748bf99e906aa9f4236-1790208246:ship-demo:review_demo:0`.
- Restore end-to-end Project and Task controller execution after rewrites: `task_f4f230f6de97470fa25be460bb9552ec:migrated-task-task_f4f230f6de97470fa25be460bb9552ec-1788413693:ship-demo:review_demo:0`.
- claude 27120774: `session_fcaecb6c626e46c182a2803262bc7b62`.
- demo: `session_3211bb1b212942d2909c0ca84b7b3880`.
- demo: `session_63fc75b803664b3db1bacc9c6bb0ad23`.
- pr-review: `session_06bcfcf41cc041aa9c1a9b64382a0f9a`.
- pr-review: `session_4fd10ead97ad4fd6819fcdc2e5437e6b`.
- pr-review: `session_6c312e9dbf144e84b30262524c953122`.
- repo: `142f167d3ba14d8e80e6c0240a702f85`.

### Final sweep

The final sweep covered both production windows and both local Dev windows,
including 25 retained Session surface occurrences. Six additional active Sessions
appeared during the pass (some were warmed by opening Desktop); all are listed
below. Existing active processes and drafts were left intact.

| Session | Final observation |
|---|---|
| LOO-800 launch checks (`a1f6ee01550d42cea338c291d1231b00`) | Desktop shows prompt; passive check only, unmodified. |
| LOO-809 Attach tickets for remote clients (`aff39379845f44b8a64b682a75705da9`) | Desktop shows prompt; passive check only, unmodified. |
| LOO-812 Session ownership (`a4f6654f7c0f43f4b5273601f739549d`) | Desktop shows prompt; passive check only, unmodified. |
| MTG-260 Elemental comparisons (`8eeadf4c3bae40448241e4b23dfa0335`) | Desktop shows prompt; passive check only, unmodified. |
| MTG-261 Development speedups (`ee411e2f7d2c415eb83899367441b93a`) | Desktop shows prompt; passive check only, unmodified. |
| MTG-268 Batched rollout status (`6c1c2159e220471e97887187d01d49eb`) | Desktop shows prompt; passive check only, unmodified. |

Before/after captures and the passive reconstructed screens are saved in
`scratch/desktop-recovery-captures.json`; final Desktop captures are included.
The 74 original active/detached Sessions plus these six were inventoried.

## Debug continuation after credential revocation (2026-11-08)

- Re-read the full inventory and the Nov 7 reconnect findings in
  `loopflow.desktop-bug-repors/scratch/incident-codex-reconnect.md`.
- Isolated parser experiment: `uv run --with pyte python` feeding only
  `ESC[<65535u ESC[=5;1u` renders `65535u5;1u`. The literal text in the passive
  pyte captures is therefore not evidence of native draft corruption. No draft
  was erased and no keyboard reset patch was made.
- Production LOO-696 initially captured its retained `whats wsas` draft;
  explicit Desktop focus succeeded. A later capture returned `The retained pane
  has no native surface; no client was acquired.` No input or native restart
  was performed on 696 during this pass. Physical keyboard behavior remains
  unverified; the changing retained surface needs a new observation.
- Exact-Session lookup unnecessarily waited for every historical page, allowing
  an unrelated later page failure to prevent opening a record already found.
  It now stops at that record and merges it into the latest inventory. The
  regression supplies a failing later page and checks the selected conversation
  and retained unrelated record. This is an established code failure, not a
  demonstrated explanation of every production opening failure.
- Production LOO-793 returned `opening` while its selected record was absent;
  the Dev opening later reported `Opening canceled by navigation or a newer
  request.` Do not count either receipt as recovery. MTG-243/244 captures still
  returned `no native surface`. MTG-245 initially captured its quota dialog and
  empty composer in production.
- `lf account next --json` reported Claude blocked (both logins cooling), and
  Codex `jackstah-1066ea9c99d1` ready. A bounded same-Session MTG-245 move with
  `--account codex=jackstah@gmail.com` displayed the old client's exact error:
  `this store needs an lf that knows task_session_promotion (it has task_session_promotion; latest known wave_removal); run lf install`.
  The replacement displayed Codex's sign-in screen. The Session record changed
  from `manabot-eng` to `jackstah-1066ea9c99d1`; native PID 44361 started at
  18:30:07 local. No model turn, credit purchase or goal resume was submitted.
  The temporary viewer detached with Ctrl+]. Desktop switch/open was requested
  again, but the first capture had no native surface. Cached account state says
  connected for this login while provider-level credentials say expired;
  this discrepancy is not yet diagnosed. Do not bulk-move the remaining clients
  based only on `account next`.
- Parent native PID 30832 was freshly confirmed alive with start time
  15:39:36 local. No protected process was signaled. No missing-socket owner was
  stopped; the Nov 7 missing provider endpoints and current owner sockets remain
  distinct failures.

Checks: `swift test --package-path swift --filter
'WorkDestinationTests|SessionsStoreTests|DesktopPaneControlTests'`: 129 passed.
The first pass exposed three fixtures still expecting unqualified task-location
commands from before this branch's repository fix, plus an old test expecting
exact opening to enumerate unrelated history. Fixtures now use the scoped
command; full inventory pagination is still tested separately from opening.
`uv run python scripts/loopflow-dev.py install`: passed (existing Ghostty symbol
warnings); installed Dev only. `git diff --check`: passed. No Rust changes in
this pass; no publication or merge. Production installation remains unchanged.
Raw observations and logs: `/tmp/loo813-recovery/debug-*`.

Remaining: establish why the ready Codex route reaches sign-in, restore and
verify production retained surfaces including 696/434/engine-performance, inspect
native drafts for the missing-owner clients, and implement safe interactive
quota fallback. This incident is not fully repaired and is not ready to be
reported as all Sessions working.

## Continued in the fork, 2026-11-08 19:00–19:15 local

- Protected continuation: `ce8db5ba4fcb45a59769b7aaacf19d8a`. The earlier
  headless debug Session was closed; no Graph was running.
- A fresh `account/read` against the stored `jackstah-1066ea9c99d1` login
  returned JSON-RPC error `-32603` with the phrases `refresh token`, `revoked`,
  and `sign in`. Only those phrases and the error code were emitted by the
  diagnostic. Account inspection called it `unavailable` and retained
  `connected`: subscription polling did not use the agent runner's existing
  revoked-token classification. The regression now checks both definitive
  rejection and recovery from a working copy of the same login.
- `lf account codex=loopflow-eng@loopflow.studio --json` freshly reported
  `verification: accepted`, active, with a weekly-capacity reading. This differs
  from the earlier missing-credential observation; no reconnect was performed
  by this continuation.
- Both Desktop applications were closed at the first current app inventory.
  A Desktop state request timed out against its leftover socket. Opening
  `/Users/jack/Applications/Loopflow Dev.app` restored API responses.
- Exact LOO-696 opening in the earlier Dev build ended with `Opening was
  closed or released.` while startup still showed saved Work. A regression
  reproduced this: navigation detached the saved workspace and cleared the
  opening request assigned immediately before navigation. Assigning that
  request afterward lets the addressed Session install. Native verification
  of the rebuilt app is still pending below.
- Official Codex app-server docs describe `account/read` as refreshing the
  current managed login and `chatgptAuthTokens` as an experimental mode whose
  host owns token refresh. They do not establish that managed accounts can be
  switched in place while retaining a native composer. No external-token mode
  was added: https://learn.chatgpt.com/docs/app-server#auth-endpoints .

Checks: revoked-refresh regression 1 passed; same-login recovery suite 7 passed;
Swift WorkDestination/SessionsStore/DesktopPaneControl suites 130 passed;
`cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed. Rust test
linking reported its existing large `__eh_frame` warning. Dev build/install
and native recovery checks follow; automatic interactive fallback and the
twenty uncertain native clients remain unfinished.

### Subscription truncation, 19:13–19:20 local

The rebuilt Dev app's LOO-696 opening reached `usable`, then its current
Session reading said `Selected Session is absent from the current Session
reading`; capture again reported no native surface. The exact Session still
existed, active, and an unlimited repository list contained it among 348 rows.
The server's unpaged `Read::SessionList` omitted `--limit`, inheriting the CLI's
100-row default. Its next subscription frame discarded older exact openings.
The unpaged served read and Swift CLI fallback now request `--limit 0`; paged
reads remain bounded. A 101-Session regression exercises the parsed filter
against SQLite. The shared server must be updated for native verification.

Live account inspection with the fixed binary reported the jackstah login
`missing`/`rejected` and supplied `lf account connect codex=jackstah@gmail.com`.
It is no longer advertised as connected by cached routing.

Installation preview found the shared store already at `server_request_stops`
and all 21 executable references resolvable, with no migration to apply. A
CLI-only preview refused because the installed app and CLI must update
together. No CLI-only installation was attempted.

### Production checks after promotion, 19:23–19:34 local

Promoted the matching signed app and CLI to `/Applications/Loopflow.app` and
`~/.local/bin/lf`; no schema migration. The service reports revision
`64cffb3cc`. The installer retained the prior artifacts. Added the existing
Take back operation to the Desktop action registry for the same addressed-pane
control as Refresh; CUA still returned `cgWindowNotFound`.

- LOO-696 remained in the current inventory after subscription updates. Its
  current native client was at Codex sign-in, unlike the earlier draft
  observation. Reopened its saved thread with the freshly verified engineering
  login; it reached the composer and reported 21% weekly capacity. The old token
  error is in saved output. No model turn was submitted. Production Take back
  acquired its terminal; captured `x`, deleted it, and captured the empty
  composer again. Refresh and navigation retained the conversation.
- Production LOO-434 and LOO-718 acquired their native terminals. Each captured
  `x`, then an empty composer after Backspace. LOO-434 survived Refresh. LOO-718
  still displayed `Goal hit usage limits (/goal resume)`; no goal resumed.
- A fresh passive inventory found no Codex sign-in screen among reachable
  clients. MTG-245 already displayed a prompt and 21% weekly capacity; this
  continuation did not change it. The original twenty missing-socket clients
  still failed the passive connection check; no replacement was attempted.

Checks: Swift SessionsStore/DesktopPaneControl suites: 62 passed after adding
the Take back action and its DTO fixture; `git diff --check` passed. Earlier
151-test Swift, Rust focused tests and clippy results apply to unchanged code.
Current observations: `/tmp/loo813-current/`; production captures:
`/tmp/loo813-696-after-refresh.txt`. Automatic interactive account/provider
fallback remains unimplemented; this is not an all-Sessions recovery claim.

Engine-performance's production pane also captured `x` and acknowledged
Backspace. Capture after Refresh timed out twice. Samples from production PID
14527 show the main thread in Task-index rebuilding and SwiftUI attribute
comparison, not waiting on a provider. Both the app and service were debug
builds; replacing them with normal release builds is the next check.
Samples: `/tmp/loo813-desktop-stall.sample`,
`/tmp/loo813-desktop-stall2.sample`. No extra keystroke was sent after the
timeout.

### Release installation, 2026-11-09 morning

The build processes survived the overnight sleep. `swift build --package-path
swift -c release` and `cargo build --release -p loopflow --bin lf` passed. Cargo
recompiled locally after sccache stopped. The signed release bundle and CLI were
promoted together at `a1cf57374`; no migration. Prior installation artifacts
remain available through the installer.

Both repository windows then reported current readings. Production captures of
LOO-696, LOO-434, LOO-718 and engine-performance succeeded. Refresh completed for
696, 434 and engine-performance, followed by successful captures. LOO-696 again
captured `x` and then its empty composer after Backspace. LOO-718 still displayed
`Goal hit usage limits (/goal resume)`. No goal resumed or model turn submitted.
The debug-build stall did not recur during these release checks; its root cause
has not been established. Physical keyboard interaction remains unverified.

Release logs: `/tmp/loo813-release-build.log`, `/tmp/loo813-release-cli.log`,
`/tmp/loo813-promote-release.log`. Session readings:
`/tmp/loo813-release-sessions.json`. The automatic interactive quota handoff and
the pending choice about replacing the twenty uncertain clients remain open.
