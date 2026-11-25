#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// One Find bar searches the cells its reader actually displays.
@MainActor @Observable
final class FileFind {
    struct Scan<Revision: Equatable>: Equatable {
        let query: String?
        let revision: Revision
    }

    enum Cell: Equatable, Sendable {
        case line(Int), editor, markdown(block: Int, cell: Int)
        case diff(row: String, side: ReviewSelection.Side, line: Int)
        case html(page: Int, occurrence: Int)
    }
    struct Match: Equatable, Sendable {
        let cell: Cell
        let range: NSRange
    }

    var query = "" { didSet { if oldValue != query { reset() } } }
    var focusRequest = UUID()
    private(set) var matches: [Match] = []
    private(set) var index = 0
    private(set) var searching = false
    private var pages: [Int: Int] = [:]
    var current: Match? { matches.indices.contains(index) ? matches[index] : nil }
    var label: String {
        guard !query.isEmpty else { return "" }
        if searching { return "Searching…" }
        return matches.isEmpty ? "No matches" : "\(index + 1) of \(matches.count)"
    }

    func reset() {
        matches = []
        index = 0
        searching = !query.isEmpty
        pages = [:]
    }

    func step(_ offset: Int) {
        guard !matches.isEmpty else { return }
        index = (index + offset + matches.count) % matches.count
    }

    func search(text: String) async {
        await runSearch { needle in
            var hits: [Match] = []
            var lineNumber = 1
            for raw in text.utf8.split(separator: 10, omittingEmptySubsequences: false) {
                if Task.isCancelled { return [] }
                let slice = raw.last == 13 ? raw.dropLast() : raw
                Self.appendMatches(of: needle, in: String(decoding: slice, as: UTF8.self) as NSString,
                                   cell: .line(lineNumber), into: &hits)
                lineNumber += 1
            }
            return hits
        }
    }

    func search(_ cells: [(Cell, String)]) async {
        await runSearch { needle in
            var hits: [Match] = []
            for (cell, string) in cells {
                if Task.isCancelled { return [] }
                Self.appendMatches(of: needle, in: string as NSString, cell: cell, into: &hits)
            }
            return hits
        }
    }

    private func runSearch(_ work: @escaping @Sendable (String) -> [Match]) async {
        let needle = query
        guard !needle.isEmpty else { return }
        let scan = Task.detached(priority: .userInitiated) { work(needle) }
        let hits = await withTaskCancellationHandler { await scan.value } onCancel: { scan.cancel() }
        guard !Task.isCancelled, query == needle else { return }
        receive(hits)
    }

    private nonisolated static func appendMatches(of needle: String, in text: NSString, cell: Cell, into hits: inout [Match]) {
        var start = 0
        while start < text.length {
            if Task.isCancelled { return }
            let hit = text.range(of: needle, options: .caseInsensitive,
                                 range: NSRange(location: start, length: text.length - start))
            guard hit.location != NSNotFound, hit.length > 0 else { break }
            hits.append(Match(cell: cell, range: hit))
            start = NSMaxRange(hit)
        }
    }

    /// Two rendered HTML sides finish independently. Publish their total together.
    func receive(page: Int, count: Int, expected: Int) {
        pages[page] = count
        guard pages.count == expected else { return }
        receive(pages.keys.sorted().flatMap { page in
            (0..<(pages[page] ?? 0)).map { Match(cell: .html(page: page, occurrence: $0), range: NSRange()) }
        })
    }

    private func receive(_ hits: [Match]) {
        let previous = current
        matches = hits
        index = previous.flatMap { hits.firstIndex(of: $0) } ?? 0
        searching = false
    }

    func highlight(_ text: AttributedString, in cell: Cell, color: Color) -> AttributedString {
        guard let current, current.cell == cell, let range = Range(current.range, in: text) else { return text }
        var highlighted = text
        highlighted[range].backgroundColor = color
        return highlighted
    }
}

struct FileFindBar: View {
    @Bindable var find: FileFind
    let store: TaskFilesStore
    let focused: Bool
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.sm) {
            SearchBox {
                PaletteSearchField(text: $find.query, placeholder: "Find in file", identifier: "files-find-field",
                                   move: find.step, submit: { find.step(1) },
                                   cancel: { store.find = nil }, focusOnMount: focused, submitBackwards: { find.step(-1) })
                    .id(find.focusRequest)
                Text(find.label).font(Typography.meta).monospacedDigit().foregroundStyle(palette.textSecondary)
            }
            Button { find.step(-1) } label: { Image(systemName: "chevron.up") }.accessibilityLabel("Previous match")
            Button { find.step(1) } label: { Image(systemName: "chevron.down") }.accessibilityLabel("Next match")
            Button { store.chooseFile(.grep) } label: { Image(systemName: "folder.badge.magnifyingglass") }
                .help("Search checkout for this text (⇧⌘F)").accessibilityLabel("Search checkout")
            Button { store.find = nil } label: { Image(systemName: "xmark") }.accessibilityLabel("Close Find")
        }
        .buttonStyle(.plain).foregroundStyle(palette.textSecondary)
        .padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.xs)
    }
}

/// A search input drawn as a field, so it never reads as another header row.
struct SearchBox<Content: View>: View {
    @ViewBuilder let content: Content
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.xs) {
            Image(systemName: "magnifyingglass").foregroundStyle(palette.textSecondary)
            content
        }
        .padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.xs)
        .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.control))
        .overlay(RoundedRectangle(cornerRadius: CornerRadius.control).strokeBorder(palette.borderStrong))
    }
}
#endif
