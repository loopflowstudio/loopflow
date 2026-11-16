# Presentation source

SwiftUI state/commit callbacks do not identify the frame that displayed a particular
change. The shared record rejects those as completed UX measurements. Open choice:
verify with local capture while a journey is pending (using an already-granted
permission), or keep those presentation latencies unmeasured until such a source
is enabled. No capture was added or permission requested by the app.

Remaining independent work: UIKit event capture and complete server-side work
attribution. Keep the harness and journal together in PR #1950.
