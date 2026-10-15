#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// Compact document controls leave room for the file itself.
enum TaskFileChrome {
    static let headerHeight: CGFloat = 40
}

struct TaskFilesView: View {
    @Bindable var store: TaskFilesStore
    let prURL: URL?
    /// The recorded base moves with a store commit, not a file.
    let checkoutBase: String?
    var task: RoadmapTask? = nil
    var isFocused = true
    @Environment(\.palette) private var palette
    @FocusState private var navigatorFocused: Bool

    private var diffIdentity: String {
        "\(store.selection ?? "")|\(store.changes?.baseCommit ?? "")|\(store.selectedDocument?.editVersion ?? 0)|\(store.mode)"
    }

    var body: some View {
        Group {
            if let task, store.page == .task {
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        Text(task.task.identifier).font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                        Text(task.task.name).font(Typography.strong(16))
                        TaskDueDateView(task: task)
                        MarkdownBlocks(source: task.task.description).equatable()
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(14).textSelection(.enabled)
                }
                .accessibilityIdentifier("browser-task-overview")
            } else if task != nil, store.page == .pullRequest {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Pull request").font(Typography.strong(16))
                    if let prURL {
                        Link(prURL.absoluteString, destination: prURL).buttonStyle(WorkLinkStyle())
                        Text("Open the pull request to review discussion and checks.")
                            .foregroundStyle(palette.textSecondary)
                        Button("Browse changes") { store.showsChanges = true; store.page = .files }
                    } else {
                        Text("No pull request published yet.").foregroundStyle(palette.textSecondary)
                    }
                    Spacer()
                }
                .frame(maxWidth: .infinity, alignment: .leading).padding(14)
                .accessibilityIdentifier("browser-pull-request")
            } else {
                VStack(spacing: 0) {
                    navigationBar
                    GeometryReader { geometry in
                        HSplitView {
                            if store.navigator == .pinned {
                                navigator.frame(minWidth: 140, idealWidth: 220, maxWidth: 300)
                            }
                            documentPane
                        }
                        .overlay(alignment: .topLeading) {
                            if store.navigator == .dropdown {
                                ZStack(alignment: .topLeading) {
                                    Color.black.opacity(0.001).contentShape(Rectangle())
                                        .onTapGesture { dismissNavigator() }
                                    navigator
                                        .frame(width: min(360, geometry.size.width),
                                               height: max(0, geometry.size.height - 8))
                                        .clipShape(RoundedRectangle(cornerRadius: 10))
                                        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(palette.border))
                                        .shadow(color: .black.opacity(0.2), radius: 12, x: 3, y: 6)
                                        .accessibilityIdentifier("files-navigation-dropdown")
                                }
                            }
                        }
                    }
                }
                .background {
                    HStack {
                        Button("Choose file") { toggleNavigator() }
                            .keyboardShortcut("p", modifiers: .command)
                        Button("Pin file navigation") { store.togglePinnedNavigator() }
                            .keyboardShortcut("b", modifiers: [.control, .command])
                    }.disabled(!isFocused).hidden().accessibilityHidden(true).frame(width: 0, height: 0)
                }
            }
        }
        .background(palette.background)
        .tint(palette.actionInk)
        .task(id: "\(store.page)|\(checkoutBase ?? "")") {
            guard task == nil || store.page == .files else { return }
            store.observeFiles()
            await store.refreshChanges()
        }
        .task(id: "\(store.selection ?? "")|\(store.mode)") {
            if store.mode == .file { await store.loadFile() }
        }
        .task(id: diffIdentity) { if store.mode == .diff { await store.loadDiff() } }
    }

    private func toggleNavigator() {
        store.toggleNavigator()
        navigatorFocused = store.navigator != .closed
    }

    private func dismissNavigator() {
        store.dismissNavigator()
        navigatorFocused = false
        if let editor = store.selectedDocument?.editor, editor.window != nil {
            editor.window?.makeFirstResponder(editor)
        }
    }

    private var navigator: some View {
        VStack(spacing: 0) {
            HStack {
                Image(systemName: "magnifyingglass").foregroundStyle(palette.textSecondary)
                TextField("Find in scratch and changes", text: $store.search)
                    .textFieldStyle(.plain).focused($navigatorFocused)
                    .accessibilityIdentifier("files-search")
                    .onSubmit {
                        let paths = store.scratchPaths + store.changedPaths
                        if let path = paths.first {
                            selectFile(path, mode: store.scratchPaths.contains(path) ? .file : .diff)
                        }
                    }
                Button { store.togglePinnedNavigator() } label: {
                    Image(systemName: store.navigator == .pinned ? "pin.slash" : "pin")
                }
                .buttonStyle(.plain)
                .accessibilityLabel(store.navigator == .pinned ? "Close file navigation" : "Pin file navigation")
                .accessibilityIdentifier("files-pin-navigator")
            }.padding(10)
            Divider()
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 4) {
                    sectionHeader("scratch", expanded: $store.showsScratch)
                    if store.showsScratch {
                        ForEach(store.scratchPaths, id: \.self) { path in
                            row(path, label: String(path.dropFirst("scratch/".count)), mode: .file)
                        }
                        if store.changes?.scratchTruncated == true {
                            Text("Scratch list truncated").font(.caption).foregroundStyle(palette.textSecondary)
                        }
                    }
                    sectionHeader("Changed files", expanded: $store.showsChanges).padding(.top, 12)
                    if store.showsChanges {
                        ForEach(store.changedPaths, id: \.self) { path in row(path, label: path, mode: .diff) }
                    }
                    if store.isRefreshingChanges, store.changes == nil { ProgressView().controlSize(.small) }
                    if let reason = store.changesError {
                        Text(reason).font(.caption).foregroundStyle(palette.waiting)
                    } else if store.changes != nil, store.scratchPaths.isEmpty, store.changedPaths.isEmpty {
                        Text(store.search.isEmpty ? "No scratch or changed files" : "No matching files")
                            .font(.caption).foregroundStyle(palette.textSecondary)
                    }
                }.padding(10)
            }
            if let sha = store.changes?.baseCommit {
                Text("Parent · \(sha.prefix(10))").font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(palette.textSecondary).padding(8)
            }
            if store.query.remoteMachine == nil, let directory = store.changes?.recoveryDirectory {
                Button("Open saved versions ↗") {
                    NSWorkspace.shared.open(URL(fileURLWithPath: directory))
                }.buttonStyle(WorkPlainButtonStyle()).font(.caption).lineLimit(1).padding(8)
            }
        }
        .background(palette.surfaceMuted)
        .onAppear { if store.navigator == .dropdown { navigatorFocused = true } }
        .onExitCommand { dismissNavigator() }
    }

    private var navigationBar: some View {
            HStack(spacing: 8) {
                Button { toggleNavigator() } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "magnifyingglass")
                        Text(store.selection ?? "Scratch & changes")
                            .font(Typography.strong(12)).lineLimit(1).truncationMode(.middle)
                        Spacer(minLength: 0)
                        Text("⌘P").font(Typography.caption(10)).foregroundStyle(palette.textSecondary)
                        Image(systemName: store.navigator == .closed ? "chevron.down" : "chevron.up")
                            .font(.system(size: 9, weight: .semibold))
                    }.contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Choose file")
                .accessibilityIdentifier("files-toggle-navigator")
                if let document = store.selectedDocument, document.snapshot?.state == .text {
                    Button {
                        Task {
                            await document.save(issue: store.issue, cwd: store.cwd, query: store.query)
                            await store.refresh()
                        }
                    } label: {
                        Image(systemName: document.saving ? "arrow.triangle.2.circlepath" : "square.and.arrow.down")
                    }
                    .buttonStyle(.plain)
                    .help(document.saving ? "Saving…" : "Save")
                    .accessibilityLabel("Save")
                    .disabled(!document.canSave)
                    .keyboardShortcut("s", modifiers: .command)
                    .fixedSize()
                }
                Picker("View", selection: $store.mode) {
                    Text("File").tag(TaskFilesStore.Mode.file)
                    Text("Diff").tag(TaskFilesStore.Mode.diff)
                        .disabled(store.changesError != nil)
                }.pickerStyle(.menu).labelsHidden()
                    .accessibilityLabel("View").frame(width: 65).fixedSize()
            }
            .font(Typography.caption(11)).padding(.horizontal, 10)
            .frame(height: TaskFileChrome.headerHeight)
            .background(palette.surfaceMuted)
    }

    private var documentPane: some View {
        VStack(spacing: 0) {
            if let error = store.error {
                Text(error).font(.caption).foregroundStyle(palette.waiting).padding(8)
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
        .frame(minWidth: 160, maxWidth: .infinity, maxHeight: .infinity)
    }

    private func sectionHeader(_ title: String, expanded: Binding<Bool>) -> some View {
        Button { expanded.wrappedValue.toggle() } label: {
            HStack(spacing: 6) {
                Image(systemName: expanded.wrappedValue ? "chevron.down" : "chevron.right")
                    .font(.system(size: 9, weight: .semibold)).frame(width: 10)
                Text(title).font(Typography.caption(11))
                Spacer(minLength: 0)
            }.padding(.vertical, 2).contentShape(Rectangle())
        }
        .buttonStyle(.plain).accessibilityLabel(title)
        .accessibilityValue(expanded.wrappedValue ? "Expanded" : "Collapsed")
    }

    private func selectFile(_ path: String, mode: TaskFilesStore.Mode) {
        let wasDropdown = store.navigator == .dropdown
        store.selectFile(path, mode: mode)
        if wasDropdown { dismissNavigator() }
    }

    private func row(_ path: String, label: String, mode: TaskFilesStore.Mode) -> some View {
        Button {
            selectFile(path, mode: mode)
        } label: {
            HStack {
                Text(label)
                    .font(.system(size: 11, design: .monospaced)).lineLimit(1).truncationMode(.middle)
                if store.documents[path]?.dirty == true { Text("•") }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 4).padding(.vertical, 2)
            .workSelection(store.selection == path && store.mode == mode)
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
