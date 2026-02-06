# Design: Linux + macOS Compatibility

## Goal

Make `lf` and `lfd` fully functional on Linux and macOS. No Windows support needed.

## Current State

The codebase is macOS-primary. CI already builds and tests Rust on `ubuntu-latest`, and the release workflow produces Linux binaries (x86_64, aarch64). But several runtime code paths call macOS-only tools with no Linux fallback.

## Findings

### Must Fix (breaks on Linux)

**1. Clipboard: `pbcopy` / `pbpaste` (3 call sites)**

| File | Line | Function | Direction |
|------|------|----------|-----------|
| `loopflow-engine/src/prompt.rs` | 913-923 | `read_clipboard()` | read (pbpaste) |
| `lf/src/commands/util.rs` | 18-34 | `copy_to_clipboard()` | write (pbcopy) |
| `lf/src/commands/ops/mod.rs` | 613-626 | `copy_to_clipboard()` | write (pbcopy) |

Linux equivalents: `xclip -selection clipboard`, `xsel --clipboard`, `wl-copy`/`wl-paste` (Wayland).

**2. URL open: hardcoded `open` command (1 call site)**

| File | Line | Function |
|------|------|----------|
| `lf/src/commands/util.rs` | 36-45 | `open_web_client()` |

Uses `Command::new("open")` unconditionally. The same function in `loopflow-ops` (`land.rs:288`, `pr.rs:336`) already has the correct `cfg!(target_os = "macos")` / `xdg-open` pattern.

### Already Handled

- `loopflow-ops/src/land.rs:288-295` — `open_url()` uses `cfg!(target_os)` correctly
- `loopflow-ops/src/pr.rs:336-343` — same pattern, correct
- `loopflow-py/build.rs` — linker flag only applied on macOS, correct
- `which` command — works on both OSes
- `dirs::home_dir()` — cross-platform crate, correct
- `git`, `gh`, `claude`, `codex`, `gemini` — all cross-platform CLIs

### Cosmetic (informational, not broken)

**3. Doctor install hints reference `brew` only**

`lf/src/commands/ops/mod.rs:663-712` prints install hints like `brew install node`, `brew install gh`, `brew install --cask warp`. On Linux these should suggest `apt`/`dnf`/`pacman` or just the tool's own install instructions.

This isn't a blocker — doctor is advisory and the tool itself still works. But it's confusing on Linux.

## Proposal

### Clipboard helper in `loopflow-engine`

Create a `clipboard` module in `loopflow-engine` since both `lf` and `loopflow-engine` need clipboard access.

```rust
// loopflow-engine/src/clipboard.rs

pub fn read() -> Option<String> {
    if cfg!(target_os = "macos") {
        read_via("pbpaste", &[])
    } else {
        // Try xclip first (X11), then xsel, then wl-paste (Wayland)
        read_via("xclip", &["-selection", "clipboard", "-o"])
            .or_else(|| read_via("xsel", &["--clipboard", "--output"]))
            .or_else(|| read_via("wl-paste", &[]))
    }
}

pub fn write(text: &str) -> Result<()> {
    if cfg!(target_os = "macos") {
        write_via("pbcopy", &[], text)
    } else {
        write_via("xclip", &["-selection", "clipboard"], text)
            .or_else(|_| write_via("xsel", &["--clipboard", "--input"], text))
            .or_else(|_| write_via("wl-copy", &[], text))
    }
}
```

Fallback chain: try common tools in order, fail gracefully. No new dependencies — just `Command`.

### Consolidate `open_url`

Three copies exist. Consolidate into one function in `loopflow-engine` (or a shared util):

```rust
pub fn open_url(url: &str) {
    let cmd = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let _ = Command::new(cmd).arg(url).status();
}
```

Call sites: `lf/src/commands/util.rs:open_web_client`, `loopflow-ops/src/pr.rs`, `loopflow-ops/src/land.rs`.

### Doctor hints

Make install hints OS-aware:

```rust
if cfg!(target_os = "macos") {
    println!("- gh: brew install gh");
} else {
    println!("- gh: https://cli.github.com/");
}
```

For macOS-only tools (Warp, Cursor) — skip them on Linux rather than suggesting brew.

## Changes

| File | Change |
|------|--------|
| `loopflow-engine/src/clipboard.rs` | New module: `read()`, `write()` |
| `loopflow-engine/src/lib.rs` | Add `pub mod clipboard;` |
| `loopflow-engine/src/prompt.rs` | Replace `read_clipboard()` with `clipboard::read()` |
| `lf/src/commands/util.rs` | Replace `copy_to_clipboard()` with `clipboard::write()`, fix `open_web_client()` |
| `lf/src/commands/ops/mod.rs` | Replace `copy_to_clipboard()` with `clipboard::write()`, make doctor hints OS-aware |
| `loopflow-ops/src/pr.rs` | Remove local `open_url()`, use shared version |
| `loopflow-ops/src/land.rs` | Remove local `open_url()`, use shared version |

## Scope

~100 lines of new code. No new crate dependencies. No behavior changes on macOS. Existing tests continue to pass (clipboard/url-open are side effects, not tested directly).

## Not in scope

- Windows support
- Wayland-first clipboard (Wayland support is covered by the `wl-copy`/`wl-paste` fallback)
- Linux service management for `lfd` (no launchd equivalent needed yet — daemon runs as a foreground process)
