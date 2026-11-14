# Sparkle crash-report noise — LOO-716

## Current direction — November 14, 2026

Jack Heart approved starting implementation through PR review in the Task's
primary conversation. This supersedes the earlier preparation-without-launch
boundary. Rename the negative-control bundle and executable to `BareSwiftPM`,
retain the real missing-Sparkle assertion and all positive launch checks, and
run `uv run pytest python/tests/test_app_bundle.py`. Stop at the code Graph's
interactive demo/review step; merging is not yet requested.

## Implementation plan

Maintainers reading macOS crash reports should recognize the deliberate
missing-Sparkle abort as a test. Implemented: the negative control launches
`BareSwiftPM.app/Contents/MacOS/BareSwiftPM`; correctly packaged Loopflow apps
keep their existing names and launch checks.

### Current architecture and risk findings

- `python/tests/test_app_bundle.py::_swiftpm_build` links a small Mach-O
  executable to a synthetic Sparkle framework with only `@loader_path`.
  Its build product is `LoopflowMac`, shared by the positive fixtures.
- `test_bare_swiftpm_binary_in_a_bundle_cannot_load_sparkle` copied only that
  executable to `Loopflow.app/Contents/MacOS/Loopflow` (now `BareSwiftPM`). It both checks
  `verify_rpath_libraries` raises for missing Sparkle and launches the actual
  executable, checking dyld's missing-library message. Only the destination
  path changed; the linked dependency and rpath are unchanged. The November 2
  reports below establish the naming problem.
- `scripts/app_bundle.py::install_app_executable` reads `CFBundleExecutable`,
  embeds Sparkle, adds the Frameworks rpath and verifies resolution. Install,
  Dev, release, benchmark and third-party builders already use this owner.
  No production launch path needs a change.
- The same test file launches packaged Dev, benchmark and third-party bundles
  after removing the build directory, proving they resolve embedded Sparkle.
  Missing executable/framework errors are also covered. Preserve these checks.
- The suite requires Darwin and `cc`; elsewhere it skips. A skip cannot prove
  this outcome. The Wave's source-versus-installed evidence distinction applies:
  these fixture launches prove test behavior, not a Desktop release repair.

### Chosen mechanism and alternatives

Change only the negative control's destination bundle and executable basename
to `BareSwiftPM`, with a short comment explaining the crash-report identity.
Keep `_swiftpm_build`, its product name and the positive fixtures unchanged.
The negative control launches directly and has no Info.plist to migrate.

Removing the real launch would lose the dyld regression check. Suppressing or
deleting reports would hide evidence. Adding a production preflight or updating
stale worktrees solves a different problem. None is part of this Task.

### Remaining review

Code is complete. Remaining: publication and the code Graph's interactive
demo/review step. Realign, November 14: origin/main has no commits past the
checkout base and no open PR touches `python/tests/test_app_bundle.py`.

Gate and headless demo: `uv run pytest python/tests/test_app_bundle.py`.
Expected: all five tests pass on macOS; the negative control still reports
`Library not loaded: @rpath/Sparkle.framework` from the `BareSwiftPM` executable,
and the packaged apps exit successfully without their build directories.
Review the destination path in the diff alongside the passing suite. Any crash
report emitted by this control should identify `BareSwiftPM`; report creation
is asynchronous, so polling or counting global reports is not a test condition.

Done when deliberate aborts identify the test executable and maintainers keep
the genuine launch-failure coverage. Success preserves diagnostic evidence with
a recognizable name; failure would be renaming only the bundle while leaving
the crashing executable named Loopflow, or weakening the dyld assertion.
This serves Infrastructure's reliable diagnostic evidence; there is no new
performance metric or additional product decision.

Check: `uv run pytest python/tests/test_app_bundle.py` — 5 passed on macOS,
November 14, 2026 (1.66 s); both missing-Sparkle assertions and positive launches retained.

## Concept review — November 14, 2026

Outcome for loop-or-next: nothing to change and no open product choice; advance.
The implemented destination gives the deliberate abort its own identity at both
the bundle and executable levels. No next implement direction is needed.

Maintainer walkthrough: run the bundle suite, then investigate a Desktop exit.
The negative control launches `BareSwiftPM` and retains the missing-library
diagnostic; packaged fixtures launch successfully after their build directories
are removed. A genuine Loopflow launch failure retains its product identity and
its diagnostic evidence. Recovery remains the existing embedding/build path,
followed by launching the repaired bundle; the test rename changes no recovery
action. Older Loopflow-named test reports remain historical evidence and still
need their pytest path and parent process to establish origin.

The model is a filesystem identity, not a new crash-report category or lifecycle.
`shutil.copy` sets the destination; `verify_rpath_libraries` and `subprocess.run`
exercise it. The shared build product, production APIs, persistence and execution
ownership need no changes. Existing usage docs and skill instructions remain
accurate. Counterexamples checked against the diff: renaming only the bundle
would leave the executable ambiguous; removing the actual launch would weaken
coverage; globally filtering Loopflow reports would hide genuine failures.
None occurs in the implemented change.

The recorded five-test macOS pass above applies to the unchanged code diff;
this review did not rerun it or observe a newly emitted system crash report.
Review page: `scratch/concept-review.html`. Next show the destination diff and
recorded focused pass at the Graph's existing review boundary.

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

