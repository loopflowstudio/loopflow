import Loopflow
import SwiftTerm
import SwiftUI
import UIKit

private final class TerminalViewport: UIScrollView {
    override func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        guard gestureRecognizer === panGestureRecognizer else {
            return super.gestureRecognizerShouldBegin(gestureRecognizer)
        }
        let velocity = panGestureRecognizer.velocity(in: self)
        if abs(velocity.x) > abs(velocity.y) {
            return contentSize.width > bounds.width
        }
        // At an edge, let the terminal's own scrollback take the next gesture.
        return velocity.y < 0
            ? contentOffset.y < contentSize.height - bounds.height
            : contentOffset.y > 0
    }
}

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

    func setFontPreservingGrid(_ font: UIFont) {
        // SwiftTerm's font setter otherwise reflows the grid against the old
        // pixel frame. An observer must retain even an alternate screen's cells.
        let origin = frame.origin
        frame = .zero
        self.font = font
        let size = getOptimalFrameSize().size
        frame = CGRect(origin: origin, size: CGSize(width: ceil(size.width) + 1, height: ceil(size.height) + 1))
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
        send(txt: bracketed ? "\u{1b}[200~\(clean)\u{1b}[201~" : clean)
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
        controller.workspace.setGeometry = nil
        controller.terminal.updateUiClosed()
    }
}

final class SessionTerminalController: UIViewController, @preconcurrency TerminalViewDelegate {
    let workspace: MobileWorkspace
    let terminal = SessionTerminalView(frame: .zero)
    private let viewport = TerminalViewport()
    private var terminalWidth: NSLayoutConstraint!
    private var terminalHeight: NSLayoutConstraint!
    private var geometry: TerminalGeometry?
    private var appliedTheme: String?
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
        // The owner parses reports; the active renderer still answers capability
        // queries. The workspace suppresses replies from observers and snapshots.
        terminal.getTerminal().parser.oscHandlers[7501] = { [weak workspace] bytes in
            guard bytes.elementsEqual([UInt8(ascii: "?")]) else { return }
            workspace?.send(Data("\u{1b}]7501;?\u{7}".utf8))
        }
        terminal.font = preferredTerminalFont
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (controller: SessionTerminalController, _: UITraitCollection) in
            controller.updateAccessoryFonts()
            controller.updateTerminalFont()
        }
        terminal.inputAccessoryView = nil
        terminal.accessibilityLabel = "Session terminal"
        terminal.accessibilityIdentifier = "session-terminal"
        terminal.pasteUnavailable = { [weak self] text in self?.message.text = text }
        workspace.render = { [weak terminal] bytes, reset in
            guard let terminal else { return }
            if reset { terminal.feed(text: "\u{1b}c") }
            terminal.feed(byteArray: Array(bytes)[...])
        }
        workspace.setGeometry = { [weak self] geometry in
            guard let self else { return }
            self.geometry = geometry
            self.view.setNeedsLayout()
            self.view.layoutIfNeeded()
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
        updateAccessoryFonts()
        message.font = .preferredFont(forTextStyle: .caption1)
        message.adjustsFontForContentSizeCategory = true
        message.numberOfLines = 0
        viewport.contentInsetAdjustmentBehavior = .never
        viewport.isDirectionalLockEnabled = true
        terminal.panGestureRecognizer.require(toFail: viewport.panGestureRecognizer)
        viewport.accessibilityIdentifier = "terminal-viewport"
        terminal.translatesAutoresizingMaskIntoConstraints = false
        viewport.addSubview(terminal)
        terminalWidth = terminal.widthAnchor.constraint(equalToConstant: 1)
        terminalHeight = terminal.heightAnchor.constraint(equalToConstant: 1)
        for subview in [viewport, message, bar] {
            subview.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview(subview)
        }
        NSLayoutConstraint.activate([
            viewport.topAnchor.constraint(equalTo: view.topAnchor, constant: 10),
            viewport.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 12),
            viewport.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -12),
            viewport.bottomAnchor.constraint(equalTo: message.topAnchor),
            terminal.leadingAnchor.constraint(equalTo: viewport.contentLayoutGuide.leadingAnchor),
            terminal.trailingAnchor.constraint(equalTo: viewport.contentLayoutGuide.trailingAnchor),
            terminal.topAnchor.constraint(equalTo: viewport.contentLayoutGuide.topAnchor),
            terminal.bottomAnchor.constraint(equalTo: viewport.contentLayoutGuide.bottomAnchor),
            terminalWidth, terminalHeight,
            message.leadingAnchor.constraint(equalTo: viewport.leadingAnchor, constant: 8),
            message.trailingAnchor.constraint(equalTo: viewport.trailingAnchor, constant: -8),
            message.bottomAnchor.constraint(equalTo: bar.topAnchor),
            bar.leadingAnchor.constraint(equalTo: viewport.leadingAnchor),
            bar.trailingAnchor.constraint(equalTo: viewport.trailingAnchor),
            bar.heightAnchor.constraint(greaterThanOrEqualToConstant: 44),
            bar.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor),
        ])
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        layoutTerminal()
    }

    private var preferredTerminalFont: UIFont {
        let base = UIFont(name: "JetBrainsMono-Regular", size: 14) ?? .monospacedSystemFont(ofSize: 14, weight: .regular)
        return UIFontMetrics(forTextStyle: .body).scaledFont(for: base, compatibleWith: traitCollection)
    }

    private func updateAccessoryFonts() {
        let base = UIFont(name: "Lato-Regular", size: 15) ?? .systemFont(ofSize: 15)
        let font = UIFontMetrics(forTextStyle: .body).scaledFont(for: base, compatibleWith: traitCollection)
        for button in [pasteButton, newlineButton, keyboardButton] {
            var configuration = button.configuration ?? .plain()
            configuration.title = button.currentTitle
            configuration.image = button.currentImage
            configuration.contentInsets = NSDirectionalEdgeInsets(top: 6, leading: 4, bottom: 6, trailing: 4)
            configuration.titleLineBreakMode = .byWordWrapping
            configuration.titleAlignment = .center
            configuration.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
                var attributes = attributes
                attributes.font = font
                return attributes
            }
            configuration.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(font: font)
            button.configuration = configuration
        }
    }

    private func updateTerminalFont() {
        let font = preferredTerminalFont
        guard terminal.font != font else { return }
        terminal.setFontPreservingGrid(font)
        layoutTerminal()
    }

    private func layoutTerminal() {
        guard viewport.bounds.width > 0, viewport.bounds.height > 0 else { return }
        let grid = terminal.getTerminal()
        let frame = terminal.getOptimalFrameSize()
        let cell = CGSize(width: frame.width / CGFloat(grid.cols), height: frame.height / CGFloat(grid.rows))
        let size: CGSize
        if let geometry {
            // Size the actual renderer, so UIKit layout never briefly reflows a
            // desktop snapshot at phone width. The surrounding viewport pans.
            // Leave a point for UIKit's pixel rounding; truncating a fractional
            // cell boundary would otherwise silently lose the last column.
            size = CGSize(width: ceil(CGFloat(geometry.columns) * cell.width) + 1,
                          height: ceil(CGFloat(geometry.rows) * cell.height) + 1)
        } else {
            size = viewport.bounds.size
        }
        if terminalWidth.constant != size.width || terminalHeight.constant != size.height {
            terminalWidth.constant = size.width
            terminalHeight.constant = size.height
            viewport.layoutIfNeeded()
        }
        workspace.resize(columns: max(1, Int(viewport.bounds.width / cell.width)),
                         rows: max(1, Int(viewport.bounds.height / cell.height)))
    }

    func update(theme: ThemeDefinition) {
        let canWrite = workspace.state == .controlling
        let changed = terminal.canWrite != canWrite
        terminal.canWrite = canWrite
        for button in [pasteButton, newlineButton, keyboardButton] { button.isEnabled = canWrite }
        if changed {
            if canWrite {
                // A new input grant starts an intentional typing interaction, even if
                // the previous attachment had hidden its software keyboard.
                keyboardHidden = false
                terminal.inputView = nil
                updateKeyboardButton()
                terminal.reloadInputViews()
                terminal.becomeFirstResponder()
            }
            else {
                // Commit/discard any platform composition only after the transport input gate closed.
                terminal.unmarkText()
                terminal.resignFirstResponder()
            }
        }
        guard appliedTheme != theme.id else { return }
        appliedTheme = theme.id
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
        updateKeyboardButton()
    }

    private func updateKeyboardButton() {
        keyboardButton.accessibilityLabel = keyboardHidden ? "Show keyboard" : "Hide keyboard"
        keyboardButton.setImage(UIImage(systemName: keyboardHidden ? "keyboard" : "keyboard.chevron.compact.down"), for: .normal)
    }

    func send(source: TerminalView, data: ArraySlice<UInt8>) { workspace.send(Data(data)) }
    func sizeChanged(source: TerminalView, newCols: Int, newRows: Int) {
        if geometry == nil { workspace.resize(columns: newCols, rows: newRows) }
    }
    func setTerminalTitle(source: TerminalView, title: String) {}
    func hostCurrentDirectoryUpdate(source: TerminalView, directory: String?) {}
    func scrolled(source: TerminalView, position: Double) {}
    func rangeChanged(source: TerminalView, startY: Int, endY: Int) {}
    func requestOpenLink(source: TerminalView, link: String, params: [String: String]) {}
}
