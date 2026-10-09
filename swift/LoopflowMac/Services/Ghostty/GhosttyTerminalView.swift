// SwiftUI view for embedding Ghostty terminal.
// Uses NSViewRepresentable to bridge the Metal-rendered terminal surface.

import SwiftUI
import AppKit
import OSLog
import QuartzCore
import Observation
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
        if case .shell(let id, _) = terminal {
            return buildWorkspaceShellCommand(id: id, argv: argv, env: env)
        }
        return buildGhosttyShellCommand(argv: argv, env: env)
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

    func makeNSView(context: Context) -> GhosttyTerminalMount {
        let view: GhosttyMetalView
        if let surfacePool {
            view = surfacePool.view(for: terminal)
        } else {
            view = GhosttyMetalView(terminal: terminal)
        }
        if case .uninitialized = manager.state {
            manager.initialize()
        }

        view.recordMountLifecycle("make")
        return GhosttyTerminalMount(terminal: view)
    }

    func updateNSView(_ mount: GhosttyTerminalMount, context: Context) {
        let nsView = mount.terminal
        nsView.recordMountLifecycle("update focused=\(isFocused) enabled=\(isEnabled)", host: mount)
        let enabled = isEnabled
        mount.configure = { nsView in
            nsView.workingDirectory = workingDirectory
            nsView.command = command
            nsView.onSurfaceCreated = onSurfaceCreated
            nsView.onFocus = onFocus
            nsView.sizeDidChange(size)

            if case .ready = manager.state, nsView.surface == nil, !nsView.childExited,
               size.width > 0, size.height > 0 {
                nsView.createSurface(manager: manager)
            }
            nsView.updateFocus(isFocused: isFocused, isEnabled: enabled)
        }
        guard nsView.superview === mount else { return }
        mount.configure?(nsView)
    }
}

/// SwiftUI owns each mount; the workspace owns the terminal. Splitting a pane
/// can retire the old representable after its terminal has moved to a new one.
/// Only a mount attached to a window may adopt the terminal: SwiftUI can lay out
/// and discard an off-window replacement without ever displaying it.
final class GhosttyTerminalMount: NSView {
    let terminal: GhosttyMetalView
    var configure: ((GhosttyMetalView) -> Void)?

    init(terminal: GhosttyMetalView) {
        self.terminal = terminal
        super.init(frame: terminal.frame)
        terminal.recordMountLifecycle("mount-created", host: self)
    }

    required init?(coder: NSCoder) { nil }

    override func viewDidMoveToSuperview() {
        super.viewDidMoveToSuperview()
        terminal.recordMountLifecycle("mount-superview", host: self)
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        terminal.recordMountLifecycle("mount-window", host: self)
        guard window != nil else { return }
        configure?(terminal)
        terminal.frame = bounds
        terminal.autoresizingMask = [.width, .height]
        addSubview(terminal)
    }
}

/// Retains live terminal views for one window's Sessions workspace so pane
/// churn and Sessions↔Work navigation never free a surface. Each window owns
/// its own pool: an NSView can only live in one view hierarchy, so sharing a
/// pool across windows would silently steal terminals between them.
@MainActor @Observable
final class GhosttySurfacePool {
    private var views: [TerminalIdentity: GhosttyMetalView] = [:]

    func view(for id: TerminalIdentity) -> GhosttyMetalView {
        if let existing = views[id] { return existing }
        let view = GhosttyMetalView(terminal: id)
        view.pool = self
        views[id] = view
        return view
    }

    /// Inspection never allocates a view or retires an exited child. The
    /// surface owner supplies its lifetime, independently of pane occurrences.
    func surfaceIncarnation(for id: TerminalIdentity) -> String? {
        guard let view = views[id], view.surface != nil else { return nil }
        return view.programStatus.incarnation.uuidString.lowercased()
    }

    func readText(_ request: DesktopTextRequest, terminal: TerminalIdentity) throws -> DesktopTextResult {
        // Do not use view(for:) or hasSurface: those allocate or schedule cleanup.
        guard let view = views[terminal], view.surface != nil else {
            return .unavailable(reason: .missingSurface)
        }
        guard view.programStatus.incarnation.uuidString.lowercased() == request.surface else {
            throw RegistryQueryError("The terminal surface was replaced. Inspect Desktop again; no text was read.")
        }
        // lf2 has no bounded reader. Replace this result with the fixed-buffer
        // extraction only after the verified lf3 artifact is selected.
        return .unavailable(reason: .boundedReaderUnavailable)
    }

    func insertText(_ text: String, terminal: TerminalIdentity, surface: String) throws {
        try validateDesktopLiteralText(text)
        let view = try inputView(terminal: terminal, surface: surface)
        _ = view.insertTerminalText(text)
    }

    func sendKey(_ key: DesktopKey, terminal: TerminalIdentity, surface: String) throws {
        let view = try inputView(terminal: terminal, surface: surface)
        view.sendTerminalKey(key)
    }

    private func inputView(terminal: TerminalIdentity, surface incarnation: String) throws -> GhosttyMetalView {
        // Never allocate, clean up, acquire a client, or fall back to focus.
        guard let view = views[terminal], let surface = view.surface else {
            throw RegistryQueryError("The retained pane has no native surface; no input was sent.")
        }
        guard view.programStatus.incarnation.uuidString.lowercased() == incarnation else {
            throw RegistryQueryError("The terminal surface was replaced. Inspect Desktop again; no input was sent.")
        }
        guard !view.childExited, !ghostty_surface_process_exited(surface) else {
            throw RegistryQueryError("The terminal child exited; no input was sent or client reopened.")
        }
        guard !view.hasMarkedText() else {
            throw RegistryQueryError("The terminal has an active input-method composition; finish it before sending input.")
        }
        return view
    }

    func programStatus(for id: TerminalIdentity) -> ProgramStatusSurface? {
        views[id]?.programStatus
    }

    func associateProgramStatus(_ records: [SessionRecord]) {
        for view in views.values {
            let marker = view.terminalMarker
            if case .session(let id, _) = view.terminal {
                if let record = records.first(where: {
                    $0.id == id && $0.workspace?.machineId == view.terminal.machineId
                }) {
                    view.programStatus.associate(sessionId: id, terminalId: marker, generation: record.providerGeneration)
                }
                continue
            }
            let matches = records.filter {
                $0.workspace?.machineId == view.terminal.machineId
                    && $0.state == .active && $0.terminalIds.contains(marker)
            }
            view.programStatus.associate(
                sessionId: matches.count == 1 ? matches[0].id : nil,
                terminalId: matches.count == 1 ? marker : nil,
                generation: matches.count == 1 ? matches[0].providerGeneration : nil
            )
        }
    }

    func title(for id: TerminalIdentity) -> String? {
        views[id]?.terminalTitle
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

/// Space between the pane edge and the terminal grid, in points. Ghostty lays
/// the grid out from the top-left with this padding; block overlays and
/// pointer rows use the same numbers.
enum GhosttyTerminalPadding {
    static let x: CGFloat = 12
    static let y: CGFloat = 8
}

func ghosttyViewportRow(
    at point: CGPoint,
    bounds: CGRect,
    rows: Int,
    cellHeight: CGFloat
) -> Int? {
    guard rows > 0, cellHeight > 0 else { return nil }
    let distanceFromTop = bounds.height - point.y - GhosttyTerminalPadding.y
    guard distanceFromTop >= 0, distanceFromTop < CGFloat(rows) * cellHeight else { return nil }
    return Int(distanceFromTop / cellHeight)
}

/// The full-width band covering a block's rows. Under the Loopflow prompt a
/// blank row precedes every header; each edge that borders one moves half a
/// row down, so the gap is shared evenly by the blocks on either side.
func ghosttyCommandBlockFrame(
    _ block: GhosttyCommandBlockLayout,
    bounds: CGRect,
    cellHeight: CGFloat,
    blankRowAbove: Bool = false,
    blankRowBelow: Bool = false
) -> CGRect {
    let origin = bounds.height - GhosttyTerminalPadding.y
    let top = origin - (CGFloat(block.startRow) + (blankRowAbove ? 0.5 : 0)) * cellHeight
    let bottom = origin - (CGFloat(block.endRow + 1) + (blankRowBelow ? 0.5 : 0)) * cellHeight
    return CGRect(x: 0, y: bottom, width: bounds.width, height: max(0, top - bottom))
}

/// Where a header row's text is drawn: on its row, at the grid's left edge.
func ghosttyBlockHeaderFrame(row: Int, bounds: CGRect, cellHeight: CGFloat) -> CGRect {
    CGRect(
        x: GhosttyTerminalPadding.x,
        y: bounds.height - GhosttyTerminalPadding.y - CGFloat(row + 1) * cellHeight,
        width: max(0, bounds.width - 2 * GhosttyTerminalPadding.x),
        height: cellHeight
    )
}

/// Block chrome. A resting block has none; hover is faint; selection is a
/// cream tint and bar; a reported failure is red, selected or not. Every fill keeps
/// the dimmest palette color (8) at 3:1 or better, which caps red near 0.18.
struct GhosttyCommandBlockStyle {
    let fill: NSColor
    let accent: NSColor

    init(block: GhosttyCommandBlockLayout, hovered: Bool) {
        // Red means failed and nothing else. Selection and hover are the
        // terminal's cream, so a selected failure still reads as both.
        let cream = NSColor(red: 0xF5 / 255, green: 0xF1 / 255, blue: 0xEA / 255, alpha: 1)
        let red = NSColor(red: 0xD4 / 255, green: 0x45 / 255, blue: 0x3A / 255, alpha: 1)
        if block.failed {
            fill = red.withAlphaComponent(0.14)
        } else if block.selected {
            fill = cream.withAlphaComponent(0.06)
        } else if hovered {
            fill = cream.withAlphaComponent(0.03)
        } else {
            fill = .clear
        }
        if block.selected {
            accent = cream.withAlphaComponent(0.9)
        } else if block.failed {
            accent = red.withAlphaComponent(hovered ? 1 : 0.75)
        } else if hovered {
            accent = cream.withAlphaComponent(0.3)
        } else {
            accent = .clear
        }
    }
}

/// A right-click on the selected block opens the menu for that block. Ghostty
/// would select the word under the pointer instead, which ends the block
/// selection; a press inside a text selection it already leaves alone.
func ghosttyRightClickKeepsBlock(row: Int?, blocks: [GhosttyCommandBlockLayout]) -> Bool {
    guard let row else { return false }
    return blocks.contains { $0.selected && $0.contains(row) }
}

@MainActor
final class GhosttyMetalView: NSView, @preconcurrency NSTextInputClient {
    /// Bounded debug evidence for a pooled surface moving between SwiftUI mounts.
    private(set) var mountLifecycle: [String] = []
    private var lastMountObservation: String?

    func recordMountLifecycle(_ event: String, host: NSView? = nil) {
        #if DEBUG
        func identity(_ view: NSView?) -> String {
            guard let view else { return "nil" }
            return "\(type(of: view))@\(ObjectIdentifier(view))"
        }
        let host = host ?? superview
        let parents = sequence(first: host?.superview, next: { $0?.superview })
            .prefix(4).map { identity($0) }.joined(separator: "/")
        let observation = "\(event) host=\(identity(host)) owner=\(identity(superview)) hostWindow=\(host?.window?.windowNumber.description ?? "nil") window=\(window?.windowNumber.description ?? "nil") parents=\(parents) frame=\(frame)"
        guard observation != lastMountObservation else { return }
        lastMountObservation = observation
        mountLifecycle.append("\(ProcessInfo.processInfo.systemUptime): \(observation)")
        if mountLifecycle.count > 48 { mountLifecycle.removeFirst(mountLifecycle.count - 48) }
        #endif
    }

    override func viewDidMoveToSuperview() {
        super.viewDidMoveToSuperview()
        recordMountLifecycle("terminal-superview")
    }

    var workingDirectory: String = ""
    var command: String?
    let terminal: TerminalIdentity
    var onSurfaceCreated: () -> Void = {}
    var onFocus: () -> Void = {}
    weak var pool: GhosttySurfacePool?
    /// Set when the surface's child ended; blocks implicit relaunch — reopening
    /// a Session or shell is an explicit action that mints a fresh view.
    private(set) var childExited = false
    let programStatus = ProgramStatusSurface()
    var terminalMarker: String {
        switch terminal {
        case .shell(let id, _): id
        case .session: programStatus.incarnation.uuidString.lowercased()
        }
    }
    var terminalTitle: String?
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
    /// Header text by viewport row, read from the terminal on each refresh.
    private var blockHeaders: [Int: String] = [:]
    /// Set once this shell has shown a Loopflow header: its prompts start
    /// with a blank row, which block edges then share.
    private var promptHasBlankRow = false
    private var hoveredRow: Int?
    /// Where the left button went down, until a drag moves away from it.
    private var clickOrigin: CGPoint?
    private var lastCommandBlockRefresh: CFTimeInterval = 0
    private var focusRequested = false
    private var inputEnabled = true
    /// Open from a key press until the next display-link draw.
    private var keyToDraw: OSSignpostIntervalState?

    init(terminal: TerminalIdentity, frame frameRect: NSRect = .zero) {
        self.terminal = terminal
        super.init(frame: frameRect)
        if case .session(let id, _) = terminal {
            programStatus.associate(sessionId: id, terminalId: terminalMarker, generation: nil)
        }
        setupView()
    }

    required init?(coder: NSCoder) {
        // Terminal identity comes from its application owner, never a nib.
        return nil
    }

    private func setupView() {
        wantsLayer = true
        layer?.backgroundColor = TerminalPalette.nsBackground.cgColor
        layerContentsRedrawPolicy = .onSetNeedsDisplay
        autoresizingMask = [.width, .height]
        registerForDraggedTypes(ghosttyDropTypes)
    }

    func createSurface(manager: GhosttyManager) {
        guard surface == nil, !childExited else { return }

        let surfaceCommand: String?
        if case .session = terminal, let command {
            let script = "export LF_TERMINAL_ID=\(shellEscape(terminalMarker)); export LF_TERMINAL_TTY=\"$(tty)\"; exec \(command)"
            surfaceCommand = ["/bin/sh", "-c", script].map(shellEscape).joined(separator: " ")
        } else {
            surfaceCommand = command
        }
        surface = manager.createSurface(
            workingDirectory: workingDirectory,
            command: surfaceCommand,
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
        recordMountLifecycle("terminal-window")
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

    func updateFocus(isFocused: Bool, isEnabled: Bool) {
        inputEnabled = isEnabled
        let requested = isEnabled && isFocused
        let changed = focusRequested != requested
        focusRequested = requested
        if requested && changed {
            // Attachment handles a request made before the view has a window.
            // Later polls and resizes must leave the search field's focus alone.
            window?.makeFirstResponder(self)
        } else if !isEnabled || changed, window?.firstResponder === self {
            window?.makeFirstResponder(nil)
        }
    }

    func destroySurface() {
        programStatus.close()
        // A released view may still be mounted until SwiftUI reconciles it.
        // Only an explicit reopen with a fresh view may launch another child.
        childExited = true
        displayLink?.invalidate()
        displayLink = nil
        commandBlockOverlay.removeFromSuperlayer()
        commandBlocks = []
        blockHeaders = [:]
        hoveredRow = nil
        clickOrigin = nil

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
        if let keyToDraw {
            self.keyToDraw = nil
            Perf.signposter.endInterval(Perf.terminalKeyToDraw, keyToDraw)
        }
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
                startRow: Int($0.start_row),
                endRow: Int($0.end_row),
                exitCode: $0.exit_code < 0 ? nil : Int($0.exit_code),
                selected: $0.selected
            )
        }
        let nextHeaders = readBlockHeaders(blocks: nextBlocks, size: size)
        if !nextHeaders.isEmpty { promptHasBlankRow = true }
        guard nextBlocks != commandBlocks
            || nextHeaders != blockHeaders
            || commandBlockOverlay.frame != bounds
        else { return }
        commandBlocks = nextBlocks
        blockHeaders = nextHeaders
        renderCommandBlocks()
    }

    /// A selected block is for copying. Once input goes to the shell the
    /// highlight would sit stale above the new command, so it ends there:
    /// on text and on every key, Escape included, except Command chords,
    /// which are app and Ghostty bindings such as copy and jump to prompt.
    private func clearBlockSelection() {
        guard isShellPane, let surface, commandBlocks.contains(where: \.selected) else { return }
        // No block holds this row, which clears the selection.
        _ = ghostty_surface_select_command_block(surface, .max)
        refreshCommandBlocks()
    }

    private func readBlockHeaders(
        blocks: [GhosttyCommandBlockLayout],
        size: ghostty_surface_size_s
    ) -> [Int: String] {
        guard let surface else { return [:] }
        var headers: [Int: String] = [:]
        for row in GhosttyBlockHeader.candidateRows(blocks: blocks, rows: Int(size.rows)) {
            func point(_ column: Int) -> ghostty_point_s {
                ghostty_point_s(
                    tag: GHOSTTY_POINT_VIEWPORT,
                    coord: GHOSTTY_POINT_COORD_EXACT,
                    x: UInt32(column),
                    y: UInt32(row)
                )
            }
            // Ghostty clamps the end column to the grid.
            let line = readText {
                ghostty_surface_read_text(
                    surface,
                    ghostty_selection_s(
                        top_left: point(0),
                        bottom_right: point(GhosttyBlockHeader.columns - 1),
                        rectangle: true
                    ),
                    $0
                )
            }
            headers[row] = line.flatMap(GhosttyBlockHeader.text(inRow:))
        }
        return headers
    }

    /// The non-empty text a Ghostty read produced.
    private func readText(_ read: (UnsafeMutablePointer<ghostty_text_s>) -> Bool) -> String? {
        guard let surface else { return nil }
        var text = ghostty_text_s()
        guard read(&text) else { return nil }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let bytes = text.text, text.text_len > 0 else { return nil }
        return String(decoding: Data(bytes: bytes, count: Int(text.text_len)), as: UTF8.self)
    }

    private func renderCommandBlocks() {
        guard isShellPane, let surface else { return }
        let surfaceSize = ghostty_surface_size(surface)
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        let cellHeight = CGFloat(surfaceSize.cell_height_px) / scale

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        commandBlockOverlay.frame = bounds
        let blockLayers = commandBlocks.map { block in
            let frame = ghosttyCommandBlockFrame(
                block,
                bounds: bounds,
                cellHeight: cellHeight,
                // A block scrolled past its header has no blank row in view.
                blankRowAbove: blockHeaders[block.startRow + 1] != nil,
                blankRowBelow: promptHasBlankRow
            )
            let style = GhosttyCommandBlockStyle(
                block: block,
                hovered: hoveredRow.map(block.contains) ?? false
            )

            let surfaceLayer = CALayer()
            surfaceLayer.frame = frame
            surfaceLayer.backgroundColor = style.fill.cgColor

            // A hairline above each block separates it from the one before.
            let separator = CALayer()
            separator.frame = CGRect(x: 0, y: frame.height - 1, width: frame.width, height: 1)
            separator.backgroundColor = NSColor.white.withAlphaComponent(0.07).cgColor
            surfaceLayer.addSublayer(separator)

            // The accent bar sits in the left gutter, clear of the text.
            let accentLayer = CALayer()
            accentLayer.frame = CGRect(x: 4, y: 2, width: 3, height: max(0, frame.height - 5))
            accentLayer.cornerRadius = 1.5
            accentLayer.backgroundColor = style.accent.cgColor
            surfaceLayer.addSublayer(accentLayer)
            return surfaceLayer
        }
        let font = NSFont.monospacedSystemFont(ofSize: GhosttyBlockHeader.fontSize, weight: .regular)
        let lineHeight = ceil(font.ascender - font.descender + font.leading)
        let headerLayers = blockHeaders.map { row, text in
            let rowFrame = ghosttyBlockHeaderFrame(row: row, bounds: bounds, cellHeight: cellHeight)
            let textLayer = CATextLayer()
            textLayer.frame = CGRect(
                x: rowFrame.minX,
                y: rowFrame.midY - lineHeight / 2,
                width: rowFrame.width,
                height: lineHeight
            )
            textLayer.string = text
            textLayer.font = font
            textLayer.fontSize = font.pointSize
            textLayer.foregroundColor = GhosttyBlockHeader.color.cgColor
            textLayer.truncationMode = .start
            textLayer.contentsScale = scale
            return textLayer
        }
        commandBlockOverlay.sublayers = blockLayers + headerLayers
        CATransaction.commit()
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

    override var acceptsFirstResponder: Bool { inputEnabled }

    override func becomeFirstResponder() -> Bool {
        guard inputEnabled else { return false }
        let accepted = super.becomeFirstResponder()
        if accepted, let surface {
            ghostty_surface_set_focus(surface, true)
            // The Session pane is visible and accepting input.
            if case .session(let id, _) = terminal { Perf.endAfterCommit(Perf.taskWorkspaceReady, id: id) }
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

        if !mods.contains(.command) { clearBlockSelection() }

        // Let other command shortcuts through to the terminal
        let key = translateKey(event)
        return ghostty_surface_key(surface, key)
    }

    override func keyDown(with event: NSEvent) {
        guard let surface else {
            super.keyDown(with: event)
            return
        }
        programStatus.receive(.interaction, incarnation: programStatus.incarnation)
        if keyToDraw == nil {
            keyToDraw = Perf.signposter.beginInterval(Perf.terminalKeyToDraw, id: Perf.signposter.makeSignpostID())
        }
        if !event.modifierFlags.contains(.command) { clearBlockSelection() }

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

    // The terminal owns both selections. Every pointer event reaches Ghostty,
    // so a drag selects text inside a block; a plain click that selected no
    // text selects the block under it, or clears the block outside one.
    override func mouseDown(with event: NSEvent) {
        focusForPointerInput()
        guard let surface else { return }
        let point = convert(event.locationInWindow, from: nil)
        let mods = translateMods(event.modifierFlags)
        clickOrigin = event.clickCount == 1 ? point : nil
        ghostty_surface_mouse_pos(surface, point.x, bounds.height - point.y, mods)
        _ = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_LEFT, mods)
    }

    override func mouseUp(with event: NSEvent) {
        guard let surface else { return }
        _ = ghostty_surface_mouse_button(
            surface,
            GHOSTTY_MOUSE_RELEASE,
            GHOSTTY_MOUSE_LEFT,
            translateMods(event.modifierFlags)
        )
        let selectionModifiers: NSEvent.ModifierFlags = [.command, .control, .option, .shift]
        guard isShellPane,
              clickOrigin != nil,
              event.modifierFlags.intersection(selectionModifiers).isEmpty,
              !ghostty_surface_has_selection(surface)
        else { return }
        clickOrigin = nil
        // A row outside the grid selects nothing, which clears the block.
        let row = viewportRow(at: convert(event.locationInWindow, from: nil))
        _ = ghostty_surface_select_command_block(surface, row.map(UInt16.init) ?? .max)
        refreshCommandBlocks()
    }

    override func rightMouseDown(with event: NSEvent) {
        focusForPointerInput()
        guard let surface else { return }
        let point = convert(event.locationInWindow, from: nil)
        guard !ghosttyRightClickKeepsBlock(row: viewportRow(at: point), blocks: commandBlocks) else {
            showContextMenu(at: point)
            return
        }

        // Send to terminal first, at the press's own position: Ghostty keeps
        // a text selection the press lands in and otherwise selects the word.
        let mods = translateMods(event.modifierFlags)
        ghostty_surface_mouse_pos(surface, point.x, bounds.height - point.y, mods)
        let consumed = ghostty_surface_mouse_button(surface, GHOSTTY_MOUSE_PRESS, GHOSTTY_MOUSE_RIGHT, mods)
        // A word selected by the press ended any block selection.
        refreshCommandBlocks()

        // If terminal didn't consume it, show context menu
        if !consumed {
            showContextMenu(at: point)
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
            let row = viewportRow(at: point)
            if row != hoveredRow {
                hoveredRow = row
                renderCommandBlocks()
            }
        }
        let y = bounds.height - point.y
        ghostty_surface_mouse_pos(surface, point.x, y, translateMods(event.modifierFlags))
    }

    override func mouseDragged(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        if let origin = clickOrigin, hypot(point.x - origin.x, point.y - origin.y) > 3 {
            clickOrigin = nil
        }
        mouseMoved(with: event)
        // A drag that started a text selection ended the block selection.
        if commandBlocks.contains(where: \.selected) { refreshCommandBlocks() }
    }

    override func mouseExited(with event: NSEvent) {
        if hoveredRow != nil {
            hoveredRow = nil
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

        menu.popUp(positioning: nil, at: point, in: self)
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

    /// Copies whichever selection the terminal holds, read at copy time.
    private func copySelection() -> Bool {
        guard let surface,
              let selection = readText({
                  ghostty_surface_has_selection(surface)
                      ? ghostty_surface_read_selection(surface, $0)
                      : ghostty_surface_read_selected_command_block(surface, $0)
              })
        else { return false }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        return pasteboard.setString(selection, forType: .string)
    }

    private func focusForPointerInput() {
        onFocus()
        window?.makeFirstResponder(self)
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

    fileprivate func insertTerminalText(_ text: String) -> Bool {
        guard let surface else { return false }
        programStatus.receive(.interaction, incarnation: programStatus.incarnation)
        clearBlockSelection()
        text.withCString { ptr in
            ghostty_surface_text(surface, ptr, UInt(text.utf8.count))
        }
        return true
    }

    fileprivate func sendTerminalKey(_ input: DesktopKey) {
        guard let surface else { return }
        programStatus.receive(.interaction, incarnation: programStatus.incarnation)
        clearBlockSelection()
        var key = ghostty_input_key_s()
        key.action = GHOSTTY_ACTION_PRESS
        key.mods = GHOSTTY_MODS_NONE
        key.consumed_mods = GHOSTTY_MODS_NONE
        key.keycode = input.macKeyCode
        _ = ghostty_surface_key(surface, key)
        key.action = GHOSTTY_ACTION_RELEASE
        _ = ghostty_surface_key(surface, key)
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
        let path = terminalTemporaryDirectory()
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
@MainActor @Observable
final class GhosttySurfacePool {
    func surfaceIncarnation(for id: TerminalIdentity) -> String? { nil }
    func readText(_ request: DesktopTextRequest, terminal: TerminalIdentity) throws -> DesktopTextResult {
        .unavailable(reason: .missingSurface)
    }
    func insertText(_ text: String, terminal: TerminalIdentity, surface: String) throws {
        try validateDesktopLiteralText(text)
        throw RegistryQueryError("This Desktop build has no native terminal; no input was sent.")
    }
    func sendKey(_ key: DesktopKey, terminal: TerminalIdentity, surface: String) throws {
        throw RegistryQueryError("This Desktop build has no native terminal; no input was sent.")
    }
    func hasSurface(_ id: TerminalIdentity) -> Bool { false }
    func programStatus(for id: TerminalIdentity) -> ProgramStatusSurface? { nil }
    func associateProgramStatus(_ records: [SessionRecord]) {}
    func title(for id: TerminalIdentity) -> String? { nil }
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
        .background(TerminalPalette.background)
    }
}

#endif

// Block geometry and the header format stay outside the Ghostty build: the
// shell bootstrap below names the header marker in every configuration.

/// The directory line above a command. The Loopflow prompt prints it as
/// concealed terminal text, so it scrolls, reflows and survives with its rows;
/// the overlay reads it back and draws it smaller and dimmer than the command.
enum GhosttyBlockHeader {
    /// Starts a header row. Nothing a person types begins a prompt row with it.
    static let marker = "\u{B6} "
    /// The prompt truncates the directory to this many characters.
    static let maxLength = 60
    /// Columns read from each candidate row.
    static let columns = marker.count + maxLength

    /// Smaller than the terminal's 13pt body.
    static let fontSize: CGFloat = 11
    static let color = NSColor(TerminalPalette.dim)

    /// The header a terminal row holds, or `nil` for any other row.
    static func text(inRow row: String) -> String? {
        guard row.hasPrefix(marker) else { return nil }
        let text = row.dropFirst(marker.count).trimmingCharacters(in: .whitespaces)
        return text.isEmpty ? nil : text
    }

    /// Viewport rows that can hold a header: a block's first two rows (its
    /// first row when the blank row above scrolled away) and every row outside
    /// a completed block, where the live prompt and empty prompts sit.
    static func candidateRows(blocks: [GhosttyCommandBlockLayout], rows: Int) -> [Int] {
        (0..<rows).filter { row in
            guard let block = blocks.first(where: { $0.contains(row) }) else { return true }
            return row <= block.startRow + 1
        }
    }
}

/// A completed shell command as Ghostty reports it: viewport rows, the exit
/// status the shell gave, and whether it is the terminal's selected block.
struct GhosttyCommandBlockLayout: Equatable {
    let startRow: Int
    let endRow: Int
    /// `nil` when the shell reported no status; never drawn as a failure.
    let exitCode: Int?
    let selected: Bool

    var failed: Bool { (exitCode ?? 0) > 0 }

    func contains(_ row: Int) -> Bool {
        startRow...endRow ~= row
    }
}

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

/// zsh reads `ZDOTDIR/.zshenv` first. Loopflow's gives a shell still on the
/// macOS default prompt a blank row, a header row holding the directory, and a
/// command line, leaves every other prompt alone, then hands over to Ghostty's
/// integration, which restores the user's own ZDOTDIR.
///
/// The header row is concealed text behind `GhosttyBlockHeader.marker`: the
/// terminal keeps it as the directory at submission, and the overlay draws it
/// in its own size and color.
enum LoopflowZshBootstrap {
    static let macOSDefaultPrompt = "%n@%m %1~ %# "

    static let zshenv = """
    if [[ -o interactive ]]; then
        # Runs once, after the user's rc files have had their say.
        _loopflow_prompt() {
            precmd_functions=(${precmd_functions:#_loopflow_prompt})
            [[ $PROMPT == '\(macOSDefaultPrompt)' ]] || builtin return 0
            PROMPT=$'\\n%{\\e[8m%}\(GhosttyBlockHeader.marker)%\(GhosttyBlockHeader.maxLength)<\u{2026}<%~%<<%{\\e[28m%}\\n%F{\(accent)}\u{276F}%f '
            # The command is bold; zsh ends the highlight before output starts.
            zle_highlight=(${zle_highlight:#default:*} default:bold)
        }
        builtin typeset -ga precmd_functions zle_highlight
        precmd_functions+=(_loopflow_prompt)
    fi
    builtin source -- "$GHOSTTY_RESOURCES_DIR/shell-integration/zsh/.zshenv"

    """

    private static let accent = TerminalPalette.css(TerminalPalette.accentHex)

    /// Writes the bootstrap and returns its directory. Rewritten per shell:
    /// the system may clear temporary files while the app stays open, and a
    /// ZDOTDIR without a `.zshenv` would skip the user's rc files.
    static func install() -> String? {
        let directory = terminalTemporaryDirectory()
            .appendingPathComponent("loopflow-zsh", isDirectory: true)
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try zshenv.write(
                to: directory.appendingPathComponent(".zshenv"),
                atomically: true,
                encoding: .utf8
            )
            return directory.path
        } catch {
            return nil
        }
    }
}

/// Keep a real shell after the initial conversation exits or hands off to an
/// app. The PTY marker lets successive manual lf launches find this terminal.
func buildWorkspaceShellCommand(id: String, argv: [String], env: [String: String]) -> String {
    let shell = ProcessInfo.processInfo.environment["SHELL"] ?? "/bin/zsh"
    let initial = buildGhosttyShellCommand(argv: argv, env: env).map { $0 + "\n" } ?? ""
    let script = """
    export LF_TERMINAL_ID=\(shellEscape(id))
    export LF_TERMINAL_TTY="$(tty)"
    \(initial)
    if [ -n "$GHOSTTY_RESOURCES_DIR" ]; then
        export GHOSTTY_SHELL_FEATURES="${GHOSTTY_SHELL_FEATURES-}"
        case \(shellEscape(URL(fileURLWithPath: shell).lastPathComponent)) in
            zsh)
                if [ "${ZDOTDIR+x}" = x ]; then export GHOSTTY_ZSH_ZDOTDIR="$ZDOTDIR"; fi
                export ZDOTDIR=\(LoopflowZshBootstrap.install().map(shellEscape) ?? #""$GHOSTTY_RESOURCES_DIR/shell-integration/zsh""#)
                ;;
            bash)
                export GHOSTTY_BASH_ENV="${ENV-}"
                export GHOSTTY_BASH_INJECT=1
                export ENV="$GHOSTTY_RESOURCES_DIR/shell-integration/bash/ghostty.bash"
                exec \(shellEscape(shell)) --posix -l
                ;;
            fish)
                exec \(shellEscape(shell)) -l -C 'source "$GHOSTTY_RESOURCES_DIR/shell-integration/fish/ghostty-shell-integration.fish"'
                ;;
        esac
    fi
    exec \(shellEscape(shell)) -l
    """
    return ["/bin/sh", "-c", script].map(shellEscape).joined(separator: " ")
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

// Control bytes in a paste can submit or edit a shell when bracketed paste is
// disabled. Keep the text/key boundary independent of the child program's mode.
func validateDesktopLiteralText(_ text: String) throws {
    guard !text.unicodeScalars.contains(where: { $0.value < 0x20 || (0x7f...0x9f).contains($0.value) }) else {
        throw RegistryQueryError("Literal text cannot contain control characters; use an explicit key action. No input was sent.")
    }
}

private extension DesktopKey {
    var macKeyCode: UInt32 {
        switch self {
        case .enter: 0x24
        case .tab: 0x30
        case .escape: 0x35
        case .backspace: 0x33
        case .delete: 0x75
        case .left: 0x7b
        case .right: 0x7c
        case .up: 0x7e
        case .down: 0x7d
        case .home: 0x73
        case .end: 0x77
        }
    }
}
