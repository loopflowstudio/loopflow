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
- **Mix in some of vision D** — Jack Heart, 2026-10-28. What vision D kept (explore,
  `e03537f7ca`) was the app installing and updating itself and showing what's out on
  its stream; Sparkle (slice 7) provides that.
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
  controls. Its delegate vetoes any update whose catalog fails `can_open`.
- **How `lf install` updates Loopflow.app** — Jack Heart, 2026-10-29: "b i guess".
  When the app is running, `lf install` asks it to update (Sparkle). When it isn't
  (no GUI session, e.g. over SSH), `lf install` replaces the bundle itself and
  promotes the store. Replaces the earlier "Sparkle is the only updater" line and
  the "1 c?" lean.
- **Release channels move here from LOO-654** — Jack Heart, 2026-10-28. LOO-654's
  brief was edited to point here.
- **One lf, linked; `lf install` routes through the app when it's there** —
  Jack Heart, 2026-10-28: "yeah can we just have this detect if you have the app
  installed and then re-route through that path."
  - `~/.local/bin/lf` is always a link (Jack: "sure", 2026-10-28). With the app it points into
    `Loopflow.app/Contents/MacOS/lf`. On Linux or a Mac without the app it
    points into `~/.local/share/loopflow/versions/<v>/lf`.
  - `lf install` on a Mac with a running Loopflow.app asks it to update through
    Sparkle on the recorded stream; otherwise it installs the build and moves the
    link (decided "b" above).
  - The stream is recorded once in Home config. `lf install --dev|--preview|--stable`,
    the app's Updates setting and Sparkle's feed choice all read and write that one value.
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

Jack: "we can stack on the migrations branch if necessary." #1755 merged before
slice 1 landed; slice 2 onward sits on origin/main. Slices 1 and 2 landed as
their own PRs. **Slices 3–8 ship as one PR** (Jack Heart, 2026-10-29: asked
"or one PR?", then "ok"): build each slice working and tested on this branch,
commit it, and do not land; publish and land once, after slice 8.

1. **Minors from main — merged as #1763 (`efe7e9106`), 2026-10-28.** `release.rs` drops
   `MinorRelease`, the closing patch, `prepared_tree`, the minor-pair refusal
   inside `NeedsIntegration` (the variant stays: an ordinary release PR behind
   main still rebuilds on it), the prepare skip and the fix-forward ban; `collect_version_changes`
   bases a minor's notes (and its no-changes check) on the previous `.0`.
   Compress folded `is_minor_version`/`minor_notes_base` into `notes_base` and
   inlined `release_run_inner` (one caller left).
   All #1763 CI checks passed; no live minor yet (v0.16.0 will be the first).
2. **Streams as published pointers — merged as #1772 (`b3db55806`), 2026-10-29.** `release_publish
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
   Names (concept review, 2026-10-29): GitHub's "Latest" badge, `releases/latest`
   and `Loopflow-latest.dmg` stay as the external spelling of `stable`. They are
   fixed URLs that the website, `install.sh` and released `lf` read; Loopflow's own
   docs and help say "stable".
   Compress: one `parse_version` now serves `bump_version`, `notes_base` and
   `published_releases`; the promote test split into success and tampered cases.
   Second compress: the publisher's release asset set is one `RELEASE_ASSETS`;
   the `stable_advance` test helper derives versions from tags.
   All #1772 CI checks passed, including rust-lint and rust-test that the local gate deferred.
3. **`regressed_in` Task field — built 2026-10-29, unpublished.** Migration
   `task_regressed_in.sql` (additive `tasks.regressed_in`); `WorkValue::TaskRegressedIn`
   written through `planning_write::local` (no Linear receipt, like `TaskDeferred`);
   `lf task create|edit --regressed-in <v>` (`0.15.3` reads as `v0.15.3`); `Task`,
   `TaskSummary`, `lf task status` and Desktop's `TaskPlanningFactsView` (was
   `TaskDueDateView`) show it. `stable_advance` skips releases a `Regression`
   covers: ≥ `regressed_in` and, once the Task is done, published before it closed.
   `advance_stable` reads `SqliteStore::regression_tasks(repo)` and logs which Task
   keeps stable off which releases. Assumptions in `scratch/questions.md`; Linear label mapping stays Open.
   Compress: `planning_write::read` takes optional Task columns through one helper.
   `cargo test --lib ops::release:: store::sqlite::planning` + `--test release_tests stable regression`: pass.
   **Streams in `lf release status` (slice 3 concept review, 2026-10-29) — built.**
   `stable_stream` (from `published_releases`, `regressions`, `stable_advance`) returns
   `StableStream { current, next, holds }`; `StableHold` prints
   `v0.16.2 soaks until 2026-11-05` / `LOO-12 holds v0.16.0, v0.16.1` for every release
   newer than where stable is or is about to be. `advance_stable` logs those holds and
   moves to `next`, so the cron log and status read one view.
   `ReleaseStatusResult.latest_tag` became `preview_tag` (newest tag, published or
   completing) with `preview_published_at` and `stable` for `github-release` targets; the CLI prints
   `Preview: <tag> (published <date>)`, `Stable: <tag>` (with `moves to <tag> on the next
   release run` when pending) and the holds indented under it.
   Compress: preview lives only in `preview_tag` (its date looked up beside it), so the view no
   longer carries a second preview that could name a different release than the tag.
   `ReleaseRunOutcome::NoChanges.latest_tag` keeps its name: it is the notes baseline, not a stream.
   `cargo test --lib ops::release::` 48 + `--test release_tests -- stable status regression` 9 + clippy: pass after compress.
4. **Stream flags and the recorded stream — built 2026-10-29, unpublished.**
   `lf install --dev|--preview|--stable` (one Clap group; `--main` gone, every refusal
   now names `lf install --dev`) records `stream:` in `~/.lf/config.yaml`; no flag reads
   it, default stable; the launchd schedule runs plain `lf install`, so it follows.
   `published::install(Option<Stream>)` resolves preview from the GitHub releases API
   (first non-draft, non-prerelease) and stable from `releases/latest`. Every release
   install checks the release's `reader-catalog.txt` (digest-verified) with `can_open`
   before downloading; a store it can't open keeps the installed lf and exits 0.
   `install.sh --channel stable|preview` (recorded only when passed; a rerun reads it;
   `dev` refused with a pointer to `lf install --dev`).
   Compress: the stream record and portfolio share `config::update_global_config_at`
   (one locked read-modify-write of `~/.lf/config.yaml`), so neither drops the other's change.
   `--lib install::published::` 8, `install_exposes_refresh` 1, `store::migrations::` 121,
   `frontier_tests` 10, `documented_commands` 4, `test_shell_installer.py` 16, clippy: pass.
5. **One lf, linked — built 2026-10-29, unpublished.** Promotion links
   `~/.local/bin/lf` straight to `<app>/Contents/MacOS/lf` when the install includes
   the app (`public_cli_target`), else to the selected `~/.lf/bin/lf-<digest>`; the
   content-addressed store is the versions directory. The entry gate
   (`~/.lf-machine/install/gates/1/lf`) is still refreshed: cron and Graph steps pin
   it. An install whose link still reaches the gate counts as not current, so the
   next `lf install` relinks it. `lf install` on a Mac whose running app has
   `SUPublicEDKey` opens `loopflow://update` and stops; slice 7 added that route and
   the key. A running app without it is quit, replaced and reopened as before.
   `--lib lf::commands::install::` 27, clippy: pass.
6. **Self-update — built 2026-10-29, unpublished.** `install.sh` and `lf install --dev`
   end with `lf install schedule`; with no frequency it keeps the installed
   schedule's cadence, else daily. Linux gets a marked crontab line (`cron_table`
   replaces only its own). A HOME that isn't the account's (release smoke tests)
   schedules nothing, since launchd and cron are per account. `lf install adopt`
   (hidden) is the Sparkle entry point: run by the installed app's own `lf`, it
   promotes with the app as both source and target, so `prepare_artifacts` stages no
   bundle and nothing is quit; store migration, receipt and link go through the
   ordinary switch. Slice 7 calls it at first launch.
   `--lib lf::commands::install::` 28, `install_exposes_refresh` 1, `documented_commands` 4,
   `test_shell_installer.py` 16, clippy: pass. `adopt` itself needs the real account home
   (like every promote test); left to demo / `scripts/test_task_installation.py`.
7. **Sparkle — built 2026-10-29, unpublished.** Sparkle 2.10.1 (SwiftPM and
   project.yml); `AppUpdater` starts only in the installed bundle
   (`/Applications/Loopflow.app` or `LF_APPLICATIONS_DIR`), feeds from
   `appcast-<stream>.xml` per `~/.lf/config.yaml` (dev has none: no updater runs), vetoes a release
   whose `lf install can-open v<version>` fails, and handles `loopflow://update`. First
   launch of a new `CFBundleVersion` runs `lf install adopt`. `Info.plist` carries
   `SUPublicEDKey` (automatic daily checks and installs); the private half is Doppler
   `SPARKLE_ED_PRIVATE_KEY` (generated 2026-10-29, never printed), now a required publisher
   secret. The publisher signs `appcast-preview.xml` at publish and `appcast-stable.xml`
   at promote; `release-loopflow.py` embeds `Sparkle.framework` with an
   `@executable_path/../Frameworks` rpath, and its packaged-app launch check would catch a
   missing framework before publishing.
   `swift build --product LoopflowMac`, `swift test --filter AppUpdatesTests` 3,
   `test_release_publisher.py`+`test_release_automation.py` 55, `test_release_loopflow.py` 14,
   `--lib lf::commands::install::` 29, clippy, `third_party.py refresh`: pass.
   **Remaining:** Developer ID signing of dev builds in CI (dev stays CLI-only), and a
   live check that a notarized release launches and updates (demo).
   Compress (slices 4–7): `published.rs` fetches release assets through `release_asset`/`release_manifest`
   (no per-caller `SHA256SUMS` fetch), resolves a stream's tag in `stream_release`, and launchd and cron
   share `installed_frequency`. `--lib lf::commands::install::` 28 + clippy: pass.
8. **Learn from the best — built 2026-10-29.** `release/README.md` "What we took from
   other projects": adopt/decline table, evidence pointer to `e03537f7ca`.

**Concept review after slices 4–8 (2026-10-29) — both fixes built 2026-10-29.**
- **dev no longer flip-flops `lf`.** `AppUpdates.feedURL("dev")` is nil, so `AppUpdater` starts
  no updater while dev is recorded, and a switch to dev while the app runs vetoes further checks
  (`updater(_:mayPerform:)`). The dev CLI stays linked until CI signs dev app builds.
- **`lf install` finishes a Sparkle install the app hasn't adopted.** When the app's bundled `lf`
  previews `promote_and_migrate` (ahead of the store), `lf install` on preview/stable runs that
  `lf install adopt` before the stream lookup. The store refusal and `lf doctor` now say
  "run `lf install` to finish installing this build" instead of `lf install --dev`.
  `swift test --filter AppUpdatesTests` 4: pass. `--lib lf::commands::install::` (incl. new
  `an_app_ahead_of_the_store_is_adopted_in_place`), the updated frontier refusal test and clippy:
  not run — the lib test build was killed at the 1 h limit still compiling; owner: gate.

Check (slices 3–8, 2026-10-29): focused suites per slice above pass; full gate (nextest, clippy, Swift, release pytest) not yet run on the combined tree — owner: gate before the one PR.

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
- **Order**: LOO-656 (PR #1755) landed first. It deleted
  `canonicalize_migrations.py`, the prepare hook, BREAKING.md generation and
  release-run's inspect/repair loop (about 663 lines of `ops/release.rs`). LOO-669 redid
  only the minor-pairing removal on top (slice 1). An ordinary release must follow LOO-656's merge right away,
  so the bundled Desktop reader admits slug rows.

## Open

- **v0.16.0 timing settled as "after":** #1772 merged before any release, so v0.16.0
  reaches stable once it is 7 days old; dev and preview get it at once.

- **Contract via follow-up Tasks** — Jack Heart, 2026-10-28: "i think we can
  probably just do this with follow-up tasks (we can update loopflow to do tasks
  that automatically start at specific times or when other tasks land)". An expand
  PR files the contract (drop) as its own Task, set to start N days after the
  expand lands. That needs Tasks with a start time or a "starts when X lands"
  trigger, which `lf task` doesn't have today (no such option on `create`;
  `interrupt` mentions "scheduled automation"). Filed as LOO-671 (Jack Heart,
  2026-10-28). Open: N (> stable lag, so 30 days still works).

- Sparkle's key and per-stream appcasts exist (slice 7). Still missing: Developer ID
  signing of dev builds in CI (dev stays CLI-only until then), and a live check that a
  notarized release launches and updates itself (demo).
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
