# Download, drag into Applications, open

## Feedback and agreed change

Jack said: "I think drag to app -> open is nice, lets do it".

The primary Mac path keeps the direct DMG download. A newcomer drags the app
into Applications and opens it; first launch handles activation and leads into
coding-tool and GitHub setup. No initial external Terminal command. This
supersedes kickoff's proposed switch to shell-install instructions.

## Design changes

[Current design](a-newcomer-gets-from-download.md) now includes first-launch
activation, the running-app restart/retry case, bundled helper registration,
actual app destination retention, and shared PATH setup. The existing Rust
installer remains the authority; Swift presents progress and recovery. The
first slice is copied-app activation, replacing shell-only PATH work as the
primary cut. Acceptance must exercise download/drag/open on a fresh account.

## Evidence and limits

- [App entry](../swift/LoopflowMac/LoopflowApp.swift) invokes
  [bootstrapLoopflowApp](../swift/Loopflow/AppBootstrap.swift), which currently
  registers fonts, then enriches PATH and constructs the primary window.
- [Bundled helper selection](../swift/LoopflowMac/MacLocalWaveAgentLauncher.swift)
  uses the bundled helper for installed apps, with no PATH fallback.
- [Installer](../rust/loopflow/src/lf/commands/install.rs) already owns preflight,
  staging, promotion receipts, and app quiesce/resume. Its quiesce path derives
  executable paths from the prior selection; the no-prior-installation case
  needs investigation before claiming it handles a running first-launch app.
- [Shell entry](../release/install.sh) currently verifies downloaded artifacts
  and passes CLI, daemon and app source/target paths to promotion.

These are source observations. No copied-app launch, promotion, login or clean
account trial was performed in this review.

## Remaining work and next useful action

Trace the unregistered helper's machine entry gate and no-prior-receipt
promotion case, record the live baseline, then implement the minimum entry
through the existing owner. Prove first open, interruption/retry, repeat open,
updates at the copied location, and safe handling of an older copied bundle.

Jack chose to avoid an init conversation; see [init review](init-setup-review.md).
The `gh` acquisition mechanism remains engineering work constrained by
[account requirements](account-requirements-review.md). No Flow navigation
decision is recorded.
