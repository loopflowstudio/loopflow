// SwiftUI view for embedding Ghostty terminal.
// Uses NSViewRepresentable to bridge the Metal-rendered terminal surface.

import SwiftUI
import AppKit

#if canImport(GhosttyKit)
import GhosttyKit

struct GhosttyTerminalView: NSViewRepresentable {
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

    func makeNSView(context: Context) -> GhosttyMetalView {
        let view = GhosttyMetalView()
        view.workingDirectory = workingDirectory
        view.command = command

        // Initialize manager if needed
        if case .uninitialized = manager.state {
            manager.initialize()
        }

        // Create surface when manager is ready
        if case .ready = manager.state {
            view.createSurface(manager: manager)
        }

        return view
    }

    func updateNSView(_ nsView: GhosttyMetalView, context: Context) {
        // Create surface if manager just became ready
        if case .ready = manager.state, nsView.surface == nil {
            nsView.createSurface(manager: manager)
        }
    }
}

final class GhosttyMetalView: NSView {
    var workingDirectory: String = ""
    var command: String?
    var surface: ghostty_surface_t?

    private var displayLink: CVDisplayLink?
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
    }

    func createSurface(manager: GhosttyManager) {
        guard surface == nil else { return }

        surface = manager.createSurface(
            workingDirectory: workingDirectory,
            command: command,
            view: self
        )

        if surface != nil {
            setupDisplayLink()
            setupTrackingArea()
        }
    }

    private func setupDisplayLink() {
        var link: CVDisplayLink?
        CVDisplayLinkCreateWithActiveCGDisplays(&link)
        guard let link else { return }

        let opaqueView = Unmanaged.passUnretained(self).toOpaque()
        CVDisplayLinkSetOutputCallback(link, { _, _, _, _, _, userdata -> CVReturn in
            guard let userdata else { return kCVReturnSuccess }
            let view = Unmanaged<GhosttyMetalView>.fromOpaque(userdata).takeUnretainedValue()
            DispatchQueue.main.async {
                view.draw()
            }
            return kCVReturnSuccess
        }, opaqueView)

        CVDisplayLinkStart(link)
        displayLink = link
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

    private func draw() {
        guard let surface else { return }
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
        guard let surface, let window else { return }
        let scale = window.backingScaleFactor
        ghostty_surface_set_content_scale(surface, scale, scale)
    }

    private func updateSurfaceSize() {
        guard let surface else { return }
        let size = bounds.size
        ghostty_surface_set_size(surface, UInt32(size.width), UInt32(size.height))
    }

    // MARK: - Input Handling

    override func keyDown(with event: NSEvent) {
        guard let surface else {
            super.keyDown(with: event)
            return
        }

        let key = translateKey(event)
        _ = ghostty_surface_key(surface, key)
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
        let point = convert(event.locationInWindow, from: nil)
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
        key.keycode = UInt32(event.keyCode)
        key.key = GHOSTTY_KEY_UNIDENTIFIED

        if let chars = event.characters, !chars.isEmpty {
            key.composing = false
        }

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
        if let displayLink {
            CVDisplayLinkStop(displayLink)
        }
        if let surface {
            GhosttyManager.shared.destroySurface(surface)
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
