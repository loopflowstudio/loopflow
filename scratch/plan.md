# LOO-1064 · Remove Session transition and opening headers

> Remove the transition ("Resumed in a new process", "New conversation", "Same process", "Forked from ...") and "Opening… Input paused." headers shown in Desktop session panes below the toolbar strip

## Done when

Open a Task’s conversation and continue when the pane accepts input. The terminal starts immediately below the provider and program status strip during initial opening and after a resume, fork, reattach or fresh start. While preparing before the terminal mounts, `SessionOpeningView` shows a quiet spinner (or retained screen) with no `"Opening… Input paused."` header. Input stays paused until attachment and native input readiness arrive, without rendering `connectionBar`. `connectionBar` appears only for disconnection, unavailability, or end states (`.reconnecting`, `.unavailable`, `.ended`). **Retry connection** is available when the relay can receive it. **Refresh session** can reopen the viewer if the status connection is gone. Recorded provider changes are available in `lf session history <session>`.

## Implementation

The provider and status strip leads straight into pane content. During preparation the spinner has a VoiceOver label and any retained output stays readable. Once the terminal mounts, only reconnecting, unavailable and ended readings have a title. Native readiness still controls attachment, keyboard input and focus restoration.

Desktop ignores the relay’s transition field. Older clients still receive it, and the Session DTO still carries recorded transitions. The recovery controls and close reasons follow the existing status channel. [Original concept review](concept-review.html)

## Checks

`scripts/test_desktop.sh --skip-build --filter 'ServerConnectionTests|providerTransitionRoundTrips|openingSessionShowsProgressOverItsRetainedScreen|sessionStripShowsProviderWithoutTransitionHeader|queuedWorkingTaskShowsOpening|visibleTilesOpenWithoutSavedVisits|primaryPaneNeverOffersAnOpenGate'` passed all 13 tests headlessly after `swift build --build-tests --package-path swift`.

Gate owns the wider Desktop suites and the `documented_commands::` check. The live Desktop demo remains.

**Gate.** `scripts/test_desktop.sh --filter 'WorkspacePanesTests|CreationSubmissionTests|ServerConnectionTests|DTOFixtureTests|WorkCacheTests' </dev/null` exercises all consumers and the wire fixture headlessly. Resumed and fresh Sessions keep their strip with no transition or opening headers, input stays paused until native readiness arrives, and `.reconnecting`, `.unavailable`/Retry, and `.ended` close reasons still render and pass.

**Demo.** In Loopflow Dev, open a Task’s Session with a recorded resume in a new provider process (and a fresh conversation). During opening and once the provider is ready, no transition or `"Opening… Input paused."` header appears below the pane strip; the terminal starts directly below the strip with its top lines visible. Refresh the Session and observe no opening header while input waits for readiness, and confirm `.reconnecting`/`.unavailable` banners still appear when connection drops. `lf session history <session>` still reports the transition.
