import AppKit
import Loopflow

struct TerminalSearchResults: Equatable {
    var total: Int?
    var selected: Int?

    var label: String {
        guard let total else { return "Searching…" }
        guard total > 0 else { return "No matches" }
        guard let selected, selected >= 0, selected < total else { return total == 1 ? "1 match" : "\(total) matches" }
        return "\(selected + 1) of \(total)"
    }
}

func terminalCopyMessage(_ text: String) -> String? {
    guard !text.isEmpty else { return nil }
    // CRLF is one Swift Character; count terminal line feeds in UTF-8 instead.
    let lines = text.utf8.reduce(0) { $0 + ($1 == 10 ? 1 : 0) } + (text.utf8.last == 10 ? 0 : 1)
    return lines == 1 ? "Copied 1 line" : "Copied \(lines) lines"
}

#if GHOSTTY_ENABLED
/// Owned by the native terminal, mounted beside it rather than over its cells.
@MainActor
final class TerminalSearchBar: NSView, NSSearchFieldDelegate {
    static let height: CGFloat = 34
    weak var terminal: GhosttyMetalView?
    private let field = NSSearchField()
    private let count = NSTextField(labelWithString: "")
    private var navigationButtons: [NSButton] = []
    private var results = TerminalSearchResults()

    init(terminal: GhosttyMetalView) {
        self.terminal = terminal
        super.init(frame: .zero)
        isHidden = true
        appearance = NSAppearance(named: .darkAqua)
        wantsLayer = true
        layer?.backgroundColor = TerminalPalette.nsBackground.cgColor
        field.placeholderString = "Find in terminal"
        field.setAccessibilityLabel("Find in terminal")
        field.setAccessibilityIdentifier("terminal-find-field")
        field.delegate = self
        field.sendsSearchStringImmediately = true
        count.font = .systemFont(ofSize: 11)
        count.textColor = NSColor(TerminalPalette.dim)
        count.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        let previous = button("chevron.up", label: "Previous match", action: #selector(previousMatch))
        let next = button("chevron.down", label: "Next match", action: #selector(nextMatch))
        navigationButtons = [previous, next]
        let close = button("xmark", label: "Close Find", action: #selector(closeSearch))
        let stack = NSStackView(views: [field, count, previous, next, close])
        stack.orientation = .horizontal
        stack.spacing = 6
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            stack.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    required init?(coder: NSCoder) { nil }

    private func button(_ image: String, label: String, action: Selector) -> NSButton {
        let button = NSButton(title: "", target: self, action: action)
        button.image = NSImage(systemSymbolName: image, accessibilityDescription: label)
        button.bezelStyle = .inline
        button.isBordered = false
        button.toolTip = label
        button.setAccessibilityLabel(label)
        button.widthAnchor.constraint(equalToConstant: 24).isActive = true
        return button
    }

    override func layout() {
        count.isHidden = bounds.width < 400
        for button in navigationButtons { button.isHidden = bounds.width < 260 }
        super.layout()
    }

    func open(needle: String?) {
        let shouldFocus = terminal.map { window?.firstResponder === $0 } ?? false
        isHidden = false
        if let needle { field.stringValue = needle }
        updateQuery()
        resizeTerminal()
        if shouldFocus {
            window?.makeFirstResponder(field)
            field.selectText(nil)
        }
    }

    func receive(total: Int) {
        guard !isHidden else { return }
        results.total = total >= 0 ? total : nil
        updateCount()
    }

    func receive(selected: Int) {
        guard !isHidden else { return }
        results.selected = selected >= 0 ? selected : nil
        updateCount()
    }

    func end() {
        guard !isHidden else { return }
        let hadFocus = window?.firstResponder === field.currentEditor()
        isHidden = true
        results = TerminalSearchResults()
        resizeTerminal()
        // A callback arriving after a context switch must not steal focus.
        if hadFocus {
            window?.makeFirstResponder(terminal)
        }
    }

    private func resizeTerminal() {
        guard let mount = terminal?.superview as? GhosttyTerminalMount else { return }
        mount.needsLayout = true
        mount.layoutSubtreeIfNeeded()
    }

    private func updateCount() {
        count.stringValue = field.stringValue.isEmpty ? "" : results.label
    }

    private func updateQuery() {
        results = TerminalSearchResults()
        updateCount()
        terminal?.performBinding("search:\(field.stringValue)")
    }

    func controlTextDidChange(_ notification: Notification) { updateQuery() }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        switch selector {
        case #selector(NSResponder.cancelOperation(_:)): closeSearch()
        case #selector(NSResponder.insertNewline(_:)):
            if NSApp.currentEvent?.modifierFlags.contains(.shift) == true { previousMatch() }
            else { nextMatch() }
        default: return false
        }
        return true
    }

    @objc private func nextMatch() { terminal?.performBinding("navigate_search:next") }
    @objc private func previousMatch() { terminal?.performBinding("navigate_search:previous") }
    @objc private func closeSearch() {
        terminal?.performBinding("end_search")
        end()
    }
}

/// Copy feedback must not intercept selection drags or terminal clicks.
final class TerminalCopyNotice: NSTextField {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}
#endif
