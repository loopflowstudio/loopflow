# Rebuild with a modified libintl

```sh
# Copy ThirdPartySources out of the signed app before working on it.
uv run --python 3.12 python rebuild.py prepare ~/src/loopflow-rebuild
# Edit ~/src/loopflow-rebuild/gettext-0.24/gettext-runtime/intl as desired.
uv run --python 3.12 python rebuild.py build ~/src/loopflow-rebuild
```

Requires macOS, Xcode command line tools/SDK, Swift 6 and Rust/Cargo. Builds an
ad-hoc-signed `Loopflow Rebuilt.app` in the work directory. It does not install,
launch, notarize, or need Loopflow's signing keys. Cargo/SwiftPM may fetch their
locked build dependencies; registry source archives also accompany this package.

GhosttyKit statically combines GNU libintl 0.24 (LGPL-2.1-or-later) with other
libraries. `libintl-arm64.a` and `libintl-x86_64.a` are the same library's objects
extracted from the distributed combined library, uncombined with other libraries.
`gettext.tar.gz` contains its complete corresponding source; `ghostty.tar.gz`
contains the macOS configuration, headers and build recipe under `pkg/libintl/`.
The LGPL text is in the offline notices and gettext's `COPYING.LIB`.

`rebuild.py` compiles the library for both architectures, replaces only its
members in a copy of the packaged `GhosttyKit.zip`, and links the application's
source against that local framework. It embeds Sparkle.framework through the
adjacent `app_bundle.py`, which checks the app can load it. No new upstream binary
is needed. The application source, build scripts, interfaces, Ghostty patches and
Cargo lockfile are in `loopflow.tar.gz`. Other source archives are pinned by the adjacent
`ThirdPartyNotices/inventory.json` in the original app/framework. Changes to
Ghostty's library integration are documented in `GhosttyKitPatches/README.md`;
libintl's upstream `config.h` also documents its macOS adaptation. Loopflow makes
no additional changes to libintl's C source.

Modification for personal use and reverse engineering for debugging modifications
to libintl are permitted. This permission does not limit rights under the other
included licenses. After modifying the library, retain its notices and document
changes/date if redistributing. Rebuild the package's notices for changed inputs;
the original inventory describes the original distributed bytes.

For changes outside libintl, unpack the pinned source archives and use the
included source/build recipes. `uv run python scripts/loopflow-dev.py ghostty-build`
builds patched GhosttyKit with Zig 0.16.0 and macOS's Metal tools; it does not
publish it. For MPL-covered z2d and option-ext, their original source archives
accompany this package. Shell integration is already complete source in the app
resources and the Loopflow source archive, with original GPL/MIT headers intact.

Breakpad's packaged archive omits only its two prebuilt i386 crash-test executables,
which Apple rejects inside notarized apps. Their source and every other upstream
file remain unchanged. The inventory records the original archive hash, omitted
paths and the deterministic packaged archive hash.
