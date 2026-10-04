# LOO-376 — Open Desktop immediately from one cached workspace

Status: draft, implemented in slices. Jack Heart requested the outcome in the
Task directive (2026-10-04); the design below is the implementation's, not
separately reviewed by Jack.

## Measured starting point (2026-10-04, installed `lf` 0.12.32, Jack's real Home)

One shell sample each, read-only, machine under normal load. Lower-level timing,
not rendered evidence.

| Startup read | Wall time | Output |
|---|---|---|
| `lf home id --json` | 1.2 s | 136 B |
| `lf wave list --all --current --json` | 2.4 s | 25 KB |
| `lf roadmap --all --json` | 14.5 s | 604 KB |
| `lf session list --json --page --limit 100` | 12.4 s | 67 KB |
| `lf ps --json` | 1.2 s | 61 KB |

Before this Task every launch waited on all of these: the window body waited on
`home id`, the outline on `session list` and `roadmap`. No `lf` read fits the
1000 ms usable-workspace budget, so a returning launch must not wait on any.

## Design

- **One owner, `WorkspaceCache`**, stored at `<Home>/desktop-cache/workspace.json`
  (`LF_HOME`, else `~/.lf`). Living inside the Home scopes it structurally; the
  saved `homeId` is checked against `lf home id` and a mismatch drops everything.
- **It keeps wire text, not models.** Restore runs the decoder live reads use,
  so an incompatible file fails to decode and is skipped. Versioned envelope;
  absent/corrupt/other-version files load as nothing and are removed.
- **Saved text is quiet.** On save, every pinned Flow becomes `unknown`, every
  Flow control unavailable, every Session `unknown` with only its `open` action
  (`lf session connect` re-validates). No credentials are read or stored.
- **Bounded:** 8 most recent repositories, 8 MB per read. Only the default
  interactive Session listing is saved; explicit history is read on request.
- **One refresh owner:** `PodiumModel.keepWorkspaceCurrent()` replaces the two
  view-owned loops whose Session reads overlapped at launch and superseded each
  other. Planning (15 s) and Sessions (2 s) keep separate cadences.
- **One vocabulary:** `WorkspaceStatus` — `Loading workspace…` (first launch
  only), `Updating…` (saved workspace shown), `Couldn't update…` (last-good
  kept), or nothing. One line in the outline; the surface shows a wordless spinner.
- **Signposts:** `cold_start` ends at first non-empty rows (now from saved
  text); new `workspace_current` ends when every part was read by this launch.

## Delete — do not maintain

- Done: `Reading planning…`, `Reading Sessions…`, `Reading roadmap…`,
  `Reading workspace Home` and their per-part error lines; the two
  `.task(id: repoPath)` refresh loops in `PodiumView`.
- Remaining: `RoadmapView`/`WavesView` each build a private `PodiumModel` and
  call `refresh()`; fold them onto the window's model or delete with their window.

## Remaining work

1. **Rendered startup runner** (acceptance, blocked in headless runs): agent
   runs have no Aqua session (`launchctl managername` = Background), so a window
   cannot be launched. Still owed on a capable host: launch the built app with a
   replaying `lf` beside its executable (allow-listing read verbs so nothing
   launches or mutates live work), 20+ samples for p95, first frame, main-thread
   stalls, CPU/RSS, subprocess counts, warm reopen, and refresh arriving after a
   selection or repository change. Headless equivalent delivered:
   `startup.py` + `20261004-startup-inprocess/` (usable from saved text in
   12 ms median vs 16.4 s uncached; offline keeps content). LOO-371's snapshot
   runner is not on main; reuse it when it lands.
2. **Critical-path costs still on the main thread before first frame:**
   `WaveOrigin.resolve` (two `git` execs) and `PortfolioDiscovery.resolveLaunchRepo`
   in `PodiumView.init`. Measure with the runner first, then move or cache.
3. **Repository list** still appears only after discovery; save it if measured.
4. **`lf ps --json` every 2 s** (1.2 s each) stays a separate loop; steady-state
   cost belongs to LOO-304. The 12–15 s `roadmap`/`session list` reads are CLI
   latency (LOO-375 owns `wt list`; no owner yet for these two).
5. Selection saved per repository is the Work selection only; the open Session
   is not restored (see `scratch/questions.md`).

## Checks

- `scripts/test_desktop.sh -Xswiftc -gnone --filter "WorkspaceCacheTests|PodiumModelTests|WorkspaceNavigationTests|DesktopHeadlessTests"`: 61 tests in 5 suites passed (2026-10-04). Full suite, Xcode app build and the rendered startup runner are owed to gate and item 1.
- Saved-roadmap cost on Jack's real 607 KB payload, debug build, in-process: quieting 28 ms (cache queue), decode 9 ms (main thread). Lower-level timing only.
- `startup.py run --samples 5` on a capture of Jack's Home: passed, receipt in `scripts/benchmarks/desktop-performance/20261004-startup-inprocess/`.
