#if os(macOS)
import CoreText
import Loopflow
import SwiftUI

struct TaskCodeReader: View {
    let path: String
    let text: String
    var location: FileVisit?
    var store: TaskFilesStore?
    var revision: String?
    var addToConversation: ((String) -> Bool)?
    let ready: () -> Void
    @Environment(\.palette) private var palette
    @State private var parsedVersion = 0
    @State private var lines: [CodeLine]?
    @State private var widestLine: CGFloat = 0

    @State private var scrolling = false
    /// Not observed: the rows report their frames on every scroll without re-running the reader.
    @State var rows = RowFrames()
    nonisolated static let space = "files-code-reader"

    var body: some View {
        Group {
            if let lines {
                GeometryReader { viewport in
                    reader(lines, width: viewport.size.width)
                }
                .onAppear(perform: ready)
                .accessibilityIdentifier("files-code-reader")
            } else {
                ProgressView()
            }
        }
        .onDisappear { store?.jumps.target = nil }
        .task(id: text) {
            store?.jumps.target = nil
            let document = store?.selectedDocument
            let version = document?.editVersion
            let result = await Task.detached(priority: .userInitiated) { [text, path, needsOutline = document != nil] in
                let grammar = Grammar.named(path)
                let tree = grammar?.parse(text)
                let highlights = tree.flatMap { grammar?.highlights(in: $0, text: text) } ?? []
                let lines = CodeLine.read(text, highlights: highlights)
                let font = Self.monoFont()
                let widest = lines.reduce(0.0) { max($0, CTLineGetTypographicBounds(Self.typeset($1.text, font: font), nil, nil, nil)) }
                return (lines, needsOutline ? FileOutline(tree: tree, text: text, path: path) : nil, widest)
            }.value
            guard !Task.isCancelled else { return }
            if let outline = result.1, let version { document?.receive(outline, version: version) }
            lines = result.0
            widestLine = result.2
            parsedVersion += 1
        }
    }

    private func reader(_ lines: [CodeLine], width: CGFloat) -> some View {
        ScrollViewReader { proxy in
            ScrollView([.horizontal, .vertical]) {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if let store {
                        ForEach(store.comments.items.filter { $0.original.path == path && $0.original.source.isFile && $0.detached }) {
                            ReviewCommentRow(store: store, comment: $0).padding(.horizontal, Spacing.sm)
                        }
                    }
                    ForEach(lines.indices, id: \.self) { index in
                        row(index, in: lines)
                    }
                }
                .scrollTargetLayout()
                // A lazy stack takes its width from the rows it has measured, which leaves longer lines out of reach.
                .frame(minWidth: 2 * Spacing.sm + gutter(lines.count) + Spacing.md + widestLine, alignment: .leading)
                .font(Typography.mono)
                .padding(.vertical, Spacing.sm)
            }
            .defaultScrollAnchor(.topLeading, for: .alignment)
            .onScrollPhaseChange { _, phase in
                scrolling = phase == .tracking || phase == .interacting || phase == .decelerating
            }
            .onScrollTargetVisibilityChange(idType: Int.self, threshold: 0.1) { visible in
                if scrolling, let store, let first = visible.min() { store.jumps.readingLine = first + 1 }
            }
            .task(id: FileFind.Scan(query: store?.find?.query, revision: parsedVersion)) {
                if let find = store?.find, !find.query.isEmpty {
                    await find.search(lines: lines)
                }
            }
            .onChange(of: store?.find?.current) {
                guard let match = store?.find?.current, case .line(let line) = match.cell,
                      lines.indices.contains(line - 1) else { return }
                proxy.scrollTo(line - 1, anchor: scrollAnchor(line: line, text: lines[line - 1].text, range: match.range, width: width))
                store?.jumps.readingLine = line
            }
            .task(id: location) {
                guard let location, lines.indices.contains(location.line - 1) else { return }
                let anchor = scrollAnchor(for: location, text: lines[location.line - 1].text, width: width)
                proxy.scrollTo(location.line - 1, anchor: anchor)
            }
            .coordinateSpace(.named(Self.space))
            .background {
                if store != nil {
                    ReaderClicks { point, modifiers in click(at: point, command: modifiers.contains(.command), in: lines) }
                }
            }
        }
    }

    func row(_ index: Int, in lines: [CodeLine]) -> some View {
        let number = index + 1
        let isCommentSelected = selected(number)
        return VStack(alignment: .leading, spacing: 0) {
            NumberedLine(number: number, line: lines[index], marks: marks(number, in: lines[index]),
                         gutter: store.map { NumberedLine.Gutter(selected: isCommentSelected, tone: changeTone(number, in: $0)) },
                         rows: store == nil ? nil : rows,
                         select: store.map { store in {
                             store.comments.select(path: path, source: .file(draft: store.selectedDocument?.dirty == true),
                                                   line: number, extend: NSEvent.modifierFlags.contains(.shift))
                         } }).equatable()
            if let store {
                if let selection = store.comments.selection, isCommentSelected, selection.last == number {
                    ReviewCommentEditor(store: store, selection: selection, excerpt: {
                        lines[(selection.first - 1)...index].map(\.text)
                    }, addToConversation: addToConversation)
                }
                ForEach(store.comments.items.filter {
                    !$0.detached && $0.original.source.isFile && $0.original.path == path && $0.selection.last == number
                }) { ReviewCommentRow(store: store, comment: $0) }
                if let peek = store.jumps.peek, peek.lookup != .outline, peek.origin.line == number {
                    FileSymbolPeek(store: store, peek: peek)
                }
            }
        }
        // Inside the row, so scrolling to a row's leading edge keeps the margin.
        .padding(.horizontal, Spacing.sm)
    }

    private func changeTone(_ line: Int, in store: TaskFilesStore) -> ThemeTone? {
        guard let review = store.diffReview.fileReview, review.path == path,
              review.editVersion == store.selectedDocument?.editVersion,
              review.baseCommit == store.workingChanges?.baseCommit,
              store.workingTree.filesByPath[path] != nil || store.selectedDocument?.dirty == true,
              let mark = review.marks[line] else { return nil }
        return mark == .removed ? palette.failureTone : palette.successTone
    }

    private func selected(_ number: Int) -> Bool {
        guard let selection = store?.comments.selection, selection.path == path, selection.source.isFile else { return false }
        return (selection.first...selection.last).contains(number)
    }

    /// A click on a source line underlines the identifier under it; ⌘-click also looks up its definition.
    /// `point` is in the reader's coordinate space.
    func click(at point: CGPoint, command: Bool, in lines: [CodeLine]) {
        guard let store, store.filePicker == nil,
              let (number, frame) = rows.frames.first(where: { $0.value.contains(point) }),
              lines.indices.contains(number - 1) else { return }
        store.jumps.readingLine = number
        let local = CGPoint(x: point.x - frame.minX, y: point.y - frame.minY)
        let target = target(in: lines[number - 1].text, number: number, at: local)
        store.jumps.target = target
        if command, let target {
            store.jumps.search(.definition(target.word), line: number, in: store)
        }
    }

    private nonisolated static func monoFont() -> NSFont {
        NSFont(name: Typography.monoFace, size: Typography.monoSize)
            ?? .monospacedSystemFont(ofSize: Typography.monoSize, weight: .regular)
    }

    private nonisolated static func typeset(_ text: String, font: NSFont? = nil) -> CTLine {
        CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: [.font: font ?? monoFont()]))
    }

    func scrollAnchor(for visit: FileVisit, text: String, width: CGFloat) -> UnitPoint {
        let range = visit.hit.flatMap { $0.revision == revision ? $0.range(in: text) : nil }
        return scrollAnchor(line: visit.line, text: text, range: range, width: width)
    }

    /// Centers `line` vertically and keeps its start in view, scrolling sideways only as far as `range` needs.
    /// The default center anchor would scroll to the middle of the widest line.
    func scrollAnchor(line number: Int, text: String, range: NSRange?, width: CGFloat) -> UnitPoint {
        let leading = UnitPoint(x: 0, y: 0.5)
        guard let range else { return leading }
        let line = Self.typeset(text)
        let inset = Spacing.sm + gutter(number) + Spacing.md
        let overflow = inset + CTLineGetTypographicBounds(line, nil, nil, nil) + Spacing.sm - width
        guard overflow > 0 else { return leading }
        let start = CTLineGetOffsetForStringIndex(line, range.location, nil)
        let end = CTLineGetOffsetForStringIndex(line, NSMaxRange(range), nil)
        // scrollTo aligns the same fractional point in the row and the viewport.
        let offset = inset + start + min(end - start, width) / 2 - width / 2
        return UnitPoint(x: max(0, min(1, offset / overflow)), y: 0.5)
    }

    private func gutter(_ number: Int) -> CGFloat {
        store == nil ? CTLineGetTypographicBounds(Self.typeset(String(number)), nil, nil, nil) : ReviewStream.gutterWidth + Spacing.xs
    }

    func target(in text: String, number: Int, at point: CGPoint) -> FileJumps.Target? {
        let line = Self.typeset(text)
        var index = CTLineGetStringIndexForPosition(line, point)
        guard index != kCFNotFound, point.x >= 0,
              point.x < CTLineGetTypographicBounds(line, nil, nil, nil) else { return nil }
        // Core Text returns the nearest caret. The right half of a glyph
        // therefore names the insertion position after the clicked character.
        if CTLineGetOffsetForStringIndex(line, index, nil) > point.x { index -= 1 }
        let source = text as NSString
        let characters = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "_$")).inverted
        guard index >= 0, index < source.length else { return nil }
        let before = source.rangeOfCharacter(from: characters, options: .backwards, range: NSRange(location: 0, length: index))
        let after = source.rangeOfCharacter(from: characters, options: [], range: NSRange(location: index, length: source.length - index))
        let start = before.location == NSNotFound ? 0 : NSMaxRange(before)
        let end = after.location == NSNotFound ? source.length : after.location
        guard index < end else { return nil }
        let range = NSRange(location: start, length: end - start)
        return FileJumps.Target(path: path, line: number, word: source.substring(with: range), range: range)
    }

    /// What a row shows besides its syntax colors, as values: a change elsewhere repaints no other line.
    func marks(_ number: Int, in line: CodeLine) -> NumberedLine.Marks {
        var marks = NumberedLine.Marks()
        if let target = store?.jumps.target, target.path == path, target.line == number { marks.underline = target.range }
        marks.selected = selected(number) || location?.lines?.contains(number) == true
        if let hit = location?.hit, hit.line == number, hit.revision == revision { marks.hit = hit.range(in: line.text) }
        if let match = store?.find?.current, match.cell == .line(number) { marks.find = match.range }
        return marks
    }
}

/// A line number and its source. Equatable on what they show, so the rows a lazy
/// stack keeps built skip their bodies when the reader redraws for another line.
struct NumberedLine: View, Equatable {
    struct Marks: Equatable, Sendable {
        var underline: NSRange?
        var selected = false
        var hit: NSRange?
        var find: NSRange?
    }

    struct Gutter: Equatable, Sendable {
        let selected: Bool
        let tone: ThemeTone?
    }

    let number: Int
    let line: CodeLine
    let marks: Marks
    let gutter: Gutter?
    let rows: RowFrames?
    let select: (() -> Void)?
    @Environment(\.palette) private var palette
    @Environment(\.theme) private var theme

    nonisolated static func == (a: Self, b: Self) -> Bool {
        a.number == b.number && a.marks == b.marks && a.line == b.line && a.gutter == b.gutter
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
            if let gutter, let rows, let select {
                ReviewGutter(number: number, selected: gutter.selected, tone: gutter.tone, select: select)
                Text(painted).textSelection(.enabled)
                    .onGeometryChange(for: CGRect.self) { $0.frame(in: .named(TaskCodeReader.space)) } action: { [number] in
                        rows.frames[number] = $0
                    }
                    .onDisappear { [number] in rows.frames[number] = nil }
            } else {
                Text(verbatim: String(number)).foregroundStyle(palette.textSecondary)
                Text(painted).textSelection(.enabled)
            }
        }
        // Code scrolls sideways; a wrapped line would read as two lines beside one number.
        .fixedSize(horizontal: true, vertical: false)
    }

    private var painted: AttributedString {
        var value = AttributedString(line.text)
        value.paint(line.highlights, inks: .of(theme))
        if let underline = marks.underline, let range = Range(underline, in: value) {
            value[range].underlineStyle = Text.LineStyle(pattern: .solid, color: palette.actionInk)
        }
        if marks.selected { value.backgroundColor = palette.keyboardFocus.opacity(0.3) }
        if let hit = marks.hit, let range = Range(hit, in: value) {
            value[range].backgroundColor = palette.keyboardFocus.opacity(0.3)
        }
        if let find = marks.find, let range = Range(find, in: value) {
            value[range].backgroundColor = palette.keyboardFocus.opacity(0.45)
        }
        return value
    }
}

final class RowFrames {
    var frames: [Int: CGRect] = [:]
}

/// Selectable text takes a click before any SwiftUI gesture sees it, so the reader
/// watches its window's mouse events instead. Events pass through untouched, keeping
/// drag-selection and copy; a press released where it started counts as a click.
struct ReaderClicks: NSViewRepresentable {
    let clicked: (CGPoint, NSEvent.ModifierFlags) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(clicked: clicked) }

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        context.coordinator.install(for: view)
        return view
    }

    func updateNSView(_ view: NSView, context: Context) { context.coordinator.clicked = clicked }

    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) { coordinator.uninstall() }

    @MainActor final class Coordinator {
        var clicked: (CGPoint, NSEvent.ModifierFlags) -> Void
        weak var view: NSView?
        private var monitor: Any?
        private var pressed: CGPoint?

        init(clicked: @escaping (CGPoint, NSEvent.ModifierFlags) -> Void) { self.clicked = clicked }

        func install(for view: NSView) {
            self.view = view
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseUp]) { [weak self] event in
                self?.handle(event)
                return event
            }
        }

        func handle(_ event: NSEvent) {
            guard let view, event.window === view.window else { return }
            var point = view.convert(event.locationInWindow, from: nil)
            if !view.isFlipped { point.y = view.bounds.height - point.y }
            guard view.bounds.contains(point) else {
                pressed = nil
                return
            }
            if event.type == .leftMouseDown {
                pressed = point
                // Selectable text tracks some presses itself (⌘-click extends a selection) and takes the
                // mouse-up off the queue before any monitor sees it. Default mode resumes only once that
                // loop ends, and the up it took is then the application's current event.
                RunLoop.main.perform(inModes: [.default]) { [weak self] in
                    MainActor.assumeIsolated {
                        guard let self, self.pressed != nil, let up = NSApp.currentEvent, up.type == .leftMouseUp else { return }
                        self.handle(up)
                    }
                }
            } else if let start = pressed {
                pressed = nil
                if hypot(point.x - start.x, point.y - start.y) <= 3 { clicked(point, event.modifierFlags) }
            }
        }

        func uninstall() {
            if let monitor { NSEvent.removeMonitor(monitor) }
            monitor = nil
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
