#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// One read-only TextKit 2 code surface paired with a pinned line-number and
/// change-mark gutter. Plain monospace text mounts immediately; Tree-sitter
/// syntax colors and outline arrive off-main. Inline comments and symbol peeks
/// open beneath their line via `paragraphSpacing` without disturbing sideways scroll.
struct TaskCodeReader: View {
    let path: String
    let text: String
    var location: FileVisit?
    var store: TaskFilesStore?
    var revision: String?
    var addToConversation: ((String) -> Bool)?
    let ready: () -> Void
    @Environment(\.palette) private var palette
    @Environment(\.theme) private var theme

    private var detachedComments: [ReviewComment] {
        guard let store else { return [] }
        return store.comments.items.filter {
            $0.original?.path == path && $0.original?.source.isFile == true && $0.detached
        }
    }

    private var attachedComments: [ReviewComment] {
        guard let store else { return [] }
        return store.comments.items.filter {
            !$0.detached && $0.original?.path == path && $0.original?.source.isFile == true
        }
    }

    private var commentSelection: ReviewSelection? {
        guard let selection = store?.comments.selection,
              selection.path == path, selection.source.isFile else { return nil }
        return selection
    }

    private var inlinePeek: FileJumps.Peek? {
        guard let peek = store?.jumps.peek,
              peek.lookup != .outline, peek.origin.path == path else { return nil }
        return peek
    }

    private var changeMarks: [Int: ReviewLine.Kind] {
        guard let store,
              let review = store.diffReview.fileReview, review.path == path,
              review.editVersion == store.selectedDocument?.editVersion,
              review.baseCommit == store.workingChanges?.baseCommit,
              store.workingTree.filesByPath[path] != nil || store.selectedDocument?.dirty == true
        else { return [:] }
        return review.marks
    }

    private var target: FileJumps.Target? {
        guard let target = store?.jumps.target, target.path == path else { return nil }
        return target
    }

    private var findMatch: FileFind.Match? {
        guard let match = store?.find?.current, case .line = match.cell else { return nil }
        return match
    }

    private var findScan: FileFind.Scan<String> {
        let rev = store?.selectedDocument?.path == path
            ? "\(path)|\(store?.selectedDocument?.editVersion ?? 0)"
            : "\(path)|\(text.utf8.count)"
        return FileFind.Scan(query: store?.find?.query, revision: rev)
    }

    var body: some View {
        VStack(spacing: 0) {
            if let store, !detachedComments.isEmpty {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(detachedComments) {
                        ReviewCommentRow(store: store, comment: $0).padding(.horizontal, Spacing.sm)
                    }
                }
                Divider()
            }
            CodeReaderRepresentable(
                path: path,
                text: text,
                location: location,
                revision: revision,
                store: store,
                target: target,
                findMatch: findMatch,
                commentSelection: commentSelection,
                attachedComments: attachedComments,
                inlinePeek: inlinePeek,
                changeMarks: changeMarks,
                palette: palette,
                theme: theme,
                addToConversation: addToConversation
            )
            .onAppear(perform: ready)
            .accessibilityIdentifier("files-code-reader")
        }
        .onDisappear { store?.jumps.target = nil }
        .onChange(of: findMatch) { _, match in
            if let match, case .line(let line) = match.cell { store?.jumps.readingLine = line }
        }
        .task(id: findScan) {
            if let find = store?.find, !find.query.isEmpty {
                await find.search(text: text)
            }
        }
    }

    static let monoFont: NSFont = NSFont(name: Typography.monoFace, size: Typography.monoSize)
        ?? .monospacedSystemFont(ofSize: Typography.monoSize, weight: .regular)
}

// MARK: Line index

/// UTF-16 line offsets for a source string, computed in one linear pass.
struct CodeLineIndex: Sendable {
    struct Line: Sendable {
        /// UTF-16 offset where the line starts.
        let location: Int
        /// UTF-16 length excluding any trailing `\r` or `\n`.
        let contentLength: Int
        /// UTF-16 length including its line terminator.
        let fullLength: Int
    }

    let lines: [Line]
    let longestLineIndex: Int

    init(_ text: String) {
        let source = text as NSString
        let length = source.length
        var lines: [Line] = []
        lines.reserveCapacity(max(1, length / 40))
        var start = 0
        var longest = 0
        var longestLength = -1
        var i = 0
        while i < length {
            if source.character(at: i) == 0x0A { // \n
                let contentEnd = (i > start && source.character(at: i - 1) == 0x0D) ? i - 1 : i
                let contentLen = contentEnd - start
                let line = Line(location: start, contentLength: contentLen, fullLength: i - start + 1)
                if contentLen > longestLength {
                    longestLength = contentLen
                    longest = lines.count
                }
                lines.append(line)
                start = i + 1
            }
            i += 1
        }
        // Final line (or sole line for empty string / trailing newline).
        let contentEnd = (length > start && source.character(at: length - 1) == 0x0D) ? length - 1 : length
        let contentLen = contentEnd - start
        if contentLen > longestLength {
            longest = lines.count
        }
        lines.append(Line(location: start, contentLength: contentLen, fullLength: length - start))
        self.lines = lines
        self.longestLineIndex = longest
    }

    var count: Int { lines.count }

    func line(at number: Int) -> Line? {
        let index = number - 1
        return lines.indices.contains(index) ? lines[index] : nil
    }

    /// 1-based line number containing `utf16Offset`.
    func lineNumber(forUTF16Offset offset: Int) -> Int {
        guard !lines.isEmpty else { return 1 }
        var low = 0
        var high = lines.count - 1
        while low < high {
            let mid = (low + high + 1) / 2
            if lines[mid].location <= offset {
                low = mid
            } else {
                high = mid - 1
            }
        }
        return low + 1
    }

    func paragraphRange(forLine number: Int) -> NSRange? {
        line(at: number).map { NSRange(location: $0.location, length: $0.fullLength) }
    }

    func documentRange(line number: Int, rangeInLine: NSRange) -> NSRange? {
        guard let line = line(at: number), rangeInLine.location >= 0 else { return nil }
        let clampedStart = min(rangeInLine.location, line.contentLength)
        let clampedLength = min(rangeInLine.length, max(0, line.contentLength - clampedStart))
        guard clampedLength > 0 else { return nil }
        return NSRange(location: line.location + clampedStart, length: clampedLength)
    }

    func text(ofLine number: Int, in source: NSString) -> String {
        guard let line = line(at: number), line.location + line.contentLength <= source.length else { return "" }
        return source.substring(with: NSRange(location: line.location, length: line.contentLength))
    }
}

// MARK: AppKit views

/// Read-only TextKit 2 text view for source code: native single- and multi-line
/// drag selection, direct ⌘-click definition lookup, plain-click identifier
/// underline, and full-line background highlights.
@MainActor
final class CodeReaderTextView: NSTextView {
    private static let nonIdentifier = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "_$")).inverted

    var isInteractive = true
    var lineIndex = CodeLineIndex("")
    var filePath = ""
    var highlightedLines = IndexSet() {
        didSet { if highlightedLines != oldValue { needsDisplay = true } }
    }
    var lineHighlightColor: NSColor = .selectedTextBackgroundColor {
        didSet { if lineHighlightColor != oldValue { needsDisplay = true } }
    }
    var onPlainClick: ((Int?, FileJumps.Target?) -> Void)?
    var onCommandClick: ((Int, FileJumps.Target?) -> Void)?

    var defaultLineHeight: CGFloat {
        let font = self.font ?? TaskCodeReader.monoFont
        return ceil(font.ascender - font.descender + font.leading)
    }

    func lineNumber(for fragment: NSTextLayoutFragment) -> Int? {
        guard let contentManager = textContentStorage else { return nil }
        let offset = contentManager.offset(from: contentManager.documentRange.location, to: fragment.rangeInElement.location)
        return offset == NSNotFound ? nil : lineIndex.lineNumber(forUTF16Offset: offset)
    }

    func layoutFragment(forLine line: Int) -> NSTextLayoutFragment? {
        guard let layoutManager = textLayoutManager, let contentManager = textContentStorage,
              let info = lineIndex.line(at: line),
              let location = contentManager.location(contentManager.documentRange.location, offsetBy: info.location)
        else { return nil }
        layoutManager.ensureLayout(for: NSTextRange(location: location))
        return layoutManager.textLayoutFragment(for: location)
    }

    func textRange(for nsRange: NSRange) -> NSTextRange? {
        guard let contentManager = textContentStorage,
              let start = contentManager.location(contentManager.documentRange.location, offsetBy: nsRange.location),
              let end = contentManager.location(start, offsetBy: nsRange.length) else { return nil }
        return NSTextRange(location: start, end: end)
    }

    /// The 1-based line and fragments whose text line covers `containerY` (nil inside an inline slot gap or past EOF).
    func lineHit(atContainerY y: CGFloat) -> (line: Int, fragment: NSTextLayoutFragment, lineFragment: NSTextLineFragment)? {
        guard y >= 0, let layoutManager = textLayoutManager else { return nil }
        layoutManager.ensureLayout(for: CGRect(x: 0, y: max(0, y - 32), width: max(bounds.width, 1), height: 64))
        guard let fragment = layoutManager.textLayoutFragment(for: CGPoint(x: 0, y: y)) else { return nil }
        let localY = y - fragment.layoutFragmentFrame.minY
        guard let lineFragment = fragment.textLineFragments.first(where: {
            localY >= $0.typographicBounds.minY && localY <= $0.typographicBounds.maxY
        }), let number = lineNumber(for: fragment) else { return nil }
        return (number, fragment, lineFragment)
    }

    override func drawBackground(in rect: NSRect) {
        super.drawBackground(in: rect)
        guard !highlightedLines.isEmpty, let layoutManager = textLayoutManager else { return }
        let insetY = textContainerOrigin.y
        let searchY = max(0, rect.minY - insetY)
        let startLocation: NSTextLocation
        if let fragment = layoutManager.textLayoutFragment(for: CGPoint(x: 0, y: searchY)) {
            startLocation = fragment.rangeInElement.location
        } else if searchY <= 0 {
            startLocation = layoutManager.documentRange.location
        } else {
            return
        }
        lineHighlightColor.setFill()
        layoutManager.enumerateTextLayoutFragments(from: startLocation, options: []) { fragment in
            let y = fragment.layoutFragmentFrame.minY + insetY
            if y > rect.maxY { return false }
            if let line = self.lineNumber(for: fragment), self.highlightedLines.contains(line) {
                let height = fragment.textLineFragments.first?.typographicBounds.height ?? self.defaultLineHeight
                NSRect(x: rect.minX, y: y, width: rect.width, height: height).fill()
            }
            return true
        }
    }

    override func mouseDown(with event: NSEvent) {
        guard isInteractive else {
            super.mouseDown(with: event)
            return
        }
        let point = convert(event.locationInWindow, from: nil)
        // Single ⌘-click resolves directly without entering AppKit's modal multi-selection loop.
        if event.modifierFlags.contains(.command), event.clickCount == 1 {
            window?.makeFirstResponder(self)
            let hit = hitTestIdentifier(at: point)
            if let line = hit.line {
                onCommandClick?(line, hit.target)
            }
            return
        }
        super.mouseDown(with: event)
        finishPlainClick(at: point, clickCount: event.clickCount, modifiers: event.modifierFlags)
    }

    func finishPlainClick(at point: CGPoint, clickCount: Int, modifiers: NSEvent.ModifierFlags) {
        if selectedRange().length == 0, clickCount == 1, !modifiers.contains(.shift) {
            let hit = hitTestIdentifier(at: point)
            onPlainClick?(hit.line, hit.target)
        }
    }

    /// Resolve the 1-based line number and identifier under `point` (in view coordinates).
    func hitTestIdentifier(at point: CGPoint) -> (line: Int?, target: FileJumps.Target?) {
        let containerPoint = CGPoint(x: point.x - textContainerOrigin.x, y: point.y - textContainerOrigin.y)
        guard let (number, fragment, lineFragment) = lineHit(atContainerY: containerPoint.y) else { return (nil, nil) }
        let localX = containerPoint.x - fragment.layoutFragmentFrame.minX
        guard localX >= 0, localX < lineFragment.typographicBounds.maxX else { return (number, nil) }
        var indexInLine = lineFragment.characterIndex(for: CGPoint(x: localX, y: lineFragment.typographicBounds.midY))
        let lineText = lineIndex.text(ofLine: number, in: string as NSString)
        let maxIndex = (lineText as NSString).length
        // Caret index snaps to the nearest glyph edge; the right half of a glyph reports the next index.
        if indexInLine > 0 {
            let clamped = min(indexInLine, lineFragment.characterRange.length)
            if lineFragment.locationForCharacter(at: clamped).x > localX {
                indexInLine = clamped - 1
            }
        }
        guard indexInLine >= 0, indexInLine < maxIndex else { return (number, nil) }
        let target = Self.identifier(in: lineText, path: filePath, line: number, at: indexInLine)
        return (number, target)
    }

    /// Extract the identifier spanning `utf16Index` in `lineText`.
    static func identifier(in lineText: String, path: String, line number: Int, at utf16Index: Int) -> FileJumps.Target? {
        let source = lineText as NSString
        guard utf16Index >= 0, utf16Index < source.length else { return nil }
        guard source.rangeOfCharacter(from: nonIdentifier, options: [], range: NSRange(location: utf16Index, length: 1)).location == NSNotFound else {
            return nil
        }
        let before = source.rangeOfCharacter(from: nonIdentifier, options: .backwards, range: NSRange(location: 0, length: utf16Index))
        let after = source.rangeOfCharacter(from: nonIdentifier, options: [], range: NSRange(location: utf16Index, length: source.length - utf16Index))
        let start = before.location == NSNotFound ? 0 : NSMaxRange(before)
        let end = after.location == NSNotFound ? source.length : after.location
        guard start < end else { return nil }
        let range = NSRange(location: start, length: end - start)
        return FileJumps.Target(path: path, line: number, word: source.substring(with: range), range: range)
    }
}

/// Pinned line-number and change-mark gutter drawn directly against the text view's
/// TextKit 2 layout fragments so line numbers never scroll off horizontally and stay
/// aligned around inline comment and peek slots.
@MainActor
final class CodeReaderGutterView: NSView {
    override var isFlipped: Bool { true }

    weak var scrollView: NSScrollView?
    weak var textView: CodeReaderTextView?
    var isInteractive = true
    var palette: LoopflowPalette? {
        didSet { needsDisplay = true }
    }
    var font: NSFont = TaskCodeReader.monoFont {
        didSet { cachedDigitAdvance = nil; needsDisplay = true }
    }
    var selectedLines = IndexSet() {
        didSet { if selectedLines != oldValue { needsDisplay = true } }
    }
    var changeMarks: [Int: ReviewLine.Kind] = [:] {
        didSet { if changeMarks != oldValue { needsDisplay = true } }
    }
    var onSelectLine: ((Int, Bool) -> Void)?

    private var hoveredLine: Int? {
        didSet { if hoveredLine != oldValue { needsDisplay = true } }
    }
    private var tracking: NSTrackingArea?
    private var cachedDigitAdvance: CGFloat?

    private var digitAdvance: CGFloat {
        if let cachedDigitAdvance { return cachedDigitAdvance }
        let width = ("0" as NSString).size(withAttributes: [.font: font]).width
        cachedDigitAdvance = width
        return width
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        guard isInteractive else { return }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        tracking = area
    }

    override func mouseMoved(with event: NSEvent) {
        guard isInteractive else { return }
        let point = convert(event.locationInWindow, from: nil)
        hoveredLine = lineNumber(atY: point.y)
    }

    override func mouseExited(with event: NSEvent) {
        hoveredLine = nil
    }

    override func mouseDown(with event: NSEvent) {
        guard isInteractive else { return }
        let point = convert(event.locationInWindow, from: nil)
        if let line = lineNumber(atY: point.y) {
            onSelectLine?(line, event.modifierFlags.contains(.shift))
        }
    }

    /// 1-based line number whose text line covers `y` (nil inside an inline slot gap or past EOF).
    func lineNumber(atY y: CGFloat) -> Int? {
        guard let scrollView, let textView else { return nil }
        let containerY = y + scrollView.contentView.bounds.minY - textView.textContainerOrigin.y
        return textView.lineHit(atContainerY: containerY)?.line
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let palette else { return }
        NSColor(palette.background).setFill()
        dirtyRect.fill()
        guard let scrollView, let textView, let layoutManager = textView.textLayoutManager else { return }
        let scrollY = scrollView.contentView.bounds.minY
        let insetY = textView.textContainerOrigin.y
        let searchY = max(0, dirtyRect.minY + scrollY - insetY)
        var firstFragment = layoutManager.textLayoutFragment(for: CGPoint(x: 0, y: searchY))
        if firstFragment == nil, searchY <= textView.bounds.height {
            layoutManager.ensureLayout(for: CGRect(x: 0, y: searchY, width: max(textView.bounds.width, 1), height: dirtyRect.height + 64))
            firstFragment = layoutManager.textLayoutFragment(for: CGPoint(x: 0, y: searchY))
        }
        let startLocation: NSTextLocation
        if let fragment = firstFragment {
            startLocation = fragment.rangeInElement.location
        } else if searchY <= 0 {
            startLocation = layoutManager.documentRange.location
        } else {
            return
        }

        let defaultHeight = textView.defaultLineHeight
        let advance = digitAdvance
        let rightEdge = bounds.width - Spacing.xs
        let secondaryInk = NSColor(palette.textSecondary)
        let primaryInk = NSColor(palette.text)
        let tertiaryInk = NSColor(palette.textTertiary)
        let hoverFill = NSColor(palette.surfaceElement)

        layoutManager.enumerateTextLayoutFragments(from: startLocation, options: []) { fragment in
            let y = fragment.layoutFragmentFrame.minY + insetY - scrollY
            if y > dirtyRect.maxY { return false }
            guard let line = textView.lineNumber(for: fragment) else { return true }
            let lineBounds = fragment.textLineFragments.first?.typographicBounds
                ?? CGRect(x: 0, y: 0, width: 0, height: defaultHeight)
            let rowRect = NSRect(x: 0, y: y + lineBounds.minY, width: self.bounds.width, height: lineBounds.height)
            guard rowRect.maxY >= dirtyRect.minY else { return true }

            let tone: ThemeTone? = self.changeMarks[line].map {
                $0 == .removed ? palette.failureTone : palette.successTone
            }
            let selected = self.selectedLines.contains(line)
            let hovered = self.hoveredLine == line

            if hovered {
                hoverFill.setFill()
                rowRect.fill()
            } else if let tone {
                NSColor(tone.fill).setFill()
                rowRect.fill()
            }

            let ink: NSColor
            if !self.isInteractive {
                ink = secondaryInk
            } else if selected || hovered {
                ink = primaryInk
            } else if let tone {
                ink = NSColor(tone.ink.opacity(0.8))
            } else {
                ink = tertiaryInk
            }

            let label = String(line) as NSString
            let x = max(Spacing.sm, rightEdge - CGFloat(label.length) * advance)
            label.draw(at: NSPoint(x: x, y: rowRect.minY), withAttributes: [
                .font: self.font,
                .foregroundColor: ink,
            ])
            return true
        }
    }
}

/// Hosts the pinned `CodeReaderGutterView` beside `NSScrollView` + `CodeReaderTextView`.
@MainActor
final class CodeReaderContainerView: NSView {
    override var isFlipped: Bool { true }

    let gutterView: CodeReaderGutterView
    let scrollView: NSScrollView
    let textView: CodeReaderTextView
    var gutterWidth: CGFloat = ReviewStream.gutterWidth + Spacing.sm + Spacing.xs {
        didSet { if gutterWidth != oldValue { needsLayout = true } }
    }
    var widestLineWidth: CGFloat = 0 {
        didSet { if widestLineWidth != oldValue { needsLayout = true } }
    }
    var onLayout: (() -> Void)?

    init(interactive: Bool) {
        gutterView = CodeReaderGutterView()
        scrollView = NSScrollView()
        textView = CodeReaderTextView(usingTextLayoutManager: true)
        super.init(frame: .zero)
        // Since macOS 14 views don't clip by default, and AppKit hands the gutter dirty
        // rects reaching past it; its background fill would paint over the Files header and rail.
        clipsToBounds = true
        gutterView.clipsToBounds = true

        textView.isInteractive = interactive
        textView.isEditable = false
        textView.isSelectable = true
        textView.isRichText = false
        textView.usesFindBar = false
        textView.drawsBackground = true
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = true
        textView.autoresizingMask = []
        textView.font = TaskCodeReader.monoFont
        textView.textContainerInset = NSSize(width: Spacing.md, height: Spacing.sm)
        textView.textContainer?.widthTracksTextView = false
        textView.textContainer?.heightTracksTextView = false
        textView.textContainer?.lineFragmentPadding = 0
        textView.textContainer?.containerSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)

        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.drawsBackground = true
        scrollView.documentView = textView
        scrollView.contentView.postsBoundsChangedNotifications = true

        gutterView.isInteractive = interactive
        gutterView.scrollView = scrollView
        gutterView.textView = textView
        if interactive {
            gutterView.toolTip = "Select line; ⇧-click extends"
        }

        addSubview(gutterView)
        addSubview(scrollView)
        setAccessibilityElement(false)
        setAccessibilityIdentifier("files-code-reader")
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    override func layout() {
        super.layout()
        gutterView.frame = NSRect(x: 0, y: 0, width: gutterWidth, height: bounds.height)
        let scrollWidth = max(0, bounds.width - gutterWidth)
        scrollView.frame = NSRect(x: gutterWidth, y: 0, width: scrollWidth, height: bounds.height)
        let minWidth = max(scrollWidth, ceil(widestLineWidth + 2 * textView.textContainerInset.width))
        let minHeight = max(bounds.height, 0)
        textView.minSize = NSSize(width: minWidth, height: minHeight)
        if abs(textView.frame.width - minWidth) > 0.5 || textView.frame.height < minHeight {
            textView.setFrameSize(NSSize(width: minWidth, height: max(textView.frame.height, minHeight)))
        }
        gutterView.needsDisplay = true
        onLayout?()
    }
}

// MARK: SwiftUI bridge & coordinator

private struct CodeReaderRepresentable: NSViewRepresentable {
    let path: String
    let text: String
    let location: FileVisit?
    let revision: String?
    let store: TaskFilesStore?
    let target: FileJumps.Target?
    let findMatch: FileFind.Match?
    let commentSelection: ReviewSelection?
    let attachedComments: [ReviewComment]
    let inlinePeek: FileJumps.Peek?
    let changeMarks: [Int: ReviewLine.Kind]
    let palette: LoopflowPalette
    let theme: ThemeDefinition
    let addToConversation: ((String) -> Bool)?

    func makeCoordinator() -> CodeReaderCoordinator { CodeReaderCoordinator() }

    func makeNSView(context: Context) -> CodeReaderContainerView {
        let container = CodeReaderContainerView(interactive: store != nil)
        context.coordinator.attach(to: container)
        return container
    }

    func updateNSView(_ container: CodeReaderContainerView, context: Context) {
        context.coordinator.update(container: container, state: self)
    }

    static func dismantleNSView(_ container: CodeReaderContainerView, coordinator: CodeReaderCoordinator) {
        coordinator.detach()
    }
}

@MainActor
final class CodeReaderCoordinator {
    private struct SlotHost {
        let view: NSHostingView<AnyView>
        var height: CGFloat
    }

    private weak var container: CodeReaderContainerView?
    private var boundsObserver: NSObjectProtocol?
    private var parseTask: Task<Void, Never>?

    private struct RenderSignature: Equatable {
        let targetLine: Int?
        let targetRange: NSRange?
        let hit: TaskGrepHit?
        let revision: String?
        let findMatch: FileFind.Match?
        let theme: ThemeDefinition
    }

    private var currentPath: String?
    private var currentText: String?
    private var currentTheme: ThemeDefinition?
    private var highlights: [Grammar.Highlight] = []
    private var highlightsVersion = 0

    private var previousRenderedRanges: [NSTextRange] = []
    private var lastRenderSignature: RenderSignature?
    private var slotHosts: [Int: SlotHost] = [:]
    private var appliedSpacings: [Int: CGFloat] = [:]

    private struct ScrollTarget: Equatable {
        let visitID: UUID?
        let line: Int
        let hit: TaskGrepHit?
    }

    private var lastScrollTarget: ScrollTarget?
    private var lastFindMatch: FileFind.Match?
    private var pendingScroll: (line: Int, rangeInLine: NSRange?)?
    private var isProgrammaticScroll = false
    private var lastScrollY: CGFloat = 0

    private weak var store: TaskFilesStore?

    func attach(to container: CodeReaderContainerView) {
        self.container = container
        container.onLayout = { [weak self] in self?.handleLayout() }
        container.textView.onPlainClick = { [weak self] line, target in
            self?.handleClick(line: line, target: target, lookup: false)
        }
        container.textView.onCommandClick = { [weak self] line, target in
            self?.handleClick(line: line, target: target, lookup: true)
        }
        container.gutterView.onSelectLine = { [weak self] line, extend in
            self?.handleGutterSelect(line: line, extend: extend)
        }
        boundsObserver = NotificationCenter.default.addObserver(
            forName: NSView.boundsDidChangeNotification,
            object: container.scrollView.contentView,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.handleScroll() }
        }
    }

    func detach() {
        parseTask?.cancel()
        parseTask = nil
        if let boundsObserver { NotificationCenter.default.removeObserver(boundsObserver) }
        boundsObserver = nil
        for (_, slot) in slotHosts { slot.view.removeFromSuperview() }
        slotHosts.removeAll()
        appliedSpacings.removeAll()
    }

    fileprivate func update(container: CodeReaderContainerView, state: CodeReaderRepresentable) {
        store = state.store
        let textView = container.textView
        let gutterView = container.gutterView
        let font = TaskCodeReader.monoFont

        let textChanged = currentPath != state.path || currentText != state.text
        let themeChanged = currentTheme != state.theme

        if textChanged {
            parseTask?.cancel()
            parseTask = nil
            currentPath = state.path
            currentText = state.text
            highlights = []
            highlightsVersion += 1
            clearRenderingAttributes(in: textView)
            lastRenderSignature = nil
            clearAllSlots(in: textView)
            lastScrollTarget = nil
            lastFindMatch = nil
            pendingScroll = nil
            if let store = state.store, store.jumps.target != nil {
                Task { @MainActor [weak store] in store?.jumps.target = nil }
            }

            let index = CodeLineIndex(state.text)
            textView.lineIndex = index
            textView.filePath = state.path
            applyPalette(to: container, palette: state.palette, recolorStorage: false)
            textView.font = font
            textView.string = state.text
            currentTheme = state.theme

            // Measure only the single longest line (O(1) CoreText call) and size the gutter for max digits.
            let source = state.text as NSString
            let longestText = index.text(ofLine: index.longestLineIndex + 1, in: source)
            let longestWidth = (longestText as NSString).size(withAttributes: [.font: font]).width
            let digitWidth = (String(max(1, index.count)) as NSString).size(withAttributes: [.font: font]).width
            container.widestLineWidth = longestWidth
            container.gutterWidth = state.store != nil
                ? max(ReviewStream.gutterWidth, ceil(digitWidth)) + Spacing.sm + Spacing.xs
                : ceil(digitWidth) + Spacing.sm + Spacing.xs
            container.needsLayout = true

            scheduleParse(path: state.path, text: state.text, store: state.store)
        } else if themeChanged {
            currentTheme = state.theme
            applyPalette(to: container, palette: state.palette, recolorStorage: highlights.isEmpty)
            if !highlights.isEmpty {
                textView.textStorage?.paint(highlights, theme: state.theme, font: font)
            }
        }

        // Update gutter state.
        var selectedGutter = IndexSet()
        if let selection = state.commentSelection {
            selectedGutter.insert(integersIn: selection.first...selection.last)
        }
        gutterView.selectedLines = selectedGutter
        gutterView.changeMarks = state.changeMarks

        // Update full-line highlights (comment selection + FileVisit line range).
        var lineHighlights = selectedGutter
        if let lines = state.location?.lines {
            lineHighlights.insert(integersIn: lines)
        }
        textView.highlightedLines = lineHighlights
        textView.lineHighlightColor = NSColor(state.palette.keyboardFocus.opacity(0.3))

        // Update character-level rendering attributes (sub-ms, no NSTextStorage mutation).
        updateRenderingAttributes(in: textView, state: state)

        // Update inline comment and symbol-peek slots.
        updateSlots(in: container, state: state)

        // Programmatic scroll for Find match or FileVisit.
        if state.findMatch != lastFindMatch {
            lastFindMatch = state.findMatch
            if let match = state.findMatch, case .line(let line) = match.cell {
                scroll(container: container, toLine: line, rangeInLine: match.range)
            }
        }
        let scrollTarget = state.location.map { ScrollTarget(visitID: $0.visitID, line: $0.line, hit: $0.hit) }
        if scrollTarget != lastScrollTarget {
            lastScrollTarget = scrollTarget
            if let location = state.location {
                let rangeInLine = hitRange(for: location.hit, revision: state.revision, in: textView)
                scroll(container: container, toLine: location.line, rangeInLine: rangeInLine)
            }
        }
    }

    private func applyPalette(to container: CodeReaderContainerView, palette: LoopflowPalette, recolorStorage: Bool) {
        let textView = container.textView
        textView.applyPalette(palette)
        container.scrollView.backgroundColor = NSColor(palette.background)
        container.gutterView.palette = palette
        if recolorStorage, let storage = textView.textStorage, storage.length > 0 {
            let whole = NSRange(location: 0, length: storage.length)
            storage.beginEditing()
            storage.addAttribute(.foregroundColor, value: NSColor(palette.text), range: whole)
            storage.endEditing()
        }
    }

    private func scheduleParse(path: String, text: String, store: TaskFilesStore?) {
        let document = store?.selectedDocument
        let version = document?.path == path ? document?.editVersion : nil
        let needsOutline = version != nil
        let token = highlightsVersion
        parseTask = Task { [weak self, weak document] in
            let result = await Task.detached(priority: .userInitiated) {
                let grammar = Grammar.named(path)
                let tree = grammar?.parse(text)
                let highlights = tree.flatMap { grammar?.highlights(in: $0, text: text) } ?? []
                let outline = needsOutline ? FileOutline(tree: tree, text: text, path: path) : nil
                return (highlights, outline)
            }.value
            guard !Task.isCancelled, let self, self.highlightsVersion == token,
                  self.currentPath == path, self.currentText == text else { return }
            if let outline = result.1, let version { document?.receive(outline, version: version) }
            self.highlights = result.0
            if let container = self.container, let activeTheme = self.currentTheme {
                container.textView.textStorage?.paint(result.0, theme: activeTheme, font: TaskCodeReader.monoFont)
            }
        }
    }

    private func hitRange(for hit: TaskGrepHit?, revision: String?, in textView: CodeReaderTextView) -> NSRange? {
        guard let hit, hit.revision == revision else { return nil }
        return hit.range(in: textView.lineIndex.text(ofLine: hit.line, in: textView.string as NSString))
    }

    private func clearRenderingAttributes(in textView: CodeReaderTextView) {
        if let layoutManager = textView.textLayoutManager {
            for range in previousRenderedRanges {
                layoutManager.removeRenderingAttribute(.underlineStyle, for: range)
                layoutManager.removeRenderingAttribute(.underlineColor, for: range)
                layoutManager.removeRenderingAttribute(.backgroundColor, for: range)
            }
        }
        previousRenderedRanges.removeAll()
    }

    private func updateRenderingAttributes(in textView: CodeReaderTextView, state: CodeReaderRepresentable) {
        let signature = RenderSignature(
            targetLine: state.target?.line,
            targetRange: state.target?.range,
            hit: state.location?.hit,
            revision: state.revision,
            findMatch: state.findMatch,
            theme: state.theme
        )
        guard signature != lastRenderSignature else { return }
        lastRenderSignature = signature

        guard let layoutManager = textView.textLayoutManager else { return }

        clearRenderingAttributes(in: textView)

        if let target = state.target,
           let docRange = textView.lineIndex.documentRange(line: target.line, rangeInLine: target.range),
           let range = textView.textRange(for: docRange) {
            layoutManager.addRenderingAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, for: range)
            layoutManager.addRenderingAttribute(.underlineColor, value: NSColor(state.palette.actionInk), for: range)
            previousRenderedRanges.append(range)
        }

        if let hit = state.location?.hit,
           let inLine = hitRange(for: hit, revision: state.revision, in: textView),
           let docRange = textView.lineIndex.documentRange(line: hit.line, rangeInLine: inLine),
           let range = textView.textRange(for: docRange) {
            layoutManager.addRenderingAttribute(.backgroundColor, value: NSColor(state.palette.keyboardFocus.opacity(0.3)), for: range)
            previousRenderedRanges.append(range)
        }

        if let match = state.findMatch, case .line(let line) = match.cell,
           let docRange = textView.lineIndex.documentRange(line: line, rangeInLine: match.range),
           let range = textView.textRange(for: docRange) {
            layoutManager.addRenderingAttribute(.backgroundColor, value: NSColor(state.palette.keyboardFocus.opacity(0.45)), for: range)
            previousRenderedRanges.append(range)
        }

        textView.needsDisplay = true
    }

    // MARK: Inline slots

    private func updateSlots(in container: CodeReaderContainerView, state: CodeReaderRepresentable) {
        let textView = container.textView
        guard let store = state.store else {
            if !slotHosts.isEmpty { clearAllSlots(in: textView) }
            return
        }

        // Collect all 1-based lines that host an editor, comment row, or symbol peek.
        var activeLines = Set<Int>()
        if let sel = state.commentSelection { activeLines.insert(sel.last) }
        for comment in state.attachedComments {
            if let last = comment.selection?.last { activeLines.insert(last) }
        }
        if let peek = state.inlinePeek, let peekLine = peek.origin.line { activeLines.insert(peekLine) }

        // Remove stale slots.
        let removed = Set(slotHosts.keys).subtracting(activeLines)
        var layoutChanged = !removed.isEmpty
        for line in removed {
            slotHosts.removeValue(forKey: line)?.view.removeFromSuperview()
            setParagraphSpacing(0, onLine: line, in: textView)
        }

        guard !activeLines.isEmpty else {
            if layoutChanged { container.gutterView.needsDisplay = true }
            return
        }

        let viewportWidth = max(200, container.scrollView.contentView.bounds.width)
        let source = textView.string as NSString

        for line in activeLines.sorted() {
            let selection = state.commentSelection?.last == line ? state.commentSelection : nil
            let comments = state.attachedComments.filter { $0.selection?.last == line }
            let peek = state.inlinePeek?.origin.line == line ? state.inlinePeek : nil
            let lineIndex = textView.lineIndex
            let addToConversation = state.addToConversation
            let root = AnyView(
                InlineSlotView(
                    store: store,
                    selection: selection,
                    excerpt: {
                        guard let selection else { return [] }
                        return (selection.first...selection.last).map { lineIndex.text(ofLine: $0, in: source) }
                    },
                    comments: comments,
                    peek: peek,
                    addToConversation: addToConversation,
                    onHeightChange: { [weak self] height in
                        self?.slotHeightChanged(line: line, height: height)
                    }
                )
                .environment(\.palette, state.palette)
                .environment(\.theme, state.theme)
            )
            let host: NSHostingView<AnyView>
            if let existing = slotHosts[line] {
                host = existing.view
                host.rootView = root
            } else {
                host = NSHostingView(rootView: root)
                host.translatesAutoresizingMaskIntoConstraints = true
                textView.addSubview(host)
                slotHosts[line] = SlotHost(view: host, height: 0)
            }
            host.frame.size.width = viewportWidth
            let measured = max(1, ceil(host.fittingSize.height))
            if slotHosts[line]?.height != measured {
                slotHosts[line]?.height = measured
                setParagraphSpacing(measured, onLine: line, in: textView)
                layoutChanged = true
            }
        }
        positionSlots(in: container)
        if layoutChanged { container.gutterView.needsDisplay = true }
    }

    private func slotHeightChanged(line: Int, height: CGFloat) {
        guard let container, var slot = slotHosts[line] else { return }
        let rounded = max(1, ceil(height))
        guard abs(slot.height - rounded) >= 1 else { return }
        slot.height = rounded
        slotHosts[line] = slot
        setParagraphSpacing(rounded, onLine: line, in: container.textView)
        positionSlots(in: container)
        container.gutterView.needsDisplay = true
    }

    private func setParagraphSpacing(_ spacing: CGFloat, onLine line: Int, in textView: CodeReaderTextView) {
        let current = appliedSpacings[line] ?? 0
        guard abs(current - spacing) >= 0.5 else { return }
        if spacing <= 0 {
            appliedSpacings.removeValue(forKey: line)
        } else {
            appliedSpacings[line] = spacing
        }
        guard let storage = textView.textStorage,
              let range = textView.lineIndex.paragraphRange(forLine: line),
              range.length > 0, NSMaxRange(range) <= storage.length else { return }
        let style = NSMutableParagraphStyle()
        style.paragraphSpacing = spacing
        storage.beginEditing()
        storage.addAttribute(.paragraphStyle, value: style, range: range)
        storage.endEditing()
    }

    private func positionSlots(in container: CodeReaderContainerView) {
        guard !slotHosts.isEmpty else { return }
        let textView = container.textView
        let clipBounds = container.scrollView.contentView.bounds
        let width = max(160, clipBounds.width)
        let insetY = textView.textContainerOrigin.y
        let defaultHeight = textView.defaultLineHeight

        for (line, slot) in slotHosts {
            guard let fragment = textView.layoutFragment(forLine: line) else { continue }
            let textHeight = fragment.textLineFragments.first?.typographicBounds.height ?? defaultHeight
            let y = fragment.layoutFragmentFrame.minY + insetY + textHeight
            let frame = NSRect(x: clipBounds.minX, y: y, width: width, height: slot.height)
            if slot.view.frame != frame {
                slot.view.frame = frame
            }
        }
        // Keep space below a last-line slot reachable by scrolling.
        if let lastSlot = slotHosts[textView.lineIndex.count] {
            let bottom = lastSlot.view.frame.maxY + Spacing.sm
            if textView.frame.height < bottom {
                textView.setFrameSize(NSSize(width: textView.frame.width, height: bottom))
            }
        }
    }

    private func clearAllSlots(in textView: CodeReaderTextView) {
        for (line, slot) in slotHosts {
            slot.view.removeFromSuperview()
            setParagraphSpacing(0, onLine: line, in: textView)
        }
        slotHosts.removeAll()
        appliedSpacings.removeAll()
    }

    // MARK: Scrolling & interaction

    func scroll(container: CodeReaderContainerView, toLine line: Int, rangeInLine: NSRange?) {
        let scrollView = container.scrollView
        let clipBounds = scrollView.contentView.bounds
        let visibleHeight = clipBounds.height
        let visibleWidth = clipBounds.width
        // A newly mounted file resizes the gutter and scroll view in the next layout, which resets
        // the clip view; `handleLayout` scrolls once those frames settle.
        guard visibleWidth > 0, visibleHeight > 0, !container.needsLayout else {
            pendingScroll = (line, rangeInLine)
            return
        }
        pendingScroll = nil
        let textView = container.textView
        guard let fragment = textView.layoutFragment(forLine: line) else { return }

        let insetY = textView.textContainerOrigin.y
        let lineHeight = fragment.textLineFragments.first?.typographicBounds.height ?? textView.defaultLineHeight
        let lineMidY = fragment.layoutFragmentFrame.minY + insetY + lineHeight / 2
        // The text view keeps the previous file's height until TextKit's own viewport pass, and the
        // clamp below would pin a far line to the top; TextKit sizes it when asked to reveal the line.
        isProgrammaticScroll = true
        defer { isProgrammaticScroll = false }
        if textView.frame.height < lineMidY + visibleHeight / 2, let info = textView.lineIndex.line(at: line) {
            textView.scrollRangeToVisible(NSRange(location: info.location, length: 0))
        }
        let maxY = max(0, textView.frame.height - visibleHeight)
        let targetY = min(maxY, max(0, lineMidY - visibleHeight / 2))

        var targetX: CGFloat = 0
        if let rangeInLine, let lineFragment = fragment.textLineFragments.first {
            let maxChar = lineFragment.characterRange.length
            let startIdx = min(max(0, rangeInLine.location), maxChar)
            let endIdx = min(max(startIdx, NSMaxRange(rangeInLine)), maxChar)
            let startX = lineFragment.locationForCharacter(at: startIdx).x + textView.textContainerOrigin.x
            let endX = lineFragment.locationForCharacter(at: endIdx).x + textView.textContainerOrigin.x
            if endX > visibleWidth - Spacing.md {
                let maxX = max(0, textView.frame.width - visibleWidth)
                targetX = min(maxX, max(0, startX + min(endX - startX, visibleWidth) / 2 - visibleWidth / 2))
            }
        }

        scrollView.contentView.scroll(to: CGPoint(x: targetX, y: targetY))
        scrollView.reflectScrolledClipView(scrollView.contentView)
        lastScrollY = scrollView.contentView.bounds.minY
        container.gutterView.needsDisplay = true
        positionSlots(in: container)
    }

    private func handleLayout() {
        guard let container else { return }
        if let pending = pendingScroll,
           container.scrollView.contentView.bounds.width > 0,
           container.scrollView.contentView.bounds.height > 0 {
            scroll(container: container, toLine: pending.line, rangeInLine: pending.rangeInLine)
            return
        }
        positionSlots(in: container)
    }

    private func handleScroll() {
        guard let container else { return }
        container.gutterView.needsDisplay = true
        if !slotHosts.isEmpty { positionSlots(in: container) }
        let scrollY = container.scrollView.contentView.bounds.minY
        guard !isProgrammaticScroll, abs(scrollY - lastScrollY) > 0.5 else { return }
        lastScrollY = scrollY
        guard let store, let layoutManager = container.textView.textLayoutManager else { return }
        let topY = max(0, scrollY - container.textView.textContainerOrigin.y + 4)
        if let fragment = layoutManager.textLayoutFragment(for: CGPoint(x: 0, y: topY)),
           let line = container.textView.lineNumber(for: fragment),
           store.jumps.readingLine != line {
            store.jumps.readingLine = line
        }
    }

    private func handleClick(line: Int?, target: FileJumps.Target?, lookup: Bool) {
        guard let store, store.filePicker == nil else { return }
        if store.browser.visit?.highlight != nil {
            store.browser.visit?.highlight = nil
        }
        if let line { store.jumps.readingLine = line }
        store.jumps.target = target
        if lookup, let line, let target {
            store.jumps.search(.definition(target.word), line: line, in: store)
        }
    }

    private func handleGutterSelect(line: Int, extend: Bool) {
        guard let store, let path = currentPath else { return }
        // The gutter's line range replaces any text selection, which would otherwise shade the comment slot.
        container?.textView.setSelectedRange(NSRange(location: 0, length: 0))
        store.comments.select(
            path: path,
            source: .file(draft: store.selectedDocument?.dirty == true),
            line: line,
            extend: extend
        )
    }
}

/// Inline review editor, attached comments, and symbol peek hosted inside a line's `paragraphSpacing` slot.
private struct InlineSlotView: View {
    let store: TaskFilesStore
    let selection: ReviewSelection?
    let excerpt: () -> [String]
    let comments: [ReviewComment]
    let peek: FileJumps.Peek?
    let addToConversation: ((String) -> Bool)?
    let onHeightChange: (CGFloat) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let selection {
                ReviewCommentEditor(store: store, selection: selection, excerpt: excerpt, addToConversation: addToConversation)
            }
            ForEach(comments) { comment in
                ReviewCommentRow(store: store, comment: comment)
            }
            if let peek {
                FileSymbolPeek(store: store, peek: peek)
                    .padding(.horizontal, Spacing.sm)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .onGeometryChange(for: CGFloat.self, of: \.size.height) { height in
            if height > 0 { onHeightChange(height) }
        }
    }
}

extension TaskGrepHit {
    func range(in text: String, excerptOffset: Int? = nil) -> NSRange? {
        let offset = excerptOffset ?? 0
        guard offset >= 0, start >= offset, end > start else { return nil }
        let bytes = text.utf8
        let upperOffset = excerptOffset == nil ? end : min(end - offset, bytes.count)
        guard let from = bytes.index(bytes.startIndex, offsetBy: start - offset, limitedBy: bytes.endIndex),
              let to = bytes.index(bytes.startIndex, offsetBy: upperOffset, limitedBy: bytes.endIndex),
              let lower = from.samePosition(in: text.unicodeScalars),
              let upper = to.samePosition(in: text.unicodeScalars) else { return nil }
        return NSRange(lower..<upper, in: text)
    }
}
#endif
