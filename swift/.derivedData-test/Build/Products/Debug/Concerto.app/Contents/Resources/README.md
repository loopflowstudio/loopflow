# Ghostty Integration Guide

This document explains how Ghostty's macOS app integrates libghostty into SwiftUI, serving as a reference for our Concerto embedded terminal.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│ SwiftUI                                                      │
│  ┌─────────────────┐                                        │
│  │ Ghostty.Terminal│  SwiftUI View                          │
│  └────────┬────────┘                                        │
│           │                                                  │
│  ┌────────▼────────┐                                        │
│  │ SurfaceForApp   │  Creates SurfaceView per app           │
│  └────────┬────────┘                                        │
│           │                                                  │
│  ┌────────▼────────┐                                        │
│  │ SurfaceWrapper  │  Adds overlays (search, resize, etc)   │
│  └────────┬────────┘                                        │
│           │                                                  │
│  ┌────────▼─────────────┐                                   │
│  │ SurfaceRepresentable │  NSViewRepresentable bridge       │
│  └────────┬─────────────┘                                   │
└───────────┼─────────────────────────────────────────────────┘
            │
┌───────────▼─────────────────────────────────────────────────┐
│ AppKit                                                       │
│  ┌─────────────────┐                                        │
│  │ SurfaceView     │  NSView subclass                       │
│  │  - id: UUID     │                                        │
│  │  - surface      │  ghostty_surface_t                     │
│  │  - metalLayer   │  CAMetalLayer                          │
│  │  - displayLink  │  CVDisplayLink                         │
│  └────────┬────────┘                                        │
└───────────┼─────────────────────────────────────────────────┘
            │
┌───────────▼─────────────────────────────────────────────────┐
│ libghostty (C API)                                           │
│  ghostty_app_t      - App-level state, one per process      │
│  ghostty_config_t   - Configuration                          │
│  ghostty_surface_t  - Terminal surface (view)               │
└─────────────────────────────────────────────────────────────┘
```

## Key Components

### 1. Ghostty.App (Ghostty.App.swift)

The app-level wrapper. **One instance per process.**

```swift
class App: ObservableObject {
    enum Readiness { case loading, error, ready }

    @Published var readiness: Readiness = .loading
    @Published var app: ghostty_app_t? = nil
    @Published var config: Config

    init(configPath: String? = nil) {
        // 1. Create configuration
        self.config = Config(at: configPath)
        guard self.config.config != nil else {
            readiness = .error
            return
        }

        // 2. Set up runtime callbacks
        var runtime_cfg = ghostty_runtime_config_s(
            userdata: Unmanaged.passUnretained(self).toOpaque(),
            supports_selection_clipboard: true,
            wakeup_cb: { userdata in App.wakeup(userdata) },
            action_cb: { app, target, action in App.action(app!, target: target, action: action) },
            read_clipboard_cb: { ... },
            confirm_read_clipboard_cb: { ... },
            write_clipboard_cb: { ... },
            close_surface_cb: { ... }
        )

        // 3. Create the app
        guard let app = ghostty_app_new(&runtime_cfg, config.config) else {
            readiness = .error
            return
        }
        self.app = app
        self.readiness = .ready
    }
}
```

**Critical callbacks:**
- `wakeup_cb`: Called from any thread when libghostty needs attention. Must dispatch to main thread and call `ghostty_app_tick()`.
- `action_cb`: Called when libghostty wants the app to do something (new window, close, etc).
- `close_surface_cb`: Called when a surface should be closed.

### 2. Ghostty.Config (Ghostty.Config.swift)

Wraps `ghostty_config_t`:

```swift
class Config: ObservableObject {
    private(set) var config: ghostty_config_t? = nil

    init(at path: String? = nil, finalize: Bool = true) {
        // 1. Create config
        guard let cfg = ghostty_config_new() else { return }

        // 2. Load configuration files
        ghostty_config_load_default_files(cfg)
        ghostty_config_load_cli_args(cfg)
        ghostty_config_load_recursive_files(cfg)

        // 3. Finalize (populate defaults)
        if finalize {
            ghostty_config_finalize(cfg)
        }

        self.config = cfg
    }
}
```

### 3. Ghostty.SurfaceView (SurfaceView_AppKit.swift)

The NSView that renders the terminal. **One per terminal pane.**

```swift
class SurfaceView: NSView, ObservableObject {
    let id: UUID
    var surface: ghostty_surface_t?
    private var metalLayer: CAMetalLayer?

    init(_ app: ghostty_app_t, config: SurfaceConfiguration = .init()) {
        // 1. Set up Metal layer
        self.wantsLayer = true
        self.layer = CAMetalLayer()

        // 2. Create surface config
        var surfaceConfig = ghostty_surface_config_new()
        surfaceConfig.userdata = Unmanaged.passUnretained(self).toOpaque()
        surfaceConfig.platform_tag = GHOSTTY_PLATFORM_MACOS
        surfaceConfig.platform = ghostty_platform_u(macos: ghostty_platform_macos_s(
            nsview: Unmanaged.passUnretained(self).toOpaque()
        ))
        surfaceConfig.scale_factor = NSScreen.main!.backingScaleFactor

        // 3. Create surface
        guard let surface = ghostty_surface_new(app, &surfaceConfig) else {
            self.error = GhosttyError.surfaceCreateError
            return
        }
        self.surface = surface
    }
}
```

**Key responsibilities:**
- **Rendering**: Metal layer + CVDisplayLink for 60fps rendering
- **Input**: `keyDown`, `keyUp`, `flagsChanged`, `mouseDown`, `scrollWheel`, etc.
- **Size changes**: `ghostty_surface_set_size()` when view resizes
- **Focus**: `ghostty_surface_set_focus()` on focus changes

### 4. SwiftUI Integration (SurfaceView.swift)

The bridge from AppKit to SwiftUI:

```swift
// High-level terminal view
struct Terminal: View {
    @EnvironmentObject private var ghostty: Ghostty.App

    var body: some View {
        if let app = self.ghostty.app {
            SurfaceForApp(app) { surfaceView in
                SurfaceWrapper(surfaceView: surfaceView)
            }
        }
    }
}

// NSViewRepresentable bridge
struct SurfaceRepresentable: NSViewRepresentable {
    let view: SurfaceView
    let size: CGSize

    func makeNSView(context: Context) -> SurfaceScrollView {
        return SurfaceScrollView(contentSize: size, surfaceView: view)
    }
}
```

## Initialization Sequence

```
1. App startup
   │
2. Create Ghostty.App (singleton)
   │
   ├── ghostty_config_new()
   ├── ghostty_config_load_default_files()
   ├── ghostty_config_finalize()
   └── ghostty_app_new(&runtime_cfg, config)
   │
3. For each terminal window/tab/split:
   │
   ├── Create SurfaceView
   │   ├── Set up CAMetalLayer
   │   ├── ghostty_surface_config_new()
   │   │   └── Set platform, userdata, scale_factor
   │   └── ghostty_surface_new(app, &config)
   │
   └── Display in SwiftUI via SurfaceRepresentable

4. Render loop (CVDisplayLink):
   │
   └── ghostty_surface_draw() on each frame

5. Input handling:
   │
   ├── keyDown → ghostty_surface_key()
   ├── mouseDown → ghostty_surface_mouse_button()
   └── scrollWheel → ghostty_surface_mouse_scroll()
```

## Critical Implementation Details

### Threading

- `wakeup_cb` is called from **any thread**. Must use `DispatchQueue.main.async` to call `ghostty_app_tick()`.
- All other libghostty calls should be on the main thread.

### Memory Management

- `ghostty_config_t` - freed with `ghostty_config_free()`
- `ghostty_app_t` - freed with `ghostty_app_free()`
- `ghostty_surface_t` - freed with `ghostty_surface_free()`
- Use `didSet` observers to free old values when properties change.

### Platform Config

For macOS, the surface config must include:

```swift
surfaceConfig.platform_tag = GHOSTTY_PLATFORM_MACOS
surfaceConfig.platform = ghostty_platform_u(macos: ghostty_platform_macos_s(
    nsview: Unmanaged.passUnretained(self).toOpaque()
))
surfaceConfig.scale_factor = NSScreen.main!.backingScaleFactor
```

### Userdata Pattern

libghostty uses userdata pointers for callbacks. The pattern is:

```swift
// Store self as userdata
config.userdata = Unmanaged.passUnretained(self).toOpaque()

// Retrieve in callback
let manager = Unmanaged<GhosttyManager>.fromOpaque(userdata!).takeUnretainedValue()
```

## Our Integration (Concerto)

Components:

1. **GhosttyManager** - Singleton wrapping the app-level state, handles initialization and surface creation
2. **GhosttyTerminalView** - SwiftUI view using NSViewRepresentable to bridge GhosttyMetalView
3. **GhosttyMetalView** - NSView subclass with NSTextInputClient for IME, input handling, context menu

Features implemented:
- CADisplayLink for rendering (modern macOS 14+ API)
- Full keyboard input with Ctrl+C/D/Z, Esc, tmux support
- NSTextInputClient for IME/composition (Japanese, Korean, etc.)
- Right-click context menu with Copy/Paste/Clear
- Cmd+C/V for copy/paste
- Mouse tracking with exit detection
- Loopflow cream-on-burgundy color scheme

## File Structure

```
Concerto/Services/Ghostty/
├── README.md           # This file
├── GhosttyManager.swift    # App-level wrapper (like Ghostty.App)
├── GhosttyTerminalView.swift # SwiftUI view (like SurfaceRepresentable)
└── GhosttyTypes.swift      # Data types
```

## References

- Ghostty source: `vendor/ghostty/macos/Sources/Ghostty/`
- libghostty header: `vendor/ghostty/include/ghostty.h`
- GhosttyKit xcframework: `vendor/ghostty/macos/GhosttyKit.xcframework/`
