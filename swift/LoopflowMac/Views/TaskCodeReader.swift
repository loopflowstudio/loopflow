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
    @Environment(\.theme) private var theme
    @State private var parsedVersion = 0
    @State private var lines: [CodeLine]?

    @State private var scrolling = false

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
                return (CodeLine.read(text, highlights: highlights),
                        needsOutline ? FileOutline(tree: tree, text: text, path: path) : nil)
            }.value
            guard !Task.isCancelled else { return }
            if let outline = result.1, let version { document?.receive(outline, version: version) }
            lines = result.0
            parsedVersion += 1
        }
    }

    private func reader(_ lines: [CodeLine], width: CGFloat) -> some View {
        ScrollViewReader { proxy in
            ScrollView([.horizontal, .vertical]) {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if let store {
                        ForEach(store.comments.items.filter { $0.original.path == path && $0.original.source.isFile && $0.detached }) {
                            ReviewCommentRow(store: store, comment: $0)
                        }
                    }
                    ForEach(lines.indices, id: \.self) { index in
                        row(index, in: lines)
                    }
                }
                .scrollTargetLayout()
                .font(Typography.mono)
                .padding(Spacing.sm)
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
                if case .line(let line) = store?.find?.current?.cell {
                    proxy.scrollTo(line - 1, anchor: .center)
                    store?.jumps.readingLine = line
                }
            }
            .task(id: location) {
                guard let location, lines.indices.contains(location.line - 1) else { return }
                let anchor = scrollAnchor(for: location, text: lines[location.line - 1].text, width: width)
                proxy.scrollTo(location.line - 1, anchor: anchor)
            }
        }
    }

    func row(_ index: Int, in lines: [CodeLine]) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
                if let store {
                    let number = index + 1
                    ReviewGutter(number: number, selected: selected(number),
                                 tone: changeTone(number, in: store)) {
                        store.comments.select(path: path, source: .file(draft: store.selectedDocument?.dirty == true),
                                              line: number, extend: NSEvent.modifierFlags.contains(.shift))
                    }
                } else {
                    Text(verbatim: String(index + 1)).foregroundStyle(palette.textSecondary)
                }
                source(lines[index], number: index + 1)
            }
            // Code scrolls sideways; a wrapped line would read as two lines beside one number.
            .fixedSize(horizontal: true, vertical: false)
            if let store {
                if let selection = store.comments.selection, selected(index + 1), selection.last == index + 1 {
                    ReviewCommentEditor(store: store, selection: selection, excerpt: {
                        lines[(selection.first - 1)...index].map(\.text)
                    }, addToConversation: addToConversation)
                }
                ForEach(store.comments.items.filter {
                    !$0.detached && $0.original.source.isFile && $0.original.path == path && $0.selection.last == index + 1
                }) { ReviewCommentRow(store: store, comment: $0) }
            }
            if let store, let peek = store.jumps.peek, peek.lookup != .outline,
               peek.origin.line == index + 1 {
                FileSymbolPeek(store: store, peek: peek)
            }
        }
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

    @ViewBuilder private func source(_ line: CodeLine, number: Int) -> some View {
        let label = Text(painted(line, number: number)).textSelection(.enabled)
        if let store {
            label.simultaneousGesture(SpatialTapGesture().onEnded { tap in
                store.jumps.readingLine = number
                let target = target(in: line.text, number: number, at: tap.location)
                store.jumps.target = target
                if NSApp.currentEvent?.modifierFlags.contains(.command) == true, let target {
                    store.jumps.search(.definition(target.word), line: number, in: store)
                }
            })
        } else { label }
    }

    private func typeset(_ text: String) -> CTLine {
        let font = NSFont(name: Typography.monoFace, size: Typography.monoSize)
            ?? .monospacedSystemFont(ofSize: Typography.monoSize, weight: .regular)
        return CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: [.font: font]))
    }

    func scrollAnchor(for visit: FileVisit, text: String, width: CGFloat) -> UnitPoint {
        let leading = UnitPoint(x: 0, y: 0.5)
        guard let hit = visit.hit, hit.revision == revision, let range = hit.range(in: text) else { return leading }
        let line = typeset(text)
        let gutter = store == nil ? CTLineGetTypographicBounds(typeset(String(visit.line)), nil, nil, nil)
            : ReviewStream.gutterWidth + Spacing.xs
        let inset = gutter + Spacing.md
        let overflow = inset + CTLineGetTypographicBounds(line, nil, nil, nil) - width
        guard overflow > 0 else { return leading }
        let start = CTLineGetOffsetForStringIndex(line, range.location, nil)
        let end = CTLineGetOffsetForStringIndex(line, NSMaxRange(range), nil)
        // scrollTo aligns the same fractional point in the row and the viewport.
        let offset = inset + start + min(end - start, width) / 2 - width / 2
        return UnitPoint(x: max(0, min(1, offset / overflow)), y: 0.5)
    }

    func target(in text: String, number: Int, at point: CGPoint) -> FileJumps.Target? {
        let line = typeset(text)
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

    func painted(_ line: CodeLine, number: Int) -> AttributedString {
        var value = AttributedString(line.text)
        value.paint(line.highlights, inks: .of(theme))
        if let target = store?.jumps.target, target.path == path, target.line == number,
           let range = Range(target.range, in: value) {
            value[range].underlineStyle = Text.LineStyle(pattern: .solid, color: palette.actionInk)
        }
        if selected(number) || location?.lines?.contains(number) == true { value.backgroundColor = palette.keyboardFocus.opacity(0.3) }
        if let hit = location?.hit, hit.line == number, hit.revision == revision,
           let range = hit.range(in: line.text), let attributed = Range(range, in: value) {
            value[attributed].backgroundColor = palette.keyboardFocus.opacity(0.3)
        }
        return store?.find?.highlight(value, in: .line(number), color: palette.keyboardFocus.opacity(0.45)) ?? value
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
