# Third-party notices

```sh
uv run python scripts/third_party.py refresh
uv run python scripts/third_party.py check
uv run pytest python/tests/test_third_party.py
# Inspect a final .app before signing, or the zip before uploading:
uv run python scripts/third_party.py verify path/to/Loopflow.app/Contents/Resources
uv run python scripts/third_party.py verify-archive path/to/GhosttyKit.xcframework.zip
```

Open **About Loopflow → Open Source Licenses** for offline, selectable notices.
Use Find in the notice window to locate a component. MPL-2.0 source archives are
in the app's `Contents/Resources/ThirdPartySources` directory.

`manifest.json` owns the audited GhosttyKit pin, non-Cargo sources, preserved
notices and exceptional crate license files absent from published crates.
`refresh` derives the Rust normal-dependency closure for the four release targets
from Cargo, includes native crate notices, and generates the app's notice document
and `inventory.json`. This conservative closure includes normal dependencies of
proc macros; it does not claim every included object survives linker dead stripping.
The framework inventory includes the static archive's native libraries, Zig code,
embedded fonts and runtime, rather than assuming Ghostty's MIT license covers all.
System frameworks are not redistributed. ViewInspector is used only by tests.

For a dependency/artifact update:

1. Inspect the exact new artifact and matching source/build inputs. Update the
   source URL and SHA-256, preserved full notices, glyph attributions, and any
   crate notice exceptions. `notice_provenance` records supplemental upstream
   license locations; `notices/` retains the reviewed text and its source paths.
2. For GhosttyKit, check embedded font bytes and their licenses, and audit the
   complete pinned `SharedDeps.zig` plus transitive sources. GhosttyKit builds
   with `-Di18n=false`, so it carries no GNU libintl; confirm `ar t` on each
   slice's `libghostty.a` lists no `gettext.o`. Update the manifest pin. Do not
   overwrite an already published binary; use the existing artifact publication
   workflow and a new artifact name when authorized.
3. Run `refresh`, review the generated diff, and run the checks above. Changes to
   lockfiles, fonts, themes, scripts or Ghostty patches invalidate the old inventory.
4. Package and verify an actual app/archive. Release, local app installation,
   Xcode assembly and future GhosttyKit builds invoke the same packager before
   signing/compression. Downloads are content-addressed and always checksum-checked.

Packaging ships every notice, plus the source archive of each MPL-2.0
component (z2d, option-ext), which that license requires be available to
recipients. A crate's license comes from Cargo; a manifest component's from its
`license` field, so a new MPL-2.0 GhosttyKit dependency needs one. Every other
license in the inventory needs its notice only: libpng's GPL files are
uncompiled examples, glslang's Bison output and Breakpad's configure scripts
carry exceptions, and the GPL shell integration ships as source with its license
text in the notices. A framework archive carries only GhosttyKit's MPL sources.
A failed download, stale inventory or missing source aborts packaging.
`contents.json` records hashes, and final-artifact verification requires the
notices and those sources.

Symbols Nerd Font 3.4.0 combines glyph sets. It is distributed under OFL with its
original MIT and component attributions retained; the MIT package label does not
replace OFL terms on the incorporated font software. No font bytes, functionality
or Loopflow license are changed. The font's notices record upstream glyph
remapping/scaling and reserved names; source licenses remain attached to their
respective material. This audit is packaging evidence, not legal certification.

The historical public lf3/lf7 archives are immutable and are not republished.
They statically combine GNU libintl; archives from lf8 on are built without it.

The iOS target links the same archive's `ios-arm64` and `ios-arm64-simulator`
slices and bundles the generated `NOTICES.txt` plus the font notices. Shared theme
licenses remain in the Loopflow framework's resources.
