#if os(macOS)
import SwiftUI
import AppKit
import Loopflow

/// Context a Chat message needs to act on a detected reference: where to send a
/// Task tap inside the app, and the resolved GitHub repository for opening a PR.
struct ReferenceContext {
    /// Resolved `https://github.com/owner/repo` for the Wave's repo, or nil when
    /// the origin remote isn't a usable GitHub URL. Drives the PR popover's
    /// external link; when nil the popover discloses the PR without one.
    let githubBase: URL?
    /// Navigate to a Task's detail in the plan pane (the existing child-selection
    /// hook). The argument is the Linear issue key, e.g. `W2-174`.
    let onOpenTask: (String) -> Void

    @MainActor
    static let inert = ReferenceContext(githubBase: nil, onOpenTask: { _ in })
}

/// Selectable prose for a Chat turn that renders typed references (`W2-174`,
/// `PR #889`) as inline links. Backed by an `NSTextView` so text selection,
/// copy, and drag survive; reference ranges carry a `.link` attribute and, on
/// click, disclose a compact popover anchored at the reference. With no
/// references it behaves exactly like plain selectable prose.
///
/// Replaces the earlier plain `SelectableAssistantMessageTextView` and the
/// user-turn `Text`, so both roles linkify references through one implementation.
struct ReferenceTextView: NSViewRepresentable {
    @Environment(\.palette) private var palette

    let text: String
    let font: NSFont
    let textColor: Color
    let references: ReferenceContext

    init(
        text: String,
        font: NSFont = NSFont(name: "Lato", size: 14) ?? .systemFont(ofSize: 14),
        textColor: Color,
        references: ReferenceContext
    ) {
        self.text = text
        self.font = font
        self.textColor = textColor
        self.references = references
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(references: references)
    }

    func makeNSView(context: Context) -> ReferenceMessageTextView {
        let view = ReferenceMessageTextView(frame: .zero, textContainer: nil)
        view.delegate = context.coordinator
        applyLinkStyle(to: view)
        view.textStorage?.setAttributedString(attributed())
        return view
    }

    func updateNSView(_ nsView: ReferenceMessageTextView, context: Context) {
        context.coordinator.references = references
        applyLinkStyle(to: nsView)
        let next = attributed()
        if nsView.textStorage?.string != next.string
            || nsView.cachedTextColor != NSColor(textColor) {
            nsView.textStorage?.setAttributedString(next)
            nsView.cachedTextColor = NSColor(textColor)
            nsView.invalidateIntrinsicContentSize()
        }
    }

    private func applyLinkStyle(to view: NSTextView) {
        view.linkTextAttributes = [
            .foregroundColor: NSColor(palette.accent),
            .underlineStyle: NSUnderlineStyle.single.rawValue,
            .cursor: NSCursor.pointingHand,
        ]
    }

    private func attributed() -> NSAttributedString {
        Self.attributedString(
            for: text,
            font: font,
            textColor: NSColor(textColor),
            accentColor: NSColor(palette.accent),
            references: parseChatReferences(in: text)
        )
    }

    /// Build the styled string. Reference ranges carry the reference `.link` URL
    /// plus accent color and underline; everything else is base prose. Kept
    /// static and free of the text view so it can be asserted directly.
    nonisolated static func attributedString(
        for text: String,
        font: NSFont,
        textColor: NSColor,
        accentColor: NSColor,
        references: [ChatReferenceMatch]
    ) -> NSAttributedString {
        let result = NSMutableAttributedString(
            string: text,
            attributes: [.font: font, .foregroundColor: textColor]
        )
        for match in references {
            guard let url = referenceURL(kind: match.kind, identifier: match.identifier) else {
                continue
            }
            let nsRange = NSRange(match.range, in: text)
            result.addAttributes([
                .link: url,
                .foregroundColor: accentColor,
                .underlineStyle: NSUnderlineStyle.single.rawValue,
            ], range: nsRange)
        }
        return result
    }

    // MARK: - Reference URL scheme

    static let scheme = "x-loopflow-ref"

    nonisolated static func referenceURL(kind: ChatReferenceKind, identifier: String) -> URL? {
        let encoded = identifier.addingPercentEncoding(
            withAllowedCharacters: .alphanumerics
        ) ?? identifier
        return URL(string: "\(scheme)://\(kind.rawValue)/\(encoded)")
    }

    nonisolated static func decodeReference(_ url: URL) -> (kind: ChatReferenceKind, identifier: String)? {
        guard url.scheme == scheme,
              let host = url.host,
              let kind = ChatReferenceKind(rawValue: host) else { return nil }
        // `URL.path` is already percent-decoded.
        let identifier = url.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        guard !identifier.isEmpty else { return nil }
        return (kind, identifier)
    }

    // MARK: - Coordinator

    @MainActor
    final class Coordinator: NSObject, NSTextViewDelegate {
        var references: ReferenceContext
        private var popover: NSPopover?

        init(references: ReferenceContext) {
            self.references = references
        }

        func textView(_ textView: NSTextView, clickedOnLink link: Any, at charIndex: Int) -> Bool {
            guard let url = linkURL(link),
                  let (kind, identifier) = ReferenceTextView.decodeReference(url) else {
                return false
            }
            present(kind: kind, identifier: identifier, in: textView, at: charIndex)
            return true
        }

        private func linkURL(_ link: Any) -> URL? {
            if let url = link as? URL { return url }
            if let string = link as? String { return URL(string: string) }
            return nil
        }

        private func present(
            kind: ChatReferenceKind,
            identifier: String,
            in textView: NSTextView,
            at charIndex: Int
        ) {
            popover?.close()

            let anchor = linkRect(in: textView, at: charIndex)
            let externalURL = kind == .pullRequest
                ? githubPullRequestURL(base: references.githubBase, number: identifier)
                : nil

            let popover = NSPopover()
            popover.behavior = .transient
            let content = ReferencePopover(
                kind: kind,
                identifier: identifier,
                externalURL: externalURL,
                open: { [weak self] in
                    self?.act(kind: kind, identifier: identifier, externalURL: externalURL)
                }
            )
            popover.contentViewController = NSHostingController(rootView: content)
            popover.show(relativeTo: anchor, of: textView, preferredEdge: .maxY)
            self.popover = popover
        }

        private func act(kind: ChatReferenceKind, identifier: String, externalURL: URL?) {
            popover?.close()
            switch kind {
            case .task:
                references.onOpenTask(identifier)
            case .pullRequest:
                if let externalURL { NSWorkspace.shared.open(externalURL) }
            case .project, .evidence:
                break
            }
        }

        /// The screen rect of the clicked link's full range, so the popover points
        /// at the reference rather than the whole paragraph.
        private func linkRect(in textView: NSTextView, at charIndex: Int) -> NSRect {
            guard let layoutManager = textView.layoutManager,
                  let container = textView.textContainer,
                  let storage = textView.textStorage else {
                return textView.bounds
            }
            var linkRange = NSRange(location: charIndex, length: 1)
            _ = storage.attribute(.link, at: charIndex, effectiveRange: &linkRange)
            let glyphRange = layoutManager.glyphRange(
                forCharacterRange: linkRange,
                actualCharacterRange: nil
            )
            var rect = layoutManager.boundingRect(forGlyphRange: glyphRange, in: container)
            let origin = textView.textContainerOrigin
            rect.origin.x += origin.x
            rect.origin.y += origin.y
            return rect
        }
    }
}

/// Autosizing selectable `NSTextView` that flows in a SwiftUI `VStack`. Sizes to
/// its content height while tracking the container width, and stays non-editable
/// so `.link` ranges are clickable rather than mutable.
final class ReferenceMessageTextView: NSTextView {
    var cachedTextColor: NSColor?

    override init(frame frameRect: NSRect, textContainer container: NSTextContainer?) {
        let textStorage = NSTextStorage()
        let layoutManager = NSLayoutManager()
        let textContainer = NSTextContainer(
            size: NSSize(width: frameRect.width, height: .greatestFiniteMagnitude)
        )
        textContainer.widthTracksTextView = true
        textContainer.lineFragmentPadding = 0

        textStorage.addLayoutManager(layoutManager)
        layoutManager.addTextContainer(textContainer)

        super.init(frame: frameRect, textContainer: textContainer)

        isEditable = false
        isSelectable = true
        drawsBackground = false
        isRichText = false
        allowsUndo = false
        importsGraphics = false
        textContainerInset = NSSize(width: 0, height: 0)
        isVerticallyResizable = true
        isHorizontallyResizable = false
        autoresizingMask = [.width]

        font = NSFont(name: "Lato", size: 14) ?? .systemFont(ofSize: 14)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var intrinsicContentSize: NSSize {
        guard let layoutManager, let textContainer else {
            return super.intrinsicContentSize
        }
        layoutManager.ensureLayout(for: textContainer)
        let usedRect = layoutManager.usedRect(for: textContainer)
        let height = ceil(usedRect.height + textContainerInset.height * 2)
        return NSSize(width: NSView.noIntrinsicMetric, height: max(20, height))
    }

    override func layout() {
        super.layout()
        invalidateIntrinsicContentSize()
    }

    override var frame: NSRect {
        didSet {
            if oldValue.size.width != frame.size.width {
                invalidateIntrinsicContentSize()
            }
        }
    }
}

/// Compact disclosure for a tapped reference: type, identifier, and one action.
private struct ReferencePopover: View {
    @Environment(\.palette) private var palette

    let kind: ChatReferenceKind
    let identifier: String
    let externalURL: URL?
    let open: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(typeLabel)
                .font(Typography.caption(10).weight(.semibold))
                .foregroundStyle(palette.textSecondary)
                .textCase(.uppercase)
            Text(identifier)
                .font(Typography.body().weight(.medium))
                .foregroundStyle(palette.text)
                .textSelection(.enabled)
            if let action = actionLabel {
                Button(action: open) {
                    HStack(spacing: Spacing.xs) {
                        Image(systemName: actionIcon)
                        Text(action)
                    }
                    .font(Typography.caption())
                }
                .buttonStyle(.borderless)
                .foregroundStyle(palette.accent)
            }
        }
        .padding(Spacing.md)
        .frame(minWidth: 160, maxWidth: 260, alignment: .leading)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(typeLabel) \(identifier)")
    }

    private var typeLabel: String {
        switch kind {
        case .task: return "Task"
        case .pullRequest: return "Pull request"
        case .project: return "Project"
        case .evidence: return "Evidence"
        }
    }

    private var actionLabel: String? {
        switch kind {
        case .task: return "Show in plan"
        case .pullRequest: return externalURL == nil ? nil : "Open on GitHub"
        case .project, .evidence: return nil
        }
    }

    private var actionIcon: String {
        switch kind {
        case .task: return "list.bullet.rectangle"
        case .pullRequest: return "arrow.up.right.square"
        case .project, .evidence: return "arrow.up.right.square"
        }
    }
}

/// Resolve a Wave repo's GitHub base URL from its `origin` remote (local `git`,
/// no network). Returns nil when the remote isn't GitHub or can't be read.
func resolveGitHubBase(repoPath: String) -> URL? {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
    process.arguments = ["git", "-C", repoPath, "remote", "get-url", "origin"]
    let pipe = Pipe()
    process.standardOutput = pipe
    process.standardError = FileHandle.nullDevice
    do {
        try process.run()
    } catch {
        return nil
    }
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    guard process.terminationStatus == 0,
          let remote = String(data: data, encoding: .utf8) else { return nil }
    return githubRepoBase(fromRemote: remote)
}

#endif
