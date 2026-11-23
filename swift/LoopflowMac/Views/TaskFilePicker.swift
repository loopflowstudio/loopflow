#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// One invocation of the picker, bound to the Machine that owns its checkout.
@MainActor @Observable
final class TaskFilePicker {
    private let query: RegistryQuery
    private let issue: String
    private let open: [String]
    private let preferred: Set<String>
    private(set) weak var previousResponder: NSResponder?

    private var inventory: TaskFileInventory?
    var text = "" { didSet { filter() } }
    private(set) var paths: [String] = []
    private(set) var selection: String? { didSet { if selection != oldValue { preview = nil } } }
    private(set) var truncated = false
    private(set) var error: String?
    private(set) var loading = true

    enum Preview {
        case text(String)
        case message(String)
    }
    private(set) var preview: Preview?

    init(store: TaskFilesStore) {
        query = store.query
        issue = store.issue
        let current = store.selection
        open = [current].compactMap { $0 } + store.tabs.reversed().map(\.path).filter { $0 != current }
        let changed = store.comparison == .workingTree(base: "parent") ? store.changes?.files.map(\.path) : nil
        preferred = Set(open + (changed ?? []))
        previousResponder = NSApp?.keyWindow?.firstResponder
    }

    var remoteSearch: String? { inventory?.truncated == true && !text.isEmpty ? text : nil }

    func load() async {
        do {
            let result = try await query.taskFind(issue: issue)
            guard !Task.isCancelled else { return }
            inventory = result
            filter()
        } catch {
            guard !Task.isCancelled else { return }
            self.error = error.localizedDescription
            loading = false
        }
    }

    private func filter() {
        guard let inventory else { return }
        error = nil
        loading = remoteSearch != nil
        truncated = inventory.truncated
        if text.isEmpty {
            let opened = Set(open)
            paths = open + inventory.paths.filter { !opened.contains($0) }
        } else if loading {
            paths = []
        } else {
            paths = ranked(inventory.paths)
        }
        selection = paths.first
    }

    func search(_ needle: String) async {
        do {
            try await Task.sleep(for: .milliseconds(150))
            let result = try await query.taskFind(issue: issue, query: needle)
            guard !Task.isCancelled, remoteSearch == needle else { return }
            paths = ranked(result.paths)
            selection = paths.first
            truncated = result.truncated
        } catch {
            guard !Task.isCancelled, remoteSearch == needle else { return }
            self.error = error.localizedDescription
        }
        loading = false
    }

    private func ranked(_ candidates: [String]) -> [String] {
        let needle = text.lowercased()
        return candidates.compactMap { path -> (Int, Int, Int, String)? in
            let lower = path.lowercased()
            let scalars = lower.unicodeScalars
            var cursor = scalars.startIndex
            var start = cursor
            for (i, scalar) in needle.unicodeScalars.enumerated() {
                guard let hit = scalars[cursor...].firstIndex(of: scalar) else { return nil }
                if i == 0 { start = hit }
                cursor = scalars.index(after: hit)
            }
            let priority = path.hasPrefix("scratch/") ? 0 : preferred.contains(path) ? 1 : 2
            let name = (lower as NSString).lastPathComponent
            return (priority, name.contains(needle) ? 0 : 1, lower.utf8.distance(from: start, to: cursor), path)
        }.sorted(by: <).map(\.3)
    }

    func move(_ offset: Int) {
        guard let selection, let index = paths.firstIndex(of: selection) else { return }
        self.selection = paths[min(max(index + offset, 0), paths.count - 1)]
    }

    func loadPreview(_ path: String, draft: TaskFileDocument?) async {
        guard path == selection else { return }
        if let draft, draft.dirty {
            preview = .text(draft.text)
            return
        }
        do {
            let file = try await query.taskFile(issue: issue, path: path)
            guard !Task.isCancelled, path == selection else { return }
            preview = file.content.map(Preview.text) ?? .message(file.state.title)
        } catch {
            guard !Task.isCancelled, path == selection else { return }
            preview = .message(error.localizedDescription)
        }
    }
}

struct TaskFilePickerView: View {
    let store: TaskFilesStore
    @Bindable var picker: TaskFilePicker
    @Environment(\.palette) private var palette

    var body: some View {
        GeometryReader { geometry in
            VStack(spacing: 0) {
                HStack {
                    Image(systemName: "magnifyingglass")
                    PaletteSearchField(text: $picker.text, placeholder: "Find file in checkout",
                                       identifier: "files-search", move: picker.move,
                                       submit: { if let path = picker.selection { open(path) } }, cancel: cancel)
                    Button(action: cancel) { Image(systemName: "xmark") }
                        .buttonStyle(.plain).accessibilityLabel("Close file picker")
                }.padding(Spacing.sm)
                Divider()
                HStack(spacing: 0) {
                    results.frame(width: geometry.size.width >= 620 ? geometry.size.width * 0.42 : geometry.size.width)
                    if geometry.size.width >= 620 {
                        Divider()
                        preview.frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(palette.background)
        }
        .task { await picker.load() }
        .task(id: picker.remoteSearch) {
            if let needle = picker.remoteSearch { await picker.search(needle) }
        }
        .onExitCommand(perform: cancel)
    }

    private var results: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: Spacing.xxs) {
                    if picker.loading { ProgressView().controlSize(.small) }
                    else if let error = picker.error { FailureLine("File search unavailable.", machine: error) }
                    else {
                        if picker.paths.isEmpty { Text("No matching files.").emptyLine() }
                        ForEach(picker.paths, id: \.self) { path in
                            Button { open(path) } label: {
                                Text(verbatim: path).font(Typography.mono).lineLimit(1).truncationMode(.middle)
                                    .padding(Spacing.xs).frame(maxWidth: .infinity, alignment: .leading)
                                    .workSelection(path == picker.selection).contentShape(Rectangle())
                            }.buttonStyle(.plain).id(path).help(path)
                        }
                        if picker.truncated {
                            Text("More paths may match. Refine your search.").font(Typography.meta)
                        }
                    }
                }.padding(Spacing.sm)
            }
            .onChange(of: picker.selection) { _, path in if let path { proxy.scrollTo(path) } }
        }
    }

    @ViewBuilder private var preview: some View {
        if let path = picker.selection {
            Group {
                switch picker.preview {
                case .text(let text): TaskCodeReader(path: path, text: text, ready: {})
                case .message(let message): Text(message).font(Typography.meta)
                case nil: ProgressView()
                }
            }.id(path).task(id: path) { await picker.loadPreview(path, draft: store.documents[path]) }
        }
    }

    private func open(_ path: String) {
        store.filePicker = nil
        store.selectFileFromInput(path)
    }

    private func cancel() {
        store.filePicker = nil
        if let view = picker.previousResponder as? NSView { view.window?.makeFirstResponder(view) }
    }
}
#endif
