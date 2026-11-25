#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// Compact document controls leave room for the file itself.
enum TaskFileChrome {
    static let headerHeight: CGFloat = 32
    static let dockedWidth: CGFloat = 620
}

private extension String {
    var openRowID: String { "open:" + self }
}

private extension TaskChangedFile {
    var changedRowID: String { "changed:" + path }
}

struct TaskFilesView: View {
    @Bindable var store: TaskFilesStore
    /// The recorded base moves with a store commit, not a file.
    let checkoutBase: String?
    var task: RoadmapTask? = nil
    /// Where the Task stands, for the Brief's lead.
    var graph: TaskGraph? = nil
    var isFocused = true
    /// Types into the Task conversation without sending.
    var addToConversation: ((String) -> Bool)? = nil
    @Environment(\.palette) private var palette
    @FocusState private var treeFocused: Bool
    @State private var treePosition = ScrollPosition()

    @State private var paneWidth: CGFloat = 0
    private var docked: Bool { paneWidth >= TaskFileChrome.dockedWidth }
    private var showsRail: Bool { store.browser.showsRail ?? docked }

    private var diffIdentity: String {
        "\(store.selection ?? "")|\(store.workingChanges?.baseCommit ?? "")|\(store.selectedDocument?.editVersion ?? 0)|\(store.mode)|\(store.visit?.visitID?.uuidString ?? "")"
    }

    private var page: TaskFilesStore.Page { store.shownPage(hasTask: task != nil) }

    var body: some View {
        VStack(spacing: 0) {
            switch page {
            case .brief:
                if let task { TaskBriefView(store: store, task: task, graph: graph) }
            case .changes:
                TaskChangesView(store: store, task: task, isFocused: isFocused, addToConversation: addToConversation)
            case .files:
                documentRow
                if let find = store.find {
                    Divider()
                    FileFindBar(find: find, store: store, focused: isFocused && store.filePicker == nil)
                }
                Divider()
                ReviewCommentsStrip(store: store, isFocused: isFocused, addToConversation: addToConversation)
                filesPage
            }
            notice
        }
        .background(palette.background)
        .onGeometryChange(for: CGFloat.self, of: \.size.width) { paneWidth = $0 }
        .overlay {
            if let picker = store.filePicker {
                TaskFilePickerView(store: store, picker: picker, isFocused: isFocused).id(ObjectIdentifier(picker))
            }
        }
        .tint(palette.actionInk)
        .task(id: "\(page)|\(checkoutBase ?? "")") {
            store.observeFiles()
            if page == .files { await store.enterFiles() }
            else if store.workingChanges == nil, page == .brief || store.diffReview.reviewReading !== store.workingTree {
                await store.workingTree.refresh()
            }
        }
        .task(id: "\(store.selection ?? "")|\(store.mode)") {
            if store.mode != .diff { await store.loadFile() }
        }
        .onChange(of: store.selection) { store.find?.reset() }
        .onChange(of: store.mode) { store.find?.reset() }
        .onChange(of: store.selectedDocument?.editVersion) { store.jumps.dismiss() }
        .task(id: diffIdentity) { if page == .files, store.mode == .diff { await store.loadDiff() } }
    }

    @ViewBuilder private var notice: some View {
        switch store.browser.notice {
        case .presented(let request) where page != .brief:
            notice("Presented " + request.path) { store.openPresentation(request) }
        case .updated(let path) where page == .files:
            notice("Updated " + path) { read(path) }
        default: EmptyView()
        }
    }

    private func notice(_ text: String, open: @escaping () -> Void) -> some View {
        HStack {
            Text(text).lineLimit(1).truncationMode(.middle)
            Spacer()
            Button("Open", action: open).buttonStyle(LoopflowButtonStyle())
            Button { store.browser.notice = nil } label: { Image(systemName: "xmark") }
                .buttonStyle(.plain).accessibilityLabel("Dismiss file notice")
                .accessibilityIdentifier("files-dismiss-notice")
        }
        .font(Typography.meta).padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.xxs)
        .background(palette.surfaceMuted)
    }

    private var documentRow: some View {
        HStack(spacing: Spacing.xs) {
            Button {
                store.browser.showsRail = !showsRail
                treeFocused = showsRail
            } label: { Image(systemName: "sidebar.left") }
            .accessibilityLabel("Show or hide file browser")
            .accessibilityIdentifier("files-toggle-navigator")
            ForEach([true, false], id: \.self) { back in
                Button { store.jumps.step(backwards: back, in: store) } label: {
                    Image(systemName: back ? "chevron.left" : "chevron.right")
                }.disabled((back ? store.jumps.back : store.jumps.forward).isEmpty)
                    .accessibilityLabel(back ? "Back in Files" : "Forward in Files")
            }
            if let path = store.selection {
                // Narrower panes drop leading folders first: a bare `mod.rs` or `index.ts` names nothing.
                ViewThatFits(in: .horizontal) {
                    HStack(spacing: Spacing.xxs) {
                        folders(path)
                        filename(path)
                        ReadingSymbol(store: store)
                    }.fixedSize()
                    HStack(spacing: Spacing.xxs) {
                        folders(path, last: 1)
                        filename(path)
                        ReadingSymbol(store: store)
                    }.fixedSize()
                    HStack(spacing: Spacing.xxs) {
                        folders(path, last: 1)
                        filename(path)
                    }.fixedSize()
                    filename(path)
                }
            }
            Spacer(minLength: 0)
            if docked { navigationBar }
            Menu {
                Button("Find File…") { store.chooseFile() }
                Button("Search Checkout…") { store.chooseFile(.grep) }
                Button("File Outline") { store.jumps.search(.outline, line: store.jumps.readingLine, in: store) }
                if !docked {
                    Picker("View", selection: $store.mode) { modeChoices }
                    if let document = store.selectedDocument { saveButton(document) }
                }
                Divider()
                Text(store.machineName ?? "This Mac")
                Text(store.cwd + (store.selection.map { "/" + $0 } ?? ""))
                Button("Refresh") { Task { await store.refresh(); await store.refreshScratch(); await store.loadFile() } }
            } label: { Image(systemName: "ellipsis") }
            .menuStyle(.borderlessButton).fixedSize().accessibilityLabel("File actions")
        }
        .buttonStyle(.plain).font(Typography.meta).padding(.horizontal, Spacing.sm)
        .frame(height: TaskFileChrome.headerHeight)
    }

    private func filename(_ path: String) -> some View {
        Menu {
            ForEach(store.tabs) { tab in
                Button(tab.path + (store.documents[tab.path]?.dirty == true ? " (unsaved)" : "")) { read(tab.path) }
            }
            if let current = store.tabs.first(where: { $0.path == path }) {
                Divider()
                Button("Close " + path) { store.closeTab(current.id) }
            }
        } label: {
            Text((path as NSString).lastPathComponent).font(Typography.mono).lineLimit(1).truncationMode(.middle)
        }.menuStyle(.borderlessButton).help((store.machineName ?? "This Mac") + " · " + store.cwd + "/" + path)
            .accessibilityIdentifier("files-open")
    }

    /// The path's folders as menus, or only the `last` few after an ellipsis.
    private func folders(_ path: String, last: Int? = nil) -> some View {
        let parts = path.split(separator: "/").dropLast()
        let shown = last.map { parts.suffix($0) } ?? parts[...]
        return Group {
            if shown.startIndex > parts.startIndex {
                Text("… /").foregroundStyle(palette.textSecondary)
            }
            ForEach(shown.indices, id: \.self) { index in
                let parent = parts.prefix(index).joined(separator: "/")
                Menu(String(parts[index])) {
                    ForEach(store.tree.listings[parent]?.entries ?? [], id: \.path) { entry in
                        Button((entry.path as NSString).lastPathComponent) {
                            if entry.kind == .directory {
                                store.browser.showsRail = true
                                Task { await store.tree.revealDirectory(entry.path); store.tree.selection = entry.path }
                            } else { read(entry.path) }
                        }
                    }
                    if store.tree.listings[parent]?.nextCursor != nil {
                        Button("Load more") { Task { await store.tree.readDirectory(parent, more: true) } }
                    }
                }.menuStyle(.borderlessButton).fixedSize()
                Text("/").foregroundStyle(palette.textSecondary)
            }
        }
    }

    private var filesPage: some View {
        VStack(spacing: 0) {
            if let peek = store.jumps.peek, peek.lookup == .outline { FileSymbolPeek(store: store, peek: peek) }
            HStack(spacing: 0) {
                if docked && showsRail { navigator; Divider() }
                documentPane
            }
            .overlay(alignment: .leading) {
                if !docked && showsRail {
                    ZStack(alignment: .leading) {
                        Color.black.opacity(0.1).onTapGesture { dismissDrawer() }
                        navigator
                    }
                }
            }
        }
        .task(id: "\(store.selection ?? "")|\(store.tree.showIgnored)") {
            if let path = store.selection { await store.tree.reveal(path) }
            else if showsRail { await store.tree.openRoot() }
        }
    }

    private func dismissDrawer() {
        if !docked { store.browser.showsRail = false; treeFocused = false }
    }

    private var navigator: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    heading("Open & Scratch")
                    ForEach(store.openAndScratch, id: \.openRowID) { path in fileRow(path) }
                    if let error = store.scratchError {
                        FailureLine("Scratch unavailable.", machine: error) {
                            Button("Retry") { Task { await store.refreshScratch() } }
                        }.padding(.horizontal, Spacing.sm)
                    }
                    heading("Changed in Task")
                    if let changes = store.workingChanges {
                        ForEach(changes.files, id: \.changedRowID) { file in
                            fileRow(file.path, lines: file.lines)
                        }
                    } else if let error = store.workingTree.error {
                        FailureLine("Changes unavailable.", machine: error) {
                            Button("Retry") { Task { await store.workingTree.refresh() } }
                        }.padding(.horizontal, Spacing.sm)
                    } else { ProgressView().controlSize(.mini).padding(.horizontal, Spacing.sm) }
                    HStack(alignment: .firstTextBaseline) {
                        heading("Checkout")
                        Spacer()
                        Toggle("Show Ignored", isOn: Bindable(store.tree).showIgnored).toggleStyle(.checkbox)
                            .font(Typography.meta).padding(.trailing, Spacing.sm)
                    }
                    let badges = store.changeBadges
                    ForEach(store.tree.rows()) { row in treeRow(row, badges: badges) }
                }.padding(.bottom, Spacing.md)
            }
            .scrollPosition($treePosition)
            .onScrollGeometryChange(for: CGPoint.self, of: \.contentOffset) { _, offset in store.tree.offset = offset }
            .focusable().focused($treeFocused).focusEffectDisabled()
            .onChange(of: store.tree.selection) { _, path in
                if let path { proxy.scrollTo(CheckoutTree.Row.entryID(path)) }
            }
            .onKeyPress(keys: [.upArrow, .downArrow, .leftArrow, .rightArrow, .return]) { press in
                let move: CheckoutTree.Move = switch press.key {
                case .upArrow: .up
                case .downArrow: .down
                case .leftArrow: .collapse
                case .rightArrow: .expand
                default: .open
                }
                Task { await moveTree(move) }
                return .handled
            }
        }
        .frame(width: ChangedFilesSidebar.width).background(palette.surfaceMuted)
        .accessibilityIdentifier("files-tree")
        .onChange(of: store.selection) { if !docked { dismissDrawer() } }
        .task { await store.tree.openRoot() }
        .onAppear { treePosition.scrollTo(point: store.tree.offset) }
        .onExitCommand { dismissDrawer() }
    }

    private func heading(_ text: String) -> some View {
        Text(text).font(Typography.textStrong).foregroundStyle(palette.textSecondary)
            .padding(.horizontal, Spacing.sm).padding(.top, Spacing.md).padding(.bottom, Spacing.xxs)
    }

    @ViewBuilder private func treeRow(_ row: CheckoutTree.Row, badges: [String: CheckoutChange]) -> some View {
        // A narrow step keeps nested names readable in the 236-point rail.
        let indent = Spacing.sm + CGFloat(row.depth) * Spacing.sm
        switch row {
        case .entry(let entry, let label, _):
            Button {
                if entry.kind == .directory {
                    Task { await store.tree.setDirectory(entry.path, expanded: !store.tree.isExpanded(entry.path)) }
                } else {
                    store.tree.selection = entry.path
                    read(entry.path)
                }
            } label: {
                HStack(spacing: Spacing.xs) {
                    Image(systemName: entry.kind == .directory
                          ? (store.tree.isExpanded(entry.path) ? "chevron.down" : "chevron.right")
                          : entry.kind == .symlink ? "arrow.turn.up.right" : "doc")
                        .font(Typography.meta).fontWeight(.semibold).frame(width: 10)
                        .foregroundStyle(palette.textSecondary)
                    Text(label)
                        .font(Typography.mono).lineLimit(1).truncationMode(.middle)
                    if store.documents[entry.path]?.dirty == true { UnsavedDraftMark() }
                    Spacer(minLength: 4)
                    if let badge = badges[entry.path] {
                        Text(badge.rawValue).font(Typography.mono)
                            .foregroundStyle(badge == .untracked ? palette.successTone.ink : palette.actionInk)
                            .help(badge.help)
                    }
                }
                .padding(.leading, indent).padding(.trailing, Spacing.sm).padding(.vertical, 3)
                .frame(maxWidth: .infinity, alignment: .leading)
                .workSelection(store.tree.selection == entry.path || store.selection == entry.path)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain).help(entry.path)
            .accessibilityLabel(entry.path)
            .accessibilityValue(entry.kind == .directory ? (store.tree.isExpanded(entry.path) ? "Expanded" : "Collapsed") : "")
        case .loading:
            ProgressView().controlSize(.mini).padding(.leading, indent + 14)
        case .more(let directory, _):
            Button("Load more") { Task { await store.tree.readDirectory(directory, more: true) } }
                .buttonStyle(LoopflowButtonStyle()).padding(.leading, indent + 14)
                .accessibilityIdentifier("files-load-more")
        case .failed(let directory, let reason, _):
            FailureLine("Not read.", machine: reason) {
                Button("Retry") { Task { await store.tree.retryDirectory(directory) } }
            }.lineLimit(2).padding(.leading, indent + 14)
        }
    }

    private var navigationBar: some View {
        HStack(spacing: Spacing.sm) {
            if let document = store.selectedDocument, document.snapshot?.state == .text {
                saveButton(document).labelStyle(.iconOnly)
            }
            Picker("View", selection: $store.mode) {
                modeChoices
            }.pickerStyle(.segmented).labelsHidden()
                .accessibilityLabel("View").fixedSize()
        }
        .font(Typography.meta).padding(.horizontal, Spacing.sm)
        .frame(height: TaskFileChrome.headerHeight)
        .fixedSize(horizontal: true, vertical: false)
    }

    private func saveButton(_ document: TaskFileDocument) -> some View {
        Button {
            Task {
                await document.save(issue: store.issue, query: store.query)
                await store.refresh()
            }
        } label: {
            Label("Save", systemImage: document.saving ? "arrow.triangle.2.circlepath" : "square.and.arrow.down")
        }
        .help(document.saving ? "Saving…" : "Save")
        .disabled(!document.canSave)
        .keyboardShortcut("s")
    }

    @ViewBuilder private var modeChoices: some View {
        Text("Read").tag(TaskFilesStore.Mode.read)
        Text("Edit").tag(TaskFilesStore.Mode.edit).disabled(store.selectedDocument?.canEdit != true)
        Text("Diff").tag(TaskFilesStore.Mode.diff).disabled(store.workingTree.error != nil)
    }

    private var documentPane: some View {
        VStack(spacing: 0) {
            if let error = store.error {
                FailureLine("Latest read failed.", machine: error) {
                    Button("Retry") { Task { await store.retry() } }
                        .accessibilityIdentifier("files-retry")
                }.padding(Spacing.sm)
            }
            if let reason = store.selectedDocument?.readOnlyReason {
                Text(reason).font(Typography.meta).padding(Spacing.sm)
            }
            if let document = store.selectedDocument, let external = document.external {
                Text("\(external.state.title) · local draft retained; saving unavailable")
                    .font(Typography.meta).padding(Spacing.sm)
            }
            content
            if let document = store.selectedDocument {
                if let message = document.saveMessage {
                    Text(message).font(Typography.meta).foregroundStyle(palette.textSecondary).padding(Spacing.xs)
                } else if document.dirty {
                    Text(store.autosave && document.canSave ? "Autosave pending · draft retained" : "Unsaved draft · retained in this window")
                        .font(Typography.meta).foregroundStyle(palette.textSecondary).padding(Spacing.xs)
                }
                if !document.recoveries.isEmpty {
                    HStack {
                        Text(document.recoveries.contains { $0.changed }
                             ? "Recovery needs attention" : "Previous versions retained")
                        Menu("Recovery · \(document.recoveries.count)") {
                            ForEach(Array(document.recoveries.enumerated()), id: \.element.id) { index, recovery in
                                Button {
                                    NSWorkspace.shared.open(URL(fileURLWithPath: recovery.directory))
                                } label: {
                                    if recovery.changed {
                                        Label("Previous version \(index + 1)", systemImage: "exclamationmark.triangle")
                                    } else {
                                        Text("Previous version \(index + 1)")
                                    }
                                }.help(recovery.message)
                            }
                        }.fixedSize()
                    }.font(Typography.meta).padding(Spacing.xs)
                }
            }
        }
        .frame(minWidth: 160, maxWidth: .infinity, maxHeight: .infinity)
    }

    func moveTree(_ move: CheckoutTree.Move) async {
        if let file = await store.tree.move(move) { read(file) }
    }

    private func read(_ path: String) {
        if let presentation = store.browser.presentations.first(where: { $0.path == path }) {
            store.openPresentation(presentation)
        } else { store.selectFileFromInput(path) }
        dismissDrawer()
    }

    private func fileRow(_ path: String, lines: TaskChangedFile.ChangedLines? = nil) -> some View {
        let place = ChangedFilesSidebar.place(path, under: nil)
        return Button { read(path) } label: {
            VStack(alignment: .leading, spacing: 1) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                    Text(place.name).font(Typography.mono).lineLimit(1).truncationMode(.middle)
                    if store.documents[path]?.dirty == true { UnsavedDraftMark() }
                    if store.browser.presentations.contains(where: { $0.path == path }) {
                        Image(systemName: "doc.badge.clock").accessibilityLabel("Presented")
                    }
                    if store.browser.updated.contains(path) {
                        Image(systemName: "circle.fill").font(Typography.meta).foregroundStyle(palette.actionInk)
                            .accessibilityLabel("Updated")
                    }
                    Spacer(minLength: 4)
                    if let lines { LineCounts(added: lines.added, removed: lines.removed).fixedSize() }
                }
                if let directory = place.directory {
                    Text(directory).font(Typography.meta).foregroundStyle(palette.textSecondary)
                        .lineLimit(1).truncationMode(.head)
                }
            }
            .padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.xs)
            .frame(maxWidth: .infinity, alignment: .leading)
            .workSelection(store.selection == path).contentShape(Rectangle())
        }.buttonStyle(.plain).help(path).accessibilityLabel(path)
    }

    @ViewBuilder private var content: some View {
        if store.selection == nil {
            EmptyState(title: "Select a file")
        } else if store.mode == .diff {
            if let review = store.diffReview.fileReview, review.path == store.selection {
                TaskFileDiffView(store: store, review: review, isFocused: isFocused, addToConversation: addToConversation)
                    .onChange(of: review.path, initial: true) { reportVisible(review.path) }
            } else if let reason = store.diffReview.fileDiffError ?? store.workingTree.error {
                EmptyState(title: "Comparison unavailable", machine: reason) {
                    Button("Retry") { Task { await store.workingTree.refresh(); await store.loadDiff() } }
                        .accessibilityIdentifier("files-comparison-retry")
                }
            } else { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
        } else if let document = store.selectedDocument, let snapshot = document.snapshot {
            if snapshot.state == .text, store.mode == .read {
                TaskFileReadingView(document: document, store: store, addToConversation: addToConversation,
                                    ready: { reportVisible(document.path) })
                    .onAppear { store.browser.visibleReadPath = document.path }
                    .onDisappear {
                        if store.browser.visibleReadPath == document.path { store.browser.visibleReadPath = nil }
                    }
                    .id(document.path)
            } else if snapshot.state == .text {
                TaskDocumentView(document: document, isFocused: isFocused && store.find == nil, ready: { reportVisible(document.path) })
                    .id(document.path)
                    .task(id: FileFind.Scan(query: store.find?.query, revision: document.editVersion)) {
                        if let find = store.find { await find.search([(.editor, document.text)]) }
                    }
                    .onChange(of: store.find?.current, initial: true) {
                        guard isFocused, !document.editor.hasMarkedText(), let match = store.find?.current,
                              match.cell == .editor, NSMaxRange(match.range) <= document.text.utf16.count else { return }
                        document.editor.setSelectedRange(match.range)
                        document.editor.scrollRangeToVisible(match.range)
                    }
            } else if let data = snapshot.data, NSImage(data: data) != nil {
                ScrollView { TaskImage(file: snapshot).padding(Spacing.lg) }
                    .task(id: store.find?.query) { await store.find?.search([]) }
                    .onAppear { reportVisible(snapshot.path) }
            } else {
                EmptyState(title: snapshot.state.title)
                    .task(id: store.find?.query) { await store.find?.search([]) }
            }
        } else { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
    }

    private func reportVisible(_ path: String) {
        guard page == .files, store.selection == path else { return }
        Perf.stateObserved(.openFile, id: path, owner: store.performanceOwner)
    }
}

/// The symbol around the reading line. Scrolling changes that line, so only this label redraws.
private struct ReadingSymbol: View {
    let store: TaskFilesStore

    var body: some View {
        if let symbol = store.selectedDocument?.outline?.enclosing(store.jumps.readingLine) {
            Text("› " + symbol.name).font(Typography.meta)
        }
    }
}

/// Render the working document, including any local edits.
struct TaskFileReadingView: View {
    let document: TaskFileDocument
    let store: TaskFilesStore
    var addToConversation: ((String) -> Bool)?
    var ready: () -> Void = {}

    private var hasChanges: Bool {
        document.dirty || store.workingTree.filesByPath[document.path] != nil
    }

    var body: some View {
        switch (document.path as NSString).pathExtension.lowercased() {
        case "html" where store.visit == nil:
            let baseURL = store.machine == nil
                ? URL(fileURLWithPath: store.cwd).appendingPathComponent(document.path) : nil
            TaskHTMLView(html: document.text, baseURL: baseURL, find: store.find, ready: ready)
                .accessibilityIdentifier("files-rendered-html")
        case "md" where store.visit == nil:
            ScrollViewReader { proxy in
                ScrollView {
                    MarkdownBlocks(source: document.text, find: store.find).equatable()
                        .frame(maxWidth: 720, alignment: .leading)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(Spacing.md).textSelection(.enabled)
                }
                .onChange(of: store.find?.current) {
                    if case .markdown(let index, _) = store.find?.current?.cell { proxy.scrollTo(MarkdownBlocks.anchor(index), anchor: .center) }
                }
            }
            .accessibilityIdentifier("files-rendered-markdown")
            .onAppear(perform: ready)
        default:
            TaskCodeReader(path: document.path, text: document.text, location: store.visit, store: store,
                           revision: document.dirty ? nil : document.snapshot?.revision, addToConversation: addToConversation, ready: ready)
                .task(id: "\(document.editVersion)|\(store.workingChanges?.baseCommit ?? "")|\(hasChanges)") {
                    if hasChanges { await store.diffReview.loadDiff() }
                }
        }
    }
}

private struct TaskDocumentView: NSViewRepresentable {
    let document: TaskFileDocument
    let isFocused: Bool
    let ready: () -> Void
    @Environment(\.palette) private var palette
    @Environment(\.theme) private var theme
    func makeNSView(context: Context) -> NSView { NSView() }
    func updateNSView(_ host: NSView, context: Context) {
        if document.scroll.superview !== host {
            host.subviews.forEach { $0.removeFromSuperview() }
            document.scroll.removeFromSuperview()
            document.scroll.frame = host.bounds
            document.scroll.autoresizingMask = [.width, .height]
            document.editor.focusOnMount = isFocused
            host.addSubview(document.scroll)
        }
        document.applyTheme(theme)
        ready()
        document.scroll.backgroundColor = NSColor(palette.background)
    }
}

/// Files' Diff steps within one working file, including its unsaved draft.
private struct TaskFileDiffView: View {
    @Bindable var store: TaskFilesStore
    let review: DiffReviewState.FileReview
    let isFocused: Bool
    let addToConversation: ((String) -> Bool)?
    @Environment(\.palette) private var palette
    @State private var width: CGFloat = 0

    private var reader: DiffReviewState.ReviewReader { .file(review.path) }

    private struct SearchReading: Equatable {
        let query: String
        let file: ReviewFile?
        let layout: DiffReviewState.ReviewLayout
        let removals, moves, longLines: Set<String>
        let rendered: Bool
    }

    private var searchReading: SearchReading? {
        guard let query = store.find?.query, !query.isEmpty else { return nil }
        let state = store.diffReview
        let file = review.diff.file(review.path)
        return SearchReading(query: query, file: file.map { state.opened($0, in: reader) },
                             layout: state.layout(width: width), removals: state.openedRemovals,
                             moves: state.expandedMoves, longLines: state.openedLongLines,
                             rendered: file.flatMap { state.rendering(of: $0, in: reader) } != nil &&
                                 state.reading(for: reader).sideErrors[review.path] == nil)
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: Spacing.sm) {
                ReviewStepper(store: store, reader: reader, isFocused: isFocused)
                Text("Task base " + review.baseCommit.prefix(10) + (review.draft ? " · unsaved draft" : " · working file"))
                    .font(Typography.meta).foregroundStyle(palette.textSecondary)
                    .lineLimit(1).truncationMode(.middle)
                    .accessibilityIdentifier("files-diff-comparison")
                Spacer(minLength: 4)
                ReviewLayoutPicker(store: store, width: width)
            }
            .buttonStyle(.plain)
            .padding(.horizontal, Spacing.sm)
            .frame(height: TaskFileChrome.headerHeight)
            Divider()
            let stream = ReviewStream(store: store, reader: reader, files: review.diff.files,
                                      split: store.diffReview.layout(width: width) == .split, addToConversation: addToConversation)
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        if review.truncated { ReviewNotice(text: "Diff truncated at 1 MB") }
                        if review.diff.files.isEmpty { ReviewNotice(text: "No changes in \(review.path)") }
                        stream
                    }.scrollTargetLayout()
                }
                .scrollPosition(id: store.diffReview.anchor(reader), anchor: .top)
                .onChange(of: store.find?.current) {
                    switch store.find?.current?.cell {
                    case .diff(let row, _, _): proxy.scrollTo(row, anchor: .center)
                    case .markdown(let block, _): proxy.scrollTo(MarkdownBlocks.anchor(block), anchor: .center)
                    case .html(let page, _): proxy.scrollTo("html-side-\(page)", anchor: .center)
                    default: break
                    }
                }
            }
            .task(id: searchReading) {
                guard let find = store.find, !find.query.isEmpty else { return }
                guard let file = review.diff.file(review.path) else { await find.search([]); return }
                let state = store.diffReview
                if let rendering = state.rendering(of: file, in: reader), state.reading(for: reader).sideErrors[file.path] == nil {
                    if rendering == .image || rendering == .svg { await find.search([]) }
                } else { await find.search(stream.searchCells(in: state.opened(file, in: reader))) }
            }
            .onGeometryChange(for: CGFloat.self, of: \.size.width) { width = $0 }
            .accessibilityIdentifier("files-diff")
        }
    }
}

#endif

/// A file with edits not yet saved.
private struct UnsavedDraftMark: View {
    var body: some View {
        Image(systemName: "circle.fill").imageScale(.small).font(Typography.meta)
            .help("Unsaved draft").accessibilityLabel("Unsaved draft")
    }
}
