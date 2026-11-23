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

    @FocusState private var focused: Bool
    @State private var selectedWord: String?
    @State private var selectedLine = 1
    @State private var pendingG = false
    @State private var scrolling = false

    var body: some View {
        Group {
            if let lines {
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
                    .focusable(store != nil).focused($focused).focusEffectDisabled()
                    .onChange(of: focused) { if !focused { pendingG = false } }
                    .onKeyPress { key in
                        guard let store, key.modifiers.isEmpty, let selectedWord else { pendingG = false; return .ignored }
                        if key.characters == "d", pendingG {
                            pendingG = false
                            store.jumps.search(selectedWord, line: selectedLine, in: store)
                            return .handled
                        }
                        pendingG = key.characters == "g"
                        return pendingG ? .handled : .ignored
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
                        if let location { proxy.scrollTo(location.line - 1, anchor: UnitPoint(x: 0, y: 0.5)) }
                    }
                }
                .onAppear(perform: ready)
                .accessibilityIdentifier("files-code-reader")
            } else {
                ProgressView()
            }
        }
        .task(id: text) {
            selectedWord = nil
            pendingG = false
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
            if let store, let peek = store.jumps.peek, peek.name != nil,
               peek.origin.line == index + 1 {
                FileSymbolPeek(store: store, peek: peek)
            }
        }
    }

    private func changeTone(_ line: Int, in store: TaskFilesStore) -> ThemeTone? {
        guard let review = store.diffReview.fileReview, review.path == path,
              review.editVersion == store.selectedDocument?.editVersion,
              review.baseCommit == store.workingChanges?.baseCommit,
              store.workingTree.counts[path] != nil || store.selectedDocument?.dirty == true,
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
                focused = true
                store.jumps.readingLine = number
                selectedLine = number
                selectedWord = Self.word(in: line.text, at: tap.location)
                pendingG = false
                if NSApp.currentEvent?.modifierFlags.contains(.command) == true, let selectedWord {
                    store.jumps.search(selectedWord, line: number, in: store)
                }
            })
        } else { label }
    }

    static func word(in text: String, at point: CGPoint) -> String? {
        let font = NSFont(name: Typography.monoFace, size: Typography.monoSize)
            ?? .monospacedSystemFont(ofSize: Typography.monoSize, weight: .regular)
        let line = CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: [.font: font]))
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
        return index < end ? source.substring(with: NSRange(location: start, length: end - start)) : nil
    }

    private func painted(_ line: CodeLine, number: Int) -> AttributedString {
        var value = AttributedString(line.text)
        value.paint(line.highlights, inks: .of(theme))
        if selected(number) || location?.lines?.contains(number) == true { value.backgroundColor = palette.keyboardFocus.opacity(0.3) }
        if let hit = location?.hit, hit.line == number, hit.revision == revision,
           let range = hit.range(in: line.text), let attributed = Range(range, in: value) {
            value[attributed].backgroundColor = palette.keyboardFocus.opacity(0.3)
        }
        return store?.find?.highlight(value, in: .line(number), color: palette.keyboardFocus.opacity(0.45)) ?? value
    }
}

extension TaskGrepHit {
    func range(in text: String) -> NSRange? {
        let bytes = text.utf8
        guard start >= 0, end >= start,
              let from = bytes.index(bytes.startIndex, offsetBy: start, limitedBy: bytes.endIndex),
              let to = bytes.index(from, offsetBy: end - start, limitedBy: bytes.endIndex),
              let lower = from.samePosition(in: text.unicodeScalars),
              let upper = to.samePosition(in: text.unicodeScalars) else { return nil }
        return NSRange(lower..<upper, in: text)
    }
}
#endif
