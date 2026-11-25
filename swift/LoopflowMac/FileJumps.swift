#if os(macOS)
import Foundation
import Loopflow
import SwiftUI

struct FilePosition: Equatable {
    let path: String
    let line: Int
}

struct FileVisit: Equatable {
    let line: Int
    var hit: TaskGrepHit?
    var lines: ClosedRange<Int>?
    // Repeating an address scrolls again. Picker previews stay keyed to their hit.
    var visitID: UUID? = UUID()
}

@Observable @MainActor
final class FileJumps {
    enum Lookup: Equatable {
        case outline
        case definition(String)
        case references(String)

        var title: String {
            switch self {
            case .outline: "File outline"
            case .definition(let name): "Definitions of \(name)"
            case .references(let name): "References to \(name)"
            }
        }
    }

    final class Target {
        let path: String
        let line: Int
        let word: String
        let range: NSRange

        init(path: String, line: Int, word: String, range: NSRange) {
            self.path = path
            self.line = line
            self.word = word
            self.range = range
        }
    }

    var target: Target?

    struct Candidate {
        let label: String
        let position: FilePosition
        let hit: TaskGrepHit?

        init(_ symbol: TaskSymbol) {
            label = [symbol.container, symbol.name].compactMap { $0 }.joined(separator: ".")
            position = FilePosition(path: symbol.path, line: symbol.line)
            hit = nil
        }

        init(_ match: TaskGrepHit) {
            label = match.excerpt
            position = FilePosition(path: match.path, line: match.line)
            hit = match
        }
    }

    struct Peek {
        let lookup: Lookup
        let origin: FilePosition
        var candidates: [Candidate] = []
        var message: String?
        var loading = true
    }

    private(set) var back: [FilePosition] = []
    private(set) var forward: [FilePosition] = []
    private(set) var peek: Peek?
    var readingLine = 1
    @ObservationIgnored private var lookup: Task<Void, Never>?

    func position(in store: TaskFilesStore) -> FilePosition? {
        store.selection.map { FilePosition(path: $0, line: readingLine) }
    }

    func record(_ destination: FilePosition, from origin: FilePosition?) {
        guard let origin, origin != destination else { return }
        if back.last != origin { back.append(origin) }
        forward.removeAll()
    }

    func step(backwards: Bool, in store: TaskFilesStore) {
        let current = position(in: store)
        let destination = backwards ? back.popLast() : forward.popLast()
        guard let destination else { return }
        if let current {
            if backwards { forward.append(current) } else { back.append(current) }
        }
        store.reveal(destination.path, at: FileVisit(line: destination.line))
    }

    func dismiss() {
        lookup?.cancel()
        lookup = nil
        peek = nil
    }

    func accept(_ candidate: Candidate, in store: TaskFilesStore) {
        guard let origin = peek?.origin else { return }
        let visit = FileVisit(line: candidate.position.line, hit: candidate.hit)
        store.reveal(candidate.position.path, at: visit, from: origin)
    }

    @discardableResult
    func search(_ request: Lookup, line: Int, in store: TaskFilesStore) -> Task<Void, Never>? {
        dismiss()
        guard let document = store.selectedDocument else { return nil }
        store.filePicker = nil
        let version = document.editVersion
        let origin = FilePosition(path: document.path, line: line)
        peek = Peek(lookup: request, origin: origin)
        lookup = Task {
            do {
                let candidates: [Candidate]
                var truncated = false
                var empty = "No declarations found"
                switch request {
                case .references(let name):
                    let result = try await store.query.taskGrep(issue: store.issue, query: name, wholeWord: true)
                    candidates = result.hits.map(Candidate.init)
                    truncated = result.truncated
                    empty = "No references found"
                case .outline, .definition:
                    await document.readOutline()
                    try Task.checkCancellation()
                    var symbols = document.outline?.entries.map(\.symbol) ?? []
                    if case .definition(let name) = request {
                        symbols = symbols.filter { $0.name == name && $0.kind != "impl" && $0.kind != "extension" }
                        if symbols.isEmpty {
                            let result = try await store.query.taskSymbols(issue: store.issue, name: name)
                            symbols = result.hits.filter { $0.path != document.path || !document.dirty }
                            truncated = result.truncated
                        }
                    }
                    candidates = symbols.map(Candidate.init)
                }
                guard !Task.isCancelled else { return }
                guard store.selectedDocument === document, version == document.editVersion else { dismiss(); return }
                if case .definition = request, candidates.count == 1, !truncated {
                    accept(candidates[0], in: store)
                } else {
                    peek?.candidates = candidates
                    peek?.message = truncated ? "Results truncated" : candidates.isEmpty ? empty : nil
                    peek?.loading = false
                }
            } catch {
                guard !Task.isCancelled else { return }
                peek?.message = error.localizedDescription
                peek?.loading = false
            }
        }
        return lookup
    }
}

struct FileSymbolPeek: View {
    let store: TaskFilesStore
    let peek: FileJumps.Peek
    private let rowHeight = Typography.monoSize + 2 * Spacing.xs
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            HStack {
                Text(peek.lookup.title).font(Typography.textStrong)
                if peek.loading { ProgressView().controlSize(.small) }
                Spacer()
                Button { store.jumps.dismiss() } label: { Image(systemName: "xmark") }
                    .buttonStyle(.plain).accessibilityLabel("Close symbol list")
            }
            if let message = peek.message { Text(message).foregroundStyle(palette.textSecondary) }
            if !peek.candidates.isEmpty {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(peek.candidates.indices, id: \.self) { index in
                            let candidate = peek.candidates[index]
                            Button { store.jumps.accept(candidate, in: store) } label: {
                                HStack {
                                    Text(verbatim: candidate.label).lineLimit(1)
                                    Spacer()
                                    Text("\(candidate.position.path):\(candidate.position.line)").foregroundStyle(palette.textSecondary)
                                }.frame(height: rowHeight).contentShape(Rectangle())
                            }.buttonStyle(.plain)
                        }
                    }
                }.frame(height: CGFloat(min(peek.candidates.count, 8)) * rowHeight)
            }
        }.font(Typography.mono).padding(Spacing.sm)
            .background(palette.surfaceMuted, in: RoundedRectangle(cornerRadius: CornerRadius.control))
            // Dark themes put surfaceMuted close to the page; the edge keeps the list from reading as code.
            .overlay(RoundedRectangle(cornerRadius: CornerRadius.control).strokeBorder(palette.borderStrong))
            .padding(.vertical, Spacing.xxs)
            .onExitCommand { store.jumps.dismiss() }
            .accessibilityIdentifier("files-symbol-peek")
    }
}
#endif
