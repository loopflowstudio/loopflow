# Native Task-to-Session demo

LOO-718 · interactive demo · November 3, 2026 (America/Los_Angeles)

## Overnight implementation update

Jack Heart subsequently requested sustained autonomous improvement. Review is
not a prerequisite for further implementation. The original demo evidence below
remains historical. The current build has compact searchable rows, a dedicated
missing-Session explanation, keyboard restoration on a fresh grant and preserved
observer geometry. See [current implementation](mobile-ios.md) for the 16-test
result and the remaining live-connection and gesture proof. Updated screenshots:
[Task list](demo-assets/polished-tasks.png), [keyboard open](demo-assets/polished-keyboard.png).
The earlier HTML page still shows the original build; it is not current app evidence.

Review build: `2dfb5d2a52`, [PR #1812](https://github.com/loopflowstudio/loopflow/pull/1812). Design: [native iOS implementation](mobile-ios.md). Usage and renderer choice: [iOS README](../swift/LoopflowiOS/README.md). Review page: [demo.html](demo.html). This note records evidence; the page presents the walkthrough and response fields.

## Decision for Jack Heart

Review the native Task-list density and terminal interaction before accepting this frontend slice. The target is an explicitly offline native client with real input/rendering behavior, not a connected phone product. No acceptance or merge approval has been given during this demo.

## What was observed

The assistant built the current LoopflowiOS target successfully, installed it on iPhone 17 Pro Simulator / iOS 26.2 and launched with `--fixture`. Build output: `/tmp/loopflow-demo-build.log`. No production code changed and the previously recorded 14-test result was not rerun or presented as a new pass.

- The native Task list shows three fixture Tasks and an explicit Offline fixture / no Machine connection label. Selecting Session input study opens its existing fixture terminal.
- Use keyboard grants the fixture's input/geometry permissions. The software keyboard was made visible using Simulator's I/O → Keyboard → Toggle Software Keyboard. The Paste / New line / hide-keyboard row remains above it, with terminal content visible.
- Entering `permission` presented the fixture permission prompt. Tapping the software `n` and Return keys visibly produced “Fixture action canceled.” Initial synthetic desktop-key attempts did not visibly resolve the prompt; only the software-key interaction is claimed as observed cancellation. Physical keyboard handling still needs direct review.
- Simulator Home backgrounds the app. Reopening showed “Disconnected · Paused while the app is away,” retained output and a Reconnect action. Reconnect restored viewing mode on the same fixture Session, with input controls disabled and a fresh Use keyboard action. No second submission appeared in this walkthrough. This is local peer evidence, not network recovery proof.
- The native list differs materially from the earlier HTML study: larger Working Tasks heading and inset grouped rows; only title and generic state are rendered, with no Task identifier, provider or Session excerpt. This is an observed presentation difference for review, not an accepted redesign or a confirmed requirement to revert.

Screenshots: [native Tasks](demo-assets/native-tasks.png), [keyboard/prompt](demo-assets/native-keyboard.png), [canceled action](demo-assets/native-response.png), [disconnected](demo-assets/native-disconnected.png), [reconnected observer](demo-assets/native-reconnected.png). The before image is [the HTML study](prototype-assets/task-list.png), not a previous native release.

## Jack Heart's feedback and separate work

Jack asked whether the app needs registration and TestFlight setup and said that is probably its own Task. Captured [LOO-721 · Install and update Loopflow iOS through TestFlight](loopflow://loopflow/task/LOO-721) in this repository's Infrastructure Wave, without starting a worker. Its acceptance includes app identity/team selection, signed upload, a real-device installation and second-build update. It also handles the current `--fixture` launch-argument limitation; a TestFlight launch does not receive Xcode arguments.

No feedback on native spacing, keyboard ergonomics or reconnect policy has yet been supplied. The assistant's observations above do not stand in for Jack's judgment. The fixture's Use keyboard action does not settle remote takeover behavior.

## Remaining proof and next action

Jack can use the open Simulator and the page's action/expected-result pairs to review the experience. Record his answers here, revise mobile-ios.md if a design change is agreed, and return concrete findings for implementation if needed. Do not advance the Graph or close the Task merely because the demo is open.

Still unproven: physical-device typing/IME/dictation, physical keyboard combinations, VoiceOver navigation and sustained-output energy use; live Machine reads/owner attachment, Tailscale admission, control handoff and recovery beyond retained output. Multiline paste and marked-text handling have earlier automated evidence, but were not manually demonstrated in this walkthrough. There is no live backend connection in this app build.

The simulator is left at the reconnected Session in viewing mode. `scratch/demo.html` compares the study with the native screenshots, preserves observed gaps and provides numbered review choices. Page notes are temporary until copied back into this conversation; the Markdown record is updated only from actual feedback.
