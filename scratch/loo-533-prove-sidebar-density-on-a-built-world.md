# LOO-533: Prove sidebar density on a built world

Status: draft plan (kickoff, 2026-10-20). Jack Heart filed this on 2026-10-20 as an
autonomous follow-up to lf-world (#1612). Keep inline JSON for pure view logic,
failure and transitional states, terminal/focus proofs, and DTO contract fixtures.

## Problem

`WorkNavigationProofTests.sidebarDensityWithOrphans` (swift/LoopflowTests/WorkNavigationProofTests.swift:96)
builds its population by hand. It clones one `roadmap_snapshot.json` Task 150 times
(3 Waves × 50, every 7th started), types out seven Session records with invented
fields, and stubs `wave show` / `session` / `history`. Nothing in it comes from
`lf`, so it drifts whenever the roadmap or Session wire shape changes. It also sits
in the `.requiresDisplay` suite, so gate and CI never run it.

## Demo

```bash
cargo build -p loopflow --bin lf --bin lf-world
target/debug/lf-world build density /tmp/density    # Waves, 150 Tasks, 20 started, 4 orphan Sessions
scripts/test_desktop.sh --filter sidebarDensity     # headless; drives WorkNavigator from that world
```

The proof opens product's 8 started Tasks and selects "Keep Task observation read-only 7".
The working list shows those 8 Tasks, each Wave keeps one row, and the 4 orphan
Sessions stay out of the main navigation. It then binds the orphans with
`lf session bind` and refreshes; `unmatchedSessions` is empty.

## Risk findings (measured 2026-10-20, debug build, this Mac)

- **Scale was the blocker. 150 `task_create` calls took 659 s (~4.4 s each);
  24 Sessions took 2 s.** A sample of one more `lf task create` on that world
  (10.4 s) spent 2,746 of 2,748 samples in `planning_sync::exchange_repository`.
  `SqliteStore::import_peer_planning` recomputes `PlanningSnapshot::heads()`
  over the whole journal once per incoming edit
  (`store/sqlite/planning_peers.rs:298`). That costs O(edits × changes) on
  every acquisition, and it accounts for ~46% of samples. Every world has a
  Git origin, so every world and every real repository using the default
  user-keyed Git planning pays this on each Task save and on cold Task reads.
  `heads()` itself also rebuilds its retired set on each call.
- **The other half is linear, not quadratic** (read 2026-10-20). Each Task save
  runs `publish_destination`, which first runs `acquire_destination`. That imports
  the whole fetched journal and rewrites every winner (`ensure_record`, `read`,
  `write` per record). It then exports, merges and serializes the whole snapshot.
  That work is O(records) per save, so it should fit the budget once the
  quadratic loop is gone. Re-measure before touching it.
- Reads are fine at this size: `lf wave show --json` 0.36 s and
  `lf session list --json --all` 0.11 s on the 150-Task world.
- **Started = a bound Session (or a Flow in a checkout).** `World::session(Some(task))`
  starts a Task without a worktree. The probe world read back as product 8,
  infrastructure 7, intelligence 5 started, which matches the hand-built population.
- `World::session(None)` yields `work: null`, so `WorkProjection` treats those as
  `unmatchedSessions`. That matches the hand-built orphans. Their cwd is the world repo, not
  separate worktrees; the assertions don't depend on cwd.
- WorkNavigator inspects headless: `DesktopHeadlessTests` already builds
  `WorkNavigator(model:onOpenSession:)` and taps through it with WindowServer denied.
- A Swift test can drive real `lf`: `WorkOpeningCLITests` runs `target/debug/lf`
  (or `LOOPFLOW_TEST_LF`) with a scrubbed environment, and the swift gate and CI
  build `lf` first (`scripts/test.py` `swift-cli`; ci.yml "Build CLI for Swift
  transport tests").

## Chosen approach

1. **Fix the quadratic import.** In `import_peer_planning`, compute
   `merged.heads()` once, grouped by `edit.key()` into a `BTreeMap<(&str, &str), Vec<&str>>`,
   and look each edit up there. `merged` does not change inside the loop. Re-measure.
   If the density build still exceeds the budget below, profile again and fix the
   next superlinear planning path the same way. Never bypass the exchange for worlds.
2. **Add the `density` recipe** to `world/recipes.rs` (rung `Artifacts`: Sessions write transcripts; no checkouts,
   flows or worktrees). It builds Waves `product`, `infrastructure` and `intelligence`, each with
   50 Tasks titled from the four existing phrases plus the index, in rank order.
   Global index `g < 140 && g % 7 == 0` gets one bound Session. Four unbound
   Sessions are titled `demo`, `realign`, `luna-aria` and `loopflow`. `demos:`
   "sidebar density: 150 Tasks across three Waves, 20 started, orphan Sessions".
   `world_recipes.rs` picks it up automatically through `lf-world list`.
3. **Drive the proof from the world, headless.** Move the test out of the
   `.requiresDisplay` suite into a headless suite: either `WorkNavigationTests.swift`
   or a new `WorkDensityWorldTests.swift`, whichever reads cleaner beside it. A small
   fixture, shaped like `WorkOpeningCLITests.Fixture`:
   - resolves `lf` and `lf-world` (`LOOPFLOW_TEST_LF`/`LOOPFLOW_TEST_LF_WORLD`, or
     `target/debug/*`). It writes a `.gitconfig` into a temp HOME (as `world_recipes.rs`
     does) and runs `lf-world build density <tmp>/world`.
   - builds a `RegistryQuery` whose runner execs `lf` with the world's `env.json`
     (`LF_HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`), scrubbed PATH, `LF_BIN`, and
     `GIT_*` isolation. The default cwd is `<world>/repo`.
   - `WorkModel(query:repoPath: <world>/repo)`, then `refresh()`. Select the product
     Task "Keep Task observation read-only 7" (found by name from `model.visibleWork`; IDs are
     generated). Inspect `WorkNavigator(model:onOpenSession:)` directly.
   - It makes the same assertions as today, with real ids: 8 `work-task-*` buttons;
     one `work-wave-<id>` per visible Wave; no `WaveLensView`; no `work-session-count-*`;
     no `work-orphans-open`, `session-row-<orphan id>` or `debug-orphan-session-<orphan id>`.
     Also assert `unmatchedSessions.count == 4` before binding.
   - Phase two runs `lf session bind <orphan> --task <that Task>` for each orphan through
     the same runner, then calls `refreshSessions()`. `unmatchedSessions` is empty, and
     `work-orphans-open` is still absent.
4. **Build `lf-world` wherever the swift suite builds `lf`.** Add `--bin lf-world` to
   `scripts/test.py` `swift-cli` and to ci.yml's "Build CLI for Swift transport tests".
5. **Window captures move to the screenshot manifest.** The two-size captures were
   opt-in evidence (`LOOPFLOW_OUTLINE_CAPTURE_DIR`), and worlds already have their
   visual venue. Add `density-wide` (1440) and `density-narrow` (1100) `wave-proof`
   shots (`mode: live`, `world: density`, `wave: product`) to `scripts/screenshots.yaml`.

## Alternatives rejected

- **Skip planning exchange in worlds** (no origin, or a config knob for one recipe).
  It is fast but dishonest. A world would stop exercising the writer real repos use,
  and the 4–10 s Task save would stay for real users with long planning history.
- **Batch writer that defers exchange**: a World-only path around `task_create`.
  This contradicts lf-world's rule that recipes use the same ops `lf` uses.
- **Keep the proof display-only and only swap the population**: the minimal edit, but
  the proof still never runs in gate or CI, so drift goes unnoticed.
- **Skip import when the fetched revision equals `planning_peer_imports.revision`**:
  this is unsafe as a first fix. Conflicted records ("Selected Project is not
  available locally") are retried only by re-import, so a skip would strand them
  after local work resolves them. It also would not fire in this world, because
  each save publishes a new revision and the import table never records it.
  Only revisit this if re-measurement shows the linear re-import breaks the budget.
- **Commit `lf wave show` output from a world as a JSON fixture**: that just recreates the
  hand-maintained file this Task deletes.

## Source of truth and consumers

- The store, populated by `World` through `ops::task::task_create`,
  `provider_conversation::admit`, `bind_session` and `rename_session`. The Swift
  proof reads it only through `lf wave show/list`, `lf session list` and `lf history`.
- Consumers that change: `lf-world list` (new row), `world_recipes.rs` (builds it),
  the swift gate and CI (also build `lf-world`), and `screenshots.yaml`.
- Absent binary: the test fails through `#require` with the build command, as
  `WorkOpeningCLITests` does. It never skips silently.

## Operational boundary

- The `density` world must build in under 60 s on a debug build here, down from
  661 s. Record the measured figure below. The swift CI step's 20-minute cap includes it.
- `World` sets process env, so each build runs `lf-world` as a subprocess, never
  in the test process.

## Delete — do not maintain

- In `sidebarDensityWithOrphans`: the `roadmap_snapshot.json` cloning loop, the
  `executions` map, `record(...)`, the `attached`/`orphans` arrays, `encode`, the
  Session-list actor instance and stub `RegistryQuery`, the NSWindow size loop,
  `capture`, and the test's place in the `.requiresDisplay` suite.
- Keep: the replaceable roadmap actor (now `RoadmapSource`, used by the named drill-down proof), `_scrollView`, `settle`,
  and the other display tests in that file.

## Exclusions

- Execution variety (running/blocked/human Tasks) in the hand-built population only
  colored captures. No assertion reads it, and live states need real Processes.
- Fixing `heads()` beyond the import loop, unless re-measurement shows it is needed for the budget.
- Other hand-assembled populations in the file (`navigatorRetainsScroll` clones 80 Tasks).
  Those can be a separate Task once this pattern lands.

## Done when

- `lf-world build density` produces 150 Tasks (8/7/5 started) and 4 unbound Sessions
  in under 60 s on a debug build. `cargo test --test integration world_recipes` passes.
- The headless proof passes on that world through real `lf`, including the bind phase.
  The hand-assembled population and its window loop are gone from
  `WorkNavigationProofTests.swift`.
- Planning import has a focused Rust test showing the grouped heads still select the
  same parents. The existing planning_peers tests pass.

## Acceptance at gate

```bash
cargo build -p loopflow --bin lf --bin lf-world
env -u LF_HOME -u LF_BIN cargo test -p loopflow --test integration world_recipes < /dev/null
cargo test -p loopflow planning_peers < /dev/null
scripts/test_desktop.sh -Xswiftc -gnone --filter 'sidebarDensity'
```

Expected: all pass. The density build time is printed in the remaining-work line below.

## Remaining work

- [x] Grouped heads in `import_peer_planning` (`PlanningSnapshot::heads_by_key`); density build 661 s → 448 s under load ~50. Budget unmet; see questions.md #1.
- [x] `density` recipe.
- [x] Headless world-driven proof (`WorkDensityWorldTests.swift`); hand-built population deleted.
- [x] `lf-world` in swift-cli and CI build steps; density screenshot shots.
- [ ] Gate: `world_recipes` with density; decide whether the build cost is acceptable in CI.

Check: `cargo test -p loopflow --lib heads_by_key|planning_peers` pass; clippy clean; `scripts/test_desktop.sh --filter sidebarDensity` passed in 613 s (world build dominates, load ~50). Gate owns `world_recipes`.
