# LOO-1064 · Remove Session transition banners

> Remove the two "Resumed in a new process" (ProviderTransition) headers shown in Desktop session panes below the toolbar strip

## Done when

Open a Task’s conversation and continue when the pane accepts input. In a usable pane, the terminal starts immediately below the provider and program status strip after a resume, fork, reattach or fresh start. If input pauses, the pane shows one opening, reconnecting, unavailable or ended message. **Retry connection** is available when the relay can receive it. **Refresh session** can reopen the viewer if the status connection is gone. Recorded provider changes are available in `lf session history <session>`.

## Design

The Session is the conversation. Its provider thread can run in another process while the Session keeps its identity. The pane is a view of that conversation and can reconnect independently of the provider. Today `ProviderTransitionRow` describes the last recorded provider change and the Ghostty overlay can describe the latest attachment with the same words. After another relay reconnect to the same process, those two messages can even describe different moments. [Concept review](concept-review.html)

Use the connection reading for live pane feedback. The opening placeholder hands off to the mounted terminal and its connection overlay. On a connected reading, derive the title from `acceptsInput`. Before native readiness, show “Opening… Input paused.” Once input is enabled, return no title and use the existing zero-height hidden bar. Reconnecting, unavailable and ended keep their messages, fallbacks and controls, including the retirement reason from LOO-774. Keep the retained screen, input gating and focus restoration as they are. The strip continues to report provider and program status.

Remove `ProviderTransitionRow`, its Swift explanation formatter, and the unused `transition` member of Desktop’s `ServerConnectionReading`. Swift can decode the older relay payload by ignoring that key. Keep the Rust relay publisher and the Session DTO wire shape for existing clients. Storage continues to record observed provider changes, including fresh threads when native history cannot be restored. The existing launch diagnostics and failure paths still report what happened during opening.

## Build and check

1. Remove both rendering paths and the unused relay decoder member in one slice. Use a resumed `SessionRecord` in `WorkspacePanesTests.swift` to check that the pane shows the Claude strip without transition text. In `ServerConnectionTests.swift`, send the old relay payload including a resumed transition. Wait for the connection reading, then assert an opening title with input paused. Publish native readiness for the same generation and payload and assert a nil title with input enabled. Use the existing reconnect, unavailable/Retry and orderly-close cases for the remaining states. Move the shared `provider_transition.json` fixture check from the deleted presentation test in `CreationSubmissionTests.swift` to `DTOFixtureTests.swift` as a `ProviderTransition` round trip.
2. Update `docs/sessions.md` and `swift/README.md` to describe live connection messages, when to use **Retry connection** and **Refresh session**, and where to see recorded provider changes.

**Gate.** `scripts/test_desktop.sh --filter 'WorkspacePanesTests|CreationSubmissionTests|ServerConnectionTests|DTOFixtureTests' </dev/null` exercises both consumers and the wire fixture headlessly. The resumed Session keeps its strip, the relay’s opening banner clears when input becomes usable, and recovery and close reasons still pass.

**Demo.** In Loopflow Dev, open a Task’s Session with a recorded resume in a new provider process. Once the provider is ready, the terminal starts directly below the pane strip with its top lines visible. Refresh the Session and observe that the opening or reconnecting banner clears when input becomes available. `lf session history <session>` still reports the transition.
