import Loopflow
import SwiftTerm
import SwiftUI
import UIKit

final class SessionTerminalView: TerminalView {
    var canWrite = false
    var pasteUnavailable: ((String) -> Void)?

    override var canBecomeFirstResponder: Bool { canWrite }

    override func insertText(_ text: String) {
        guard canWrite else { return }
        super.insertText(text)
    }

    override func deleteBackward() {
        guard canWrite else { return }
        super.deleteBackward()
    }

    override func paste(_ sender: Any?) {
        guard let text = UIPasteboard.general.string else { return }
        pasteText(text)
    }

    func pasteText(_ text: String) {
        guard canWrite else { return }
        // A paste cannot terminate its own bracket or become a control command.
        let clean = String(text.unicodeScalars.filter {
            !CharacterSet.controlCharacters.contains($0) || $0 == "\n" || $0 == "\r" || $0 == "\t"
        }).replacingOccurrences(of: "\r\n", with: "\n").replacingOccurrences(of: "\r", with: "\n")
        let bracketed = getTerminal().bracketedPasteMode
        guard bracketed || !clean.contains(where: { $0 == "\n" || $0 == "\t" }) else {
            pasteUnavailable?("This program has not enabled multiline paste.")
            return
        }
        let bytes = (bracketed ? "\u{1b}[200~" : "") + clean + (bracketed ? "\u{1b}[201~" : "")
        send(txt: bytes)
    }
}

struct SessionTerminal: UIViewControllerRepresentable {
    let workspace: MobileWorkspace
    let theme: ThemeDefinition

    func makeUIViewController(context: Context) -> SessionTerminalController {
        let controller = SessionTerminalController(workspace: workspace)
        controller.loadViewIfNeeded()
        controller.update(theme: theme)
        return controller
    }

    func updateUIViewController(_ controller: SessionTerminalController, context: Context) {
        controller.update(theme: theme)
    }

    static func dismantleUIViewController(_ controller: SessionTerminalController, coordinator: ()) {
        controller.workspace.render = nil
        controller.terminal.updateUiClosed()
    }
}

final class SessionTerminalController: UIViewController, @preconcurrency TerminalViewDelegate {
    let workspace: MobileWorkspace
    let terminal = SessionTerminalView(frame: .zero)
    private let bar = UIStackView()
    private let message = UILabel()
    private let pasteButton = UIButton(type: .system)
    private let newlineButton = UIButton(type: .system)
    private let keyboardButton = UIButton(type: .system)
    private var keyboardHidden = false

    init(workspace: MobileWorkspace) {
        self.workspace = workspace
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { fatalError("Use init(workspace:)") }

    override func viewDidLoad() {
        super.viewDidLoad()
        terminal.terminalDelegate = self
        terminal.font = UIFont(name: "JetBrainsMono-Regular", size: 14) ?? .monospacedSystemFont(ofSize: 14, weight: .regular)
        terminal.inputAccessoryView = nil
        terminal.accessibilityLabel = "Session terminal"
        terminal.accessibilityIdentifier = "session-terminal"
        terminal.pasteUnavailable = { [weak self] text in self?.message.text = text }
        workspace.render = { [weak terminal] bytes, reset in
            guard let terminal else { return }
            if reset { terminal.feed(text: "\u{1b}c") }
            terminal.feed(byteArray: Array(bytes)[...])
        }

        pasteButton.setTitle("Paste", for: .normal)
        pasteButton.addAction(UIAction { [weak self] _ in self?.terminal.paste(nil) }, for: .touchUpInside)
        newlineButton.setTitle("New line", for: .normal)
        newlineButton.addAction(UIAction { [weak self] _ in self?.terminal.pasteText("\n") }, for: .touchUpInside)
        keyboardButton.setImage(UIImage(systemName: "keyboard.chevron.compact.down"), for: .normal)
        keyboardButton.accessibilityLabel = "Hide keyboard"
        keyboardButton.addAction(UIAction { [weak self] _ in self?.toggleKeyboard() }, for: .touchUpInside)
        bar.axis = .horizontal
        bar.distribution = .fillEqually
        for button in [pasteButton, newlineButton, keyboardButton] { bar.addArrangedSubview(button) }
        message.font = .preferredFont(forTextStyle: .caption1)
        message.numberOfLines = 0
        for subview in [terminal, message, bar] {
            subview.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview(subview)
        }
        NSLayoutConstraint.activate([
            terminal.topAnchor.constraint(equalTo: view.topAnchor),
            terminal.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor),
            terminal.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor),
            terminal.bottomAnchor.constraint(equalTo: message.topAnchor),
            message.leadingAnchor.constraint(equalTo: terminal.leadingAnchor, constant: 8),
            message.trailingAnchor.constraint(equalTo: terminal.trailingAnchor, constant: -8),
            message.bottomAnchor.constraint(equalTo: bar.topAnchor),
            bar.leadingAnchor.constraint(equalTo: terminal.leadingAnchor),
            bar.trailingAnchor.constraint(equalTo: terminal.trailingAnchor),
            bar.heightAnchor.constraint(equalToConstant: 44),
            bar.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor),
        ])
    }

    func update(theme: ThemeDefinition) {
        let canWrite = workspace.state == .controlling
        let changed = terminal.canWrite != canWrite
        terminal.canWrite = canWrite
        for button in [pasteButton, newlineButton, keyboardButton] { button.isEnabled = canWrite }
        if changed {
            if canWrite { terminal.becomeFirstResponder() }
            else {
                // Commit/discard any platform composition only after the transport input gate closed.
                terminal.unmarkText()
                terminal.resignFirstResponder()
            }
        }
        terminal.nativeBackgroundColor = UIColor(theme.terminal.background.color)
        terminal.nativeForegroundColor = UIColor(theme.terminal.foreground.color)
        terminal.selectedTextBackgroundColor = UIColor(theme.terminal.selectionBackground.color)
        terminal.selectedTextForegroundColor = UIColor(theme.terminal.selectionForeground.color)
        terminal.installColors(theme.terminal.indexed.prefix(16).map {
            SwiftTerm.Color(red: UInt16(($0.hex >> 16) & 255) * 257,
                            green: UInt16(($0.hex >> 8) & 255) * 257,
                            blue: UInt16($0.hex & 255) * 257)
        })
        terminal.keyboardAppearance = theme.isDark ? .dark : .light
        view.backgroundColor = UIColor(theme.palette.background)
        bar.tintColor = UIColor(theme.palette.accentInk)
        message.textColor = UIColor(theme.palette.textSecondary)
    }

    private func toggleKeyboard() {
        keyboardHidden.toggle()
        // Keep first responder so a hardware keyboard still reaches the terminal.
        terminal.inputView = keyboardHidden ? UIView(frame: .zero) : nil
        terminal.reloadInputViews()
        terminal.becomeFirstResponder()
        keyboardButton.accessibilityLabel = keyboardHidden ? "Show keyboard" : "Hide keyboard"
        keyboardButton.setImage(UIImage(systemName: keyboardHidden ? "keyboard" : "keyboard.chevron.compact.down"), for: .normal)
    }

    func send(source: TerminalView, data: ArraySlice<UInt8>) { workspace.send(Data(data)) }
    func sizeChanged(source: TerminalView, newCols: Int, newRows: Int) {
        workspace.resize(columns: newCols, rows: newRows)
    }
    func setTerminalTitle(source: TerminalView, title: String) {}
    func hostCurrentDirectoryUpdate(source: TerminalView, directory: String?) {}
    func scrolled(source: TerminalView, position: Double) {}
    func rangeChanged(source: TerminalView, startY: Int, endY: Int) {}
    func requestOpenLink(source: TerminalView, link: String, params: [String: String]) {}
}
