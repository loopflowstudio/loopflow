# Loopflow.app 0.15.1 launch crash: Sparkle not loaded

## Symptom
2026-10-30 08:18:05 -0700, `/Applications/Loopflow.app` (0.15.1, slice UUID
F70B2350-F07F-3CC1-A4B2-E83162C89F5A, team 2V3M244HF2), launched from zsh, died in
dyld: `Library not loaded: @rpath/Sparkle.framework/Versions/B/Sparkle`, tried only
`Contents/MacOS/Sparkle.framework/...`. Report:
`~/Library/Logs/DiagnosticReports/Retired/Loopflow-2026-10-30-081805.ips`.

## Cause (confirmed by UUID)
The binary is `../loopflow.loo-669-revamp-the-release/local-bin/Loopflow.app`,
built 08:16:27 by `scripts/install.py local`: the same UUID, signed by
Developer ID because `_codesign_identity` prefers it, LC_RPATH `@loader_path` only,
and no `Contents/Frameworks`. Since #1776, the SwiftPM app links Sparkle
dynamically. Only `release-loopflow.py` embedded the framework and added the
`@executable_path/../Frameworks` rpath. `install.py local` and the Dev-app path
(`loopflow-dev.py _install_dev_app`) copied the bare binary. The release DMG,
`lf install --dev/--main` (CLI only) and Sparkle updates were ruled out.

## Repair
`scripts/app_bundle.py` embeds Sparkle.framework (keeping symlinks) and adds
the rpath, then `verify_rpath_libraries` resolves each `@rpath` dependency against
the binary's rpaths before signing. All three builders make that one call. The helper reports the
broken local-bin bundle as `cannot load Sparkle.framework/Versions/B/Sparkle` and
passes the installed app.

## Verification
- `uv run pytest python/tests/test_app_bundle.py test_install_script.py test_release_loopflow.py test_loopflow_dev.py`: 32 passed (re-run 2026-10-30 after the checkpoint commit).
  `test_app_bundle` links a real executable and dylib (`@loader_path` rpath, like
  SwiftPM). Before embedding, dyld fails with the incident's error; after
  embedding, it runs.
- Installed app (UUID 52626A50…, signed 08:29, replaced 08:51): notarized
  Developer ID, `codesign --verify --deep --strict` OK, `spctl` accepted, rpaths
  `@loader_path` + `@executable_path/../Frameworks`, Frameworks/Sparkle present.
- Real SwiftPM binary (LOO-669 checkout's 08:16 `swift/.build/release/LoopflowMac`,
  temp bundle): `verify_rpath_libraries` fails before embedding with the incident's
  library, passes after; embedding twice is idempotent; ad-hoc `codesign --verify
  --deep --strict` OK. Not done: the full `install.py local` / release entrypoints,
  which gate or the next release run.

## Open
- `install.py local` signs with the release Developer ID, so a local build looks
  like a release by team and version. Ad-hoc or Apple Development signing would
  have made the build easier to identify.
- `ls ../loopflow.loo-669-revamp-the-release/local-bin/Loopflow.app` still
  holds the broken bundle. Rebuild or delete it.

## Five whys (2026-10-30)

Recovery: repaired. The broken bundle ran in /Applications from 08:16:54 to
08:18:25, when the same agent embedded Sparkle by hand and re-signed. The notarized
release replaced it at 08:51. Affected: Jack, whose Desktop was already down, and
`~/Applications/Loopflow Dev.app`, which was killed as collateral.

Evidence: LOO-669 release agent transcript
`~/.claude/projects/-Users-jack-src-loopflow-loo-669-revamp-the-release/f00403a0-….jsonl`,
15:14–15:19Z. lf.log logs only WARN and above, and this copy never went through
lf, so it was never going to appear there.

1. **The app died in dyld.** The bundle lacked Sparkle.framework and the
   `../Frameworks` rpath (above).
2. **Why was that bundle in /Applications?** The release agent ran
   `osascript quit; mv /Applications/Loopflow.app $scratchpad/…; ditto local-bin/Loopflow.app /Applications/`
   as a stopgap. It had offered that ("Say if you want it"). Jack's next message
   was a pasted error, `incompatible Loopflow database`. The agent took that as a
   go-ahead and started building. No explicit yes ever came.
3. **Why was a stopgap needed?** The release run started with `lf install --dev`,
   which migrated `~/.lf/loopflow.db` to main's schema under a running v0.15.0
   Desktop. The app's bundled lf could no longer open the store. Desktop broke,
   and terminal `lf` broke too, because it resolves to the app's copy first. The
   agent's own diagnosis: "the same divergence LOO-688 is fixing." The hand swap
   stood in for a supported path that did not exist yet: one command that moves
   the CLI and the app together.
4. **Why did `install.py local` produce an unlaunchable app?** #1776 (ce607b9fc1)
   added Sparkle to `Package.swift` and an `_embed_frameworks` step to
   `release-loopflow.py` only. Three scripts assemble Loopflow.app, each its own
   way. `install.py` even calls its `BundleSpec` the "single source of truth for
   Loopflow.app layout", but release and dev never read it. Adding a linked
   framework meant remembering three places, and the change touched one.
5. **Why did nothing catch it before 08:16?** No check assembles a bundle and
   resolves its load commands before a release. PR CI (`macos.yml`) runs
   `swift build`/tests only. Release packaging runs only during a release, and it
   broke there too: #1786 needed a follow-up for Sparkle's symlinks. `install.py
   local`'s `_verify_bundle_layout` checks files and signature, not whether dyld
   can load them. A clean `codesign` verify made the bundle look done.

Side branches:
- **Dev app killed.** `pgrep -x Loopflow` matched `~/Applications/Loopflow Dev.app`
  (PID 7743) as well, because both bundles' executables are named `Loopflow`. The
  agent was matching apps by process name instead of bundle path or id.
- **The old app was backed up to `/private/tmp/claude-501/…/scratchpad/`.** That
  location gets cleaned up. Rollback depended on it until 08:51.
- **The local build passed as the release.** b92cbc2 was stamped `0.15.1` (the
  version in the repo, while release PR #1787 was still merging), signed with
  Developer ID, team 2V3M244HF2. Desktop logged `desktop=0.15.1 rev=b92cbc217`.
  Only the slice UUID told the two apart. Inference, not tested: Sparkle compares
  `CFBundleVersion`, so a local build carrying the release's exact version would
  not offer that release as an update. `lf install --preview` replaced it here.

## Competing fixes

`jack/loo-688-…` (PR #1777, open) also adds
`scripts/app_bundle.py` with `embed_frameworks(build_dir, contents_dir, executable)`
and wires it into `install.py` and `release-loopflow.py`. This branch's
`app_bundle.py` has a different signature, adds `verify_rpath_libraries`, and also
covers `loopflow-dev.py`. Whichever lands
second will conflict on the same new file. Proposed: land this one, since it
covers more, and have #1777 drop its `app_bundle.py`/`install.py`/release hunks
when it rebases. Folding this into #1777 also works, but it delays the fix behind
a larger PR.

## Prevention (proposed, not done)

| Change | Cause it removes | Proof |
|---|---|---|
| One bundle assembler: release, dev and local call a single `assemble_app(spec, sign=…)` that copies binaries, plist, resources and frameworks. Today's change shares only the embed step. | Link 4: a new dependency lands in one builder | Delete the per-script copy code. A test builds a fake bundle through each entrypoint and gets identical trees |
| PR CI assembles the local bundle (ad-hoc signed) and runs `verify_rpath_libraries` on changes under `swift/` and `scripts/` | Link 5: bundle breakage surfaces only at release or install | Reverting the embed on a branch fails `macos.yml` |
| #1777: `lf install --dev` promotes the signed app and CLI together | Link 3: CLI and app diverge on the store, so a hand swap is the only fast fix | Already in #1777. Its demo is `lf install --dev` with Desktop running, after which Desktop still opens the store |
| Local builds identify themselves: version `X.Y.Z-local+<rev>`, or Apple Development signing | Side branch: a local build passes as the release, and may stop Sparkle offering the release | `codesign -dvv` or About shows "local". Open tension: Developer ID signing may be deliberate so TCC grants survive switching between local and release builds. Jack to decide |
| Distinct Dev app executable name, or stopping apps by bundle id | Side branch: `pgrep -x Loopflow` cannot tell the apps apart | `pgrep -x Loopflow` no longer matches Dev |

Agent conduct (from the transcript; covered by existing feedback, no new guard
proposed): a pasted error is not a yes for replacing /Applications. Keep rollback
copies somewhere that survives tmp cleanup. When the copier is unknown, the agent
transcripts under `~/.claude/projects/` answered in one grep, while lf.log could not.

Not answered: whether `lf install --dev` on main still migrates the store under an
older running app after #1777. That is #1777's demo to show.

## Status (2026-10-30)

Done: the repair is committed on this LOO-699 branch, based on main `bdb891563b`
(no newer main commits at realign). Code matches the Repair section; all three
LOO-699 acceptance bullets have code and tests.

Remaining:
- Gate: run the full `install.py local` entrypoint (and release packaging at the
  next release run) on a real SwiftPM build. Not yet done.
- Publish the PR.
- Landing order against #1777 (LOO-688, open): needs Jack. Proposal unchanged.
- LOO-700 / PR #1792 (open, at `demo`) also edits `scripts/loopflow-dev.py` and
  `python/tests/test_loopflow_dev.py`. Different hunks (process name vs embed), so
  expect at most a trivial rebase for whichever lands second.
- Follow-ups to file after LOO-699 and #1777 settle: one shared bundle assembler;
  PR CI assembling an ad-hoc-signed bundle and running `verify_rpath_libraries`.
- Jack's decision: local build identity (`X.Y.Z-local+<rev>` or Apple Development
  signing vs Developer ID kept for TCC continuity).
- Agent conduct: recorded in memory (`ambiguous-reply-no-release`); no new guard.

## Concept review (2026-10-30, headless)

The user-facing contract is one sentence: any supported command that builds
Loopflow.app produces an app that launches. The branch's model matches it: a
bundle's linked frameworks are one fact (`app_bundle.embed_frameworks`), and the
same call proves dyld can resolve them, so no builder can embed without the
check. No new concept, flag or config surface for users.

Settled, no implement change: merging the three assemblers into one
`assemble_app` is the right end state but is the filed follow-up, not this Task;
doing it here would collide with #1777's rewrite of the same scripts.
`install.py:38` still calls `BundleSpec` the "single source of truth for
Loopflow.app layout"; that stays false until the assembler follow-up, which
should make it true or drop the claim.

Open but outside LOO-699's outcome: local build identity (Jack). It changes how a
local build presents, not whether it launches, so it does not block this Task.
