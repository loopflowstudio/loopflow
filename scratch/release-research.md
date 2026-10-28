# LOO-669 release research

Research step, 2026-10-28. Input for `/explore`. External findings come from
three independent passes (CLI tools, desktop apps, loopflow's own flow); URLs are
the evidence, and anything marked *unverified* had no primary source. Inspiration
rule: ideas only, no code lifted.

## Loopflow today

**Entry.** `lf release run patch|minor` (`lf/mod.rs:2461`) → `ops/release.rs`.
The daily graph `.lf/graphs/release-run.yaml` runs `release run patch`. Hooks in
`.lf/config.yaml:7-24`: verify `check_migrations.py`; prepare
`canonicalize_migrations.py {version} --release-cut` + `third_party.py refresh`;
publisher `scripts/publish_release.py` (build/sign/notarize DMG via
`release-loopflow.py`, GitHub draft → `cargo publish` → R2 DMG → website →
`Loopflow-latest.dmg` → `lf release publish --finalize`).

**Minor pairing (committed HEAD).** `MinorRelease{version, patch_version,
notes_base, patch_commit, prepared_tree, completed}` (`release.rs:307`), receipt
`.lf/releases/minor-<target>.json`. `release_minor` (:766) runs a closing patch,
then cuts the minor's worktree from `patch_commit`, skips prepare hooks (:1902),
records `git write-tree`, refuses `NeedsIntegration` (:2088), disables fix-forward
(:1148), requires merged tree == prepared tree (:1235), and refuses any
`preparation_required` (:1258).

**Why v0.15.0 broke.** #1751 merged after v0.14.20 and added a draft migration
(`drafts/wave_setup.sql`). Both the tree-equality guard and the preparation guard
refuse. 635d99fee was hand-prepared (version files, notes, third_party inventory,
and a `store/migrations.rs:317` fix for `apply_newer_release` assuming head
minor+1). **No v0.15.0 tag exists and `wave_setup.sql` is still a draft**, so
publishing that commit as-is would hit `preparation_required` too.

**Reader floor.** `canonicalize_migrations.py` turns drafts into
`M.m.p.ord_release.sql` batches. Additive-only → floor holds; otherwise the floor
rises to the release being cut and `release/unreleased/BREAKING.md` gets
"lf < X can no longer open the store", which flows into notes and PR title.
Runtime refuses "this store needs lf X or newer". Only a release cut turns drafts
into released SQL.

**Installs.** `lf install` → `install/published.rs`: GitHub `releases/latest` or
`--version`, verify `SHA256SUMS`, run `release/install.sh` (CLI tarball +
`/Applications/Loopflow.app` from DMG). Weekly launchd schedule optional. No
channel concept. Desktop has no Sparkle or self-update; a running app isn't
restarted. Version source of truth is `Cargo.toml`; the app is stamped from
`RELEASE_TAG`. Homebrew appears only in unused cargo-dist metadata.

**Uncommitted diff in `release.rs` (from the interrupted implement step).**
−307 / +25. Deletes `MinorRelease`, receipt, `release_minor`,
`release_run_single`, `closing_patch_successor`, prepare-skip, write-tree check,
minor NeedsIntegration refusal, fix-forward ban. Minors now cut from main, run
prepare, and get notes based on the previous `.0`. Not yet touched: minor tests
(`release_tests.rs:1321-1456`), docs that call a minor "the same product
snapshot" (`release-run.md:18-21,55-57`, `release/README.md:221-235`,
`release-notes.md:59-61`, `docs/ops.md:123,131`), and the stale
`minor-default.json` receipt in the main checkout.

## What the field does

| | Streams | Built how | Channel select | Data across versions |
|---|---|---|---|---|
| **Codex CLI** | stable, alpha | each alpha = main fork + one version commit; stable = one more version commit on a chosen alpha (notes in the commit/tag message), while main's alphas move on; `alpha.N.M` = fixes cherry-picked onto that alpha | npm `latest`/`alpha`; pointer never moves backwards | versioned SQLite filenames; older binary *can* open newer db (`ignore_missing`) |
| **opencode** | dev, beta, latest | dev = every push to default branch; stable = manual bump on dev; old major on `v1` branch | npm dist-tags; desktop electron-updater feeds | 1.2 kept original files "so downgrading should work" |
| **Docker Engine** | stable, test (rc) | cut from master; release branch only when master can't carry a patch | `get.docker.com --channel` | — |
| **Docker Desktop** | one | ~weekly + fast patches; gradual rollout ~1 week | beta = feature toggles in stable | one-way migrations; no in-place downgrade |
| **Claude Code** | latest, stable | stable ≈ week-old latest that skips bad builds | setting `autoUpdatesChannel`; brew cask name; apt path; install arg | side-by-side versions behind a symlink; `minimumVersion` so stable switch never downgrades |
| **Rust** | nightly, beta, stable | 6-week train, beta branches off main | `rustup` | — |
| **Ghostty** | tip, stable | tip = moving tag on the latest green main commit, signed+notarized; stable tags on main; 1.2.x patches were an off-main backport branch, but v1.3.1 sits on main's history (verified 2026-10-28) | config `auto-update-channel` picks one of **two separate appcasts** | one bundle, presumably shared config (*unverified*) |
| **Rex (Superlogical)** | snapshot, preview, stable | snapshot frequent; preview/stable tagged | repo file; snapshot sorts below releases | **"refuses newer-format data rather than overwriting it"** — a reader floor; client/server skew refused with "which side to update" |
| **Zed** | nightly, preview, stable | `vX.Y.x` branch per minor; preview a week, then stable; cherry-pick by rule | separate apps | **per-channel SQLite** (`0-stable`, `0-preview`) + newer shared `0-global`; append-only migrations, old binary skips unknown ones |
| **VS Code** | insiders, stable (weekly since 2026-03) | release branches | separate install, isolated data | isolated |
| **Tailscale** | unstable (odd minor), stable (even) | `release-branch/1.N` | `tailscale update --track`, `--version` downgrade | — |

Sources: [Codex rust-release.yml](https://github.com/openai/codex/blob/main/.github/workflows/rust-release.yml),
[Codex npm_alpha_tag.py](https://github.com/openai/codex/blob/main/.github/scripts/npm_alpha_tag.py),
[Codex state migrations](https://github.com/openai/codex/blob/main/codex-rs/state/src/migrations.rs),
[opencode plan-release.sh](https://github.com/anomalyco/opencode/blob/dev/.github/release/plan-release.sh),
[opencode v1.2.0](https://github.com/anomalyco/opencode/releases/tag/v1.2.0),
[moby BRANCHES-AND-TAGS](https://github.com/moby/moby/blob/master/project/BRANCHES-AND-TAGS.md),
[Docker Desktop notes](https://docs.docker.com/desktop/release-notes/),
[Claude Code setup](https://code.claude.com/docs/en/setup),
[Rust forge process](https://forge.rust-lang.org/release/process.html),
[Ghostty release-tip.yml](https://github.com/ghostty-org/ghostty/blob/main/.github/workflows/release-tip.yml),
[Ghostty UpdateDelegate.swift](https://github.com/ghostty-org/ghostty/blob/main/macos/Sources/Features/Update/UpdateDelegate.swift),
[Ghostty publish-tag.yml](https://github.com/ghostty-org/ghostty/blob/main/.github/workflows/publish-tag.yml),
[Rex upgrading](https://www.superlogical.com/rex/docs/linux/upgrading.md),
[Zed db.rs](https://github.com/zed-industries/zed/blob/main/crates/db/src/db.rs),
[Zed release-notes guide](https://github.com/zed-industries/zed/blob/main/docs/src/development/release-notes.md),
[Zed auto_update.rs](https://github.com/zed-industries/zed/blob/main/crates/auto_update/src/auto_update.rs),
[Tailscale versions](https://tailscale.com/kb/1168/versions),
[Sparkle publishing](https://sparkle-project.org/documentation/publishing/).

## Observations that bear on the design

1. **Nobody pairs a minor with a patch.** Every peer either cuts releases from
   main (Docker, opencode, Codex alphas, Ghostty tip) or promotes an
   already-built artifact (Claude Code stable, Rust beta→stable, Zed
   preview→stable). Loopflow's pairing combined the costs of both: a
   snapshot-equality constraint with no soak benefit.
2. **"Promote, don't rebuild" is the dominant stable pattern.** Stable =
   a pointer moved to a build that already ran on the faster stream. Bad builds
   get skipped, not yanked; version gaps are normal.
3. **Pointers only move forward.** Codex refuses to lower an npm tag; Claude
   Code's `minimumVersion` makes a latest→stable switch a no-op instead of a
   downgrade; Zed's updater requires a greater version.
4. **The reader floor is the loopflow-specific crux.** One store, shared by CLI,
   Desktop and (with streams) every stream on the machine. Peers split three
   ways:
   - *Older can read newer* (Codex, Zed): additive-only, incompatible changes
     get a new file. No floor.
   - *Older refuses newer* (Rex, loopflow): safe, but a dev/beta build that
     raises the floor strands stable on that machine. Rex says which side to
     update.
   - *Isolate per stream* (Zed per-channel db, VS Code Insiders): no stranding,
     but Tasks would split across streams — likely wrong for loopflow, where
     the store is the work model.
5. **Desktop update.** Ghostty: two separate Sparkle appcasts chosen by a
   setting (Sparkle's own channels can't leave the default channel), staged
   appcast promoted only after every asset returns 200, `CFBundleVersion` =
   monotonic commit count. Loopflow currently has neither Sparkle nor in-app
   checking; `lf install` is the single updater for both CLI and app, which
   already keeps them in lockstep.
6. **Release notes accrue as work lands.** Zed: `Release Notes:` line per PR
   (or `N/A`). VS Code: one notes file per minor that grows from Insiders to
   Stable. Loopflow already has `release/unreleased/` — the closest analogue.
7. **Hotfixes.** Small teams fix forward on main (opencode, Codex alphas);
   train projects cherry-pick to release branches (Zed, Ghostty, Tailscale,
   Docker only when necessary). Docker's rule — branch only when main can't
   carry the patch — is the lightest that still allows a stable hotfix.
8. **Build/sign separation.** opencode: build jobs have no secrets; sign/publish
   jobs run only release scripts against attested artifacts. Loopflow's
   publisher already signs locally; worth noting, not a driver for this Task.

## Open questions for /explore

1. Which stream shape: (a) two pointers over one build line (`latest`, `stable`
   = soaked latest), (b) tip + tagged stable (Ghostty), (c) three-stream train
   with release branches (Zed/Rust)?
2. Who may raise the reader floor: (a) any release, (b) only a stable minor,
   with faster streams limited to additive migrations, (c) per-stream stores?
3. Where does "dev" come from: every green main commit (Ghostty tip,
   opencode dev) or only scheduled cuts?
4. Desktop: keep `lf install` as the single updater with a stream setting, or
   add Sparkle with one appcast per stream?
5. Does a minor still mean something once it cuts from main — "floor may rise
   here", "stable promotion", or just a bigger number?
6. Is the uncommitted `release.rs` diff the right first slice to keep, or
   should it wait for the stream design?

## Second pass (2026-10-28): Firefox, 1Password, Warp, Atuin, Deno, Bun

- **Firefox.** One profile per install since 67 (2019), after a storage change in
  Firefox 63 broke Firefox 61. `compatibility.ini` stamps `LastVersion` (version +
  build ID). An older build refuses the profile ("Create New Profile");
  `--allow-downgrade` overrides. Per-store downgrade classes: *hard* (IndexedDB,
  Cache refuse) vs *soft* (Places down-revs `user_version` and re-migrates
  idempotently). Balrog *watershed* updates force stops at intermediate versions;
  the channel lives in `channel-prefs.js`, which updates never touch.
  [SUMO](https://support.mozilla.org/kb/dedicated-profiles-firefox-installation),
  [QuotaManager schema changes](https://wiki.mozilla.org/DOM/QuotaManager/SchemaChanges),
  [update infra](https://firefox-source-docs.mozilla.org/update-infrastructure/index.html)
- **1Password.** Production/Beta/Nightly in Settings. A switch never installs an older
  version; you wait for the target stream to pass you (forum, staff). Nightly's
  db format isn't readable by beta/prod: "You've used a newer version of
  1Password on this computer". The fix is to delete the local db and resync, which
  only works because the db is a server cache. The CLI requires the latest app;
  users asked for a protocol-version check instead.
  [thread](https://www.1password.community/1password-at-home-31/switch-between-channels-doesnt-work-nightly-beta-or-stable-18501)
- **Warp.** dev nightly, preview and stable weekly; date versions. Separate
  bundle ID and db per channel; the TUI gets its own state dir because a newer
  binary "would otherwise migrate the shared database out from under the older
  one". The `channel_versions.json` manifest per channel has `version`,
  `soft_cutoff`, `update_by`, `is_rollback`, and per-binary `cli_version`/`tui_version`.
  [paths.rs](https://github.com/warpdotdev/Warp/blob/master/crates/warp_core/src/paths.rs),
  [channel_versions](https://github.com/warpdotdev/Warp/blob/master/crates/channel_versions/src/lib.rs)
- **Atuin.** A daily beta from cron (Tue–Fri), with a desktop `edge` build on every
  main push (Tauri updater). Strict sqlx refuses downgrades. The common cause is
  **two installs on PATH** (brew + `~/.atuin/bin`), and the maintainers' answer is
  "check `which atuin`". [#3524](https://github.com/atuinsh/atuin/issues/3524),
  [#3904](https://github.com/atuinsh/atuin/issues/3904)
- **Deno.** A canary on every main push; a per-target `canary-<triple>-latest.txt`
  pointer is written only after that platform's binary exists. An RC/stable
  promotion re-stamps the *same canary bytes* (`patchver`) and re-signs them, with no
  rebuild. The channel is baked into the binary; `deno upgrade canary|rc|<sha>`.
  [promote_to_release.ts](https://github.com/denoland/deno/blob/main/.github/workflows/promote_to_release.ts)
- **Bun.** A canary from main after CI passes; `bun upgrade` stays on its channel,
  and `--stable` prints "Downgrading from Bun X-canary to vY".

### What the second pass adds to the design
1. Forward-only switching is common practice: 1Password waits, and Bun says
   "downgrading" when it doesn't wait. With LOO-656's membership floors, lf can
   do better: switch when the target build knows the store's breaking set.
2. Firefox's writer stamp gives lf the refusal message: record the last writer's
   version in the store, then refuse with "written by 0.15.3; needs ≥ 0.15.1".
3. Warp's manifest shape (`version`, `soft_cutoff`, `update_by`,
   `cli_version`) fits a per-stream pointer that also names the Desktop version
   and the migration catalog.
4. Deno: promotion re-stamps and re-signs identical bytes, and per-target
   pointers never resolve to a half-uploaded release.
5. Atuin: the most likely skew bug is a duplicate install. `lf doctor` and the
   refusal should list every `lf` on PATH and every Loopflow.app with its version.
6. Warp, Zed, VS Code and Firefox all *separate* data per channel; only Rex,
   1Password (cache) and Atuin share. lf's shared store makes Desktop's rule
   explicit: it must bundle the lf it migrates with, or never migrate.

## Codex destructive migrations (checked 2026-10-28)

There's no staged wait. Each drop lands in the same PR that removes its code:
- `0042_drop_agent_jobs.sql` in "Remove CSV-backed agent jobs" (#34413, 2026-07-20)
- `0035_drop_memory_tables.sql` in "Move memory state to a dedicated SQLite DB"
  (#24591, 2026-05-26)

Renames happen in place too: `0055_thread_attachments.sql` renames a table and a
column (#43927, 2026-09-08). With `ignore_missing`, an older binary still opens that
db, then fails on the queries that touch the renamed table. So Codex avoids
refusing at open time and accepts feature-level breakage in older binaries.
Across 62 state migrations there are 8 drops or renames.
