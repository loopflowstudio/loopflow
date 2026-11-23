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
    private let workingTree: ComparisonReading
    private let preferred: Set<String>
    private(set) weak var previousResponder: NSResponder?

    enum Mode: String, CaseIterable { case files = "Files", grep = "Grep", changed = "Changed" }

    enum Result: Hashable {
        case file(String)
        case match(TaskGrepHit)

        var hit: TaskGrepHit? { if case .match(let hit) = self { hit } else { nil } }
        var path: String {
            switch self {
            case .file(let path): path
            case .match(let hit): hit.path
            }
        }
    }

    let mode: Mode
    private var inventory: TaskFileInventory?
    var text = "" { didSet { filter() } }
    private(set) var results: [Result] = []
    private(set) var selection: Result? { didSet { if selection?.path != oldValue?.path { preview = nil } } }
    private(set) var truncated = false
    private(set) var error: String?
    private(set) var loading = true

    enum Preview {
        case text(String, revision: String?)
        case message(String)
    }
    private(set) var preview: Preview?

    init(store: TaskFilesStore, mode: Mode = .files) {
        self.mode = mode
        query = store.query
        issue = store.issue
        let current = store.selection
        open = [current].compactMap { $0 } + store.tabs.reversed().map(\.path).filter { $0 != current }
        workingTree = store.workingTree
        preferred = Set(open + (store.workingChanges?.files.map(\.path) ?? []))
        let responder = NSApp?.keyWindow?.firstResponder
        let field = (responder as? NSTextView).flatMap { $0.isFieldEditor ? $0.delegate as? NSView : nil }
        previousResponder = store.filePicker?.previousResponder ?? field ?? responder
    }

    var remoteSearch: String? { !text.isEmpty && (mode == .grep || inventory?.truncated == true) ? text : nil }

    func load() async {
        if mode == .changed, workingTree.changes == nil {
            await workingTree.refresh(issue: issue, query: query)
        }
        guard mode == .files else { filter(); return }
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
        if mode == .changed {
            guard let changes = workingTree.changes else {
                error = workingTree.error
                loading = error == nil
                return
            }
            error = nil
            let changed = changes.files.map(\.path)
            results = (text.isEmpty ? changed : ranked(changed)).map(Result.file)
            selection = results.first
            loading = false
            return
        }
        // An inventory failure stays visible while typing. Grep has no initial read.
        guard mode == .grep || inventory != nil else { return }
        error = nil
        loading = remoteSearch != nil
        truncated = inventory?.truncated ?? false
        if let inventory, !loading {
            if text.isEmpty {
                let opened = Set(open)
                results = (open + inventory.paths.filter { !opened.contains($0) }).map(Result.file)
            } else {
                results = ranked(inventory.paths).map(Result.file)
            }
        } else {
            results = []
        }
        selection = results.first
    }

    func search(_ needle: String) async {
        do {
            try await Task.sleep(for: .milliseconds(150))
            switch mode {
            case .changed: return
            case .files:
                let result = try await query.taskFind(issue: issue, query: needle)
                guard !Task.isCancelled, remoteSearch == needle else { return }
                results = ranked(result.paths).map(Result.file)
                truncated = result.truncated
            case .grep:
                let result = try await query.taskGrep(issue: issue, query: needle)
                guard !Task.isCancelled, remoteSearch == needle else { return }
                results = result.hits.map(Result.match)
                truncated = result.truncated
            }
            selection = results.first
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
        guard let selection, let index = results.firstIndex(of: selection) else { return }
        self.selection = results[min(max(index + offset, 0), results.count - 1)]
    }

    func loadPreview(_ path: String, draft: TaskFileDocument?) async {
        guard path == selection?.path else { return }
        if let draft, draft.dirty {
            preview = .text(draft.text, revision: nil)
            return
        }
        do {
            let file = try await query.taskFile(issue: issue, path: path)
            guard !Task.isCancelled, path == selection?.path else { return }
            preview = file.content.map { .text($0, revision: file.revision) } ?? .message(file.state.title)
        } catch {
            guard !Task.isCancelled, path == selection?.path else { return }
            preview = .message(error.localizedDescription)
        }
    }
}

struct TaskFilePickerView: View {
    let store: TaskFilesStore
    @Bindable var picker: TaskFilePicker
    var isFocused = true
    @Environment(\.palette) private var palette

    var body: some View {
        GeometryReader { geometry in
            VStack(spacing: 0) {
                HStack {
                    Image(systemName: "magnifyingglass")
                    PaletteSearchField(text: $picker.text, placeholder: picker.mode == .grep ? "Search checkout text" : "Find file",
                                       identifier: "files-search", move: picker.move,
                                       submit: { if let result = picker.selection { open(result) } }, cancel: cancel, focusOnMount: isFocused)
                    Picker("Search mode", selection: Binding(get: { picker.mode }, set: { store.chooseFile($0) })) {
                        ForEach(TaskFilePicker.Mode.allCases, id: \.self) { Text($0.rawValue) }
                    }.pickerStyle(.menu).labelsHidden().fixedSize()
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
                        if picker.results.isEmpty {
                            Text(picker.mode == .grep && picker.text.isEmpty ? "Type text to search." : "No matches.").emptyLine()
                        }
                        ForEach(picker.results, id: \.self) { result in
                            Button { open(result) } label: {
                                VStack(alignment: .leading, spacing: Spacing.xxs) {
                                    Text(verbatim: result.hit.map { "\($0.path):\($0.line)" } ?? result.path)
                                        .font(Typography.mono).lineLimit(1).truncationMode(.middle)
                                    if let hit = result.hit {
                                        Text(verbatim: hit.excerpt).font(Typography.meta).lineLimit(1)
                                    }
                                }
                                .padding(Spacing.xs).frame(maxWidth: .infinity, alignment: .leading)
                                .workSelection(result == picker.selection).contentShape(Rectangle())
                            }.buttonStyle(.plain).id(result).help(result.path)
                        }
                        if picker.truncated {
                            Text("More results may match. Refine your search.").font(Typography.meta)
                        }
                    }
                }.padding(Spacing.sm)
            }
            .onChange(of: picker.selection) { _, path in if let path { proxy.scrollTo(path) } }
        }
    }

    @ViewBuilder private var preview: some View {
        if let result = picker.selection {
            let path = result.path
            Group {
                switch picker.preview {
                case .text(let text, let revision):
                    TaskCodeReader(path: path, text: text,
                                   location: result.hit.map { FileVisit(line: $0.line, hit: $0, visitID: nil) },
                                   revision: revision, ready: {})
                case .message(let message): Text(message).font(Typography.meta)
                case nil: ProgressView()
                }
            }.id(path).task(id: path) { await picker.loadPreview(path, draft: store.documents[path]) }
        }
    }

    private func open(_ result: TaskFilePicker.Result) {
        store.filePicker = nil
        store.find = nil
        switch result {
        case .file(let path): store.selectFileFromInput(path)
        case .match(let hit): store.openMatch(hit)
        }
    }

    private func cancel() {
        store.filePicker = nil
        if let view = picker.previousResponder as? NSView { view.window?.makeFirstResponder(view) }
    }
}
#endif
