import AppKit
import Observation

@MainActor
@Observable
final class KeyboardRouter {
    var isHelpOverlayVisible = false
    var chordPrefix: ShortcutKey?
    var chordIndicator: String?

    let shortcuts: [ShortcutBinding]
    let chords: [ChordBinding]
    let requiresWaveActions: Set<ShortcutAction>

    private var monitor: Any?
    private var windowHandlers: [Int: (ShortcutAction) -> Void] = [:]
    private var commandPaletteVisibility: [Int: Bool] = [:]
    private var chordTimer: Task<Void, Never>?
    private let chordTimeout: Duration
    private let keyDisplayByAction: [ShortcutAction: String]
    private let chordFirstKeys: Set<ShortcutKey>
    private let chordActions: [ChordPair: ShortcutAction]

    init(
        shortcuts: [ShortcutBinding] = ShortcutCatalog.shortcuts,
        chords: [ChordBinding] = ShortcutCatalog.chords,
        chordTimeout: Duration = .milliseconds(800)
    ) {
        self.shortcuts = shortcuts
        self.chords = chords
        self.chordTimeout = chordTimeout
        self.requiresWaveActions = Set(shortcuts.filter(\.requiresWave).map(\.action))

        var display: [ShortcutAction: String] = [:]
        for binding in shortcuts {
            if display[binding.action] == nil {
                display[binding.action] = binding.gesture.displayKey
            }
        }
        for chord in chords {
            if display[chord.action] == nil {
                display[chord.action] = chord.displayKey
            }
        }
        self.keyDisplayByAction = display
        self.chordFirstKeys = Set(chords.map(\.first))
        self.chordActions = Dictionary(
            uniqueKeysWithValues: chords.map { (ChordPair(first: $0.first, second: $0.second), $0.action) }
        )
    }

    func register(windowNumber: Int, actionHandler: @escaping (ShortcutAction) -> Void) {
        windowHandlers[windowNumber] = actionHandler
        installMonitorIfNeeded()
    }

    func unregister(windowNumber: Int) {
        windowHandlers.removeValue(forKey: windowNumber)
        commandPaletteVisibility.removeValue(forKey: windowNumber)

        if windowHandlers.isEmpty {
            cancelChord()
            uninstallMonitor()
        }
    }

    func setCommandPaletteVisible(_ isVisible: Bool, for windowNumber: Int) {
        commandPaletteVisibility[windowNumber] = isVisible
    }

    func keyDisplay(for action: ShortcutAction) -> String? {
        keyDisplayByAction[action]
    }

    func handleKeyEvent(_ event: NSEvent, windowNumber: Int, handler: (ShortcutAction) -> Void) -> Bool {
        let mode = resolveMode(
            firstResponder: NSApp.keyWindow?.firstResponder,
            isCommandPaletteVisible: commandPaletteVisibility[windowNumber] ?? false,
            isHelpOverlayVisible: isHelpOverlayVisible
        )

        guard let key = normalizeKey(event) else { return false }

        return routeEvent(
            key: key,
            modifiers: normalizeModifiers(event.modifierFlags),
            isRepeat: event.isARepeat,
            mode: mode,
            handler: handler
        )
    }

    func routeEvent(
        key: ShortcutKey,
        modifiers: NSEvent.ModifierFlags,
        isRepeat: Bool,
        mode: KeyboardMode,
        handler: (ShortcutAction) -> Void
    ) -> Bool {
        switch mode {
        case .textEditing, .commandPalette:
            return false
        case .terminal:
            return handleTerminalShortcut(
                key: key,
                modifiers: modifiers,
                isRepeat: isRepeat,
                handler: handler
            )
        case .helpOverlay:
            return handleHelpOverlayKey(key: key, modifiers: modifiers)
        case .global:
            if handleChordInput(key: key, modifiers: modifiers, handler: handler) {
                return true
            }

            guard let binding = findShortcut(key: key, modifiers: modifiers) else {
                return false
            }

            if isRepeat && !binding.gesture.allowsRepeat {
                return true
            }

            handler(binding.action)
            return true
        }
    }

    private func handleTerminalShortcut(
        key: ShortcutKey,
        modifiers: NSEvent.ModifierFlags,
        isRepeat: Bool,
        handler: (ShortcutAction) -> Void
    ) -> Bool {
        guard let binding = findShortcut(key: key, modifiers: modifiers),
              binding.category == .multiplexer else {
            return false
        }

        if isRepeat && !binding.gesture.allowsRepeat {
            return true
        }

        handler(binding.action)
        return true
    }

    func resolveMode(
        firstResponder: Any?,
        isCommandPaletteVisible: Bool,
        isHelpOverlayVisible: Bool
    ) -> KeyboardMode {
        if isHelpOverlayVisible {
            return .helpOverlay
        }

        if isTextResponder(firstResponder) {
            return .textEditing
        }

        if isTerminalResponder(firstResponder) {
            return .terminal
        }

        if isCommandPaletteVisible {
            return .commandPalette
        }

        return .global
    }

    func normalizeModifiers(_ modifiers: NSEvent.ModifierFlags) -> NSEvent.ModifierFlags {
        modifiers.intersection(ShortcutCatalog.normalizedModifierMask)
    }

    func normalizeKey(_ event: NSEvent) -> ShortcutKey? {
        normalizeKey(
            charactersIgnoringModifiers: event.charactersIgnoringModifiers,
            specialKey: event.specialKey,
            keyCode: event.keyCode
        )
    }

    func normalizeKey(
        charactersIgnoringModifiers: String?,
        specialKey: NSEvent.SpecialKey?,
        keyCode: UInt16
    ) -> ShortcutKey? {
        if keyCode == ShortcutCatalog.returnKeyCode ||
            keyCode == ShortcutCatalog.keypadEnterKeyCode ||
            keyCode == ShortcutCatalog.escapeKeyCode ||
            keyCode == ShortcutCatalog.fiveKeyCode ||
            keyCode == ShortcutCatalog.quoteKeyCode {
            return .keyCode(keyCode)
        }

        if let specialKey {
            return .special(specialKey)
        }

        if let chars = charactersIgnoringModifiers?.lowercased(), chars.count == 1 {
            if keyCode == ShortcutCatalog.slashKeyCode && chars != "/" {
                return .keyCode(ShortcutCatalog.slashKeyCode)
            }
            return .character(chars)
        }

        if keyCode == ShortcutCatalog.slashKeyCode {
            return .keyCode(ShortcutCatalog.slashKeyCode)
        }

        return nil
    }

    func cancelChord() {
        chordTimer?.cancel()
        chordTimer = nil
        chordPrefix = nil
        chordIndicator = nil
    }

    private func installMonitorIfNeeded() {
        guard monitor == nil else { return }

        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self else { return event }
            guard let window = NSApp.keyWindow,
                  let handler = self.windowHandlers[window.windowNumber] else {
                return event
            }

            if self.handleKeyEvent(event, windowNumber: window.windowNumber, handler: handler) {
                return nil
            }
            return event
        }
    }

    private func uninstallMonitor() {
        if let monitor {
            NSEvent.removeMonitor(monitor)
            self.monitor = nil
        }
    }

    private func handleHelpOverlayKey(key: ShortcutKey, modifiers: NSEvent.ModifierFlags) -> Bool {
        let shouldDismissWithEscape = keyMatches(key, expected: .keyCode(ShortcutCatalog.escapeKeyCode))
        let shouldDismissWithQuestion = keyMatches(key, expected: .character("/")) && modifiers == [.shift]

        if shouldDismissWithEscape || shouldDismissWithQuestion {
            isHelpOverlayVisible = false
            return true
        }

        return false
    }

    private func handleChordInput(
        key: ShortcutKey,
        modifiers: NSEvent.ModifierFlags,
        handler: (ShortcutAction) -> Void
    ) -> Bool {
        if let prefix = chordPrefix {
            if isEscapeKey(key) {
                cancelChord()
                return true
            }

            if modifiers.isEmpty,
               let action = chordActions[ChordPair(first: prefix, second: key)] {
                cancelChord()
                handler(action)
                return true
            }

            if modifiers.isEmpty,
               chordFirstKeys.contains(key) {
                beginChord(with: key)
                return true
            }

            cancelChord()
            return false
        }

        if modifiers.isEmpty,
           chordFirstKeys.contains(key) {
            beginChord(with: key)
            return true
        }

        return false
    }

    private func beginChord(with key: ShortcutKey) {
        cancelChord()
        chordPrefix = key
        chordIndicator = "\(key.displayKey.lowercased())..."

        let timeout = chordTimeout
        chordTimer = Task { [weak self] in
            try? await Task.sleep(for: timeout)
            guard let self else { return }
            if self.chordPrefix == key {
                self.cancelChord()
            }
        }
    }

    private func findShortcut(key: ShortcutKey, modifiers: NSEvent.ModifierFlags) -> ShortcutBinding? {
        shortcuts.first { binding in
            binding.gesture.modifiers == modifiers && keyMatches(key, expected: binding.gesture.key)
        }
    }

    private func keyMatches(_ key: ShortcutKey, expected: ShortcutKey) -> Bool {
        if key == expected {
            return true
        }

        if Self.returnFallbackKeys.contains(key) && Self.returnFallbackKeys.contains(expected) {
            return true
        }

        return Self.slashFallbackKeys.contains(key) && Self.slashFallbackKeys.contains(expected)
    }

    private func isEscapeKey(_ key: ShortcutKey) -> Bool {
        keyMatches(key, expected: .keyCode(ShortcutCatalog.escapeKeyCode))
    }

    private func isTextResponder(_ responder: Any?) -> Bool {
        guard let responder else { return false }
        return responder is NSText
    }

    private func isTerminalResponder(_ responder: Any?) -> Bool {
        guard let responder else { return false }
        let typeName = String(describing: type(of: responder))
        return typeName.contains("GhosttyMetalView")
    }

    private static let returnFallbackKeys: Set<ShortcutKey> = [
        .keyCode(ShortcutCatalog.returnKeyCode),
        .keyCode(ShortcutCatalog.keypadEnterKeyCode),
    ]

    private static let slashFallbackKeys: Set<ShortcutKey> = [
        .character("/"),
        .keyCode(ShortcutCatalog.slashKeyCode),
    ]
}

private struct ChordPair: Hashable {
    let first: ShortcutKey
    let second: ShortcutKey
}

enum KeyboardMode: Equatable {
    case textEditing
    case terminal
    case commandPalette
    case helpOverlay
    case global
}
