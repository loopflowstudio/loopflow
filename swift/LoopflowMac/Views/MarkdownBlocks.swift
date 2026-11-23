import Foundation
import Loopflow
import SwiftUI

/// Renders a Task's source Markdown through Foundation's parser: each run's
/// presentation intent supplies its block (heading, list item, quote, code,
/// table row, rule); inline emphasis, code and links stay attributes. Authored
/// line breaks inside a paragraph are kept. Unparseable source shows verbatim.
struct MarkdownBlocks: View, Equatable {
    struct Block: Equatable {
        enum Kind: Equatable {
            case heading(Int), paragraph, quote, code, rule, row(header: Bool)
        }
        var kind: Kind
        /// List marker on the first block of an item.
        var marker: String?
        /// List nesting depth.
        var depth: Int
        /// One entry for text blocks; one per cell for table rows.
        var cells: [AttributedString]
        var text: AttributedString { cells.first ?? AttributedString() }
    }

    let source: String
    var find: FileFind?

    /// Each block's scroll id, for an outline to jump to.
    static func anchor(_ block: Int) -> String { "markdown-block-\(block)" }

    /// Reading copy at about 1.55 line height over the `text` step (13 pt).
    static let lineSpacing: CGFloat = 4.5

    private static func indent(_ block: Block) -> CGFloat {
        Spacing.lg * CGFloat(max(block.depth - 1, 0))
    }

    var body: some View {
        let blocks = Perf.measure(Perf.markdownParse, "bytes=\(source.utf8.count)") { Self.blocks(source) }
        Rows(blocks: blocks, find: find)
    }

    private struct Rows: View {
        let blocks: [Block]
        let find: FileFind?
        @Environment(\.palette) private var palette

        var body: some View {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                ForEach(Array(blocks.enumerated()), id: \.offset) { index, block in
                    Row(block: block.highlighted(find, index: index, color: palette.keyboardFocus.opacity(0.45))).id(MarkdownBlocks.anchor(index))
                }
            }
            .textSelection(.enabled)
            .task(id: FileFind.Scan(query: find?.query, revision: blocks)) {
                if let find, !find.query.isEmpty {
                    await find.search(blocks.enumerated().flatMap { $0.element.searchCells(index: $0.offset) })
                }
            }
            .tint(palette.linkInk)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    /// One block as it reads: its list marker, then its text in the block's style.
    struct Row: View {
        let block: Block
        @Environment(\.palette) private var palette

        var body: some View {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                if let marker = block.marker {
                    Text(marker).foregroundStyle(palette.textSecondary)
                } else if block.depth > 0 {
                    Text("•").hidden()
                }
                content
            }
            .font(Typography.text)
            .lineSpacing(block.kind == .code ? 0 : MarkdownBlocks.lineSpacing)
            .padding(.leading, MarkdownBlocks.indent(block))
        }

        @ViewBuilder
        private var content: some View {
            switch block.kind {
            case .heading(let level):
                Text(block.text)
                    .font(level <= 2 ? Typography.lede : Typography.textStrong)
                    .padding(.top, Spacing.xs)
            case .paragraph:
                Text(block.text)
            case .quote:
                Text(block.text)
                    .foregroundStyle(palette.textSecondary)
                    .padding(.leading, Spacing.sm)
                    .overlay(alignment: .leading) { Rectangle().fill(palette.border).frame(width: 2) }
            case .code:
                Text(block.text)
                    .font(Typography.mono)
                    .padding(Spacing.sm)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.control))
            case .rule:
                Divider()
            case .row(let header):
                HStack(alignment: .top, spacing: Spacing.sm) {
                    ForEach(Array(block.cells.enumerated()), id: \.offset) { _, cell in
                        Text(cell).frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
                .fontWeight(header ? .semibold : .regular)
            }
        }
    }

    static func blocks(_ source: String) -> [Block] {
        guard let parsed = try? AttributedString(markdown: source) else {
            return [Block(kind: .paragraph, marker: nil, depth: 0, cells: [AttributedString(source)])]
        }
        var blocks: [Block] = []
        var blockID: Int?
        var cellID: Int?
        var markedItems: Set<Int> = []
        for run in parsed.runs {
            var text = AttributedString(parsed[run.range])
            text.presentationIntent = nil
            if run.inlinePresentationIntent?.contains(.softBreak) == true {
                text = AttributedString("\n")
            }
            let components = run.presentationIntent?.components ?? []
            guard let leaf = components.first else { continue }
            let isCell: Bool
            if case .tableCell = leaf.kind { isCell = true } else { isCell = false }
            let identity = isCell && components.count > 1 ? components[1].identity : leaf.identity
            if identity == blockID, var block = blocks.popLast() {
                if isCell, leaf.identity != cellID { block.cells.append(text) } else { block.cells[block.cells.count - 1] += text }
                blocks.append(block)
            } else {
                var marker: String?
                if let at = components.firstIndex(where: { if case .listItem = $0.kind { true } else { false } }),
                   case .listItem(let ordinal) = components[at].kind,
                   markedItems.insert(components[at].identity).inserted {
                    let ordered = components.indices.contains(at + 1) && components[at + 1].kind == .orderedList
                    marker = ordered ? "\(ordinal)." : "•"
                }
                let depth = components.filter { if case .listItem = $0.kind { true } else { false } }.count
                blocks.append(Block(kind: kind(leaf.kind, in: components), marker: marker, depth: depth, cells: [text]))
            }
            blockID = identity
            cellID = isCell ? leaf.identity : nil
        }
        return blocks.map { block in
            guard block.kind == .code, var text = block.cells.first else { return block }
            while text.characters.last == "\n" { text.characters.removeLast() }
            return Block(kind: .code, marker: block.marker, depth: block.depth, cells: [text])
        }
    }

    /// Same source, same blocks: `.equatable()` callers skip the parse when a
    /// parent re-renders for unrelated state.
    nonisolated static func == (lhs: MarkdownBlocks, rhs: MarkdownBlocks) -> Bool { lhs.source == rhs.source && lhs.find === rhs.find }

    private static func kind(_ leaf: PresentationIntent.Kind, in components: [PresentationIntent.IntentType]) -> Block.Kind {
        switch leaf {
        case .header(let level): return .heading(level)
        case .codeBlock: return .code
        case .thematicBreak: return .rule
        case .tableCell: return .row(header: components.contains { $0.kind == .tableHeaderRow })
        default: return components.contains { $0.kind == .blockQuote } ? .quote : .paragraph
        }
    }
}


extension MarkdownBlocks.Block {
    func searchCells(index: Int) -> [(FileFind.Cell, String)] {
        cells.enumerated().map { (.markdown(block: index, cell: $0.offset), String($0.element.characters)) }
    }

    @MainActor func highlighted(_ find: FileFind?, index: Int, color: Color) -> Self {
        guard let find, case .markdown(let block, let cell) = find.current?.cell,
              block == index, cells.indices.contains(cell) else { return self }
        var result = self
        result.cells[cell] = find.highlight(cells[cell], in: .markdown(block: index, cell: cell), color: color)
        return result
    }
}
