// SwiftUI view for embedding Ghostty terminal.
// Uses NSViewRepresentable to bridge the Metal-rendered terminal surface.

import SwiftUI
import AppKit

#if GHOSTTY_ENABLED
import GhosttyKit

struct GhosttyTerminalView: View {
    let workingDirectory: String
    let command: String?
    @ObservedObject var manager: GhosttyManager

    init(
        workingDirectory: String,
        command: String? = nil,
        manager: GhosttyManager = .shared
    ) {
        self.workingDirectory = workingDirectory
        self.command = command
        self.manager = manager
    }

    var body: some View {
        GeometryReader { geo in
            GhosttyTerminalRepresentable(
                workingDirectory: workingDirectory,
                command: command,
                size: geo.size,
                manager: manager
            )
        }
    }
}

struct GhosttyTerminalRepresentable: NSViewRepresentable {
    let workingDirectory: String
    let command: String?
    let size: CGSize
    @ObservedObject var manager: GhosttyManager

    func makeNSView(context: Context) -> GhosttyMetalView {
        let view = GhosttyMetalView()
        view.workingDirectory = workingDirectory
        view.command = command

        // Initialize manager if needed
        if case .uninitialized = manager.state {
            manager.initialize()
        }

        return view
    }

    func updateNSView(_ nsView: GhosttyMetalView, context: Context) {
        // Update size from SwiftUI
        nsView.sizeDidChange(size)

        // Create surface if manager is ready and we have a valid size
        if case .ready = manager.state, nsView.surface == nil, size.width > 0, size.height > 0 {
            nsView.createSurface(manager: manager)
        }
    }
}

final class GhosttyMetalView: NSView {
    var workingDirectory: String = ""
    var command: String?
    nonisolated(unsafe) var surface: ghostty_surface_t?

    private nonisolated(unsafe) var displayLink: CADisplayLink?
    private var trackingArea: NSTrackingArea?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupView()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setupView()
    }

    private func setupView() {
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor
        // Ensure layer resizes with view
        layerContentsRedrawPolicy = .onSetNeedsDisplay
        autoresizingMask = [.width, .height]
    }

    func createSurface(manager: GhosttyManager) {
        guard surface == nil else {
            print("[GhosttyMetalView] Surface already exists")
            return
        }

        print("[GhosttyMetalView] Creating surface...")
        print("[GhosttyMetalView] workingDirectory: \(workingDirectory)")
        print("[GhosttyMetalView] frame: \(frame)")

        surface = manager.createSurface(
            workingDirectory: workingDirectory,
            command: command,
            view: self
        )

        if let surface {
            print("[GhosttyMetalView] Surface created: \(surface)")
            // Set content scale and size immediately
            updateContentScale()
            updateSurfaceSize()
            setupDisplayLink()
            setupTrackingArea()
        } else {
            print("[GhosttyMetalView] Failed to create surface")
        }
    }

    private func setupDisplayLink() {
        print("[GhosttyMetalView] Setting up CADisplayLink...")

        let link = displayLink(target: self, selector: #selector(displayLinkFired))
        link.add(to: .main, forMode: .common)
        displayLink = link

        print("[GhosttyMetalView] CADisplayLink started")
    }

    @objc private func displayLinkFired(_ link: CADisplayLink) {
        draw()
    }

    private func setupTrackingArea() {
        let area = NSTrackingArea(
            rect: bounds,
            options: [.activeInKeyWindow, .mouseMoved, .mouseEnteredAndExited, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        trackingArea = area
    }

    private var drawCount = 0
    private func draw() {
        guard let surface else { return }
        drawCount += 1
        if drawCount <= 5 || drawCount % 60 == 0 {
            print("[GhosttyMetalView] draw() called, count: \(drawCount)")
        }
        ghostty_surface_draw(surface)
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea {
            removeTrackingArea(trackingArea)
        }
        setupTrackingArea()
    }

    override var acceptsFirstResponder: Bool { true }

    override func becomeFirstResponder() -> Bool {
        if let surface {
            ghostty_surface_set_focus(surface, true)
        }
        return super.becomeFirstResponder()
    }

    override func resignFirstResponder() -> Bool {
        if let surface {
            ghostty_surface_set_focus(surface, false)
        }
        return super.resignFirstResponder()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        updateContentScale()
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        updateSurfaceSize()
    }

    private func updateContentScale() {
        guard let surface else { return }
        // Use window scale if available, otherwise use main screen scale
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        ghostty_surface_set_content_scale(surface, scale, scale)
    }

    private func updateSurfaceSize() {
        guard let surface else { return }
        // Convert to backing (pixel) coordinates for retina displays
        let backingSize = convertToBacking(bounds).size
        guard backingSize.width > 0, backingSize.height > 0 else { return }
        ghostty_surface_set_size(surface, UInt32(backingSize.width), UInt32(backingSize.height))
    }

    func sizeDidChange(_ newSize: CGSize) {
        guard newSize.width > 0, newSize.height > 0 else { return }
        setFrameSize(newSize)
        // Also update content scale in case window changed
        updateContentScale()
    }

    // MARK: - Input Handling

    override func keyDown(with event: NSEvent) {
        guard let surface else {
            super.keyDown(with: event)
            return
        }

        let mods = event.modifierFlags

        // Don't pass text for control key combinations - let Ghostty handle them
        let hasControlMods = mods.contains(.control) || mods.contains(.command)

        if hasControlMods {
            // Control/command combinations: just send keycode + mods
            let key = translateKey(event)
            _ = ghostty_surface_key(surface, key)
        } else {
            // Regular keys: include the text
            let characters = event.characters ?? ""
            characters.withCString { textPtr in
                var key = translateKey(event)
                key.text = textPtr
                _ = ghostty_surface_key(surface, key)
            }
        }
    }

    override func keyUp(with event: NSEvent) {
        guard let surface else {
            super.keyUp(with: event)
            return
        }

        var key = translateKey(event)
        key.action = GHOSTTY_ACTION_RELEASE
        _ = ghostty_surface_key(surface, key)
    }

    override func flagsChanged(with event: NSEvent) {
        // Handle modifier key changes
        super.flagsChanged(with: event)
    }

    override func mouseDown(with event: NSEvent) {
        guard let surface else { return }
        _ = ghostty_surface_mouse_button(
            surface,
            GHOSTTY_MOUSE_PRESS,
            GHOSTTY_MOUSE_LEFT,
            translateMods(event.modifierFlags)
        )
    }

    override func mouseUp(with event: NSEvent) {
        guard let surface else { return }
        _ = ghostty_surface_mouse_button(
            surface,
            GHOSTTY_MOUSE_RELEASE,
            GHOSTTY_MOUSE_LEFT,
            translateMods(event.modifierFlags)
        )
    }

    override func mouseMoved(with event: NSEvent) {
        guard let surface else { return }
        let point = convert(event.locationInWindow, from: nil)
        let y = bounds.height - point.y
        ghostty_surface_mouse_pos(surface, point.x, y, translateMods(event.modifierFlags))
    }

    override func mouseDragged(with event: NSEvent) {
        mouseMoved(with: event)
    }

    override func scrollWheel(with event: NSEvent) {
        guard let surface else { return }

        var scrollMods: ghostty_input_scroll_mods_t = 0
        if event.hasPreciseScrollingDeltas {
            scrollMods |= 1 // GHOSTTY_SCROLL_MODS_PRECISE
        }

        ghostty_surface_mouse_scroll(
            surface,
            event.scrollingDeltaX,
            event.scrollingDeltaY,
            scrollMods
        )
    }

    private func translateKey(_ event: NSEvent) -> ghostty_input_key_s {
        var key = ghostty_input_key_s()
        key.action = GHOSTTY_ACTION_PRESS
        key.mods = translateMods(event.modifierFlags)
        key.consumed_mods = GHOSTTY_MODS_NONE
        key.keycode = UInt32(event.keyCode)
        key.text = nil
        key.unshifted_codepoint = 0
        key.composing = false

        return key
    }

    private func translateMods(_ flags: NSEvent.ModifierFlags) -> ghostty_input_mods_e {
        var mods = GHOSTTY_MODS_NONE.rawValue

        if flags.contains(.shift) {
            mods |= GHOSTTY_MODS_SHIFT.rawValue
        }
        if flags.contains(.control) {
            mods |= GHOSTTY_MODS_CTRL.rawValue
        }
        if flags.contains(.option) {
            mods |= GHOSTTY_MODS_ALT.rawValue
        }
        if flags.contains(.command) {
            mods |= GHOSTTY_MODS_SUPER.rawValue
        }
        if flags.contains(.capsLock) {
            mods |= GHOSTTY_MODS_CAPS.rawValue
        }

        return ghostty_input_mods_e(rawValue: mods)
    }

    deinit {
        displayLink?.invalidate()
        if let surface {
            ghostty_surface_free(surface)
        }
    }
}

#else

// Stub view when GhosttyKit is not available
struct GhosttyTerminalView: View {
    let workingDirectory: String
    let command: String?
    @ObservedObject var manager: GhosttyManager

    init(
        workingDirectory: String,
        command: String? = nil,
        manager: GhosttyManager = .shared
    ) {
        self.workingDirectory = workingDirectory
        self.command = command
        self.manager = manager
    }

    var body: some View {
        VStack {
            Image(systemName: "terminal")
                .font(.largeTitle)
                .foregroundStyle(.secondary)
            Text("Embedded terminal not available")
                .font(.headline)
            Text("Build GhosttyKit to enable this feature")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.black.opacity(0.9))
    }
}

#endif
