import Loopflow
import SwiftUI
import UIKit

/// The keys a provider's menus need that a reply field cannot type, in the
/// key row's order. The keys are Desktop's; the codes are this platform's.
extension DesktopKey {
    static let phoneRow: [DesktopKey] = [.escape, .tab, .up, .down, .left, .right, .enter]

    var title: String {
        switch self {
        case .escape: "Escape"
        case .tab: "Tab"
        case .up: "Up"
        case .down: "Down"
        case .left: "Left"
        case .right: "Right"
        case .enter: "Return"
        case .backspace: "Delete"
        case .delete: "Forward Delete"
        case .home: "Home"
        case .end: "End"
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
        case .backspace: "delete.left"
        case .delete: "delete.right"
        case .home: "arrow.up.to.line"
        case .end: "arrow.down.to.line"
        }
    }

    /// The key's USB keyboard usage, which is how the terminal names keys on iOS.
    var usage: UInt32 {
        switch self {
        case .escape: 0x29
        case .tab: 0x2B
        case .up: 0x52
        case .down: 0x51
        case .left: 0x50
        case .right: 0x4F
        case .enter: 0x28
        case .backspace: 0x2A
        case .delete: 0x4C
        case .home: 0x4A
        case .end: 0x4D
        }
    }
}

/// What the reply field and key row ask of the terminal on screen.
@MainActor
final class TerminalActions {
    fileprivate weak var controller: SessionTerminalController?
    func reply(_ text: String) { controller?.reply(text) }
    func press(_ key: DesktopKey) { controller?.press(key) }
}

struct SessionTerminal: UIViewControllerRepresentable {
    let link: SessionLink
    let actions: TerminalActions
    let theme: ThemeDefinition

    func makeUIViewController(context: Context) -> SessionTerminalController {
        let controller = SessionTerminalController(link: link)
        actions.controller = controller
        controller.apply(theme)
        controller.loadViewIfNeeded()
        return controller
    }

    func updateUIViewController(_ controller: SessionTerminalController, context: Context) {
        controller.apply(theme)
    }

    static func dismantleUIViewController(_ controller: SessionTerminalController, coordinator: ()) {
        controller.link.render = nil
        controller.terminal.close()
    }
}

/// The Session's terminal, drawn by Ghostty. The software keyboard never types
/// into it: replies go through the reply field, so a stray touch sends nothing.
/// A hardware keyboard types directly once the terminal is tapped.
final class SessionTerminalController: UIViewController, UIScrollViewDelegate {
    let link: SessionLink
    let terminal = GhosttySurfaceView(frame: .zero)
    /// Carries the finger: native scrolling over the terminal's rows, which Ghostty draws.
    private let scroller = UIScrollView()
    private let latest = UIButton(type: .system)
    private var theme: ThemeDefinition?
    private var scroll = GhosttySurfaceView.Scroll()
    private var placing = false

    init(link: SessionLink) {
        self.link = link
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { fatalError("Use init(link:)") }

    override func viewDidLoad() {
        super.viewDidLoad()
        terminal.isAccessibilityElement = true
        terminal.accessibilityLabel = "Session terminal"
        terminal.accessibilityIdentifier = "session-terminal"
        terminal.onWrite = { [weak link] in link?.send($0) }
        terminal.onGrid = { [weak link] in link?.fit(columns: $0, rows: $1) }
        terminal.onScroll = { [weak self] in self?.place($0) }
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (controller: SessionTerminalController, _: UITraitCollection) in
            controller.applyAppearance()
        }
        link.render = { [weak self] bytes, reset in self?.render(bytes, reset: reset) }

        scroller.delegate = self
        scroller.contentInsetAdjustmentBehavior = .never
        scroller.scrollsToTop = false
        scroller.showsHorizontalScrollIndicator = false
        scroller.addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(focusTerminal)))

        var style = UIButton.Configuration.filled()
        style.title = "Latest"
        style.image = UIImage(systemName: "arrow.down")
        style.imagePadding = 6
        style.cornerStyle = .capsule
        latest.configuration = style
        latest.accessibilityLabel = "Scroll to latest output"
        latest.isHidden = true
        latest.addAction(UIAction { [weak self] _ in self?.showLatest() }, for: .touchUpInside)

        for subview in [terminal, scroller, latest] {
            subview.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview(subview)
        }
        NSLayoutConstraint.activate([
            terminal.topAnchor.constraint(equalTo: view.topAnchor),
            terminal.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            terminal.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 8),
            terminal.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -8),
            scroller.topAnchor.constraint(equalTo: terminal.topAnchor),
            scroller.bottomAnchor.constraint(equalTo: terminal.bottomAnchor),
            scroller.leadingAnchor.constraint(equalTo: terminal.leadingAnchor),
            scroller.trailingAnchor.constraint(equalTo: terminal.trailingAnchor),
            latest.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            latest.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -8),
            latest.heightAnchor.constraint(greaterThanOrEqualToConstant: 44),
        ])
        applyAppearance()
    }

    func apply(_ theme: ThemeDefinition) {
        guard self.theme?.id != theme.id else { return }
        self.theme = theme
        applyAppearance()
    }

    /// Theme colours, and JetBrains Mono 12 scaled by Dynamic Type.
    private func applyAppearance() {
        guard let theme else { return }
        let size = UIFontMetrics(forTextStyle: .body).scaledValue(for: 12, compatibleWith: traitCollection)
        GhosttyRuntime.shared.apply(theme, fontSize: Double(size))
        guard isViewLoaded else { return }
        view.backgroundColor = UIColor(theme.terminal.background.color)
        scroller.indicatorStyle = theme.isDark ? .white : .black
        latest.configuration?.baseBackgroundColor = UIColor(theme.palette.surface)
        latest.configuration?.baseForegroundColor = UIColor(theme.palette.accentInk)
    }

    /// A reply as the Session should receive it: one paste, then Return, so a
    /// multi-line reply arrives whole. Nil when nothing printable is left.
    static func replyBytes(_ text: String, bracketed: Bool) -> Data? {
        let clean = String(text.unicodeScalars.filter {
            !CharacterSet.controlCharacters.contains($0) || $0 == "\n" || $0 == "\t"
        })
        guard !clean.isEmpty else { return nil }
        let body = bracketed ? "\u{1b}[200~\(clean)\u{1b}[201~" : clean.replacingOccurrences(of: "\n", with: " ")
        return Data(body.utf8)
    }

    func reply(_ text: String) {
        guard let paste = Self.replyBytes(text, bracketed: terminal.bracketedPaste) else { return }
        link.send(paste)
        link.send(Data([13]))
        showLatest()
    }

    func press(_ key: DesktopKey) { terminal.press(usage: key.usage) }

    @objc private func focusTerminal() { terminal.becomeFirstResponder() }

    private func render(_ bytes: Data, reset: Bool) {
        if reset { terminal.feed(Data("\u{1b}c".utf8)) }
        terminal.feed(bytes)
    }

    // MARK: Scrolling

    private var fingerScrolling: Bool { scroller.isTracking || scroller.isDragging || scroller.isDecelerating }

    /// Ghostty reported its rows. Earlier lines stay put while output arrives
    /// because Ghostty holds the row it shows; only the length grows under the finger.
    private func place(_ next: GhosttySurfaceView.Scroll) {
        scroll = next
        let row = terminal.rowHeight
        placing = true
        scroller.contentSize = CGSize(width: scroller.bounds.width,
                                      height: CGFloat(next.lastOffset) * row + scroller.bounds.height)
        if !fingerScrolling { scroller.contentOffset.y = CGFloat(next.offset) * row }
        placing = false
        latest.isHidden = !next.readingEarlier
    }

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        let height = terminal.rowHeight
        guard !placing, fingerScrolling, height > 0 else { return }
        let row = min(max(0, Int((scrollView.contentOffset.y / height).rounded())), scroll.lastOffset)
        guard row != scroll.offset else { return }
        scroll.offset = row
        terminal.scroll(toRow: row)
        latest.isHidden = !scroll.readingEarlier
    }

    private func showLatest() {
        scroller.setContentOffset(scroller.contentOffset, animated: false)
        terminal.scrollToLatest()
    }
}
