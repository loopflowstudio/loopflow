# LOO-1064 · Remove Session transition and opening headers

> Remove the transition ("Resumed in a new process", "New conversation", "Same process", "Forked from ...") and "Opening… Input paused." headers shown in Desktop session panes below the toolbar strip

## Done when

The terminal opens directly below the provider and status strip for fresh,
resumed, forked and reattached Sessions. Before the terminal mounts, the
opening placeholder shows a quiet spinner over any retained output. VoiceOver
labels it “Opening session.” The mounted terminal draws in place while input
and focus restoration wait for attachment and native readiness.

Only reconnecting, unavailable and ended Sessions show a connection bar.
**Retry connection** asks a reachable relay to reconnect. **Refresh session**
reopens the viewer after status loss. A long relay reason truncates beside
Retry and remains available in its tooltip. Desktop ignores the relay’s
transition field. Older clients still receive it, and recorded transitions
remain in `lf session history <session>`. [Concept review](concept-review.html)

## Checks

`scripts/test_desktop.sh --skip-build --filter 'ServerConnectionTests|providerTransitionRoundTrips|openingSessionShowsProgressOverItsRetainedScreen|sessionStripShowsProviderWithoutTransitionHeader|queuedWorkingTaskShowsOpening|visibleTilesOpenWithoutSavedVisits|primaryPaneNeverOffersAnOpenGate'` passed all 12 tests headlessly after `swift build --build-tests --package-path swift`.

`scripts/test_desktop.sh --skip-build --filter 'ServerConnectionTests|TerminalPolishTests|providerTransitionRoundTrips|openingSessionShowsProgressOverItsRetainedScreen|sessionStripShowsProviderWithoutTransitionHeader'` passed all 14 tests in the concept review. [Dev QA](qa.html) exercised fresh and resumed panes, recovery after a server outage, and Retry layout. Foreground focus, live Refresh after status loss and the forked pane remain unverified in Dev.

**Gate.** Run `scripts/test_desktop.sh --filter 'WorkspacePanesTests|CreationSubmissionTests|ServerConnectionTests|DTOFixtureTests|WorkCacheTests' </dev/null` and the `documented_commands::` check.

**Demo.** In Loopflow Dev, open a fresh Task conversation and a resumed one.
Check the first line beneath the strip during opening, after refresh and once
typing resumes. Drop the connection to see the recovery controls. Compare
the resumed pane with the recorded transition in `lf session history <session>`.
