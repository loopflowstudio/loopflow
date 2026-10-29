# LOO-669 release streams — design

Evidence: `scratch/release-research.md`, `scratch/explore.html` (cleared; in git at `e03537f7ca`).

## Decided

- **Minors cut from main like any release** (task brief). Built as slice 1.
- **Streams follow vision A, "two pointers"** (plus `dev` from green main, so three streams) — Jack Heart, 2026-10-28.
  One build line from main. Every release publishes to `preview`; `stable` is a
  pointer the release cron moves forward to the newest release ≥ 7 days old with
  no regression against it. No rebuilds. Pointers and stream switches only move
  forward. Any release may raise the reader floor; stable meets it a week later.
- **Regressions are tracked Rust-style** — Jack Heart, 2026-10-28: "ok do the
  rust ting." A regression is an ordinary bug Task marked with the version it
  regressed in (like Rust's `regression-from-stable-to-beta` labels). The daily
  release run reads open marked Tasks: stable moves to the newest release ≥ 7
  days old that no open regression covers, i.e. older than the regressed-in
  version or the release that shipped the fix. Landing the fix closes the Task,
  and the fix release becomes eligible after its own 7 days. No separate hold
  command or holds file.
- **The mark is an explicit Task field** — Jack Heart, 2026-10-28: "maybe add it
  as an explicit field?" `regressed_in: Option<version>` beside the other Task
  fields (`work/edit.rs` `WorkValue`), set with `lf task create --regressed-in
  v0.15.3` and shown in status and Desktop.
- **Stream names: `dev` / `preview` / `stable`** — Jack Heart, 2026-10-28:
  "dev / preview / stable sounds good." `dev` = green main, `preview` = every
  release, `stable` = soaked preview.
- **Mix in some of vision D** — Jack Heart, 2026-10-28. Scope open (below).
- **Desktop and the installed lf never diverge** — Jack Heart, 2026-10-28:
  "the thing i wanted to decline was having the Desktop and the installed lf be
  able to diverge." One artifact, one version, one updater on a Mac.
- **Installation follows Rex** — Jack Heart, 2026-10-28: "lets just copy Rex
  for our installation path." Ideas only (Rex is closed source; inspiration
  rule). Rex's model, from its docs (`superlogical.com/rex/docs`):
  - The Mac app is self-contained and is the install. Terminals it opens get
    the app's own CLI directory first on `PATH` (`REX_BIN_DIR`).
  - The app checks a signed feed at launch and about daily. The check can't be
    turned off; the download happens when the person chooses to install.
  - Linux and FreeBSD get the CLI and server from a package repo or `install.sh
    --channel`. Re-running is safe with the same channel. "Keep one copy of Rex
    on a machine."
  - Versions may differ only *between machines*. A missing feature names the
    version needed; too far apart is refused with "update the server" or
    "update the client".
  - Newer-format data is refused, never overwritten. A downgrade is never
    automatic: you ask for a version.
- **The lf installer on a Mac does something reasonable** — Jack Heart,
  2026-10-28: "make sure to handle what happens if people run the lf installer
  on mac and make it do something reasonable." Jack's shape: "write to the same
  place that the desktop app writes, and if you install desktop app, it
  overwrites with its own lf version." Settled as the link form below
  ("One lf, linked").
- **Self-update installs automatically** from the install's stream — Jack Heart,
  2026-10-28: "download and install automatically for now, from whatever your
  channel is." This departs from Rex, which asks first. `can_open` still gates
  every install.
- **The app updates through Sparkle**, with one appcast per stream (Ghostty's
  pattern) — Jack Heart, 2026-10-28: "use sparkle i guess", reconfirmed after
  weighing it against the app driving `lf install`: "seems worth running" for its
  signature checks, out-of-process replacement of a running signed app, and rollout
  controls. To keep one updater per Mac: with the app present, Sparkle is the only
  updater, and `lf install` or `install.sh` on that Mac asks the app to update rather
  than replacing the bundle itself. Sparkle's delegate vetoes any update
  whose catalog fails `can_open`.
- **Release channels move here from LOO-654** — Jack Heart, 2026-10-28. LOO-654's
  brief was edited to point here.
- **One lf, linked; `lf install` routes through the app when it's there** —
  Jack Heart, 2026-10-28: "yeah can we just have this detect if you have the app
  installed and then re-route through that path."
  - `~/.local/bin/lf` is always a link (Jack: "sure", 2026-10-28). With the app it points into
    `Loopflow.app/Contents/MacOS/lf`. On Linux or a Mac without the app it
    points into `~/.local/share/loopflow/versions/<v>/lf`.
  - `lf install` (and `install.sh` on a Mac) detects an installed Loopflow.app
    and asks it to update through Sparkle on the recorded stream. Only without the
    app does it install a version directory and move the link.
  - The stream is recorded once in Home config. `lf install --stream`, the app's
    Updates setting and Sparkle's feed choice all read and write that one value.
- **Stream selection by flag** — Jack Heart, 2026-10-28 (said in LOO-656's
  conversation, relayed to this Task): "i am imagining i can say --dev (or --latest or --main or
  whatever) OR --stable (default unless your config has a different
  default/current stream) or --preview/alpha/beta whatever." Mapping:
  - `lf install` with no flag installs the current stream from Home config,
    `stable` if none is set.
  - `--dev`: newest green main build (LOO-656's `--main` / `Packages (main)`; this
    Task renames `--main` into the stream flags).
  - `--preview`: newest release.
  - `--stable`: newest release that is at least 7 days old with no regression.
  - A flag switches the stream and records it as current; the schedule, Sparkle
    and self-update follow it. Switching to a slower stream runs `can_open` first.
- **Loopflow updates itself from a stream** — Jack Heart, 2026-10-28: "I want
  to make loopflow self-updating from a stream (default stable, use
  dev/latest/whatever on our local machine)." So: installs follow a stream
  automatically, not only on `lf install` or the weekly schedule; new
  installs default to `stable`; Jack's machines run the fastest stream.

## Build order (stacked on LOO-656's #1755, Jack Heart, 2026-10-28)

Jack: "we can stack on the migrations branch if necessary." This branch sits on
#1755's head; `lf sync` follows it through its squash merge. Each slice lands
working and tested before the next starts.

1. **Minors from main — merged as #1763 (`efe7e9106`), 2026-10-28.** `release.rs` drops
   `MinorRelease`, the closing patch, `prepared_tree`, the minor-pair refusal
   inside `NeedsIntegration` (the variant stays: an ordinary release PR behind
   main still rebuilds on it), the prepare skip and the fix-forward ban; `collect_version_changes`
   bases a minor's notes (and its no-changes check) on the previous `.0`.
   Compress folded `is_minor_version`/`minor_notes_base` into `notes_base` and
   inlined `release_run_inner` (one caller left).
   Tests: `minor_release_cuts_from_main_past_the_last_patch` (main moved past
   v0.9.1 with a migration file; prepare hook ran), `…right_after_a_patch…`,
   `interrupted_minor_retries_without_a_patch`, `…ships_fixes_merged_with_its_pr`.
   Docs: `release/README.md`, `release-run.md`, `release/unreleased/DECISIONS.md`.
   Check (after compress): `cargo test -p loopflow --test release_tests` 71 pass
   (`active_candidate_preparation…` needs >20s on a cold binary, passes warm);
   `--lib ops::release::` 44 pass.
2. **Streams as published pointers — built 2026-10-29.** `release_publish
   --finalize` passes `--latest=false`; `release_run` wraps `release_version`
   and, for `completion: github-release` targets, calls `advance_stable` after
   every outcome (NoChanges included): newest non-draft, non-prerelease release
   with `publishedAt` ≥ 7 days ago, moved only when ahead of the current Latest.
   The publisher's new `promote --tag` points `Loopflow-latest.dmg` at that DMG
   (checked against `SHA256SUMS`, read back) before `gh release edit --latest`;
   `publish` no longer uploads the latest DMG. `verify`/`reconcile` check the
   latest DMG only when the tag is stable; website repair is guarded by the
   newest published release instead of Latest. `reader-catalog.txt` (already in
   each `package-build.yml` artifact) joins the release assets and `SHA256SUMS`;
   targets must agree on it. `preview` and `dev` need no stored pointer.
   Slice 3 adds the regression filter inside `stable_advance`.
   Compress: one `parse_version` now serves `bump_version`, `notes_base` and
   `published_releases`; the promote test split into success and tampered cases.
   Check (after compress): `--lib ops::release::` 45 pass; `--test release_tests
   daily_run_moves_stable minor notes` 22 pass; `pytest test_release_publisher.py
   test_release_automation.py` 55 pass; clippy `--lib` clean.
3. **`regressed_in` Task field** and the stable rule reading it.
4. **Stream flags and the recorded stream.** `lf install --dev|--preview|--stable`
   (rename #1755's `--main`), no flag = current stream from Home config, default
   stable; switching slower runs `can_open` first. `install.sh --channel`.
5. **One lf, linked.** `~/.local/bin/lf` links into Loopflow.app or a versions
   directory; `lf install` on a Mac with the app updates the app. How it does
   that is still open (see Open): the Decided section has Sparkle as the only
   updater on a Mac with the app, while Jack's later lean ("1 c?") has `lf
   install` swap the bundle itself when the app isn't running. Settle before
   slice 5; it doesn't affect slices 2–4.
6. **Self-update.** Daily scheduled `lf install` on the recorded stream (Linux,
   headless); the in-place promotion entry point for an app updated in place.
7. **Sparkle, one appcast per stream**, EdDSA key in Doppler, Developer ID
   signing of dev builds in CI; the delegate vetoes updates that fail `can_open`.

Do not run a release while #1755 is unmerged.

## Neighbouring Tasks

- **LOO-656** (active, Jack): install lf from main — migration ids and floors
  fixed at merge, delete copy-on-draft/clone/drafts/
  authority split. Its `lf self install --main` is this design's `dev` stream.
  This Task does not redesign migrations; it states what streams need from them.
- **LOO-654** (unstarted): its release-channels item moved here (Jack, 2026-10-28).

## What streams need from migrations

Under LOO-656 a floor is catalog membership: a binary refuses a store that
applied a reader-breaking migration it doesn't know. **Contract enforcement is
decided here, not in LOO-656** — Jack Heart, 2026-10-28: "i also declined
contract enforcement as a separate PR. I just want to decide here in this
task." Open. Without it, a breaking migration raises the floor whenever it
lands and nothing ties it to stable's lag:

- **Stable can be stranded on a machine** that ran a newer build, until stable
  catches up. Breaking migrations have been frequent (6 of 13 patches,
  v0.14.8–v0.14.20).
- **Stream switches and self-update run `can_open` first** (store's applied
  set against the candidate's published catalog). When it fails, that's a normal
  refusal that names the build to install, or when stable reaches one that can
  open the store. It never says "delete loopflow.db".
- **New installs on stable are unaffected**: a fresh store only ever sees
  stable builds.
- **dev on Jack's machine** migrates `~/.lf` from main builds and is ahead of
  preview and stable by design; switching back waits for stable.

## Split with LOO-656 (agreed between the two Task sessions at Jack's request, 2026-10-28)

- **LOO-656 owns migrations**: slug ids by first-parent order, floors as catalog
  membership, no migration work in release; `lf self install --main` (fetch the
  green `Packages (main)` artifact, then promote); the "can build X open this store"
  predicate over published data (each package carries its known reader-breaking
  migration ids). Contract enforcement is not LOO-656's; it is decided in LOO-669.
- **LOO-669 owns streams and updates**: dev/preview/stable pointers; the recorded
  stream; the self-update/schedule following it; publishing each package's
  migration catalog next to `SHA256SUMS` per release and pointer; Desktop on
  every stream, including Developer ID signing of dev builds in CI (today
  `release-loopflow.py` signs from the local keychain); minors from main; notes and docs.
- **Order**: LOO-656 (PR #1755) lands first. It already deletes
  `canonicalize_migrations.py`, the prepare hook, BREAKING.md generation and
  release-run's inspect/repair loop (about 663 lines of `ops/release.rs`). LOO-669 redid
  only the minor-pairing removal on top (slice 1). An ordinary release must follow LOO-656's merge right away,
  so the bundled Desktop reader admits slug rows.

## Open

- **Migration after a Sparkle update (LOO-669 builds this, on top of #1755).**
  The only code that migrates `~/.lf` is `SqliteStore::open_as_promotion_boundary`,
  reached through `lf self install promote` (#1755, `install.rs` ~2125/2405). It
  takes the promotion lock, snapshots inside the transaction and refuses unmerged
  migrations, and it doesn't check where the binary lives. Promote is shaped as
  "switch to a staged candidate" (`validate_staged_app_helper`). Sparkle needs a
  narrower entry point: the running bundle is its own candidate, so lock →
  snapshot → migrate → record the receipt, with no file moves. The new app's first
  launch calls it before anything reads the store. Until it exists, a
  Sparkle-swapped app hits the ordinary-open refusal, which #1755 changes to name
  the build to install.

- How `lf install` asks the app to update: a URL scheme or app intent that
  starts Sparkle's check, launching the app if it isn't running. And what
  happens with no GUI session (SSH into a Mac with the app), since Sparkle needs
  one. Jack Heart leaned toward replacing the bundle directly (2026-10-28, "1
  c?"): when the app isn't running, `lf install` swaps Loopflow.app itself (it
  is already notarized; a curl download carries no quarantine) and promotes the
  store, keeping lockstep. When the app is running (a locked screen still has a
  session), it asks the app to update instead of replacing a live bundle.

- **Contract via follow-up Tasks** — Jack Heart, 2026-10-28: "i think we can
  probably just do this with follow-up tasks (we can update loopflow to do tasks
  that automatically start at specific times or when other tasks land)". An expand
  PR files the contract (drop) as its own Task, set to start N days after the
  expand lands. That needs Tasks with a start time or a "starts when X lands"
  trigger, which `lf task` doesn't have today (no such option on `create`;
  `interrupt` mentions "scheduled automation"). Filed as LOO-671 (Jack Heart,
  2026-10-28). Open: N (> stable lag, so 30 days still works).

- Linux and headless Macs (no app, so no Sparkle): what drives automatic updates there?
  Proposed: the existing launchd/cron schedule runs `lf install` on the
  recorded stream, daily instead of weekly.
- Sparkle needs an EdDSA key (Doppler), appcast hosting next to the DMGs, and
  Developer ID signing of dev builds in CI.
- Linear mapping for `regressed_in` (proposed): Linear has no custom issue
  fields (unverified this session), so sync uses a "Regressed in" label group
  with one label per version (`v0.15.3`). A group allows one label per issue, so
  it behaves like a single value; sync creates the version label on first use.
  It would be the first issue label lf sync manages. The release run reads the
  local field and never needs Linear.
- No install telemetry exists, so the soak only means something if someone runs
  `preview` during it.

## Existing behavior this builds on

- Loopflow.app already ships a bundled `lf` at `Contents/MacOS/lf` and uses
  only it (`MacLocalWaveAgentLauncher.controlLfPath`; PATH fallback disabled).
  The bundled helper cannot migrate the store (`swift/README.md:28`).
- `install.sh` installs the CLI tarball and the DMG's app together; full
  `lf install` restarts Desktop and its helpers; `LF_INSTALL_CLI_ONLY=1`
  skips Desktop (`release/README.md:30-43`).
- Website offers both: "Download for Mac" (`Loopflow-latest.dmg`) and
  "or the CLI" `curl -fsSL https://loopflow.studio/install.sh | sh`
  (`website/home.html:5`, `website/content.yaml:90-96`). README leads with curl.

LOO-656 accepted the refinements (2026-10-28): it generates a per-package catalog
of known reader-breaking ids and provides `can_open(store_applied,
candidate_catalog)`; file name and shape land in its PR.

Rule for this Task: stream-switch and self-update refusals name the build to
install or the date stable reaches it, never "delete loopflow.db" (Signal
finding, LOO-656 `scratch/migration-study.md`).

## Cross-machine versions (analysis, 2026-10-28; Jack asked for clear principles)

Rex (docs): the Mac app and remote servers "don't need to be the same version,
and either one can be newer". The server's API is the surface: the CLI and Lua
are derived from the connected server's API, and docs come from that server. A
missing method errors with the version needed, and versions too far apart are
refused with "update the server" or "update the client". `--json` is "stable to
parse". Its terminal Program Status Protocol has explicit rules: "Unknown keys
MUST be ignored. This is how the protocol is extended." Feature-detection
lists are fixed, and an unknown state is ignored rather than shown wrongly.
Rex publishes no semver or deprecation guarantee (beta).

Loopflow today: `lf machine` status probes the remote's version and, on *any*
difference, prints "lf versions differ … Update the remote with …" or "Update
this one with `lf install`" (`lf/commands/machine.rs:481`). There's no notion
of compatible. What crosses machines: commands run over the Machine route, and
the `lf session send` / `receive` packet.

Candidate principles (not decided):
1. One machine, one lf (decided above).
2. A store never crosses machines, so the reader floor is per-machine and is
   never a cross-machine contract.
3. The wire is the contract: cross-machine messages and `--json` gain fields
   additively, and readers ignore unknown fields.
4. One integer, a wire/capability version (Tailscale's `CapabilityVersion`),
   is bumped only when a cross-machine message changes incompatibly. `lf machine`
   compares that, not release numbers. Same → works. Different but within a
   supported range → works, and a missing feature names the version it needs.
   Outside the range → refused, naming which side to update.
5. Remote surface comes from the remote: an agent on a remote Machine reads
   that Machine's `lf help`, as Rex derives its CLI from the server.
