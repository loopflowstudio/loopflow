# libghostty-vt for lf

```sh
LOOPFLOW_ZIG=~/.local/zig/zig-aarch64-macos-0.16.0/zig \
  doppler run -- uv run python scripts/loopflow-dev.py ghostty-vt-build --publish
```

Builds `libghostty-vt.a` for the four release targets from the Ghostty revision
pinned in `../ghostty-vt.json`, packages each with its headers and license as
`libghostty-vt-<rev7>-<artifact>-<target>.tar.gz`, uploads them to the `bin` R2
bucket (`bin.loopflow.studio`, immutable; R2 credentials from Doppler), reads each
back, and writes the SHA-256 per target into the pin. Without `--publish` it only
builds and pins. Requires Zig 0.16.x (`LOOPFLOW_ZIG` or `zig` on PATH) and network
for Ghostty's dependency fetch. Zig cross-compiles every target from one machine.

`build.rs` downloads the pinned archive for its target with `curl` into `OUT_DIR`,
verifies the checksum, and links it statically into `lf`, the test binaries and
`loopflow-ffi`; no Zig on a machine that builds or runs `lf`. `LOOPFLOW_GHOSTTY_VT_DIR`
names a directory holding a local `libghostty-vt.a` instead (offline work, or
trying a Ghostty bump before publishing). A published name is immutable (the
bucket's edge caches it for a year, and a miss for four hours, so never fetch a
name before it is published): bump `artifact` in the pin whenever a patch or
build flag changes; a revision bump changes the name on its own. Zig output is
not byte-reproducible, so `--publish` keeps the checksum of a name the bucket
already holds instead of uploading over it.

Build flags: `-Demit-lib-vt -Dsimd=false -Dcpu=baseline -Dstrip=true
-Dvt-features=-all,+snapshot,+formatter,+selection`. No SIMD keeps C++ out of the
archive; baseline CPU keeps it portable; the feature set is what the Session owner
uses (`src/ghostty_vt.rs`). Desktop's GhosttyKit is built separately from the same
revision with `swift/GhosttyKitPatches`; it exports none of the `ghostty_terminal_*`
symbols, so both link into the app.

## Patches

`0001-vt-hyperlinks.patch`: the VT formatter emits OSC 8 around linked cells, as
the HTML formatter already does, so a repainted viewer keeps the program's
hyperlinks. Upstream notes this as future work. The build runs the formatter's
`hyperlink` tests before producing archives.
