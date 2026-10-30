# LOO-669 open questions

- **Website follows preview, download follows stable (slice 2, 2026-10-29).** The
  publisher still deploys the website for every release, so its docs and
  `/healthz` describe `preview`, while its install command and "Download for
  Mac" serve `stable`. Assumed acceptable for now (docs describe a superset);
  revisit if a stable-only flag confuses readers.
- **First stable move.** Latest today is the newest release (v0.15.x). Forward-only
  means stable stays there until a newer release is 7 days old; no backfill.
- **Stable advance gates on `completion: github-release`**, not on having a
  publisher, so publisher-only targets in other repos keep no Latest marker
  after `--latest=false`. Loopflow's target sets `github-release`.
- **A done regression covers releases published before it closed (slice 3, 2026-10-29).**
  The fix release is the first release published after the regression Task
  completed; releases from `regressed_in` up to it stay ineligible. A release cut
  before the fix merged but published after the Task closed would count as fixed;
  assumed rare (a release run takes minutes). A done Task with an unreadable
  completion time keeps covering, so the failure is a stalled `stable` that the
  release log names (`LOO-1 keeps stable off v0.16.0, v0.16.1`), never a shipped bug.
  Abandoned or canceled regression Tasks cover nothing.
- **`regressed_in` reaches the release host only through local planning exchange.**
  It has no Linear field, so the cron Machine sees a mark set elsewhere only
  through peer planning (git). A released lf reading a peer planning document
  that carries `task_regressed_in` fails to parse it. Until the Linear label
  mapping (Open, design) lands, mark regressions on the Machine that runs the
  release, or after the release host's lf includes this field.
- **`--regressed-in` sets or replaces the mark; nothing clears it.** Complete or
  abandon the Task instead. Add `--not-regressed` if a mistaken mark needs undoing.
- **`lf release status` shows preview and stable, not dev (concept review, 2026-10-29).**
  dev is the newest green main build, and `lf install` (slice 4) owns it. Status
  stays about releases. Add a `Dev:` line if the release view feels incomplete without it.
- **Slice 4 checks `can_open` on every release install, not only on slower switches.** Cheaper to
  reason about and covers the scheduled run after a switch. The stream is recorded before the
  download, so a failed download retries on the new stream. Releases without `reader-catalog.txt`
  (v0.15.0 and earlier) skip the check; promotion still refuses an incompatible build.
- **An explicit switch may downgrade** (preview v0.16.2 → stable v0.16.0) when the store allows
  it; the schedule never moves stable backwards. Revisit if "switches only move forward" should
  mean "stay put until the slower stream passes the installed version".
- **`install.sh latest` (positional) is no longer special**; channels replace it.
- **Slice 5 keeps `~/.lf/bin/lf-<digest>` as the versions directory**, not
  `~/.local/share/loopflow/versions/<v>/`: rollback, pruning and receipts already own
  it, and a dev build can share a version with different bytes. Rename later if the path matters.
- **`LF_INSTALL_CLI_ONLY=1` still links the CLI alone** on a Mac with the app. (`lf install
  --dev` itself installs the signed app since LOO-688.)
- **`install.sh` run by hand with the app running** still quits, replaces and reopens it;
  only `lf install` hands off to the app's updater.
- **The publisher host must inject `SPARKLE_ED_PRIVATE_KEY`.** It is now a required secret;
  if `loopflow-release-publisher` passes a fixed list of names, add it there or the release
  check fails closed naming it.
- **Sparkle's `--deep` signing** re-signs its XPC services without their entitlements; the
  app isn't sandboxed, so the installer doesn't use them. Sign Sparkle's helpers separately if
  notarization or install complains.
- **Sparkle's veto runs `lf install can-open` on the main thread** (≤ 30 s network timeout),
  only when an update is found.
- **On dev, Desktop stays put rather than following preview (concept review after slice 8).**
  The alternative keeps Sparkle on preview and has `adopt` leave a dev link alone; that keeps
  Desktop fresher but runs two streams on one Machine. Chose the simpler one-stream reading.
- **Store-ahead refusals say `lf install`, not `--dev`** (concept review after realign; built in
  `af588be646`). This follows from accepted decisions. Only
  `install.rs`'s unmerged-candidate refusal keeps `--dev`, because there the candidate is a main build.

# LOO-688 open questions

- **dev keeps "no app updater" (2026-10-30).** `lf install --dev` promotes the run's app with
  `--app-source/--app-target`, which quits a running app, replaces it and reopens it. That is
  the only path on dev, GUI session or SSH; there is no Sparkle handoff because dev has no
  appcast. A dev appcast would need public, non-expiring DMG hosting (R2) per green main;
  revisit if the quit/reopen interrupts work often.
- **Dev builds carry the last release's version number** (Cargo.toml between releases), as the
  dev CLI already does. So switching dev → preview at the same number reads as current in
  `is_current`; the dev app's `LoopflowSourceRevision` tells them apart. Stamp a distinct
  version if that bites.
- **The dev app is ARM64 only**, like the release DMG (`swift build` on Apple Silicon).
  `lf install --dev` installs it on every Mac, matching `install.sh`.
- **Signing credentials are copied into GitHub Actions secrets** (5 names, set 2026-10-30 via
  pipe from Doppler `loopflow`/`prd`). `DOPPLER_TOKEN_PRD` already exists in Actions but no
  workflow uses it; using it instead would avoid the copies (rotation in two places) at the cost
  of exposing all prd secrets to the job. Followed the Task's direction.

- Sync onto main (2026-10-30): main shipped this branch's decisions in `release/v0.15.1/DECISIONS.md`, whose last implication says `lf install --dev` links the CLI alone "until dev packages include a signed app". This PR delivers that signed app, but `release/unreleased/DECISIONS.md` no longer exists and the shipped record stays unedited. Resolved: `release/unreleased/DECISIONS.md` gains a 2026-10-30 entry for the signed dev app.
