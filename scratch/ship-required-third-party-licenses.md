# Third-party notices — LOO-466

Jack Heart directed shipping on October 15, 2026 after accepting OFL font terms.
The earlier MIT/OFL stop was premature: preserve upstream MIT attribution and
apply OFL to the composite font, without changing Loopflow's license. Review all
other glyph-source notices instead of assuming the root MIT license covers them.

Main now ships GhosttyKit a60e9e2-lf7-theme-readback1, checksum
19d6f6e2f93cf3d967aaaa9761c6ee1995683381ac432598d7b0496610934ce9.
It still embeds byte-identical Symbols Nerd Font 3.4.0, SHA256
71db104aa66567d0efe0b98758f9dfc1895573a453fe85fb53d1c38544a55106
(arm64 offset 128927953). Source revision is unchanged; use all current patches.

## Implementation

- One reviewed non-Cargo manifest and preserved notices; derive Rust dependencies
  from the locked runtime closure for the four shipping targets.
- Generate the offline notice document and inventory with digests of shipped
  resources, patches and lockfiles. Updates regenerate and review these together.
- About exposes an offline, selectable/searchable native notice view.
- App packaging adds matching source archives and Loopflow source/build inputs.
  LGPL libintl receives standalone uncombined archives and a rebuild/relink path.
  GPL shell scripts remain intact; MPL z2d and option-ext sources accompany output.
- The same packaging operation adds notice/source material to future GhosttyKit
  archives and validates completed bundle/archive contents before signing/shipping.
- Do not upload a new GhosttyKit artifact or mutate published objects.

Remaining: implementation, complete glyph/native dependency audit, tests for
missing material and actual artifacts, focused Swift build/tests, review, land,
verified merge and Task completion. No follow-up Task needed yet.

## Gate evidence (October 15)

- `swift build --package-path swift --product LoopflowMac`: passed; focused `swift test --package-path swift --filter OpenSourceLicensesTests`: 2 passed.
- `uv run pytest -q python/tests/test_third_party.py python/tests/test_release_loopflow.py python/tests/test_loopflow_dev.py python/tests/test_ghostty_artifact.py`: 31 passed; final archive mismatch regression subsequently passed with all 11 third-party tests.
- `uv run ruff check` on changed Python files: passed. No Rust source changes; compiled the current `lf` for the assembled app.
- Actual app and compressed GhosttyKit archive: all 281 material files verified; source hashes match inventory, and the relink framework matches the distributed archive.
- `uv run python scripts/rebuild_third_party.py build .lf/tmp/notices/relink-proof`: full release Swift/Rust build, replacement libintl for both architectures and ad-hoc signing passed. No installation or launch. Initial disk exhaustion and an absolute SwiftPM binary-target path were repaired; the successful proof used the corrected script now packaged.
- Both slices contain byte-identical Nerd Font payloads; native archive member names match the earlier source audit. All seven shipped shell-integration files match pinned upstream source exactly.
- Review found and fixed relative package output paths and verification that the archived relink library equals the delivered framework. The generated inventory records 277 components, conservatively including normal proc-macro dependencies and ancillary upstream notices. Source materials add about 217 MiB before DMG compression.
- Xcode build-for-testing/full platform matrix remain CI-owned; local headless SwiftPM build and focused license tests passed. No native display interaction claim.

Remaining: publish/auto-merge, confirm CI and merge, complete LOO-466. Historical
remote GhosttyKit objects remain untouched; the app and future archive workflow
carry the required materials. No follow-up Task is needed for this accepted scope.
