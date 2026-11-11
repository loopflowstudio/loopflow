import Loopflow
import SwiftTerm
import SwiftUI
import UIKit

/// Keys a provider's menus need that a reply field cannot type.
enum TerminalKey: CaseIterable {
    case escape, tab, up, down, left, right, enter

    var title: String {
        switch self {
        case .escape: "Escape"
        case .tab: "Tab"
        case .up: "Up"
        case .down: "Down"
        case .left: "Left"
        case .right: "Right"
        case .enter: "Return"
        }
    }

    var symbol: String {
        switch self {
        case .escape: "escape"
        case .tab: "arrow.right.to.line"
        case .up: "arrow.up"
        case .down: "arrow.down"
        case .left: "arrow.left"
        case .right: "arrow.right"
        case .enter: "return"
        }
    }

    func bytes(applicationCursor: Bool) -> [UInt8] {
        switch self {
        case .escape: EscapeSequences.cmdEsc
        case .tab: EscapeSequences.cmdTab
        case .up: applicationCursor ? EscapeSequences.moveUpApp : EscapeSequences.moveUpNormal
        case .down: applicationCursor ? EscapeSequences.moveDownApp : EscapeSequences.moveDownNormal
        case .left: applicationCursor ? EscapeSequences.moveLeftApp : EscapeSequences.moveLeftNormal
        case .right: applicationCursor ? EscapeSequences.moveRightApp : EscapeSequences.moveRightNormal
        case .enter: [13]
        }
    }
}

/// What the reply field and key row ask of the terminal on screen.
@MainActor
final class TerminalActions {
    fileprivate weak var controller: SessionTerminalController?
    func reply(_ text: String) { controller?.reply(text) }
    func press(_ key: TerminalKey) { controller?.press(key) }
}

struct SessionTerminal: UIViewControllerRepresentable {
    let link: SessionLink
    let actions: TerminalActions
    let theme: ThemeDefinition

    func makeUIViewController(context: Context) -> SessionTerminalController {
        let controller = SessionTerminalController(link: link)
        actions.controller = controller
        controller.loadViewIfNeeded()
        controller.apply(theme)
        return controller
    }

    func updateUIViewController(_ controller: SessionTerminalController, context: Context) {
        controller.apply(theme)
    }

    static func dismantleUIViewController(_ controller: SessionTerminalController, coordinator: ()) {
        controller.link.render = nil
    }
}

/// The software keyboard never types into the terminal: replies go through the
/// reply field, so a stray touch sends nothing. A hardware keyboard types directly.
private final class DirectTerminalView: TerminalView {
    override init(frame: CGRect) {
        super.init(frame: frame)
        inputView = UIView(frame: .zero)
        inputAccessoryView = nil
    }

    required init?(coder: NSCoder) { fatalError("Use init(frame:)") }

    /// SwiftTerm's own scroll position can trail a finger's; realign before output arrives.
    func holdReadingRow() -> Int? {
        guard canScroll, contentOffset.y + bounds.height < contentSize.height - 1 else { return nil }
        let cellHeight = getOptimalFrameSize().height / CGFloat(getTerminal().rows)
        let row = Int(max(0, contentOffset.y) / cellHeight)
        if row != getTerminal().buffer.yDisp { scrollTo(row: row, notifyAccessibility: false) }
        return getTerminal().buffer.yDisp
    }
}

final class SessionTerminalController: UIViewController, @preconcurrency TerminalViewDelegate, UIScrollViewDelegate {
    let link: SessionLink
    private let terminal = DirectTerminalView(frame: .zero)
    private let latest = UIButton(type: .system)
    private var appliedTheme: String?
    private var reading = false

    init(link: SessionLink) {
        self.link = link
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { fatalError("Use init(link:)") }

    override func viewDidLoad() {
        super.viewDidLoad()
        terminal.terminalDelegate = self
        terminal.delegate = self
        terminal.allowMouseReporting = false
        terminal.accessibilityLabel = "Session terminal"
        terminal.accessibilityIdentifier = "session-terminal"
        updateFont()
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (controller: SessionTerminalController, _: UITraitCollection) in
            controller.updateFont()
        }
        // The owner reads program status reports; the renderer in control answers the capability query.
        terminal.getTerminal().parser.oscHandlers[7501] = { [weak link] bytes in
            guard bytes.elementsEqual([UInt8(ascii: "?")]) else { return }
            link?.send(Data("\u{1b}]7501;?\u{7}".utf8))
        }
        link.render = { [weak self] bytes, reset in self?.render(bytes, reset: reset) }

        var style = UIButton.Configuration.filled()
        style.title = "Latest"
        style.image = UIImage(systemName: "arrow.down")
        style.imagePadding = 6
        style.cornerStyle = .capsule
        latest.configuration = style
        latest.accessibilityLabel = "Scroll to latest output"
        latest.isHidden = true
        latest.addAction(UIAction { [weak self] _ in self?.showLatest() }, for: .touchUpInside)

        for subview in [terminal, latest] {
            subview.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview(subview)
        }
        NSLayoutConstraint.activate([
            terminal.topAnchor.constraint(equalTo: view.topAnchor),
            terminal.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            terminal.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 8),
            terminal.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -8),
            latest.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            latest.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -8),
            latest.heightAnchor.constraint(greaterThanOrEqualToConstant: 44),
        ])
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        let grid = terminal.getTerminal()
        link.fit(columns: grid.cols, rows: grid.rows)
    }

    func apply(_ theme: ThemeDefinition) {
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
        view.backgroundColor = UIColor(theme.terminal.background.color)
        latest.configuration?.baseBackgroundColor = UIColor(theme.palette.surface)
        latest.configuration?.baseForegroundColor = UIColor(theme.palette.accentInk)
    }

    /// Sends a reply as one paste, then Return, so a multi-line reply arrives whole.
    func reply(_ text: String) {
        let clean = String(text.unicodeScalars.filter {
            !CharacterSet.controlCharacters.contains($0) || $0 == "\n" || $0 == "\t"
        })
        guard !clean.isEmpty else { return }
        let bracketed = terminal.getTerminal().bracketedPasteMode
        let body = bracketed ? "\u{1b}[200~\(clean)\u{1b}[201~" : clean.replacingOccurrences(of: "\n", with: " ")
        link.send(Data(body.utf8))
        link.send(Data([13]))
        showLatest()
    }

    func press(_ key: TerminalKey) {
        link.send(Data(key.bytes(applicationCursor: terminal.getTerminal().applicationCursor)))
    }

    private func updateFont() {
        let base = UIFont(name: "JetBrainsMono-Regular", size: 12) ?? .monospacedSystemFont(ofSize: 12, weight: .regular)
        terminal.font = UIFontMetrics(forTextStyle: .body).scaledFont(for: base, compatibleWith: traitCollection)
    }

    private func render(_ bytes: Data, reset: Bool) {
        if reset {
            reading = false
            terminal.feed(text: "\u{1b}c")
        }
        // Output that arrives while reading earlier lines leaves them in place.
        let row = reading ? terminal.holdReadingRow() : nil
        reading = row != nil
        terminal.feed(byteArray: Array(bytes)[...])
        if let row, terminal.getTerminal().buffer.yDisp != row {
            terminal.scrollTo(row: row, notifyAccessibility: false)
        }
        updateLatest()
    }

    private var isReadingEarlier: Bool {
        terminal.contentOffset.y + terminal.bounds.height < terminal.contentSize.height - 1
    }

    private func updateLatest() { latest.isHidden = !isReadingEarlier }

    private func showLatest() {
        reading = false
        terminal.scroll(toPosition: 1)
        terminal.scrollTo(row: terminal.getTerminal().buffer.yDisp)
        updateLatest()
    }

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        if scrollView.isTracking || scrollView.isDragging || scrollView.isDecelerating {
            reading = isReadingEarlier
        }
        updateLatest()
    }

    func send(source: TerminalView, data: ArraySlice<UInt8>) { link.send(Data(data)) }
    func sizeChanged(source: TerminalView, newCols: Int, newRows: Int) { link.fit(columns: newCols, rows: newRows) }
    func scrolled(source: TerminalView, position: Double) { updateLatest() }
    func setTerminalTitle(source: TerminalView, title: String) {}
    func hostCurrentDirectoryUpdate(source: TerminalView, directory: String?) {}
    func rangeChanged(source: TerminalView, startY: Int, endY: Int) {}
    func requestOpenLink(source: TerminalView, link: String, params: [String: String]) {}
}
