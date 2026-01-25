# Embedded Terminal in Concerto (libghostty)

Embed Ghostty's terminal renderer in Concerto. Demo feature behind a flag—not shipping to users yet.

## What to build

A terminal view powered by libghostty that runs `lf` sessions inside Concerto. Keep "Open in Warp" as the default; this is opt-in via feature flag.

## Approach

Ghostty's macOS app is already Swift + libghostty (C API). We'll:
1. Vendor Ghostty as a git submodule
2. Build libghostty/xcframework using Zig
3. Create Swift bindings that wrap the C API
4. Embed a terminal view in Concerto's UI

This is exploratory. libghostty isn't officially released for embedding, but Ghostty's macOS app proves it works.

## Build integration

### Dependencies

- **Zig 0.15.2** (tip/development) — `brew install zig --HEAD` or use exact version
- **Xcode** with macOS SDK
- **gettext** — `brew install gettext`

### Vendoring Ghostty

```bash
# Add as submodule
git submodule add https://github.com/ghostty-org/ghostty.git vendor/ghostty

# Build libghostty (produces xcframework)
cd vendor/ghostty
zig build -Doptimize=ReleaseFast
```

The build produces `zig-out/` with the app and internal libraries. We need to figure out how to extract just the xcframework for embedding.

### Xcode integration

Option A: Link xcframework directly from `zig-out/`
Option B: Copy built framework to `swift/Frameworks/` and reference from Xcode project

TBD: Exact path to xcframework after build.

## Data structures

```swift
// Feature flag
enum FeatureFlags {
    static var embeddedTerminal: Bool {
        UserDefaults.standard.bool(forKey: "enableEmbeddedTerminal")
    }
}

// Terminal session
struct GhosttySession: Identifiable {
    let id: String
    let worktree: String
    let command: [String]
    var surface: ghostty_surface_t?
    var status: TerminalStatus
}

enum TerminalStatus {
    case initializing
    case running
    case completed(exitCode: Int32)
    case failed(error: String)
}
```

## Key functions

```swift
// GhosttyManager - wraps the C API
@MainActor
class GhosttyManager {
    private var app: ghostty_app_t?

    func initialize() throws {
        // Set up runtime config with callbacks
        var runtime = ghostty_runtime_config_s()
        runtime.wakeup_cb = { ... }
        runtime.action_cb = { ... }
        runtime.read_clipboard_cb = { ... }
        runtime.write_clipboard_cb = { ... }

        let config = ghostty_config_new()
        ghostty_config_load_default_files(config)
        ghostty_config_finalize(config)

        app = ghostty_app_new(&runtime, config)
    }

    func createSurface(workingDir: String) -> ghostty_surface_t? {
        var surfaceConfig = ghostty_surface_config_new()
        // Configure surface...
        return ghostty_surface_new(app, &surfaceConfig)
    }

    func tick() {
        ghostty_app_tick(app)
    }
}

// GhosttyView - NSViewRepresentable wrapper
struct GhosttyView: NSViewRepresentable {
    let session: GhosttySession

    func makeNSView(context: Context) -> GhosttyMetalView {
        // Create Metal-backed view for rendering
    }
}
```

## C API surface (from ghostty.h)

Key types we'll use:
- `ghostty_app_t` — application instance (one per app)
- `ghostty_config_t` — configuration
- `ghostty_surface_t` — terminal surface (one per terminal)
- `ghostty_runtime_config_s` — callbacks for clipboard, wakeup, actions

Key functions:
```c
ghostty_app_t ghostty_app_new(ghostty_runtime_config_s*, ghostty_config_t);
void ghostty_app_tick(ghostty_app_t);
ghostty_surface_t ghostty_surface_new(ghostty_app_t, ghostty_surface_config_s*);
void ghostty_surface_draw(ghostty_surface_t);
void ghostty_surface_key(ghostty_surface_t, ghostty_input_key_s);
```

## UI changes

### Feature flag

Add to Concerto settings (hidden/debug menu):
- "Enable Embedded Terminal (Experimental)"
- Default: OFF

### Where it goes

Replace OutputPanel when flag enabled:
- Current: 200px bottom panel with log lines
- New: Metal-rendered terminal view
- Fallback: Original log view when flag disabled

### Interaction flow

1. User has feature flag enabled
2. User clicks "Run" on a prompt
3. If embedded terminal enabled:
   - Create Ghostty surface in OutputPanel area
   - Run `lf <step> <args>` via PTY
   - Render output with Metal
4. If disabled (default):
   - Launch Warp as today (keep this working)

### Terminal appearance

- Match loopflow design system (cream/slate)
- Configure via Ghostty config or programmatically
- JetBrains Mono font
- 200-300px height, resizable

## Constraints

- **Zig build dependency** — CI needs Zig installed
- **Feature flag** — Must not break default experience
- **Keep Warp working** — "Open in Warp" remains the primary path
- **macOS only** — libghostty Metal renderer is macOS-specific
- **Build complexity** — Vendoring Ghostty adds ~80% Zig codebase

## Risks

| Risk | Mitigation |
|------|------------|
| libghostty API changes | We're using internal API; expect breakage. Feature flag lets us disable. |
| Build fails on CI | Add Zig to CI; fallback to disabling feature if too complex. |
| Performance issues | Metal should be fast; profile if issues. |
| Ghostty upstream breaks | Pin submodule to known-good commit. |

## Done when

```bash
# Build Ghostty submodule
cd vendor/ghostty && zig build -Doptimize=ReleaseFast

# Build Concerto
cd swift && xcodegen generate && xcodebuild -scheme Concerto

# Enable feature flag
defaults write com.loopflow.Concerto enableEmbeddedTerminal -bool true

# In Concerto:
# 1. Open a repo
# 2. Run a prompt
# 3. See Ghostty-rendered terminal in bottom panel
# 4. Disable flag, verify Warp still works
```

Verification:
- Terminal renders with Metal (GPU-accelerated)
- Can type input, see output
- Colors render correctly
- Feature flag disables cleanly
- "Open in Warp" still works when flag off

## Open questions

1. **Exact xcframework location** — Need to build and inspect `zig-out/`
2. **CI complexity** — Is Zig in GitHub Actions straightforward?
3. **Config theming** — How to apply loopflow colors to Ghostty?
4. **PTY management** — Does libghostty handle this or do we?

## Next steps

1. Add Ghostty submodule, attempt build
2. Find xcframework output path
3. Create minimal Swift bridging code
4. Get "hello world" terminal rendering
5. Integrate into Concerto behind flag
