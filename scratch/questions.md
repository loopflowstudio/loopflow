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
- **Proposed Wave memory edit (not made; `.lf/wave/` read-only here):** infrastructure MEMORY.md
  line ~663 says `lf self install --main`; it is now `lf install --dev`.
- **Slice 5 keeps `~/.lf/bin/lf-<digest>` as the versions directory**, not
  `~/.local/share/loopflow/versions/<v>/`: rollback, pruning and receipts already own
  it, and a dev build can share a version with different bytes. Rename later if the path matters.
- **`lf install --dev` and `LF_INSTALL_CLI_ONLY=1` still link the CLI alone** on a Mac with
  the app, so the terminal can run a newer build than Desktop until CI signs dev app builds
  (slice 7 left dev CLI-only).
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
