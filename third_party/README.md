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
Use Find in the notice window to locate a component. Sources and rebuild tools
are in the app's `Contents/Resources/ThirdPartySources` directory.

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
2. For GhosttyKit, compare both archive member lists against `pkg/libintl/build.zig`,
   check embedded font bytes and their licenses, and audit the complete pinned
   `SharedDeps.zig` plus transitive sources. Update the manifest pin and object
   list. Do not overwrite an already published binary; use the existing artifact
   publication workflow and a new artifact name when authorized.
3. Run `refresh`, review the generated diff, and run the checks above. Changes to
   lockfiles, fonts, themes, scripts or Ghostty patches invalidate the old inventory.
4. Package and verify an actual app/archive. Release, local app installation,
   Xcode assembly and future GhosttyKit builds invoke the same packager before
   signing/compression. Downloads are content-addressed and always checksum-checked.

Packaging includes pinned dependency source archives, an exact copy of the
framework for relinking, the current Loopflow source/build inputs, and standalone
libintl archives extracted from both framework slices. These are carried inside
the app/archive, without relying on an upstream source URL or a future written
offer. [REBUILD.md](REBUILD.md) gives the recipient's modification/relink procedure.
A failed download, stale inventory, changed object layout or missing source aborts
packaging. `contents.json` records hashes, and final-artifact verification requires
all inventory sources, notices, rebuild tools and both uncombined libraries.

Symbols Nerd Font 3.4.0 combines glyph sets. It is distributed under OFL with its
original MIT and component attributions retained; the MIT package label does not
replace OFL terms on the incorporated font software. No font bytes, functionality
or Loopflow license are changed. The font's notices record upstream glyph
remapping/scaling and reserved names; source licenses remain attached to their
respective material. This audit is packaging evidence, not legal certification.

The historical public lf3/lf7 archives are immutable and are not republished by
this change. Current app packaging supplies the material they lack; future
GhosttyKit archives include it before publication.
