# Desktop "database mismatch" — 2026-11-02

## Symptom

Jack's Loopflow Desktop reported a database mismatch, while installed `lf` 0.15.1 and
`/Applications/Loopflow.app/Contents/MacOS/lf` opened `~/.lf/loopflow.db` cleanly
(`lf doctor`: "selected database is compatible with this build").

## Cause

The running Desktop was not the installed app. `ps` showed pid 66691, launched
2026-11-02 10:25 PST by launchd:

    /Users/jack/benchmarks/installed-sync-hotfix-20261027/stage/Loopflow.app/Contents/MacOS/Loopflow

That is a benchmark staging copy: 0.14.19, bundled `lf 0.14.19+1e0aa05d1.dirty`, same bundle
identifier `com.loopflow.mac`. Its bundled lf can't read a 0.15.x store; `~/.lf/logs/lf.log`
from 18:25Z onward shows each of its reads failing with
`invalid data: incompatible Loopflow database; delete loopflow.db and rerun the command
… lf=0.14.19+1e0aa05d1.dirty`. That's the message Desktop displayed.

The ship_stop / task_regressed_in draft rows were a red herring: 0.15.1 knows both and
their checksums match.

The wrong copy launched because `lf open` runs `open -b com.loopflow.mac`
(`rust/loopflow/src/lf/commands/open.rs`, `desktop_bundle`), and LaunchServices resolves a
bundle identifier to any registered copy. Before the repair,
`path to application id "com.loopflow.mac"` resolved to the benchmark copy, while
`NSWorkspace.urlForApplication` returned `/Applications/Loopflow.app`. LaunchServices
also held about 770 registrations for Loopflow bundles that no longer exist: `/Applications/.Loopflow.app.promote.*`,
`.superseded.*`, `/private/tmp/**/Loopflow*.app`, and DerivedData builds.

## Repair (no store changes)

1. Quit the stale Desktop (pid 66691) with an AppleScript quit. Session servers are
   separate processes and stayed up.
2. `lsregister -u` on the benchmark stage app. The files are unchanged.
3. `lsregister -u` on every registered Loopflow app path that no longer exists.
4. `open -b com.loopflow.mac` now launches `/Applications/Loopflow.app` 0.15.1 (pid 99798).

`loopflow.db` and `schema_migrations` were not touched.

## Verification

- `path to application id "com.loopflow.mac"` → `/Applications/Loopflow.app/`.
- Running Desktop: `/Applications/Loopflow.app/Contents/MacOS/Loopflow`, serving
  `~/.lf/run/desktop/com.loopflow.mac.sock`. `lf desktop state` answers
  "Desktop reading" with the manabot repository window.
- lf.log since 18:37Z shows only `lf=0.15.1` entries and no new "incompatible" errors.

## Open

- Spotlight or Finder can register the benchmark copy again and bring the problem back.
  Deleting `~/benchmarks/installed-sync-hotfix-20261027/stage/Loopflow.app`, or moving
  it under a `.noindex` directory, would prevent that. I left it in place because it is benchmark evidence.
- Other live existing copies are still registered with the same bundle identifier: worktree
  DerivedData, `swift/dist`, and `local-bin` builds, plus job tmp apps under `~/.lf/accounts/…`.
  `open -b` can pick any of them. A durable fix would launch Desktop by the path of the
  installed app instead of by bundle identifier. That needs its own Task; no code changed here.
- `lf doctor` still warns that 28 processes run Loopflow-0.15.0.app lf and 3 run 0.14.x lf,
  and none of those can open the store. `lf monitor rollover` moves the resumable ones.

## Five whys (2026-11-02)

Recovery status is unchanged: the repair above holds, the store was never damaged, and no
code changed in this analysis.

### Did #1755 (LOO-656) claim to prevent this?

Only for readers built after it. The PR says refusals "name the migration … install it with
`lf self install --main`", and its unrun "Try it" step expects "an older installed lf" to do
so. Only builds that contain #1755 can produce that message. The stale Desktop's lf was
`0.14.19+1e0aa05d1.dirty`, built 2026-10-27 from the unmerged LOO-664 branch, two days before
#1755 merged (2026-10-29 02:30Z). No change to new code can alter what an old binary prints.

The incident is also bigger than the benchmark copy. Of 2,740 "delete loopflow.db" lines in
`~/.lf/logs/lf.log`, 9 come from 0.14.19, **908 from released 0.15.0, and 1,823 from
`0.15.0+b1f29930c.dirty`**. v0.15.0 was cut (2026-10-28 15:00Z) before #1755 merged. On
2026-10-30 12:34:49Z a `ce607b9` (#1776) development build applied `ship_stop` and
`task_regressed_in` to `~/.lf` through the install switch. From that second on, every
0.15.0 process on the Machine, including the installed app and its Sessions, told Jack to
delete the store. That lasted until v0.15.1 shipped at 15:02Z.

### Causal chain

1. **Desktop said "delete loopflow.db".** Its in-process store reader (#1773) is 0.14.19.
2. **Why that message, and not "needs lf 0.15.x, install it"?** Pre-#1755 readers know only
   release-scoped ids (`0.15.0.001_release`). In `pending_migrations` and
   `admitted_newer_floor` (at `1e0aa05d1:rust/loopflow/src/store/migrations.rs` ~1282, ~1343), an
   applied id that does not parse as release-scoped is classified "not our database" →
   `RECREATE_MESSAGE`. #1755 introduced bare slug ids (`ship_stop`). Every older reader therefore
   reads a slug-bearing store as foreign. The reader-floor rows were correct (`reader_floor` =
   `0.15.0.001_release`), but old readers never reach the floor check. Contrast 2026-10-29
   17:36Z: before any slug was applied, the same 0.14.x readers correctly said
   "this store needs lf 0.15.0 or newer … install it with lf install".
3. **Why did old readers treat an unknown id as foreign?** That fallback was written for the
   pre-loop flat ledger. It assumed the id vocabulary was closed: any new id format would
   come from someone else's database, never from a future Loopflow. The fallback also gave
   destructive advice to the user, and nothing tested it against a future id format.
4. **Why did #1755 ship a vocabulary change with no expand step?** Its design worked out
   reader floors among post-#1755 builds, but it never shipped the new id format to the
   readers that were already installed. The change needed a release that taught 0.15.x to
   classify unknown non-release ids as "newer Loopflow" before any store carried a slug. The
   memory note [[no-slow-rollouts]] (reversed 2026-10-27) calls for exactly this expand-first
   staging, two days before #1755 merged. It was applied to schema shape, not to the ledger's
   own id format.
5. **Why could a dev build migrate `~/.lf` while released readers were running?** The #1755
   install switch "applies pending migrations that origin/main carries" without regard to
   live readers. #1776's "Never breaks the store" check runs in the other direction: it stops
   `lf install` from *installing* something that can't read the store. Nothing stops a build
   from *migrating* the store past readers that are already running. `lf doctor` still reports
   28 0.15.0 and 3 0.14.x processes that can't open it.

### Why could a non-installed app copy reach `~/.lf`?

Every Loopflow.app is a full store reader of the ambient home, wherever it sits on disk. Since
#1773, Desktop reads in-process, so it has no `lf` on PATH to shield it. `swift/project.yml:108` gives
every Xcode/DerivedData build the production id `com.loopflow.mac`. Renaming is opt-in per
script: `scripts/loopflow-dev.py` uses `.dev`, the release check `.release-check`, the desktop benchmark
`launch.py` `.bench`. The installed-sync benchmark staged a production-shaped copy and
kept the production id. The app has no notion of "I am not the installed app".

### Why does `lf open` resolve by bundle id?

`rust/loopflow/src/lf/commands/open.rs:850-858,1012`: `desktop_bundle()` returns `LF_DESKTOP` or the
constant `com.loopflow.mac`, and `send` runs `open -b <id>`. The id carries a real requirement.
`LF_DESKTOP` lets an agent inside the Dev app (`com.loopflow.mac.dev`) route back to *its* Desktop,
and URLs must reach the running instance. But "which bundle" stands in for "which
installation". LaunchServices picks among about 770 registrations, and that set includes 14
`.promote.*`/`.superseded.*` bundles (0.12.13–0.15.0) still sitting in `/Applications`
today. `prune_superseded_artifacts` keeps them whenever live executables are unknown
(`install.rs:2297`).

### Does #1776's "one Loopflow per Mac" cover this?

No. It covers the CLI: `~/.local/bin/lf` → `/Applications/Loopflow.app/Contents/MacOS/lf`
(verified; both 0.15.1). It says nothing about other app bundles on disk, about which bundle
LaunchServices launches, or about `--dev`, which by its own terms is CLI-only and leaves the
app's lf behind the dev CLI. On a dev Machine that split is the 2026-10-30 outage by design.

## Prevention (proposed, not approved)

Structural first; each removes a way to spell the failure rather than adding a check.

1. **Stop sending users to delete the store.** No reader may say "delete loopflow.db". An
   unrecognized ledger id means "written by a Loopflow this build doesn't know". The message
   should name the id, this build's version, the build recorded in the ledger
   (`package_version`/`source_revision` are already there), and say `lf install`. Partly true now:
   `rust/loopflow/src/store/migrations.rs:1118` recognizes release ids *and* slugs as "newer lf". Any
   other format still goes to `incompatible(conn)` as "not our database". That message now says
   "keep loopflow.db", but the closed-vocabulary assumption that caused this incident is still
   there, waiting for the next id format. Delete the pre-loop "not our database" branch. Those stores
   are long gone, and any non-empty ledger is ours. *Proof:* a fixture with `future:foo` in
   `schema_migrations` → message names `lf install` and keeps the store.
2. **Ledger vocabulary is part of the schema contract.** Changes to how `schema_migrations`
   ids, floors or classifications are spelled go through expand → release → contract, the same
   as destructive DDL. *Proof:* an upgrade fixture that seeds the store the last release wrote,
   applies main's migrations, then opens it with the **released** binary's reader (CI can
   fetch the last release artifact) and asserts a non-destructive refusal or a successful open.
   That one test would have failed on #1755.
3. **Launch the installation, not a bundle id.** `lf open` should launch the installed app by
   path. `installation.rs` already knows it, and `~/.local/bin/lf` resolves into it. Keep the bundle
   id only to address an *already running* instance named by `LF_DESKTOP`. That deletes the
   LaunchServices choice from the default path. *Proof:* with a stale same-id copy registered,
   `lf open` launches `/Applications/Loopflow.app`.
4. **Only the installed app carries the production identity.** Make `swift/project.yml`
   default to a non-production id (e.g. `com.loopflow.mac.dev`) and let only the packaging step
   stamp `com.loopflow.mac`. Then every DerivedData, `swift/dist`, benchmark and tmp build
   becomes a different app to LaunchServices, without each script having to remember. Watch the
   Xcode `PRODUCT_MODULE_NAME` trap noted in [[concerto-to-loopflow]].
5. **Promotion leaves no bundles in `/Applications`.** Stage outside `/Applications` (or in a
   `.noindex` dir) and delete the superseded bundle once the swap is done. Fourteen orphans
   dating back to August show the current prune doesn't converge.
6. **Open question, deliberately unresolved.** Should a build be allowed to migrate `~/.lf`
   past readers that are still running (the dev-CLI/stable-app split #1776 allows)? Refusing to
   migrate would be a guard. The structural alternatives are (a) a dev stream that updates the
   app too, restoring one lf per Mac, or (b) `lf monitor rollover` as part of the install
   switch, so the readers left behind are moved forward rather than left broken. Jack should
   choose.

## Unanswered / contradicting evidence

- Who launched the benchmark copy at 10:25 PST today: an `lf open`, a URL click, or Spotlight?
  launchd is the parent in every case. The mechanism holds either way.
- Whether `.promote.*`/`.superseded.*` in `/Applications` are still registered after the
  repair (the repair unregistered only *missing* paths). They exist on disk, so they likely are.
- `b1f29930c.dirty` (1,823 lines) is a local 0.15.0 branch build that some long-lived process
  was running. Not identified.
