# Terminal state

```bash
cargo test -p loopflow-terminal --lib
```

Ghostty parses output, snapshots terminal state and paints a reconnecting viewer.
This crate owns the safe FFI handle, repaint rules, native archive pin and patches.
Its build script verifies and links the archive for the selected target; ordinary
builds need no Zig. See [archive builds](ghostty-vt-patches/README.md) for updating
it or using `LOOPFLOW_GHOSTTY_VT_DIR` offline.

Session ownership, sockets, input control, screen-file persistence and native exit
settlement remain application responsibilities. They consume this crate directly.
The headless tests exercise snapshots, queries, keyboard modes, colors, links,
scrollback and redraw after attachment; application transport fixtures check the
lifecycle around that state.
