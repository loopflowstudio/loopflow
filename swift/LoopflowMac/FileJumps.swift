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
    let hit: TaskGrepHit?
    let visitID: UUID?
    var lines: ClosedRange<Int>?
}

@Observable @MainActor
final class FileJumps {
    struct Peek {
        let name: String?
        var title: String { name.map { "Definitions of \($0)" } ?? "File outline" }
        let origin: FilePosition
        var symbols: [TaskSymbol] = []
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
        store.reveal(destination)
    }

    func dismiss() {
        lookup?.cancel()
        lookup = nil
        peek = nil
    }

    func accept(_ symbol: TaskSymbol, in store: TaskFilesStore) {
        guard let origin = peek?.origin else { return }
        let destination = FilePosition(path: symbol.path, line: symbol.line)
        record(destination, from: origin)
        store.reveal(destination)
    }

    /// A nil name opens the complete outline. A definition first resolves
    /// in the live buffer, then searches the checkout on its owning Machine.
    @discardableResult
    func search(_ name: String?, line: Int, in store: TaskFilesStore) -> Task<Void, Never>? {
        dismiss()
        guard let document = store.selectedDocument else { return nil }
        store.filePicker = nil
        let version = document.editVersion
        let origin = FilePosition(path: document.path, line: line)
        peek = Peek(name: name, origin: origin)
        lookup = Task {
            await document.readOutline()
            guard !Task.isCancelled else { return }
            var symbols = document.outline?.entries.map(\.symbol) ?? []
            var truncated = false
            if let name {
                symbols = symbols.filter { $0.name == name && $0.kind != "impl" && $0.kind != "extension" }
                if symbols.isEmpty {
                    do {
                        let result = try await store.query.taskSymbols(issue: store.issue, name: name)
                        symbols = result.hits.filter { $0.path != document.path || !document.dirty }
                        truncated = result.truncated
                    } catch {
                        guard !Task.isCancelled else { return }
                        peek?.message = error.localizedDescription
                        peek?.loading = false
                        return
                    }
                }
            }
            guard !Task.isCancelled else { return }
            guard version == document.editVersion else { dismiss(); return }
            if name != nil, symbols.count == 1, !truncated {
                accept(symbols[0], in: store)
            } else {
                peek?.symbols = symbols
                peek?.message = truncated ? "Results truncated" : symbols.isEmpty ? "No declarations found" : nil
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
                Text(peek.title).font(Typography.textStrong)
                if peek.loading { ProgressView().controlSize(.small) }
                Spacer()
                Button { store.jumps.dismiss() } label: { Image(systemName: "xmark") }
                    .buttonStyle(.plain).accessibilityLabel("Close symbol list")
            }
            if let message = peek.message { Text(message).foregroundStyle(palette.textSecondary) }
            if !peek.symbols.isEmpty {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(peek.symbols.indices, id: \.self) { index in
                            let symbol = peek.symbols[index]
                            Button { store.jumps.accept(symbol, in: store) } label: {
                                HStack {
                                    Text([symbol.container, symbol.name].compactMap { $0 }.joined(separator: "."))
                                    Spacer()
                                    Text("\(symbol.path):\(symbol.line)").foregroundStyle(palette.textSecondary)
                                }.frame(height: rowHeight).contentShape(Rectangle())
                            }.buttonStyle(.plain)
                        }
                    }
                }.frame(height: CGFloat(min(peek.symbols.count, 8)) * rowHeight)
            }
        }.font(Typography.mono).padding(Spacing.sm)
            .background(palette.surfaceMuted)
            .onExitCommand { store.jumps.dismiss() }
            .accessibilityIdentifier("files-symbol-peek")
    }
}
#endif
