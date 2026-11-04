# libghostty-vt findings (2026-11-02, research agent, Ghostty a60e9e2)

Full report kept for LOO-710 and LOO-672's redesign. Key points:

- The library: terminal (vt_write, resize, 47 options incl. SCROLLBACK_MAX_BYTES,
  CONTINUATION_MAX_BYTES, WRITE_PTY callback that answers DSR/DA/DECRQM itself, and a
  MEMORY_USAGE data key), snapshot encode/decode (GHOSTSNP v1, CRC32C records, READY
  before history, history newest-first, incremental `next()`), formatter (PLAIN/VT/HTML;
  VT extras: cursor, SGR, OSC 8, Kitty keyboard, charsets, palette, modes, DECSTBM, tabstops,
  OSC 7 pwd), render state, search, selection, kitty graphics. No PTY, no threads, no fonts.
- The formatter paints the active screen only. Full-fidelity attach (scrollback, alt
  screen) means sending GHOSTSNP bytes to a libghostty-capable viewer, which is what Rex
  does (Mitchell Hashimoto confirmed in ghostty discussion #12176; gorex verified the
  record sequence against a real Rex server).
- GhosttyKit (the embedder API Desktop uses) exposes no snapshot or formatter calls.
  Desktop decoding an lf-written snapshot needs a new GhosttyKit patch
  (`ghostty_surface_snapshot_*`) on top of the five patches already carried.
- Gotchas: the C default `CONTINUATION_MAX_BYTES` is 0, and encoding a terminal that is
  mid-escape-sequence then fails with GHOSTTY_INVALID_VALUE; set it before feeding bytes.
  Default scrollback for a C-created terminal is about one page; set SCROLLBACK_MAX_BYTES
  explicitly (Ghostty's app default is 50 MB). A page is ~400 KB uncompressed, demand-paged;
  `ghostty_terminal_compress` shrinks idle history. All calls on one terminal must be
  serialized (callbacks run on the vt_write thread).
- Build: Zig 0.16.x pinned (`minimum_zig_version`). `zig build -Demit-lib-vt` yields
  `libghostty-vt.a` (static, libc-only or no-libc with `-Dsimd=false`), headers, pkg-config,
  and on Darwin an xcframework; `-Dvt-features=-all,+snapshot,+formatter` trims. CI covers
  macOS and Linux glibc/musl on both arches. No Rust binding in-tree; upstream
  `Uzaaft/libghostty-rs` runs Zig from build.rs; `mitchellh/go-libghostty` wraps the same API.
- Stability: every header says the API is not yet stable; snapshot format v1 carries no
  compatibility guarantee. Persisted snapshots must record the producing revision. MIT.
- Recommendation: prebuilt per-target archives produced by the existing
  `scripts/loopflow-dev.py ghostty-build` pipeline (add `-Demit-lib-vt`), linked statically
  by `build.rs` with a hand-written `extern "C"` module; keep Zig out of `cargo build` and
  CI. One pinned revision then governs both the app and `lf`.
