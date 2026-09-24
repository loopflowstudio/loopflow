// SwiftUI view for embedding Ghostty terminal.
// Uses NSViewRepresentable to bridge the Metal-rendered terminal surface.

import SwiftUI
import AppKit
import QuartzCore
import Loopflow

#if GHOSTTY_ENABLED
import GhosttyKit

struct GhosttyTerminalView: View {
    let workingDirectory: String
    let argv: [String]
    let env: [String: String]
    let terminal: TerminalIdentity
    let surfacePool: GhosttySurfacePool?
    let isFocused: Bool
    let onSurfaceCreated: () -> Void
    let onFocus: () -> Void
    @ObservedObject var manager: GhosttyManager

    init(
        workingDirectory: String,
        argv: [String] = [],
        env: [String: String] = [:],
        terminal: TerminalIdentity,
        surfacePool: GhosttySurfacePool? = nil,
        isFocused: Bool = false,
        onSurfaceCreated: @escaping () -> Void = {},
        onFocus: @escaping () -> Void = {},
        manager: GhosttyManager = .shared
    ) {
        self.workingDirectory = workingDirectory
        self.argv = argv
        self.env = env
        self.terminal = terminal
        self.surfacePool = surfacePool
        self.isFocused = isFocused
        self.onSurfaceCreated = onSurfaceCreated
        self.onFocus = onFocus
        self.manager = manager
    }

    var body: some View {
        GeometryReader { geo in
            GhosttyTerminalRepresentable(
                workingDirectory: workingDirectory,
                command: shellCommand,
                terminal: terminal,
                surfacePool: surfacePool,
                isFocused: isFocused,
                onSurfaceCreated: onSurfaceCreated,
                onFocus: onFocus,
                size: geo.size,
                manager: manager
            )
        }
    }

    private var shellCommand: String? {
        buildGhosttyShellCommand(argv: argv, env: env)
    }
}

struct GhosttyTerminalRepresentable: NSViewRepresentable {
    @Environment(\.isEnabled) private var isEnabled
    let workingDirectory: String
    let command: String?
    let terminal: TerminalIdentity
    let surfacePool: GhosttySurfacePool?
    let isFocused: Bool
    let onSurfaceCreated: () -> Void
    let onFocus: () -> Void
    let size: CGSize
    @ObservedObject var manager: GhosttyManager

    func makeNSView(context: Context) -> GhosttyMetalView {
        let view: GhosttyMetalView
        if let surfacePool {
            view = surfacePool.view(for: terminal)
        } else {
            view = GhosttyMetalView(terminal: terminal)
        }
        view.workingDirectory = workingDirectory
        view.command = command
        view.onSurfaceCreated = onSurfaceCreated
        view.onFocus = onFocus

        if case .uninitialized = manager.state {
            manager.initialize()
        }

        return view
    }

    func updateNSView(_ nsView: GhosttyMetalView, context: Context) {
        nsView.onFocus = onFocus
        nsView.sizeDidChange(size)

        if case .ready = manager.state, nsView.surface == nil, !nsView.childExited,
           size.width > 0, size.height > 0 {
            nsView.createSurface(manager: manager)
        }
        nsView.setFocusRequested(isEnabled && isFocused)
    }
}

/// Retains live terminal views for one window's Sessions workspace so pane
/// churn and Sessions↔Work navigation never free a surface. Each window owns
/// its own pool: an NSView can only live in one view hierarchy, so sharing a
/// pool across windows would silently steal terminals between them.
@MainActor
final class GhosttySurfacePool {
    private var views: [TerminalIdentity: GhosttyMetalView] = [:]

    func view(for id: TerminalIdentity) -> GhosttyMetalView {
        if let existing = views[id] { return existing }
        let view = GhosttyMetalView(terminal: id)
        view.pool = self
        views[id] = view
        return view
    }

    /// True only while the surface's child is still running. A provider killed
    /// from another client (Move here in Warp or a second window) leaves the
    /// surface displaying an exit banner; that must not read as live, and the
    /// dead surface is freed so an explicit reopen relaunches.
    func hasSurface(_ id: TerminalIdentity) -> Bool {
        guard let view = views[id], let surface = view.surface else { return false }
        if ghostty_surface_process_exited(surface) {
            Task { @MainActor in view.handleSurfaceClose() }
            return false
        }
        return true
    }

    /// Drops a view whose child exited so a later reopen mints a fresh view
    /// instead of reviving a dead one.
    func discard(_ view: GhosttyMetalView) {
        guard views[view.terminal] === view else { return }
        views.removeValue(forKey: view.terminal)
    }

    func release(_ id: TerminalIdentity) {
        views.removeValue(forKey: id)?.destroySurface()
    }

    func focus(_ id: TerminalIdentity) {
        guard let view = views[id] else { return }
        view.window?.makeFirstResponder(view)
    }
}

// MARK: - GhosttyMetalView

struct GhosttyCommandBlockLayout: Equatable {
    let id: UInt64
    let startRow: Int
    let endRow: Int

    func contains(_ row: Int) -> Bool {
        startRow...endRow ~= row
    }
}

func ghosttyCommandBlock(
    atViewportRow row: Int,
    in blocks: [GhosttyCommandBlockLayout]
) -> GhosttyCommandBlockLayout? {
    blocks.first { $0.contains(row) }
}

func ghosttyViewportRow(
    at point: CGPoint,
    bounds: CGRect,
    rows: Int,
    cellHeight: CGFloat
) -> Int? {
    guard rows > 0, cellHeight > 0 else { return nil }
    let contentHeight = CGFloat(rows) * cellHeight
    let topInset = max(0, (bounds.height - contentHeight) / 2)
    let distanceFromTop = bounds.height - point.y - topInset
    guard distanceFromTop >= 0, distanceFromTop < contentHeight else { return nil }
    return Int(distanceFromTop / cellHeight)
}

func ghosttyCommandBlockFrame(
    _ block: GhosttyCommandBlockLayout,
    bounds: CGRect,
    rows: Int,
    cellHeight: CGFloat
) -> CGRect {
    let contentHeight = CGFloat(rows) * cellHeight
    let topInset = max(0, (bounds.height - contentHeight) / 2)
    let top = bounds.height - topInset - CGFloat(block.startRow) * cellHeight
    let bottom = bounds.height - topInset - CGFloat(block.endRow + 1) * cellHeight
    return CGRect(
        x: 4,
        y: bottom + 1,
        width: max(0, bounds.width - 8),
        height: max(0, top - bottom - 2)
    )
}

@MainActor
final class GhosttyMetalView: NSView, @preconcurrency NSTextInputClient {
    var workingDirectory: String = ""
    var command: String?
    let terminal: TerminalIdentity
    var onSurfaceCreated: () -> Void = {}
    var onFocus: () -> Void = {}
    weak var pool: GhosttySurfacePool?
    /// Set when the surface's child ended; blocks implicit relaunch — reopening
    /// a Session or shell is an explicit action that mints a fresh view.
    private(set) var childExited = false
    nonisolated(unsafe) var surface: ghostty_surface_t?

    private nonisolated(unsafe) var displayLink: CADisplayLink?
    private var trackingArea: NSTrackingArea?
    private var _markedText = NSMutableAttributedString()
    private var _markedRange = NSRange(location: NSNotFound, length: 0)
    private var _selectedRange = NSRange(location: 0, length: 0)
    private var _didInsertText = false
    private var _currentKeyEventModifiers: NSEvent.ModifierFlags = []
    private var dropHighlight: NSView?
    private let commandBlockOverlay = CALayer()
    private var commandBlocks: [GhosttyCommandBlockLayout] = []
    private var hoveredCommandBlock: GhosttyCommandBlockLayout?
    private var selectedCommandBlock: (id: UInt64, text: String)?
    private var commandBlockMouseDown = false
    private var lastCommandBlockRefresh: CFTimeInterval = 0
    private var focusRequested = false

    init(terminal: TerminalIdentity, frame frameRect: NSRect = .zero) {
        self.terminal = terminal
        super.init(frame: frameRect)
        setupView()
    }

    required init?(coder: NSCoder) {
        // Terminal identity comes from its application owner, never a nib.
        return nil
    }

    private func setupView() {
        wantsLayer = true
        layer?.backgroundColor = NSColor.loopflowDarkBackground.cgColor
        layerContentsRedrawPolicy = .onSetNeedsDisplay
        autoresizingMask = [.width, .height]
        registerForDraggedTypes(ghosttyDropTypes)
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
            installCommandBlockOverlay()
            if window != nil { setupDisplayLink() }
            setupTrackingArea()
            onSurfaceCreated()
        }
    }

    private func setupDisplayLink() {
        let link = displayLink(target: self, selector: #selector(displayLinkFired))
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil {
            displayLink?.invalidate()
            displayLink = nil
        } else if surface != nil, displayLink == nil {
            setupDisplayLink()
            updateContentScale()
            updateSurfaceSize()
        }
        if focusRequested { window?.makeFirstResponder(self) }
    }

    func setFocusRequested(_ requested: Bool) {
        let changed = focusRequested != requested
        focusRequested = requested
        if requested && changed {
            // Attachment handles a request made before the view has a window.
            // Later polls and resizes must leave the search field's focus alone.
            window?.makeFirstResponder(self)
        } else if !requested, window?.firstResponder === self {
            window?.makeFirstResponder(nil)
        }
    }

    func destroySurface() {
        // A released view may still be mounted until SwiftUI reconciles it.
        // Only an explicit reopen with a fresh view may launch another child.
        childExited = true
        displayLink?.invalidate()
        displayLink = nil
        commandBlockOverlay.removeFromSuperlayer()
        commandBlocks = []
        hoveredCommandBlock = nil
        selectedCommandBlock = nil
        commandBlockMouseDown = false

        if let trackingArea {
            removeTrackingArea(trackingArea)
            self.trackingArea = nil
        }

        guard let surface else { return }
        self.surface = nil
        ghostty_surface_free(surface)
    }

    /// The surface's child process ended — a provider exit, a shell exit, or an
    /// external `--replace` takeover. Frees the dead surface so pool and
    /// Session state stop reporting it live, and announces the closure so the
    /// UI can reclassify immediately instead of waiting for the next poll.
    func handleSurfaceClose() {
        guard surface != nil else { return }
        destroySurface()
        pool?.discard(self)
        NotificationCenter.default.post(name: .ghosttySurfaceClosed, object: terminal)
    }

    @objc private func displayLinkFired(_ link: CADisplayLink) {
        guard let surface,
              window != nil,
              !isHiddenOrHasHiddenAncestor,
              window?.occlusionState.contains(.visible) != false
        else { return }
        ghostty_surface_draw(surface)
        let now = CACurrentMediaTime()
        if isShellPane, now - lastCommandBlockRefresh >= 0.1 {
            lastCommandBlockRefresh = now
            refreshCommandBlocks()
        }
    }

    private var isShellPane: Bool {
        if case .shell = terminal { return true }
        return false
    }

    private func installCommandBlockOverlay() {
        guard isShellPane, let rootLayer = layer else { return }
        commandBlockOverlay.removeFromSuperlayer()
        commandBlockOverlay.frame = bounds
        commandBlockOverlay.autoresizingMask = [.layerWidthSizable, .layerHeightSizable]
        commandBlockOverlay.masksToBounds = true
        commandBlockOverlay.zPosition = 20
        commandBlockOverlay.contentsScale = window?.backingScaleFactor ?? 2
        rootLayer.addSublayer(commandBlockOverlay)
        refreshCommandBlocks()
    }

    private func refreshCommandBlocks() {
        guard isShellPane, let surface else { return }
        let size = ghostty_surface_size(surface)
        let capacity = max(Int(size.rows), 1)
        var rawBlocks = Array(repeating: ghostty_command_block_s(), count: capacity)
        let count = rawBlocks.withUnsafeMutableBufferPointer { buffer in
            ghostty_surface_command_blocks(surface, buffer.baseAddress, buffer.count)
        }
        let nextBlocks = rawBlocks.prefix(min(count, rawBlocks.count)).map {
            GhosttyCommandBlockLayout(
                id: $0.id,
                startRow: Int($0.start_row),
                endRow: Int($0.end_row)
            )
        }
        guard nextBlocks != commandBlocks || commandBlockOverlay.frame != bounds else { return }
        commandBlocks = nextBlocks
        if let selectedCommandBlock,
           !commandBlocks.contains(where: { $0.id == selectedCommandBlock.id }) {
            self.selectedCommandBlock = nil
        }
        if let hoveredCommandBlock,
           !commandBlocks.contains(where: {
               $0.startRow == hoveredCommandBlock.startRow
                   && $0.endRow == hoveredCommandBlock.endRow
           }) {
            self.hoveredCommandBlock = nil
        }
        renderCommandBlocks(size: size)
    }

    private func renderCommandBlocks(size: ghostty_surface_size_s? = nil) {
        guard isShellPane, let surface else { return }
        let surfaceSize = size ?? ghostty_surface_size(surface)
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        let cellHeight = CGFloat(surfaceSize.cell_height_px) / scale
        let rowCount = Int(surfaceSize.rows)

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        commandBlockOverlay.frame = bounds
        commandBlockOverlay.sublayers = commandBlocks.map { block in
            let frame = ghosttyCommandBlockFrame(
                block,
                bounds: bounds,
                rows: rowCount,
                cellHeight: cellHeight
            )
            let hovered = hoveredCommandBlock.map {
                $0.startRow == block.startRow && $0.endRow == block.endRow
            } ?? false
            let selected = selectedCommandBlock?.id == block.id

            let surfaceLayer = CALayer()
            surfaceLayer.frame = frame
            surfaceLayer.cornerRadius = 3
            surfaceLayer.backgroundColor = commandBlockColor(
                selected: selected,
                hovered: hovered
            ).cgColor

            let accentLayer = CALayer()
            accentLayer.frame = CGRect(x: 0, y: 0, width: 3, height: frame.height)
            accentLayer.cornerRadius = 1.5
            accentLayer.backgroundColor = commandBlockAccentColor(
                selected: selected,
                hovered: hovered
            ).cgColor
            surfaceLayer.addSublayer(accentLayer)
            return surfaceLayer
        }
        CATransaction.commit()
    }

    private func commandBlockColor(selected: Bool, hovered: Bool) -> NSColor {
        if selected {
            return NSColor(red: 0x72 / 255, green: 0x2F / 255, blue: 0x37 / 255, alpha: 0.28)
        }
        if hovered {
            return NSColor(red: 0x72 / 255, green: 0x2F / 255, blue: 0x37 / 255, alpha: 0.16)
        }
        return NSColor.white.withAlphaComponent(0.045)
    }

    private func commandBlockAccentColor(selected: Bool, hovered: Bool) -> NSColor {
        if selected {
            return NSColor(red: 0xB8 / 255, green: 0x62 / 255, blue: 0x6C / 255, alpha: 0.95)
        }
        if hovered {
            return NSColor(red: 0x8B / 255, green: 0x3D / 255, blue: 0x47 / 255, alpha: 0.8)
        }
        return NSColor.white.withAlphaComponent(0.16)
    }

    private func viewportRow(at point: CGPoint) -> Int? {
        guard let surface else { return nil }
        let size = ghostty_surface_size(surface)
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        return ghosttyViewportRow(
            at: point,
            bounds: bounds,
            rows: Int(size.rows),
            cellHeight: CGFloat(size.cell_height_px) / scale
        )
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
        let accepted = super.becomeFirstResponder()
        if accepted, let surface {
            ghostty_surface_set_focus(surface, true)
        }
        return accepted
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
        if isShellPane { refreshCommandBlocks() }
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
        // AppKit visits sibling views for key equivalents; only the keyboard
        // responder may consume a terminal shortcut or send it to its PTY.
        guard window?.firstResponder === self else { return false }
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

        // Let other command shortcuts through to the terminal
        let key = translateKey(event)
        return ghostty_surface_key(surface, key)
    }

    override func keyDown(with event: NSEvent) {
        guard let surface else {
            super.keyDown(with: event)
            return
        }

        if ghosttyShouldHandleKeyDownDirectly(
            characters: event.characters,
            modifiers: event.modifierFlags,
            hasMarkedText: hasMarkedText(),
            selectedInputSource: inputContext?.selectedKeyboardInputSource
        ) {
            sendKeyPress(to: surface, event: event)
            return
        }

        // Reserve interpretKeyEvents for composition-capable input paths.
        // If insertText or setMarkedText fires, it already sent the text/preedit state.
        _currentKeyEventModifiers = event.modifierFlags
        defer { _currentKeyEventModifiers = [] }
        _didInsertText = false
        interpretKeyEvents([event])

        if _didInsertText || hasMarkedText() { return }

        sendKeyPress(to: surface, event: event)
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

        let text: String
        if let attrString = string as? NSAttributedString {
            text = attrString.string
        } else if let str = string as? String {
            text = str
        } else {
            return
        }

        if ghosttyShouldHandleTextAsKeyEvent(text, modifiers: _currentKeyEventModifiers) {
            return
        }

        _didInsertText = true
        unmarkText()

        text.withCString { ptr in
            ghostty_surface_text(surface, ptr, UInt(text.utf8.count))
        }
    }

    override func doCommand(by selector: Selector) {
        _ = selector
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
        focusForPointerInput()
        guard let surface else { return }
        let point = convert(event.locationInWindow, from: nil)
        let selectionModifiers: NSEvent.ModifierFlags = [.command, .control, .option, .shift]
        if isShellPane,
           event.modifierFlags.intersection(selectionModifiers).isEmpty,
           let row = viewportRow(at: point),
           let block = ghosttyCommandBlock(atViewportRow: row, in: commandBlocks),
           let text = readCommandBlock(surface: surface, viewportRow: row) {
            commandBlockMouseDown = true
            selectedCommandBlock = (id: block.id, text: text)
            renderCommandBlocks()
            return
        }
        clearCommandBlockSelection()
        _ = ghostty_surface_mouse_button(
            surface,
            GHOSTTY_MOUSE_PRESS,
            GHOSTTY_MOUSE_LEFT,
            translateMods(event.modifierFlags)
        )
    }

    override func mouseUp(with event: NSEvent) {
        if commandBlockMouseDown {
            commandBlockMouseDown = false
            return
        }
        guard let surface else { return }
        _ = ghostty_surface_mouse_button(
            surface,
            GHOSTTY_MOUSE_RELEASE,
            GHOSTTY_MOUSE_LEFT,
            translateMods(event.modifierFlags)
        )
    }

    override func rightMouseDown(with event: NSEvent) {
        focusForPointerInput()
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
        if isShellPane {
            let nextHover = viewportRow(at: point).flatMap {
                ghosttyCommandBlock(atViewportRow: $0, in: commandBlocks)
            }
            if nextHover != hoveredCommandBlock {
                hoveredCommandBlock = nextHover
                renderCommandBlocks()
            }
        }
        let y = bounds.height - point.y
        ghostty_surface_mouse_pos(surface, point.x, y, translateMods(event.modifierFlags))
    }

    override func mouseDragged(with event: NSEvent) {
        if commandBlockMouseDown { return }
        mouseMoved(with: event)
    }

    override func mouseExited(with event: NSEvent) {
        if hoveredCommandBlock != nil {
            hoveredCommandBlock = nil
            renderCommandBlocks()
        }
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
        let clearCmd = "clear\n"
        clearCmd.withCString { ptr in
            ghostty_surface_text(surface, ptr, UInt(clearCmd.utf8.count))
        }
    }

    // MARK: - Copy/Paste

    private func copySelection() -> Bool {
        if let selectedCommandBlock {
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            return pasteboard.setString(selectedCommandBlock.text, forType: .string)
        }
        guard let surface else { return false }
        var text = ghostty_text_s()
        guard ghostty_surface_read_selection(surface, &text) else { return false }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let bytes = text.text, text.text_len > 0 else { return false }

        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        let selection = String(decoding: Data(bytes: bytes, count: Int(text.text_len)), as: UTF8.self)
        return pasteboard.setString(selection, forType: .string)
    }

    private func focusForPointerInput() {
        onFocus()
        window?.makeFirstResponder(self)
    }

    private func clearCommandBlockSelection() {
        guard selectedCommandBlock != nil else { return }
        selectedCommandBlock = nil
        renderCommandBlocks()
    }

    private func readCommandBlock(surface: ghostty_surface_t, viewportRow: Int) -> String? {
        var text = ghostty_text_s()
        guard ghostty_surface_read_command_block(surface, UInt16(viewportRow), &text) else {
            return nil
        }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let bytes = text.text, text.text_len > 0 else { return nil }
        return String(decoding: Data(bytes: bytes, count: Int(text.text_len)), as: UTF8.self)
    }

    private func pasteFromClipboard() -> Bool {
        if case .session = terminal,
           ghosttyPrefersImageShortcut(NSPasteboard.general) {
            return sendImagePasteShortcut()
        }
        guard let text = terminalPasteText(from: NSPasteboard.general) else { return false }
        return insertTerminalText(text)
    }

    private func sendImagePasteShortcut() -> Bool {
        guard let surface else { return false }
        var key = ghostty_input_key_s()
        key.action = GHOSTTY_ACTION_PRESS
        key.mods = GHOSTTY_MODS_CTRL
        key.consumed_mods = GHOSTTY_MODS_NONE
        key.keycode = 0x09 // macOS V key
        key.unshifted_codepoint = 118 // v
        _ = ghostty_surface_key(surface, key)
        key.action = GHOSTTY_ACTION_RELEASE
        _ = ghostty_surface_key(surface, key)
        return true
    }

    override func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation {
        guard surface != nil,
              ghosttyAcceptsDrop(sender.draggingPasteboard.types)
        else { return [] }
        showDropHighlight()
        return .copy
    }

    override func draggingExited(_ sender: (any NSDraggingInfo)?) {
        hideDropHighlight()
    }

    override func draggingEnded(_ sender: any NSDraggingInfo) {
        hideDropHighlight()
    }

    override func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        hideDropHighlight()
        guard let text = terminalPasteText(from: sender.draggingPasteboard) else { return false }
        return insertTerminalText(text)
    }

    private func showDropHighlight() {
        if dropHighlight == nil {
            let highlight = NSView()
            highlight.wantsLayer = true
            highlight.layer?.borderWidth = 2
            highlight.layer?.cornerRadius = 6
            highlight.layer?.borderColor = NSColor.controlAccentColor.cgColor
            highlight.layer?.backgroundColor =
                NSColor.controlAccentColor.withAlphaComponent(0.08).cgColor
            highlight.autoresizingMask = [.width, .height]
            addSubview(highlight)
            dropHighlight = highlight
        }
        dropHighlight?.frame = bounds
        dropHighlight?.isHidden = false
    }

    private func hideDropHighlight() {
        dropHighlight?.isHidden = true
    }

    private func insertTerminalText(_ text: String) -> Bool {
        guard let surface else { return false }
        text.withCString { ptr in
            ghostty_surface_text(surface, ptr, UInt(text.utf8.count))
        }
        return true
    }

    // MARK: - Key Translation

    private func sendKeyPress(to surface: ghostty_surface_t, event: NSEvent) {
        var key = translateKey(event)
        if let chars = ghosttyKeyText(characters: event.characters, modifiers: event.modifierFlags) {
            chars.withCString { textPtr in
                key.text = textPtr
                _ = ghostty_surface_key(surface, key)
            }
            return
        }

        _ = ghostty_surface_key(surface, key)
    }

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
        MainActor.assumeIsolated {
            destroySurface()
        }
    }
}

/// Raw image bytes go to the provider's Ctrl+V clipboard-image shortcut; a
/// copied file — even an image file — pastes as its path, matching drops.
func ghosttyPrefersImageShortcut(_ pasteboard: NSPasteboard) -> Bool {
    if let urls = pasteboard.readObjects(forClasses: [NSURL.self]) as? [URL],
       urls.contains(where: \.isFileURL) {
        return false
    }
    return NSImage(pasteboard: pasteboard) != nil
}

func terminalPasteText(from pasteboard: NSPasteboard) -> String? {
    if let urls = pasteboard.readObjects(forClasses: [NSURL.self]) as? [URL],
       let first = urls.first(where: \.isFileURL) {
        return shellEscape(first.path)
    }

    if let image = NSImage(pasteboard: pasteboard),
       let tiff = image.tiffRepresentation,
       let bitmap = NSBitmapImageRep(data: tiff),
       let png = bitmap.representation(using: .png, properties: [:]) {
        let path = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-image-\(UUID().uuidString.lowercased()).png")
        guard (try? png.write(to: path, options: .atomic)) != nil else { return nil }
        return shellEscape(path.path)
    }

    if let text = pasteboard.string(forType: .string) { return text }
    if let url = pasteboard.string(forType: .URL) { return shellEscape(url) }
    return nil
}

#else

// Stub view when GhosttyKit is not available
@MainActor
final class GhosttySurfacePool {
    func hasSurface(_ id: TerminalIdentity) -> Bool { false }
    func release(_ id: TerminalIdentity) {}
    func focus(_ id: TerminalIdentity) {}
}

struct GhosttyTerminalView: View {
    let workingDirectory: String
    let argv: [String]
    let env: [String: String]
    let terminal: TerminalIdentity
    let surfacePool: GhosttySurfacePool?
    let isFocused: Bool
    let onSurfaceCreated: () -> Void
    let onFocus: () -> Void
    @ObservedObject var manager: GhosttyManager

    init(
        workingDirectory: String,
        argv: [String] = [],
        env: [String: String] = [:],
        terminal: TerminalIdentity,
        surfacePool: GhosttySurfacePool? = nil,
        isFocused: Bool = false,
        onSurfaceCreated: @escaping () -> Void = {},
        onFocus: @escaping () -> Void = {},
        manager: GhosttyManager = .shared
    ) {
        self.workingDirectory = workingDirectory
        self.argv = argv
        self.env = env
        self.terminal = terminal
        self.surfacePool = surfacePool
        self.isFocused = isFocused
        self.onSurfaceCreated = onSurfaceCreated
        self.onFocus = onFocus
        self.manager = manager
    }

    var body: some View {
        VStack {
            Image(systemName: "terminal")
                .font(Typography.heroTitle())
                .foregroundStyle(.secondary)
            Text("Embedded terminal not available")
                .font(Typography.sectionTitle())
            Text("Build GhosttyKit to enable this feature")
                .font(Typography.caption())
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(LoopflowPalette.dark.background.opacity(0.9))
    }
}

#endif

let ghosttyDropTypes: [NSPasteboard.PasteboardType] = [
    .fileURL, .URL, .tiff, .png, .string,
]

func ghosttyAcceptsDrop(_ types: [NSPasteboard.PasteboardType]?) -> Bool {
    guard let types else { return false }
    return !Set(types).isDisjoint(with: ghosttyDropTypes)
}

func buildGhosttyShellCommand(argv: [String], env: [String: String]) -> String? {
    let command = argv.map(shellEscape).joined(separator: " ")
    guard !command.isEmpty else { return nil }

    let envPrefix = env
        .sorted { $0.key < $1.key }
        .map { key, value in "\(key)=\(shellEscape(value))" }
        .joined(separator: " ")

    if envPrefix.isEmpty {
        return command
    }

    return ["env", envPrefix, command].joined(separator: " ")
}

func ghosttyShouldHandleTextAsKeyEvent(_ text: String, modifiers: NSEvent.ModifierFlags) -> Bool {
    let controlLikeModifiers: NSEvent.ModifierFlags = [.control, .command]
    guard !modifiers.intersection(controlLikeModifiers).isEmpty else { return false }
    guard text.unicodeScalars.count == 1, let scalar = text.unicodeScalars.first else { return false }
    return scalar.value < 0x20
}

func ghosttyShouldBypassInterpretKeyEvents(modifiers: NSEvent.ModifierFlags) -> Bool {
    !modifiers.intersection([.control, .command]).isEmpty
}

func ghosttyShouldHandleKeyDownDirectly(
    characters: String?,
    modifiers: NSEvent.ModifierFlags,
    hasMarkedText: Bool,
    selectedInputSource: String?
) -> Bool {
    if ghosttyShouldBypassInterpretKeyEvents(modifiers: modifiers) {
        return true
    }

    guard !hasMarkedText else { return false }
    guard !modifiers.contains(.option) else { return false }
    guard ghosttyKeyText(characters: characters, modifiers: modifiers) != nil else { return false }
    return !ghosttyInputSourceUsesTextComposition(selectedInputSource)
}

func ghosttyKeyText(characters: String?, modifiers: NSEvent.ModifierFlags) -> String? {
    let controlLikeModifiers: NSEvent.ModifierFlags = [.control, .command]
    guard modifiers.intersection(controlLikeModifiers).isEmpty else { return nil }
    guard let characters, !characters.isEmpty else { return nil }
    guard ghosttyIsPrintableKeyText(characters) else { return nil }
    return characters
}

private func ghosttyInputSourceUsesTextComposition(_ inputSource: String?) -> Bool {
    guard let inputSource else { return false }
    return inputSource.localizedCaseInsensitiveContains("inputmethod")
}

private func ghosttyIsPrintableKeyText(_ text: String) -> Bool {
    !text.unicodeScalars.contains { scalar in
        switch scalar.value {
        case 0x00..<0x20, 0x7F, 0xF700..<0xF900:
            return true
        default:
            return false
        }
    }
}

private func shellEscape(_ value: String) -> String {
    let escaped = value.replacingOccurrences(of: "'", with: "'\\''")
    return "'\(escaped)'"
}
