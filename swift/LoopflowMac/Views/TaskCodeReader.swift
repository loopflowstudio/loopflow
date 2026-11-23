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
    let ready: () -> Void
    @Environment(\.palette) private var palette
    @Environment(\.theme) private var theme
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
                            ForEach(lines.indices, id: \.self) { index in
                                VStack(alignment: .leading, spacing: 0) {
                                    HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
                                        Text(verbatim: String(lines.count)).hidden()
                                            .overlay(alignment: .trailing) { Text(verbatim: String(index + 1)) }
                                            .foregroundStyle(palette.textSecondary)
                                        source(lines[index], number: index + 1)
                                    }
                                    if let store, let peek = store.jumps.peek, peek.name != nil,
                                       peek.origin.line == index + 1 {
                                        FileSymbolPeek(store: store, peek: peek)
                                    }
                                }
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
        }
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
        if location?.lines?.contains(number) == true { value.backgroundColor = palette.keyboardFocus.opacity(0.3) }
        if let hit = location?.hit, hit.line == number, hit.revision == revision,
           let range = hit.range(in: line.text), let attributed = Range(range, in: value) {
            value[attributed].backgroundColor = palette.keyboardFocus.opacity(0.3)
        }
        return value
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
