#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// The breadcrumb, PR row and document toolbar share one baseline.
enum TaskFileChrome {
    static let headerHeight: CGFloat = 40
}

struct TaskFilesView: View {
    @Bindable var store: TaskFilesStore
    let prURL: URL?
    /// The recorded base moves with a store commit, not a file.
    let prBase: String?
    @Environment(\.palette) private var palette
    @FocusState private var navigatorFocused: Bool

    private var diffIdentity: String {
        "\(store.selection ?? "")|\(store.changes?.baseCommit ?? "")|\(store.selectedDocument?.editVersion ?? 0)|\(store.mode)"
    }

    var body: some View {
        HSplitView {
            navigator
            documentPane
        }
        .background(palette.background)
        .task(id: "\(store.base)|\(store.needsComparison)|\(prBase ?? "")") {
            // Later readings follow the checkout and its Git metadata changing.
            if store.needsComparison { await store.refreshChanges() }
        }
        .task(id: store.showIgnored) {
            store.observeFiles()
            await store.refreshDirectories()
        }
        .task(id: "\(store.selection ?? "")|\(store.mode)") {
            if store.mode == .file { await store.loadFile() }
        }
        .task(id: diffIdentity) { if store.mode == .diff { await store.loadDiff() } }
    }

    private var navigator: some View {
        VStack(spacing: 0) {
            if let prURL {
                HStack {
                    Link("Pull request ↗", destination: prURL)
                    Spacer()
                }
                .font(Typography.caption(11))
                .padding(.horizontal, 10)
                .frame(height: TaskFileChrome.headerHeight)
                .background(palette.surfaceMuted)
                Divider()
            } else {
                // Preserve the document/terminal baseline without an empty PR strip.
                Color.clear.frame(height: TaskFileChrome.headerHeight)
            }
            ScrollViewReader { reader in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 2) {
                        Toggle("Show ignored", isOn: $store.showIgnored).font(.caption)
                        directoryRows("")
                        sectionHeader("Changes", expanded: $store.showsChanges).padding(.top, 12)
                        if store.showsChanges {
                            if let reason = store.changesError {
                                Text(reason).font(.caption).foregroundStyle(palette.textSecondary)
                            }
                            Picker("Compare against", selection: $store.base) {
                                Text("Parent").tag("parent")
                                Text("HEAD").tag("head")
                            }
                            .pickerStyle(.segmented).labelsHidden()
                            .accessibilityLabel("Compare against")
                            .padding(.vertical, 4)
                            if let sha = store.changes?.baseCommit {
                                Text(String(sha.prefix(10)))
                                    .font(.system(size: 10, design: .monospaced))
                                    .foregroundStyle(palette.textSecondary).padding(.bottom, 4)
                            }
                            ForEach(store.changes?.files ?? []) { file in
                                row(file.path, label: file.path, mode: .diff)
                            }
                        }
                    }
                    .padding(8)
                }
                .focusable().focused($navigatorFocused)
                .onMoveCommand { direction in
                    let rows = visibleFiles("").map { ($0, TaskFilesStore.Mode.file) }
                        + (store.showsChanges ? (store.changes?.files ?? []).map { ($0.path, TaskFilesStore.Mode.diff) } : [])
                    guard !rows.isEmpty, direction == .up || direction == .down else { return }
                    let current = rows.firstIndex { $0.0 == store.selection && $0.1 == store.mode }
                    let index = min(max((current ?? -1) + (direction == .down ? 1 : -1), 0), rows.count - 1)
                    store.selection = rows[index].0
                    store.mode = rows[index].1
                    reader.scrollTo("\(rows[index].1.rawValue):\(rows[index].0)")
                }
            }
            if let directory = store.changes?.recoveryDirectory {
                Button("Open saved versions ↗") {
                    NSWorkspace.shared.open(URL(fileURLWithPath: directory))
                }.buttonStyle(.plain).font(.caption).lineLimit(1).padding(8)
            }
        }
        .background(palette.surfaceMuted)
        .frame(minWidth: 190, idealWidth: 230, maxWidth: 320)
    }

    private func directoryRows(_ path: String) -> AnyView {
        AnyView(VStack(alignment: .leading, spacing: 2) {
            if let error = store.directoryErrors[path] {
                Text(error).font(.caption).foregroundStyle(Color.statusWarning)
            }
            ForEach(store.directories[path]?.entries ?? []) { entry in
                if entry.kind == .directory {
                    Button {
                        if store.expandedDirectories.contains(entry.path) {
                            store.expandedDirectories.remove(entry.path)
                        } else {
                            store.expandedDirectories.insert(entry.path)
                            Task { await store.loadDirectory(entry.path) }
                        }
                    } label: {
                        Label((entry.path as NSString).lastPathComponent,
                              systemImage: store.expandedDirectories.contains(entry.path) ? "folder.fill" : "folder")
                            .font(.system(size: 11, design: .monospaced))
                            .padding(.vertical, 5)
                    }.buttonStyle(.plain)
                    if store.expandedDirectories.contains(entry.path) {
                        directoryRows(entry.path).padding(.leading, 12)
                    }
                } else {
                    row(entry.path, label: (entry.path as NSString).lastPathComponent
                        + (entry.kind == .symlink ? " ↗" : ""), mode: .file)
                }
            }
            if store.directories[path]?.nextCursor != nil {
                Button("Load more") { Task { await store.loadDirectory(path, more: true) } }.font(.caption)
            }
        })
    }

    private func visibleFiles(_ path: String) -> [String] {
        (store.directories[path]?.entries ?? []).flatMap { entry in
            if entry.kind == .directory {
                return store.expandedDirectories.contains(entry.path) ? visibleFiles(entry.path) : []
            }
            return [entry.path]
        }
    }

    private var documentPane: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Text(store.selection ?? "Files").lineLimit(1).truncationMode(.middle)
                Spacer(minLength: 0)
                if let document = store.selectedDocument, document.snapshot?.state == .text {
                    Button(document.saving ? "Saving…" : "Save") {
                        Task {
                            await document.save(issue: store.issue, cwd: store.cwd, query: store.query)
                            await store.refresh()
                        }
                    }
                    .disabled(!document.canSave)
                    .keyboardShortcut("s", modifiers: .command)
                    .fixedSize()
                }
                Picker("View", selection: $store.mode) {
                    Text("File").tag(TaskFilesStore.Mode.file)
                    Text("Diff").tag(TaskFilesStore.Mode.diff)
                        .disabled(store.changesError != nil)
                }.pickerStyle(.segmented).labelsHidden()
                    .accessibilityLabel("View").frame(width: 110).fixedSize()
            }
            .font(Typography.caption(11)).padding(.horizontal, 10)
            .frame(height: TaskFileChrome.headerHeight)
            .background(palette.surfaceMuted)
            Divider()
            if let error = store.error {
                Text(error).font(.caption).foregroundStyle(Color.statusWarning).padding(8)
            }
            if let reason = store.selectedDocument?.readOnlyReason {
                Text(reason).font(.caption).padding(8)
            }
            if let document = store.selectedDocument, let external = document.external {
                Text("\(stateLabel(external.state)) · local draft retained; saving unavailable")
                    .font(.caption).padding(8)
            }
            content
            if let document = store.selectedDocument {
                if let message = document.saveMessage {
                    Text(message).font(.system(size: 10)).foregroundStyle(palette.textSecondary).padding(6)
                } else if document.dirty {
                    Text(store.autosave && document.canSave ? "Autosave pending · draft retained" : "Unsaved draft · retained in this window")
                        .font(.system(size: 10)).foregroundStyle(palette.textSecondary).padding(6)
                }
                if !document.recoveries.isEmpty {
                    HStack {
                        Text(document.recoveries.contains { $0.changed }
                             ? "Recovery needs attention" : "Previous versions retained")
                        Menu("Recovery · \(document.recoveries.count)") {
                            ForEach(Array(document.recoveries.enumerated()), id: \.element.id) { index, recovery in
                                Button("\(recovery.changed ? "⚠ " : "")Previous version \(index + 1)") {
                                    NSWorkspace.shared.open(URL(fileURLWithPath: recovery.directory))
                                }.help(recovery.message)
                            }
                        }.fixedSize()
                    }.font(.caption).padding(6)
                }
            }
        }
        .frame(minWidth: 280, maxWidth: .infinity, maxHeight: .infinity)
    }

    private func sectionHeader(_ title: String, expanded: Binding<Bool>) -> some View {
        Button { expanded.wrappedValue.toggle() } label: {
            HStack(spacing: 6) {
                Image(systemName: expanded.wrappedValue ? "chevron.down" : "chevron.right")
                    .font(.system(size: 9, weight: .semibold)).frame(width: 10)
                Text(title).font(Typography.caption(11))
                Spacer(minLength: 0)
            }.padding(.vertical, 5).contentShape(Rectangle())
        }
        .buttonStyle(.plain).accessibilityLabel(title)
        .accessibilityValue(expanded.wrappedValue ? "Expanded" : "Collapsed")
    }

    private func row(_ path: String, label: String, mode: TaskFilesStore.Mode) -> some View {
        Button {
            store.selection = path
            store.mode = mode
            navigatorFocused = true
        } label: {
            HStack {
                Text(label)
                    .font(.system(size: 11, design: .monospaced)).lineLimit(1).truncationMode(.middle)
                if store.documents[path]?.dirty == true { Text("•") }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 4).padding(.vertical, 5)
            .background(store.selection == path && store.mode == mode ? palette.selectionTint : .clear)
            .contentShape(Rectangle())
        }.buttonStyle(.plain).help(path)
            .id("\(mode.rawValue):\(path)")
            .accessibilityLabel(path)
    }

    @ViewBuilder private var content: some View {
        if store.selection == nil {
            ContentUnavailableView("Select a file", systemImage: "doc.text")
        } else if store.mode == .diff {
            if let diff = store.diff {
                if diff.binary { ContentUnavailableView("Binary diff", systemImage: "doc") }
                else {
                    TaskPatchView(patch: diff.patch, lines: store.patchLines)
                    if diff.truncated { Text("Diff truncated at 1 MB").font(.caption) }
                }
            } else if let reason = store.changesError {
                ContentUnavailableView("Comparison unavailable", systemImage: "doc", description: Text(reason))
            } else { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
        } else if let document = store.selectedDocument, let snapshot = document.snapshot {
            if snapshot.state == .text {
                TaskDocumentView(document: document)
            } else {
                ContentUnavailableView(stateLabel(snapshot.state), systemImage: "doc")
            }
        } else { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
    }

    private func stateLabel(_ state: TaskFileState) -> String {
        switch state {
        case .text: "File"
        case .binary: "Binary file"
        case .missing: "File no longer exists"
        case .truncated: "File exceeds 1 MB · preview unavailable"
        case .unsupportedEncoding: "Unsupported text encoding"
        }
    }
}

private struct TaskDocumentView: NSViewRepresentable {
    let document: TaskFileDocument
    @Environment(\.palette) private var palette
    func makeNSView(context: Context) -> NSView { NSView() }
    func updateNSView(_ host: NSView, context: Context) {
        if document.scroll.superview !== host {
            host.subviews.forEach { $0.removeFromSuperview() }
            document.scroll.removeFromSuperview()
            document.scroll.frame = host.bounds
            document.scroll.autoresizingMask = [.width, .height]
            host.addSubview(document.scroll)
        }
        document.editor.backgroundColor = NSColor(palette.background)
        document.editor.textColor = NSColor(palette.text)
    }
}

private struct TaskPatchView: NSViewRepresentable {
    let patch: String
    let lines: [TaskPatchLine]
    @Environment(\.palette) private var palette
    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSScrollView()
        let text = NSTextView(usingTextLayoutManager: true)
        text.isEditable = false
        text.isRichText = false
        text.usesFindBar = true
        text.isVerticallyResizable = true
        text.autoresizingMask = [.width]
        text.textContainer?.widthTracksTextView = true
        text.textContainerInset = NSSize(width: 12, height: 12)
        scroll.hasVerticalScroller = true
        scroll.documentView = text
        return scroll
    }
    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let text = scroll.documentView as? NSTextView else { return }
        let display = patch.isEmpty ? "No textual changes." : patch
        guard text.string != display else { return }
        let attributed = NSMutableAttributedString(string: display, attributes: [
            .font: NSFont.monospacedSystemFont(ofSize: 12, weight: .regular),
            .foregroundColor: NSColor(palette.text),
        ])
        for line in lines {
            let color = (line.added ? NSColor.systemGreen : NSColor.systemRed).withAlphaComponent(0.10)
            attributed.addAttribute(.backgroundColor, value: color, range: line.range)
        }
        text.textStorage?.setAttributedString(attributed)
        text.backgroundColor = NSColor(palette.background)
    }
}
#endif
