#if os(macOS)
import AppKit
import Loopflow
import SwiftUI
import WebKit

/// Compact document controls leave room for the file itself.
enum TaskFileChrome {
    static let headerHeight: CGFloat = 32
    static let dockedWidth: CGFloat = 620
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
                Divider()
                filesPage
            }
        }
        .overlay(alignment: .bottom) { notice }
        .background(palette.background)
        .onGeometryChange(for: CGFloat.self, of: \.size.width) { paneWidth = $0 }
        .overlay {
            if let picker = store.filePicker {
                TaskFilePickerView(store: store, picker: picker).id(ObjectIdentifier(picker))
            }
        }
        .tint(palette.actionInk)
        .task(id: "\(page)|\(checkoutBase ?? "")") {
            store.observeFiles()
            if page == .files { await store.enterFiles() }
            else if store.workingChanges == nil, page == .brief || store.diffReview.comparison != .workingTree(base: "parent") {
                await store.refreshWorkingChanges()
            }
        }
        .task(id: "\(store.selection ?? "")|\(store.mode)") {
            if store.mode != .diff { await store.loadFile() }
        }
        .onChange(of: store.selectedDocument?.editVersion) { store.jumps.dismiss() }
        .task(id: diffIdentity) { if store.mode == .diff { await store.loadDiff() } }
    }

    @ViewBuilder private var notice: some View {
        if let request = store.browser.presented {
            notice("Presented " + request.path) { store.openPresentation(request) }
        } else if let path = store.browser.updated.last {
            notice("Updated " + path) { read(path) }
        }
    }

    private func notice(_ text: String, open: @escaping () -> Void) -> some View {
        HStack {
            Text(text).lineLimit(1).truncationMode(.middle)
            Spacer()
            Button("Open", action: open).buttonStyle(LoopflowButtonStyle())
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
                ViewThatFits(in: .horizontal) {
                    HStack(spacing: Spacing.xxs) {
                        folders(path)
                        filename(path)
                        if let symbol = store.selectedDocument?.outline?.enclosing(store.jumps.readingLine) {
                            Text("› " + symbol.name).font(Typography.meta)
                        }
                    }.fixedSize()
                    filename(path)
                }
            }
            Spacer(minLength: 0)
            if paneWidth >= TaskFileChrome.dockedWidth { navigationBar }
            Menu {
                Button("Find File…") { store.chooseFile() }
                Button("Search Checkout…") { store.chooseFile(.grep) }
                Button("File Outline") { store.jumps.search(nil, line: store.jumps.readingLine, in: store) }
                if paneWidth < TaskFileChrome.dockedWidth {
                    Picker("View", selection: $store.mode) { modeChoices }
                    if let document = store.selectedDocument {
                        Button("Save") { Task { await document.save(issue: store.issue, query: store.query) } }
                            .disabled(!document.canSave).keyboardShortcut("s")
                    }
                }
                Divider()
                Text(store.query.remoteMachine ?? "This Mac")
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
        }.menuStyle(.borderlessButton).help((store.query.remoteMachine ?? "This Mac") + " · " + store.cwd + "/" + path)
            .accessibilityIdentifier("files-open")
    }

    private func folders(_ path: String) -> some View {
        let parts = path.split(separator: "/").dropLast()
        return ForEach(parts.indices, id: \.self) { index in
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

    private var filesPage: some View {
        VStack(spacing: 0) {
            if let peek = store.jumps.peek, peek.name == nil { FileSymbolPeek(store: store, peek: peek) }
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
                LazyVStack(alignment: .leading, spacing: Spacing.xxs) {
                    Text("Open & Scratch").font(Typography.textStrong)
                    ForEach(store.openAndScratch, id: \.self) { path in fileRow(path) }
                    if let error = store.scratchError {
                        FailureLine("Scratch unavailable.", machine: error) {
                            Button("Retry") { Task { await store.refreshScratch() } }
                        }
                    }
                    Text("Changed in Task").font(Typography.textStrong).padding(.top, Spacing.sm)
                    if let changes = store.workingChanges {
                        ForEach(changes.files) { file in
                            fileRow(file.path, count: file.lines.map { "+\($0.added) −\($0.removed)" })
                        }
                    } else if let error = store.workingTree.error {
                        FailureLine("Changes unavailable.", machine: error) {
                            Button("Retry") { Task { await store.refreshWorkingChanges() } }
                        }
                    } else { ProgressView().controlSize(.mini) }
                    HStack {
                        Text("Checkout").font(Typography.textStrong)
                        Spacer()
                        Toggle("Show Ignored", isOn: Bindable(store.tree).showIgnored).toggleStyle(.checkbox)
                    }.font(Typography.meta).padding(.top, Spacing.sm)
                    let badges = store.changeBadges
                    ForEach(store.tree.rows()) { row in treeRow(row, badges: badges) }
                }.padding(Spacing.sm)
            }
            .scrollPosition($treePosition)
            .onScrollGeometryChange(for: CGPoint.self, of: \.contentOffset) { _, offset in store.tree.offset = offset }
            .focusable().focused($treeFocused).focusEffectDisabled()
            .onChange(of: store.tree.selection) { _, path in
                if let path { proxy.scrollTo("entry:" + path) }
            }
            .onKeyPress(keys: [.upArrow, .downArrow, .leftArrow, .rightArrow, .return]) { press in
                let move: CheckoutTree.Move = switch press.key {
                case .upArrow: .up
                case .downArrow: .down
                case .leftArrow: .collapse
                case .rightArrow: .expand
                default: .open
                }
                Task {
                    await store.moveTree(move)
                    if let selection = store.tree.selection {
                        proxy.scrollTo("entry:" + selection)
                        if move == .open, selection == store.selection { dismissDrawer() }
                    }
                }
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

    @ViewBuilder private func treeRow(_ row: CheckoutTree.Row, badges: [String: CheckoutChange]) -> some View {
        let indent = CGFloat(row.depth) * 12
        switch row {
        case .entry(let entry, _):
            let label = store.tree.label(for: entry)
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
                .padding(.leading, indent + 2).padding(.trailing, Spacing.xs).padding(.vertical, Spacing.xxs)
                .frame(maxWidth: .infinity, alignment: .leading)
                .workSelection(store.tree.selection == entry.path || store.selection == entry.path)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain).help(entry.path)
            .accessibilityLabel(entry.path)
            .accessibilityValue(entry.kind == .directory ? (store.tree.isExpanded(entry.path) ? "Expanded" : "Collapsed") : "")
        case .loading:
            ProgressView().controlSize(.mini).padding(.leading, indent + 16)
        case .more(let directory, _):
            Button("Load more") { Task { await store.tree.readDirectory(directory, more: true) } }
                .buttonStyle(LoopflowButtonStyle()).padding(.leading, indent + 16)
                .accessibilityIdentifier("files-load-more")
        case .failed(let directory, let reason, _):
            FailureLine("Not read.", machine: reason) {
                Button("Retry") { Task { await store.tree.retryDirectory(directory) } }
            }.lineLimit(2).padding(.leading, indent + 16)
        }
    }

    private var navigationBar: some View {
        HStack(spacing: Spacing.sm) {
            if let document = store.selectedDocument, document.snapshot?.state == .text {
                Button {
                    Task {
                        await document.save(issue: store.issue, query: store.query)
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
            Picker("View", selection: Binding(get: { store.mode }, set: { store.mode = $0 })) {
                modeChoices
            }.pickerStyle(.segmented).labelsHidden()
                .accessibilityLabel("View").fixedSize()
        }
        .font(Typography.meta).padding(.horizontal, Spacing.sm)
        .frame(height: TaskFileChrome.headerHeight)
        .fixedSize(horizontal: true, vertical: false)
    }

    @ViewBuilder private var modeChoices: some View {
        Text("Read").tag(TaskFilesStore.Mode.read)
        Text("Edit").tag(TaskFilesStore.Mode.edit).disabled(store.selectedDocument?.canEdit != true)
        Text("Diff").tag(TaskFilesStore.Mode.diff).disabled(store.workingTree.error != nil)
    }

    private var documentPane: some View {
        VStack(spacing: 0) {
            if let error = store.error {
                FailureLine("Latest read failed.", machine: error).padding(Spacing.sm)
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

    private func read(_ path: String) {
        if let presentation = store.browser.presentations.first(where: { $0.path == path }) {
            store.openPresentation(presentation)
        } else { store.selectFileFromInput(path) }
        dismissDrawer()
    }

    private func fileRow(_ path: String, count: String? = nil) -> some View {
        Button { read(path) } label: {
            HStack {
                Text(path).font(Typography.mono).lineLimit(1).truncationMode(.middle)
                if store.documents[path]?.dirty == true { UnsavedDraftMark() }
                if store.browser.updated.contains(path) {
                    Image(systemName: "circle.fill").font(Typography.meta).foregroundStyle(palette.actionInk)
                        .accessibilityLabel("Updated")
                }
                Spacer(minLength: 0)
                if let count { Text(count).font(Typography.mono).foregroundStyle(palette.textSecondary) }
            }.padding(.horizontal, Spacing.xs).padding(.vertical, Spacing.xxs)
                .workSelection(store.selection == path).contentShape(Rectangle())
        }.buttonStyle(.plain).help(path)
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
                    Button("Retry") { Task { await store.refreshWorkingChanges(); await store.loadDiff() } }
                        .accessibilityIdentifier("files-comparison-retry")
                }
            } else { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
        } else if let document = store.selectedDocument, let snapshot = document.snapshot {
            if snapshot.state == .text, store.mode == .read {
                TaskFileReadingView(document: document, baseURL: store.query.remoteMachine == nil
                    ? URL(fileURLWithPath: store.cwd).appendingPathComponent(document.path) : nil,
                    store: store, visit: store.visit, ready: { reportVisible(document.path) })
                    .id(document.path)
            } else if snapshot.state == .text {
                TaskDocumentView(document: document, isFocused: isFocused, ready: { reportVisible(document.path) })
            } else if let data = snapshot.data, NSImage(data: data) != nil {
                ScrollView { TaskImage(file: snapshot).padding(Spacing.lg) }
                    .onAppear { reportVisible(snapshot.path) }
            } else {
                EmptyState(title: snapshot.state.title)
            }
        } else { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
    }

    private func reportVisible(_ path: String) {
        guard page == .files, store.selection == path else { return }
        Perf.stateObserved(.openFile, id: path, owner: store.performanceOwner)
    }
}

/// Render the working document, including any local edits.
struct TaskFileReadingView: View {
    let document: TaskFileDocument
    /// Where relative references resolve; `nil` for a peer's checkout.
    let baseURL: URL?
    var store: TaskFilesStore?
    var visit: FileVisit?
    var ready: () -> Void = {}

    var body: some View {
        switch (document.path as NSString).pathExtension.lowercased() {
        case "html" where visit == nil:
            TaskHTMLView(html: document.text, baseURL: baseURL, ready: ready)
                .accessibilityIdentifier("files-rendered-html")
        case "md" where visit == nil:
            ScrollView {
                MarkdownBlocks(source: document.text).equatable()
                    .frame(maxWidth: 720, alignment: .leading)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(Spacing.md).textSelection(.enabled)
            }
            .accessibilityIdentifier("files-rendered-markdown")
            .onAppear(perform: ready)
        default:
            TaskCodeReader(path: document.path, text: document.text, location: visit, store: store,
                           revision: document.dirty ? nil : document.snapshot?.revision, ready: ready)
        }
    }
}

/// Pages run their scripts but never reach the network; links open in the browser.
/// A rewrite reloads in place at the same scroll position.
struct TaskHTMLView: View {
    let html: String
    let baseURL: URL?
    var ready: () -> Void = {}

    @State private var page: WebPage?
    @State private var position = ScrollPosition()
    @State private var offset = CGPoint.zero
    /// Offsets during a load are the page settling, not the person scrolling.
    @State private var loading = false

    var body: some View {
        Group {
            if let page {
                WebView(page)
                    .webViewScrollPosition($position)
                    .webViewOnScrollGeometryChange(for: CGPoint.self, of: \.contentOffset) { _, new in
                        if !loading { offset = new }
                    }
            } else {
                Color.clear
            }
        }
        .task(id: html) { await show() }
    }

    private func show() async {
        if page != nil {
            do { try await Task.sleep(for: .milliseconds(800)) } catch { return }
        }
        if page == nil {
            guard let rules = await TaskHTMLRules.offline() else {
                let blank = WebPage()
                page = blank
                await Self.load(blank, html: "<p>Page unavailable: network blocking could not be set up.</p>", baseURL: nil)
                return
            }
            guard !Task.isCancelled else { return }
            let configuration = WebPage.Configuration()
            configuration.userContentController.add(rules)
            page = WebPage(configuration: configuration, navigationDecider: TaskHTMLLinks())
        }
        guard let page else { return }
        let restore = offset
        loading = true
        defer { loading = false }
        guard await Self.load(page, html: html, baseURL: baseURL), !Task.isCancelled else { return }
        position.scrollTo(point: restore)
        ready()
    }

    /// Whether the page finished loading; a superseded or failed load reports false.
    @discardableResult
    private static func load(_ page: WebPage, html: String, baseURL: URL?) async -> Bool {
        do {
            for try await _ in page.load(html: html, baseURL: baseURL ?? URL(string: "about:blank")!) {}
            return true
        } catch { return false }
    }
}

/// Links leave the page for the browser; everything else loads in place.
private struct TaskHTMLLinks: WebPage.NavigationDeciding {
    func decidePolicy(for action: WebPage.NavigationAction,
                      preferences: inout WebPage.NavigationPreferences) async -> WKNavigationActionPolicy {
        guard action.navigationType == .linkActivated, let url = action.request.url,
              ["http", "https", "mailto"].contains(url.scheme?.lowercased() ?? "") else { return .allow }
        NSWorkspace.shared.open(url)
        return .cancel
    }
}

/// One compiled rule list blocks every network scheme for rendered pages.
@MainActor
private enum TaskHTMLRules {
    private static var compiled: Task<WKContentRuleList?, Never>?
    private static let rules = #"""
    [{"trigger":{"url-filter":"^https?://"},"action":{"type":"block"}},
     {"trigger":{"url-filter":"^wss?://"},"action":{"type":"block"}},
     {"trigger":{"url-filter":"^ftp://"},"action":{"type":"block"}}]
    """#

    static func offline() async -> WKContentRuleList? {
        let compiled = compiled ?? Task {
            try? await WKContentRuleListStore.default().compileContentRuleList(
                forIdentifier: "loopflow-offline-pages", encodedContentRuleList: rules)
        }
        Self.compiled = compiled
        return await compiled.value
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
            ReviewCommentsStrip(store: store, isFocused: isFocused, addToConversation: addToConversation)
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if review.truncated { ReviewNotice(text: "Diff truncated at 1 MB") }
                    if review.diff.files.isEmpty {
                        ReviewNotice(text: "No changes in \(review.path)")
                    }
                    ReviewStream(store: store, reader: reader, files: review.diff.files, split: store.diffReview.layout(width: width) == .split,
                                 addToConversation: addToConversation)
                }
                .scrollTargetLayout()
            }
            .scrollPosition(id: store.diffReview.anchor(reader), anchor: .top)
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
