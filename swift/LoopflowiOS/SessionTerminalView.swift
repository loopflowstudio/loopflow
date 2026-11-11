import Loopflow
import SafariServices
import SwiftTerm
import SwiftUI
import UIKit

private final class TerminalViewport: UIScrollView {
    private var lastLayout: (height: CGFloat, contentHeight: CGFloat)?

    override func layoutSubviews() {
        let followedBottom = lastLayout.map { contentOffset.y + $0.height >= $0.contentHeight - 1 } ?? false
        super.layoutSubviews()
        lastLayout = (bounds.height, contentSize.height)
        // Status/header and keyboard changes must keep a bottom-aligned reader
        // at the bottom, without taking over a drag or an earlier reading position.
        if followedBottom, !isTracking, !isDecelerating {
            let bottom = max(0, contentSize.height - bounds.height)
            if abs(contentOffset.y - bottom) > 1 {
                setContentOffset(CGPoint(x: contentOffset.x, y: bottom), animated: false)
            }
        }
    }

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
    var allowsReadingFocus = true
    var pasteUnavailable: ((String) -> Void)?

    // Selection and link gestures need a local responder even while observing.
    // The controller hides its software keyboard; input still requires canWrite.
    override var canBecomeFirstResponder: Bool { allowsReadingFocus }

    override func insertText(_ text: String) {
        guard canWrite, allowsReadingFocus else { return }
        super.insertText(text)
    }

    override func deleteBackward() {
        guard canWrite, allowsReadingFocus else { return }
        super.deleteBackward()
    }

    func synchronizeReadingPosition() {
        guard canScroll else { return }
        let cellHeight = getOptimalFrameSize().height / CGFloat(getTerminal().rows)
        let row = Int(max(0, contentOffset.y) / cellHeight)
        if row != getTerminal().buffer.yDisp {
            // Native scrolling can leave SwiftTerm's logical row behind. Its
            // public scroll API also stops new output from following the tail.
            scrollTo(row: row, notifyAccessibility: false)
        }
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
        guard canWrite, allowsReadingFocus else { return }
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
        controller.workspace.findInTerminal = nil
        controller.workspace.setGeometry = nil
        controller.terminal.updateUiClosed()
    }
}

final class SessionTerminalController: UIViewController, @preconcurrency TerminalViewDelegate, UIScrollViewDelegate, UITextFieldDelegate {
    let workspace: MobileWorkspace
    let terminal = SessionTerminalView(frame: .zero)
    private let viewport = TerminalViewport()
    private var terminalWidth: NSLayoutConstraint!
    private var terminalHeight: NSLayoutConstraint!
    private var geometry: TerminalGeometry?
    private var appliedTheme: String?
    private let bar = UIStackView()
    private var barHeight: NSLayoutConstraint!
    private let message = UILabel()
    private let pasteButton = UIButton(type: .system)
    private let newlineButton = UIButton(type: .system)
    private let keyboardButton = UIButton(type: .system)
    private let keysButton = UIButton(type: .system)
    private let navigationBar = UIStackView()
    private var navigationButtons: [UIButton] = []
    private var typingButtons: [UIButton] { [pasteButton, newlineButton, keysButton, keyboardButton] }
    private let latestButton = UIButton(type: .system)
    private var keyboardHidden = false
    private let finder = UIStackView()
    private let findField = UITextField()
    private let findDone = UIButton(type: .system)
    private let findPrevious = UIButton(type: .system)
    private let findNext = UIButton(type: .system)
    private let findFeedback = UILabel()
    private var findHeight: NSLayoutConstraint!
    private var finding = false
    private var findGeometry: TerminalGeometry?
    private var displayedSessionID: String?
    private var snapshotSessionID: String?
    private var revealLatestAfterLayout = false
    private var readingHistory = false

    init(workspace: MobileWorkspace) {
        self.workspace = workspace
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { fatalError("Use init(workspace:)") }

    override func viewDidLoad() {
        super.viewDidLoad()
        terminal.terminalDelegate = self
        // SwiftTerm's programmatic scroll callback does not cover touch scrolling.
        terminal.delegate = self
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
        terminal.inputView = UIView(frame: .zero)
        terminal.allowMouseReporting = false
        // Touch has no hover state: expose program-supplied OSC 8 links directly.
        terminal.linkHighlightMode = .always
        terminal.accessibilityLabel = "Session terminal"
        terminal.accessibilityIdentifier = "session-terminal"
        terminal.pasteUnavailable = { [weak self] text in self?.message.text = text }
        workspace.render = { [weak self] bytes, reset in
            guard let self else { return }
            if reset {
                readingHistory = false
                if bytes.isEmpty {
                    snapshotSessionID = nil
                    revealLatestAfterLayout = false
                }
                terminal.clearSearch()
                terminal.feed(text: "\u{1b}c")
            } else if readingHistory {
                terminal.synchronizeReadingPosition()
            }
            terminal.feed(byteArray: Array(bytes)[...])
            if reset, !bytes.isEmpty, snapshotSessionID != workspace.sessionID {
                snapshotSessionID = workspace.sessionID
                revealLatestAfterLayout = true
                view.setNeedsLayout()
                view.layoutIfNeeded()
            }
            if reset, finding { findFeedback.text = "Search again" }
            updateLatestButton()
        }
        workspace.findInTerminal = { [weak self] in self?.beginFind() }
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
        keysButton.setTitle("Keys", for: .normal)
        keysButton.accessibilityLabel = "Navigation keys"
        keysButton.accessibilityIdentifier = "terminal-navigation-keys"
        keysButton.addAction(UIAction { [weak self] _ in self?.showNavigationKeys(true) }, for: .touchUpInside)
        navigationBar.axis = .horizontal
        navigationBar.distribution = .fillEqually
        navigationBar.isHidden = true
        for key in NavigationKey.allCases {
            let button = UIButton(type: .system)
            button.setImage(UIImage(systemName: key.symbol), for: .normal)
            button.accessibilityLabel = key.title
            button.addAction(UIAction { [weak self] _ in self?.sendNavigationKey(key) }, for: .touchUpInside)
            navigationButtons.append(button)
        }
        let typing = UIButton(type: .system)
        typing.setImage(UIImage(systemName: "keyboard"), for: .normal)
        typing.accessibilityLabel = "Typing keys"
        typing.addAction(UIAction { [weak self] _ in self?.showNavigationKeys(false) }, for: .touchUpInside)
        navigationButtons.append(typing)
        navigationButtons.forEach { navigationBar.addArrangedSubview($0) }
        var latestStyle = UIButton.Configuration.filled()
        latestStyle.title = "Latest"
        latestStyle.image = UIImage(systemName: "arrow.down")
        latestStyle.imagePadding = 6
        latestStyle.cornerStyle = .capsule
        latestButton.configuration = latestStyle
        latestButton.accessibilityLabel = "Scroll to latest output"
        latestButton.accessibilityIdentifier = "terminal-latest"
        latestButton.isHidden = true
        latestButton.addAction(UIAction { [weak self] _ in self?.showLatest() }, for: .touchUpInside)
        bar.axis = .horizontal
        bar.distribution = .fillEqually
        for button in typingButtons { bar.addArrangedSubview(button) }
        // Both modes share the typing row's layout, including wrapped large
        // labels. Switching keys must not resize the provider or hide the keyboard.
        navigationBar.translatesAutoresizingMaskIntoConstraints = false
        bar.addSubview(navigationBar)
        NSLayoutConstraint.activate([
            navigationBar.leadingAnchor.constraint(equalTo: bar.leadingAnchor),
            navigationBar.trailingAnchor.constraint(equalTo: bar.trailingAnchor),
            navigationBar.topAnchor.constraint(equalTo: bar.topAnchor),
            navigationBar.bottomAnchor.constraint(equalTo: bar.bottomAnchor),
        ])
        configureFind()
        updateAccessoryFonts()
        message.font = .preferredFont(forTextStyle: .caption1)
        message.adjustsFontForContentSizeCategory = true
        message.numberOfLines = 0
        viewport.contentInsetAdjustmentBehavior = .never
        viewport.isDirectionalLockEnabled = true
        viewport.delegate = self
        terminal.panGestureRecognizer.require(toFail: viewport.panGestureRecognizer)
        viewport.accessibilityIdentifier = "terminal-viewport"
        terminal.translatesAutoresizingMaskIntoConstraints = false
        viewport.addSubview(terminal)
        terminalWidth = terminal.widthAnchor.constraint(equalToConstant: 1)
        terminalHeight = terminal.heightAnchor.constraint(equalToConstant: 1)
        for subview in [finder, viewport, message, bar, latestButton] {
            subview.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview(subview)
        }
        NSLayoutConstraint.activate([
            finder.topAnchor.constraint(equalTo: view.topAnchor, constant: 4),
            finder.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 12),
            finder.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -12),
            viewport.topAnchor.constraint(equalTo: finder.bottomAnchor, constant: 6),
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
            view.keyboardLayoutGuide.topAnchor.constraint(equalToSystemSpacingBelow: bar.bottomAnchor, multiplier: 2),
            latestButton.centerXAnchor.constraint(equalTo: viewport.centerXAnchor),
            latestButton.bottomAnchor.constraint(equalTo: viewport.bottomAnchor, constant: -8),
            latestButton.heightAnchor.constraint(greaterThanOrEqualToConstant: 44),
            latestButton.widthAnchor.constraint(lessThanOrEqualTo: viewport.widthAnchor),
        ])
        findHeight = finder.heightAnchor.constraint(equalToConstant: 0)
        findHeight.isActive = true
        updateBar()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        layoutTerminal()
        if revealLatestAfterLayout, viewport.bounds.height > 0, viewport.contentSize.height > 0 {
            revealLatestAfterLayout = false
            showLatest()
        }
        updateLatestButton()
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        // Begin local reading immediately, so the first tap can open a link.
        // Observer inputView is empty and remote input is still gated.
        if finding { _ = findField.becomeFirstResponder() }
        else { _ = terminal.becomeFirstResponder() }
    }

    private var preferredTerminalFont: UIFont {
        let base = UIFont(name: "JetBrainsMono-Regular", size: 14) ?? .monospacedSystemFont(ofSize: 14, weight: .regular)
        return UIFontMetrics(forTextStyle: .body).scaledFont(for: base, compatibleWith: traitCollection)
    }

    private func updateAccessoryFonts() {
        let base = UIFont(name: "Lato-Regular", size: 15) ?? .systemFont(ofSize: 15)
        let font = UIFontMetrics(forTextStyle: .body).scaledFont(for: base, compatibleWith: traitCollection)
        for button in typingButtons + navigationButtons {
            var configuration = button.configuration ?? .plain()
            configuration.title = button.currentTitle ?? configuration.title
            configuration.image = button.currentImage ?? configuration.image
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
        // An attributed title makes the pill's intrinsic width follow its font,
        // unlike a transformer applied only when drawing the title label.
        var latest = latestButton.configuration
        var attributes = AttributeContainer()
        attributes.font = font
        latest?.attributedTitle = AttributedString("Latest", attributes: attributes)
        latest?.titleLineBreakMode = .byClipping
        latest?.contentInsets = NSDirectionalEdgeInsets(top: 8, leading: 12, bottom: 8, trailing: 12)
        latest?.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(font: font)
        latestButton.configuration = latest
        findField.font = font
        findDone.titleLabel?.font = font
        findFeedback.font = UIFontMetrics(forTextStyle: .caption1).scaledFont(for: base, compatibleWith: traitCollection)
        for button in [findPrevious, findNext] {
            button.setPreferredSymbolConfiguration(UIImage.SymbolConfiguration(font: font), forImageIn: .normal)
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
        if let geometry = geometry ?? findGeometry {
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
        if !finding {
            workspace.resize(columns: max(1, Int(viewport.bounds.width / cell.width)),
                             rows: max(1, Int(viewport.bounds.height / cell.height)))
        }
    }

    func update(theme: ThemeDefinition) {
        if displayedSessionID != workspace.sessionID {
            displayedSessionID = workspace.sessionID
            endFind(restoreFocus: false)
            findField.text = nil
        }
        let canWrite = workspace.state == .controlling
        let changed = terminal.canWrite != canWrite
        if changed, canWrite, finding { endFind(restoreFocus: false) }
        terminal.canWrite = canWrite
        terminal.allowMouseReporting = canWrite && !finding
        for button in typingButtons + navigationButtons {
            button.isEnabled = canWrite
        }
        if changed, !canWrite { showNavigationKeys(false) }
        updateBar()
        if changed {
            if canWrite {
                // A new input grant starts an intentional typing interaction, even if
                // the previous attachment had hidden its software keyboard.
                keyboardHidden = false
                terminal.inputView = nil
                updateKeyboardButton()
                terminal.reloadInputViews()
                _ = terminal.becomeFirstResponder()
            }
            else {
                // Commit/discard any platform composition only after the transport input gate closed.
                terminal.unmarkText()
                terminal.inputView = UIView(frame: .zero)
                terminal.reloadInputViews()
                _ = terminal.resignFirstResponder()
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
        latestButton.configuration?.baseBackgroundColor = UIColor(theme.palette.surface)
        latestButton.configuration?.baseForegroundColor = UIColor(theme.palette.accentInk)
        findField.textColor = UIColor(theme.palette.text)
        findField.backgroundColor = UIColor(theme.palette.surface)
        findField.attributedPlaceholder = NSAttributedString(string: "Find in Session", attributes: [
            .foregroundColor: UIColor(theme.palette.textSecondary),
        ])
        findField.keyboardAppearance = theme.isDark ? .dark : .light
        finder.tintColor = UIColor(theme.palette.accentInk)
        findFeedback.textColor = UIColor(theme.palette.textSecondary)
    }

    private func updateLatestButton() {
        let readingHistory = terminal.contentOffset.y + terminal.bounds.height < terminal.contentSize.height - 1
        let aboveBottom = viewport.contentOffset.y + viewport.bounds.height < viewport.contentSize.height - 1
        latestButton.isHidden = finding || !(readingHistory || aboveBottom)
    }

    private func updateBar() {
        let hidden = !finding && workspace.state != .controlling
        guard barHeight == nil || bar.isHidden != hidden else { return }
        barHeight?.isActive = false
        bar.isHidden = hidden
        barHeight = hidden ? bar.heightAnchor.constraint(equalToConstant: 0)
                           : bar.heightAnchor.constraint(greaterThanOrEqualToConstant: 44)
        barHeight.isActive = true
        view.setNeedsLayout()
        view.layoutIfNeeded()
    }

    enum NavigationKey: CaseIterable {
        case up, down, left, right, tab, escape

        var title: String {
            switch self {
            case .up: "Up"
            case .down: "Down"
            case .left: "Left"
            case .right: "Right"
            case .tab: "Tab"
            case .escape: "Escape"
            }
        }

        var symbol: String {
            switch self {
            case .up: "arrow.up"
            case .down: "arrow.down"
            case .left: "arrow.left"
            case .right: "arrow.right"
            case .tab: "arrow.right.to.line"
            case .escape: "escape"
            }
        }

        func bytes(applicationCursor: Bool) -> [UInt8] {
            switch self {
            case .up: applicationCursor ? EscapeSequences.moveUpApp : EscapeSequences.moveUpNormal
            case .down: applicationCursor ? EscapeSequences.moveDownApp : EscapeSequences.moveDownNormal
            case .left: applicationCursor ? EscapeSequences.moveLeftApp : EscapeSequences.moveLeftNormal
            case .right: applicationCursor ? EscapeSequences.moveRightApp : EscapeSequences.moveRightNormal
            case .tab: EscapeSequences.cmdTab
            case .escape: EscapeSequences.cmdEsc
            }
        }
    }

    private func showNavigationKeys(_ visible: Bool) {
        guard !visible || (workspace.state == .controlling && !finding) else { return }
        for button in typingButtons {
            // Keep intrinsic sizes in layout while the alternative row is shown.
            button.alpha = visible ? 0 : 1
            button.isUserInteractionEnabled = !visible
            button.accessibilityElementsHidden = visible
        }
        navigationBar.isHidden = !visible
    }

    func sendNavigationKey(_ key: NavigationKey) {
        // An action can outlive its input grant or the start of local Find.
        guard workspace.state == .controlling, !finding else { return }
        terminal.send(key.bytes(applicationCursor: terminal.getTerminal().applicationCursor))
    }

    private func configureFind() {
        finder.axis = .horizontal
        finder.alignment = .center
        finder.spacing = 8
        finder.isHidden = true
        findField.placeholder = "Find in Session"
        findField.accessibilityIdentifier = "terminal-find-query"
        findField.accessibilityLabel = "Find in Session"
        findField.borderStyle = .roundedRect
        findField.clearButtonMode = .whileEditing
        findField.autocorrectionType = .no
        findField.autocapitalizationType = .none
        findField.spellCheckingType = .no
        findField.returnKeyType = .search
        findField.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        findField.delegate = self
        findField.addAction(UIAction { [weak self] _ in self?.queryChanged() }, for: .editingChanged)
        findDone.setTitle("Done", for: .normal)
        findDone.accessibilityLabel = "Close Find"
        findDone.addAction(UIAction { [weak self] _ in self?.endFind() }, for: .touchUpInside)
        findDone.widthAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
        findDone.setContentHuggingPriority(.required, for: .horizontal)
        findDone.setContentCompressionResistancePriority(.required, for: .horizontal)
        for item in [findField, findDone] {
            finder.addArrangedSubview(item)
            let height = item.heightAnchor.constraint(greaterThanOrEqualToConstant: 44)
            height.priority = .defaultHigh
            height.isActive = true
        }
        findPrevious.setImage(UIImage(systemName: "chevron.up"), for: .normal)
        findPrevious.accessibilityLabel = "Previous match"
        findPrevious.addAction(UIAction { [weak self] _ in self?.find(backward: true) }, for: .touchUpInside)
        findNext.setImage(UIImage(systemName: "chevron.down"), for: .normal)
        findNext.accessibilityLabel = "Next match"
        findNext.addAction(UIAction { [weak self] _ in self?.find(backward: false) }, for: .touchUpInside)
        findFeedback.textAlignment = .center
        findFeedback.numberOfLines = 2
        findFeedback.accessibilityIdentifier = "terminal-find-feedback"
        for button in [findPrevious, findNext] {
            button.widthAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
            button.heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
        }
    }

    private func beginFind() {
        if !finding {
            showNavigationKeys(false)
            let grid = terminal.getTerminal()
            findGeometry = TerminalGeometry(columns: grid.cols, rows: grid.rows)
            finding = true
            terminal.allowsReadingFocus = false
            terminal.allowMouseReporting = false
            finder.isHidden = false
            findHeight.isActive = false
            bar.arrangedSubviews.forEach { $0.removeFromSuperview() }
            bar.distribution = .fill
            [findPrevious, findFeedback, findNext].forEach { bar.addArrangedSubview($0) }
            updateBar()
            view.setNeedsLayout()
            view.layoutIfNeeded()
            queryChanged()
        }
        _ = findField.becomeFirstResponder()
    }

    private func endFind(restoreFocus: Bool = true) {
        guard finding else { return }
        terminal.allowsReadingFocus = true
        terminal.allowMouseReporting = workspace.state == .controlling
        findField.resignFirstResponder()
        terminal.clearSearch()
        finder.isHidden = true
        findHeight.isActive = true
        bar.arrangedSubviews.forEach { $0.removeFromSuperview() }
        bar.distribution = .fillEqually
        typingButtons.forEach { bar.addArrangedSubview($0) }
        // Restore the typing bar while Find still freezes remote geometry.
        // Intermediate intrinsic sizes must not resize the provider's terminal.
        view.setNeedsLayout()
        view.layoutIfNeeded()
        finding = false
        findGeometry = nil
        updateBar()
        layoutTerminal()
        updateLatestButton()
        if restoreFocus { _ = terminal.becomeFirstResponder() }
    }

    private func queryChanged() {
        terminal.clearSearch()
        find(backward: false)
    }

    private func find(backward: Bool) {
        let query = findField.text ?? ""
        findPrevious.isEnabled = !query.isEmpty
        findNext.isEnabled = !query.isEmpty
        guard !query.isEmpty else {
            findFeedback.text = "Text loaded on this phone"
            return
        }
        let found = backward ? terminal.findPrevious(query) : terminal.findNext(query)
        findFeedback.text = found ? "Match found" : "No matches"
        if found, let selection = terminal.selection {
            let grid = terminal.getTerminal()
            let frame = terminal.getOptimalFrameSize()
            let cell = CGSize(width: frame.width / CGFloat(grid.cols), height: frame.height / CGFloat(grid.rows))
            let columns = selection.start.row == selection.end.row ? max(1, selection.end.col - selection.start.col) : 1
            let selected = CGRect(x: CGFloat(selection.start.col) * cell.width, y: CGFloat(selection.start.row) * cell.height,
                                  width: CGFloat(columns) * cell.width, height: cell.height)
            viewport.scrollRectToVisible(terminal.convert(selected, to: viewport).insetBy(dx: -8, dy: -8), animated: false)
        }
        updateLatestButton()
    }

    func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        find(backward: false)
        return false
    }

    private func showLatest() {
        readingHistory = false
        terminal.scroll(toPosition: 1)
        // A UIKit swipe can move the visible content without changing SwiftTerm's
        // logical row. scrollTo also aligns the view when that row is unchanged.
        terminal.scrollTo(row: terminal.getTerminal().buffer.yDisp)
        viewport.setContentOffset(CGPoint(x: viewport.contentOffset.x,
            y: max(0, viewport.contentSize.height - viewport.bounds.height)), animated: false)
        updateLatestButton()
    }

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        if scrollView === terminal,
           scrollView.isTracking || scrollView.isDragging || scrollView.isDecelerating {
            readingHistory = scrollView.contentOffset.y + scrollView.bounds.height < scrollView.contentSize.height - 1
        }
        updateLatestButton()
    }

    private func toggleKeyboard() {
        keyboardHidden.toggle()
        // Keep first responder so a hardware keyboard still reaches the terminal.
        terminal.inputView = keyboardHidden ? UIView(frame: .zero) : nil
        terminal.reloadInputViews()
        _ = terminal.becomeFirstResponder()
        updateKeyboardButton()
    }

    private func updateKeyboardButton() {
        keyboardButton.accessibilityLabel = keyboardHidden ? "Show keyboard" : "Hide keyboard"
        keyboardButton.setImage(UIImage(systemName: keyboardHidden ? "keyboard" : "keyboard.chevron.compact.down"), for: .normal)
    }

    func send(source: TerminalView, data: ArraySlice<UInt8>) { workspace.send(Data(data)) }
    func sizeChanged(source: TerminalView, newCols: Int, newRows: Int) {
        if geometry == nil, !finding { workspace.resize(columns: newCols, rows: newRows) }
    }
    func setTerminalTitle(source: TerminalView, title: String) {}
    func hostCurrentDirectoryUpdate(source: TerminalView, directory: String?) {}
    func scrolled(source: TerminalView, position: Double) { updateLatestButton() }
    func rangeChanged(source: TerminalView, startY: Int, endY: Int) {}
    func requestOpenLink(source: TerminalView, link: String, params: [String: String]) {
        guard let url = URL(string: link), let scheme = url.scheme?.lowercased(),
              ["https", "http"].contains(scheme),
              let host = url.host, !host.isEmpty else {
            message.text = "This link cannot be opened on this device."
            return
        }
        guard presentedViewController == nil else { return }
        message.text = nil
        let browser = SFSafariViewController(url: url)
        browser.dismissButtonStyle = .close
        present(browser, animated: true)
    }
}
