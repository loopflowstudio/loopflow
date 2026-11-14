# Sparkle crash-report noise — LOO-716

## Design and remaining work

Give the deliberate missing-Sparkle abort a recognizable test identity:
`BareSwiftPM.app/Contents/MacOS/BareSwiftPM`. Retain the real dyld failure
assertion and positive packaged-app launch checks. Publication and the code
Graph's interactive demo/review remain; merging is outside the current scope.

`python/tests/test_app_bundle.py::_swiftpm_build` produces a Mach-O executable
linked to synthetic Sparkle with only `@loader_path`. The negative control
copies its returned executable without the framework; no Info.plist is involved.
Using that return value avoids repeating the helper's build-product filename.
The destination's bundle and executable names both identify the test.

`scripts/app_bundle.py::install_app_executable` owns embedding and rpath repair
for the production builders. Existing Dev and benchmark fixtures launch after
removing their build directories. Keep those checks and the missing-input check.
No production API, builder, report filter or system crash-report setting changes.

## Delete — do not maintain

No obsolete code path, symbol or exclusive fixture remains to delete. The old
negative-control destination is replaced directly. Drop duplicated plan/review
explanations; retain the incident observations below and the earlier review at
`be894af388dc64ce348f4418e825bc88be396061:scratch/concept-review.html`.

## Review and checks — November 14, 2026

Review the destination diff alongside the focused pass. A bundle-only rename
would leave an ambiguous executable; removing the actual launch would weaken
coverage. Neither occurs. Old reports still need their pytest path and parent
process to establish origin. Any new report from the control should identify
`BareSwiftPM`; asynchronous system report creation is not a test condition.
No new system crash report was inspected during this pass.

`uv run pytest python/tests/test_app_bundle.py` — 4 passed on macOS in 1.10 s,
including Mach-O compilation and real launches; gate/CI own later checks.
The earlier recorded result was 5 passed in 1.66 s; the current checkout
collects four tests and has no third-party bundle test. That earlier count is
historical, not the current suite expectation. Non-Darwin or missing-cc skips
cannot establish this outcome. Fixture launches prove no Desktop release repair.

`lf context` — Wave memory and scratch are within token and byte targets.

## Incident evidence

Source: /Users/jack/src/loopflow/scratch/desktop-exit-2026-11-02.md, Chain B, November 2, 2026. Selected scope: rename the deliberate negative-control executable/bundle. The optional stale-branch launcher check below is not selected work; main already contains the packaging fix.

### Chain B — today's Sparkle dyld crashes (separate from the silent exits)

Status: no defect in current main; both sources are explained.

1. **Pytest temp bundles (10:32, 11:03, 11:53, 12:21, 13:04): deliberate.** All five `.ips`
   files name `pytest-*/test_bare_swiftpm_binary_in_a_0/Loopflow.app`, slice UUID
   `647F6486…` each time, parent `python`. That is
   `python/tests/test_app_bundle.py::test_bare_swiftpm_binary_in_a_bundle_cannot_load_sparkle`,
   the control test #1796 added: it copies the bare SwiftPM binary into a bundle and asserts
   it aborts in dyld. Every run of that suite files a real-looking "Loopflow" crash report.
   Not a LOO-699 regression; it is noise in the crash-report evidence that a later incident
   reader (this one, earlier today) can mistake for one.
2. **`~/Applications/Loopflow Dev.app` at 13:17:36: built from a checkout older than the fix.**
   - The crash shows `Contents/MacOS/Loopflow` and dyld trying only
     `Contents/MacOS/Sparkle.framework` (the SwiftPM `@loader_path` rpath). Current
     `loopflow-dev.py` names the executable `Loopflow Dev` (#1792) and
     `install_app_executable` adds `@executable_path/../Frameworks` and embeds Sparkle
     (#1794/#1796). The 13:17 binary had neither, so it was not built by current code.
   - The Dev app is LOO-673's (`--repo …loopflow.loo-673-desktop-polish-fix`). That
     worktree's reflog: at `cf8000104a` (Oct 30 13:32 PDT) until `rebase (start): checkout
     origin/main` at **13:18:17**, 41 s after the crash. `cf8000104a` contains #1776
     (Sparkle became a linked dynamic framework, Oct 30 12:05Z) and lacks #1794 (22:56Z):
     exactly the window in which `loopflow-dev.py` copied a bare binary.
   - After the rebase the bundle was rebuilt at 14:08–14:11 with `Contents/Frameworks/
     Sparkle.framework` and `MacOS/Loopflow Dev`; pid 483 has run from it since 14:11:09.
   - Why LOO-699 "didn't hold": it did, for every builder on main. The builders are
     scripts inside each checkout, so a Task branch based before the fix keeps the broken
     builder until it syncs. 17 of 34 worktrees still predate #1794 (e.g. `loo-669` detached
     at the 0.15.1 release, `loo-698`, `loo-701`); any of them running `loopflow-dev.py` or
     `install.py local` produces the same crash. LOO-699's outcome was stated for "every way
     of building" but delivered per-revision; nothing tells a stale checkout its builder is
     known-broken.
   - Who launched the Dev app at 13:17 (parent `launchd`, so via `open`) was not traced.
   - Prevention (proposed, not approved): (a) none needed for main; (b) optionally name the control test's bare executable something other
     than `Loopflow` (for example `BareSwiftPM`), so its expected dyld abort stops filing
     `Loopflow-*.ips` reports that look like product crashes; (c) for stale branches, the Dev-app
     launcher could run `verify_rpath_libraries` before `open`, which catches any bundle
     regardless of who built it. Proof for (c): launch from a checkout at `cf8000104a` and
     see a build-time error instead of a dyld crash.

