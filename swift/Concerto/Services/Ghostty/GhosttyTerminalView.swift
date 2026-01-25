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

        if case .uninitialized = manager.state {
            manager.initialize()
        }

        return view
    }

    func updateNSView(_ nsView: GhosttyMetalView, context: Context) {
        nsView.sizeDidChange(size)

        if case .ready = manager.state, nsView.surface == nil, size.width > 0, size.height > 0 {
            nsView.createSurface(manager: manager)
        }
    }
}

// MARK: - GhosttyMetalView

final class GhosttyMetalView: NSView, @preconcurrency NSTextInputClient {
    var workingDirectory: String = ""
    var command: String?
    nonisolated(unsafe) var surface: ghostty_surface_t?

    private nonisolated(unsafe) var displayLink: CADisplayLink?
    private var trackingArea: NSTrackingArea?
    private var _markedText = NSMutableAttributedString()
    private var _markedRange = NSRange(location: NSNotFound, length: 0)
    private var _selectedRange = NSRange(location: 0, length: 0)

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
        layerContentsRedrawPolicy = .onSetNeedsDisplay
        autoresizingMask = [.width, .height]
    }

    func createSurface(manager: GhosttyManager) {
        guard surface == nil else { return }

        surface = manager.createSurface(
            workingDirectory: workingDirectory,
            command: command,
            view: self
        )

        if surface != nil {
            updateContentScale()
            updateSurfaceSize()
            setupDisplayLink()
            setupTrackingArea()
        }
    }

    private func setupDisplayLink() {
        let link = displayLink(target: self, selector: #selector(displayLinkFired))
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    @objc private func displayLinkFired(_ link: CADisplayLink) {
        guard let surface else { return }
        ghostty_surface_draw(surface)
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
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2.0
        ghostty_surface_set_content_scale(surface, scale, scale)
    }

    private func updateSurfaceSize() {
        guard let surface else { return }
        let backingSize = convertToBacking(bounds).size
        guard backingSize.width > 0, backingSize.height > 0 else { return }
        ghostty_surface_set_size(surface, UInt32(backingSize.width), UInt32(backingSize.height))
    }

    func sizeDidChange(_ newSize: CGSize) {
        guard newSize.width > 0, newSize.height > 0 else { return }
        setFrameSize(newSize)
        updateContentScale()
    }

    // MARK: - Keyboard Input

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard let surface else { return super.performKeyEquivalent(with: event) }

        let mods = event.modifierFlags
        // Handle Cmd+C (copy), Cmd+V (paste) ourselves
        if mods.contains(.command) {
            if event.charactersIgnoringModifiers == "c" {
                return copySelection()
            } else if event.charactersIgnoringModifiers == "v" {
                return pasteFromClipboard()
            }
        }

        // Let other command shortcuts through to the terminal (e.g., tmux prefix)
        let key = translateKey(event)
        return ghostty_surface_key(surface, key)
    }

    override func keyDown(with event: NSEvent) {
        guard let surface else {
            super.keyDown(with: event)
            return
        }

        // Use interpretKeyEvents for IME support
        interpretKeyEvents([event])

        // For non-IME keys, send directly to Ghostty
        if _markedRange.location == NSNotFound {
            var key = translateKey(event)

            // For control combinations, don't include text
            let mods = event.modifierFlags
            if mods.contains(.control) || mods.contains(.command) {
                _ = ghostty_surface_key(surface, key)
            } else if let chars = event.characters, !chars.isEmpty {
                chars.withCString { textPtr in
                    key.text = textPtr
                    _ = ghostty_surface_key(surface, key)
                }
            } else {
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
        guard let surface else {
            super.flagsChanged(with: event)
            return
        }

        // Detect which modifier changed and send appropriate key event
        var key = ghostty_input_key_s()
        key.mods = translateMods(event.modifierFlags)
        key.keycode = UInt32(event.keyCode)

        // Determine if this is a press or release based on the specific flag
        let isPress: Bool
        switch event.keyCode {
        case 0x39: // Caps Lock
            isPress = event.modifierFlags.contains(.capsLock)
        case 0x38, 0x3C: // Shift
            isPress = event.modifierFlags.contains(.shift)
        case 0x3B, 0x3E: // Control
            isPress = event.modifierFlags.contains(.control)
        case 0x3A, 0x3D: // Option
            isPress = event.modifierFlags.contains(.option)
        case 0x37, 0x36: // Command
            isPress = event.modifierFlags.contains(.command)
        default:
            isPress = true
        }

        key.action = isPress ? GHOSTTY_ACTION_PRESS : GHOSTTY_ACTION_RELEASE
        _ = ghostty_surface_key(surface, key)
    }

    // MARK: - NSTextInputClient

    func insertText(_ string: Any, replacementRange: NSRange) {
        guard let surface else { return }

        unmarkText()

        let text: String
        if let attrString = string as? NSAttributedString {
            text = attrString.string
        } else if let str = string as? String {
            text = str
        } else {
            return
        }

        text.withCString { ptr in
            ghostty_surface_text(surface, ptr, UInt(text.utf8.count))
        }
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        if let attrString = string as? NSAttributedString {
            _markedText = NSMutableAttributedString(attributedString: attrString)
        } else if let str = string as? String {
            _markedText = NSMutableAttributedString(string: str)
        }

        _selectedRange = selectedRange
        _markedRange = NSRange(location: 0, length: _markedText.length)

        // Send preedit to Ghostty
        if let surface, _markedText.length > 0 {
            _markedText.string.withCString { ptr in
                ghostty_surface_preedit(surface, ptr, UInt(_markedText.string.utf8.count))
            }
        }
    }

    func unmarkText() {
        _markedText = NSMutableAttributedString()
        _markedRange = NSRange(location: NSNotFound, length: 0)

        // Clear preedit
        if let surface {
            ghostty_surface_preedit(surface, nil, 0)
        }
    }

    func selectedRange() -> NSRange {
        _selectedRange
    }

    func markedRange() -> NSRange {
        _markedRange
    }

    func hasMarkedText() -> Bool {
        _markedRange.location != NSNotFound
    }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        nil
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] {
        []
    }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        guard let surface else { return .zero }

        var x: Double = 0, y: Double = 0, w: Double = 0, h: Double = 0
        ghostty_surface_ime_point(surface, &x, &y, &w, &h)

        let point = NSPoint(x: x, y: bounds.height - y - h)
        let windowPoint = convert(point, to: nil)
        let screenPoint = window?.convertPoint(toScreen: windowPoint) ?? windowPoint

        return NSRect(x: screenPoint.x, y: screenPoint.y, width: w, height: h)
    }

    func characterIndex(for point: NSPoint) -> Int {
        0
    }

    // MARK: - Mouse Input

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
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

    override func rightMouseDown(with event: NSEvent) {
        guard let surface else { return }

        // Send to terminal first
        let consumed = ghostty_surface_mouse_button(
            surface,
            GHOSTTY_MOUSE_PRESS,
            GHOSTTY_MOUSE_RIGHT,
            translateMods(event.modifierFlags)
        )

        // If terminal didn't consume it, show context menu
        if !consumed {
            showContextMenu(at: event.locationInWindow)
        }
    }

    override func rightMouseUp(with event: NSEvent) {
        guard let surface else { return }
        _ = ghostty_surface_mouse_button(
            surface,
            GHOSTTY_MOUSE_RELEASE,
            GHOSTTY_MOUSE_RIGHT,
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

    override func mouseExited(with event: NSEvent) {
        guard let surface else { return }
        // Send -1, -1 to indicate mouse left the view
        ghostty_surface_mouse_pos(surface, -1, -1, translateMods(event.modifierFlags))
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

    // MARK: - Context Menu

    private func showContextMenu(at point: NSPoint) {
        let menu = NSMenu()

        let copyItem = NSMenuItem(title: "Copy", action: #selector(copyAction), keyEquivalent: "c")
        copyItem.target = self
        menu.addItem(copyItem)

        let pasteItem = NSMenuItem(title: "Paste", action: #selector(pasteAction), keyEquivalent: "v")
        pasteItem.target = self
        menu.addItem(pasteItem)

        menu.addItem(NSMenuItem.separator())

        let clearItem = NSMenuItem(title: "Clear", action: #selector(clearAction), keyEquivalent: "")
        clearItem.target = self
        menu.addItem(clearItem)

        let screenPoint = convert(point, to: nil)
        menu.popUp(positioning: nil, at: screenPoint, in: self)
    }

    @objc private func copyAction() {
        _ = copySelection()
    }

    @objc private func pasteAction() {
        _ = pasteFromClipboard()
    }

    @objc private func clearAction() {
        guard let surface else { return }
        // Send clear screen sequence
        "clear\n".withCString { ptr in
            ghostty_surface_text(surface, ptr, 6)
        }
    }

    // MARK: - Copy/Paste

    private func copySelection() -> Bool {
        // The selection is handled by Ghostty's write_clipboard_cb callback
        // For now, return false to let the system handle it
        return false
    }

    private func pasteFromClipboard() -> Bool {
        guard let surface else { return false }

        guard let string = NSPasteboard.general.string(forType: .string) else {
            return false
        }

        string.withCString { ptr in
            ghostty_surface_text(surface, ptr, UInt(string.utf8.count))
        }
        return true
    }

    // MARK: - Key Translation

    private func translateKey(_ event: NSEvent) -> ghostty_input_key_s {
        var key = ghostty_input_key_s()
        key.action = GHOSTTY_ACTION_PRESS
        key.mods = translateMods(event.modifierFlags)
        key.consumed_mods = GHOSTTY_MODS_NONE
        key.keycode = UInt32(event.keyCode)
        key.text = nil
        key.composing = hasMarkedText()

        // Get unshifted codepoint for proper key identification
        if let chars = event.charactersIgnoringModifiers, let scalar = chars.unicodeScalars.first {
            key.unshifted_codepoint = scalar.value
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
