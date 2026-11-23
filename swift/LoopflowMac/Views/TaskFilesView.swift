#if os(macOS)
import AppKit
import Loopflow
import SwiftUI
import WebKit

/// Compact document controls leave room for the file itself.
enum TaskFileChrome {
    static let headerHeight: CGFloat = 30
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
    @FocusState private var navigatorFocused: Bool
    @FocusState private var treeFocused: Bool
    @State private var treePosition = ScrollPosition()

    private var diffIdentity: String {
        "\(store.selection ?? "")|\(store.changes?.baseCommit ?? "")|\(store.selectedDocument?.editVersion ?? 0)|\(store.mode)"
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
        .background(palette.background)
        .background {
            // ⌘P from any page opens Files with its navigator.
            HStack {
                Button("Choose file") { toggleNavigator() }
                    .keyboardShortcut("p", modifiers: .command)
                Button("Pin file navigation") { store.togglePinnedNavigator() }
                    .keyboardShortcut("b", modifiers: [.control, .command])
            }.disabled(!isFocused).hidden().accessibilityHidden(true).frame(width: 0, height: 0)
        }
        .tint(palette.actionInk)
        .task(id: "\(page)|\(checkoutBase ?? "")") {
            guard page == .files else { return }
            store.observeFiles()
            await store.refreshChanges()
        }
        .task(id: "\(store.selection ?? "")|\(store.mode)") {
            if store.mode != .diff { await store.loadFile() }
        }
        .task(id: diffIdentity) { if store.mode == .diff { await store.loadDiff() } }
    }

    /// The open document, named in full with its place among the others, as
    /// one menu that switches between them; then its controls. Only the Files
    /// page has them.
    private var documentRow: some View {
        HStack(spacing: Spacing.xs) {
            if let selection = store.selection {
                Menu {
                    ForEach(store.tabs) { tab in
                        Toggle(isOn: Binding(get: { store.selection == tab.path },
                                             set: { _ in store.selectFileFromInput(tab.path) })) {
                            Text(store.documents[tab.path]?.dirty == true ? "\(tab.path) (unsaved)" : tab.path)
                        }
                    }
                    Divider()
                    if let tab = store.tabs.first(where: { $0.path == selection }) {
                        Button("Close \((selection as NSString).lastPathComponent)") { store.closeTab(tab.id); store.noteInteraction() }
                    }
                } label: {
                    HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                        Text((selection as NSString).lastPathComponent).font(Typography.mono).foregroundStyle(palette.text)
                            .lineLimit(1).truncationMode(.middle)
                        if store.documents[selection]?.dirty == true { UnsavedDraftMark() }
                        if let place = store.tabPlace {
                            Text(place).font(Typography.meta).foregroundStyle(palette.textSecondary)
                        }
                    }
                }
                .menuStyle(.borderlessButton)
                .padding(.horizontal, Spacing.sm)
                .help(selection)
                .accessibilityIdentifier("files-open")
            }
            Spacer(minLength: 4)
            navigationBar
        }
    }

    /// Where the selected file lives: the Machine as a place, then the
    /// checkout and directory as a path.
    private var location: some View {
        let checkout = URL(fileURLWithPath: store.cwd).lastPathComponent
        let directory = store.selection.map { ($0 as NSString).deletingLastPathComponent } ?? ""
        return HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
            Text(store.query.remoteMachine ?? "This Mac").font(Typography.meta).fixedSize()
            Text(checkout + (directory.isEmpty ? "" : "/" + directory)).font(Typography.mono)
                .lineLimit(1).truncationMode(.head)
        }
        .foregroundStyle(palette.textSecondary)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.xxs)
        .help(store.cwd)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("files-location")
    }

    private var filesPage: some View {
        VStack(spacing: 0) {
            location
            GeometryReader { geometry in
                let docked = store.navigator == .pinned && geometry.size.width >= 360
                HSplitView {
                    if docked {
                        navigator.frame(minWidth: 140, idealWidth: 220, maxWidth: 300)
                    }
                    documentPane
                }
                .overlay(alignment: .topLeading) {
                    if store.navigator != .closed && !docked {
                        ZStack(alignment: .topLeading) {
                            Color.black.opacity(0.001).contentShape(Rectangle())
                                .onTapGesture { dismissNavigator() }
                            navigator
                                .frame(width: min(360, geometry.size.width),
                                       height: max(0, geometry.size.height - 8))
                                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.pane))
                                .overlay(RoundedRectangle(cornerRadius: CornerRadius.pane).strokeBorder(palette.border))
                                .shadow(color: .black.opacity(0.2), radius: 12, x: 3, y: 6)
                                .accessibilityIdentifier("files-navigation-dropdown")
                        }
                    }
                }
            }
        }
    }

    private func toggleNavigator() {
        store.noteInteraction()
        let wasFiles = store.page == .files
        store.page = .files
        if wasFiles || store.navigator == .closed { store.toggleNavigator() }
        navigatorFocused = store.navigator != .closed
    }

    private func dismissNavigator() {
        store.dismissNavigator()
        navigatorFocused = false
        if let editor = store.selectedDocument?.retainedEditor, editor.window != nil {
            editor.window?.makeFirstResponder(editor)
        }
    }

    private var navigator: some View {
        VStack(spacing: 0) {
            HStack {
                Image(systemName: "magnifyingglass").foregroundStyle(palette.textSecondary)
                TextField("Find in scratch and opened folders", text: $store.search)
                    .textFieldStyle(.plain).focused($navigatorFocused)
                    .accessibilityIdentifier("files-search")
                    .onSubmit {
                        if let path = store.scratchPaths.first {
                            read(path)
                        } else if let entry = store.treeRows.compactMap(\.entry).first(where: { $0.kind != .directory }) {
                            read(entry.path)
                        }
                    }
                    .onKeyPress(.downArrow) {
                        treeFocused = true
                        Task { await store.moveTree(.down) }
                        return .handled
                    }
                Button { store.togglePinnedNavigator() } label: {
                    Image(systemName: store.navigator == .pinned ? "pin.slash" : "pin")
                }
                .buttonStyle(.plain)
                .accessibilityLabel(store.navigator == .pinned ? "Close file navigation" : "Pin file navigation")
                .accessibilityIdentifier("files-pin-navigator")
            }.padding(Spacing.sm)
            Divider()
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: Spacing.xxs) {
                        sectionHeader("scratch", expanded: $store.showsScratch)
                        if store.showsScratch {
                            ForEach(store.scratchPaths, id: \.self) { path in
                                scratchRow(path)
                            }
                            if store.changes?.scratchTruncated == true {
                                Text("Scratch list truncated").font(Typography.meta).foregroundStyle(palette.textSecondary)
                            }
                        }
                        Text(URL(fileURLWithPath: store.cwd).lastPathComponent)
                            .font(Typography.meta).foregroundStyle(palette.textSecondary)
                            .padding(.top, Spacing.md).padding(.bottom, Spacing.xxs)
                            .help(store.cwd)
                        let badges = store.changeBadges
                        ForEach(store.treeRows) { row in treeRow(row, badges: badges) }
                        if let reason = store.changesError {
                            FailureLine("Comparison unavailable.", machine: reason)
                        } else if !store.search.isEmpty, store.scratchPaths.isEmpty, store.treeRows.isEmpty {
                            Text("No matching files.").emptyLine()
                        }
                    }.padding(Spacing.sm)
                }
                .scrollPosition($treePosition)
                .onScrollGeometryChange(for: CGPoint.self, of: \.contentOffset) { _, offset in store.treeOffset = offset }
                .focusable().focused($treeFocused).focusEffectDisabled()
                .onKeyPress(keys: [.upArrow, .downArrow, .leftArrow, .rightArrow, .return]) { press in
                    let move: TaskFilesStore.TreeMove = switch press.key {
                    case .upArrow: .up
                    case .downArrow: .down
                    case .leftArrow: .collapse
                    case .rightArrow: .expand
                    default: .open
                    }
                    Task {
                        let wasDropdown = store.navigator == .dropdown
                        await store.moveTree(move)
                        if move == .open, wasDropdown, store.navigator == .closed { dismissNavigator() }
                        if let selection = store.tree.selection { proxy.scrollTo("entry:\(selection)") }
                    }
                    return .handled
                }
            }
            .accessibilityIdentifier("files-tree")
        }
        .background(palette.surfaceMuted)
        .task { await store.openTree() }
        .onAppear {
            treePosition.scrollTo(point: store.treeOffset)
            if store.navigator == .dropdown { navigatorFocused = true }
        }
        .onExitCommand { dismissNavigator() }
    }

    @ViewBuilder private func treeRow(_ row: CheckoutTree.Row, badges: [String: CheckoutChange]) -> some View {
        let indent = CGFloat(row.depth) * 12
        switch row {
        case .entry(let entry, _):
            let label = store.search.isEmpty ? (entry.path as NSString).lastPathComponent : entry.path
            Button {
                if entry.kind == .directory {
                    Task { await store.setDirectory(entry.path, expanded: !store.tree.isExpanded(entry.path)) }
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
            Button("Load more") { Task { await store.readDirectory(directory, more: true) } }
                .buttonStyle(LoopflowButtonStyle()).padding(.leading, indent + 16)
                .accessibilityIdentifier("files-load-more")
        case .failed(let directory, let reason, _):
            FailureLine("Not read.", machine: reason) {
                Button("Retry") { Task { await store.retryDirectory(directory) } }
            }.lineLimit(2).padding(.leading, indent + 16)
        }
    }

    private var navigationBar: some View {
        HStack(spacing: Spacing.sm) {
            Button { toggleNavigator() } label: {
                Image(systemName: "sidebar.left")
                    .frame(width: 24, height: 28).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Show or hide file browser (⌘P)")
            .accessibilityLabel("Show or hide file browser")
            .accessibilityIdentifier("files-toggle-navigator")
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
            Picker("View", selection: Binding(get: { store.mode }, set: { store.mode = $0; store.noteInteraction() })) {
                Text("Read").tag(TaskFilesStore.Mode.read)
                Text("Edit").tag(TaskFilesStore.Mode.edit)
                    .disabled(store.selectedDocument?.canEdit != true)
                Text("Diff").tag(TaskFilesStore.Mode.diff)
                    .disabled(store.changesError != nil)
            }.pickerStyle(.segmented).labelsHidden()
                .accessibilityLabel("View").fixedSize()
        }
        .font(Typography.meta).padding(.horizontal, Spacing.sm)
        .frame(height: TaskFileChrome.headerHeight)
        .fixedSize(horizontal: true, vertical: false)
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
        .simultaneousGesture(TapGesture().onEnded { store.noteInteraction() })
    }

    private func sectionHeader(_ title: String, expanded: Binding<Bool>) -> some View {
        Button { expanded.wrappedValue.toggle() } label: {
            HStack(spacing: Spacing.xs) {
                Image(systemName: expanded.wrappedValue ? "chevron.down" : "chevron.right")
                    .font(Typography.meta).fontWeight(.semibold).frame(width: 10)
                Text(title).font(Typography.meta)
                Spacer(minLength: 0)
            }.padding(.vertical, Spacing.xxs).contentShape(Rectangle())
        }
        .buttonStyle(.plain).accessibilityLabel(title)
        .accessibilityValue(expanded.wrappedValue ? "Expanded" : "Collapsed")
    }

    /// Open a file the navigator offers; a dropdown has done its job.
    private func read(_ path: String) {
        let wasDropdown = store.navigator == .dropdown
        store.selectFileFromInput(path)
        if wasDropdown { dismissNavigator() }
    }

    private func scratchRow(_ path: String) -> some View {
        Button {
            read(path)
        } label: {
            HStack {
                Text(path.dropFirst("scratch/".count))
                    .font(Typography.mono).lineLimit(1).truncationMode(.middle)
                if store.documents[path]?.dirty == true { UnsavedDraftMark() }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, Spacing.xs).padding(.vertical, Spacing.xxs)
            .workSelection(store.selection == path)
            .contentShape(Rectangle())
        }.buttonStyle(.plain).help(path)
            .accessibilityLabel(path)
    }

    @ViewBuilder private var content: some View {
        if store.selection == nil {
            EmptyState(title: "Select a file")
        } else if store.mode == .diff {
            if let review = store.fileReview, review.path == store.selection {
                TaskFileDiffView(store: store, review: review, isFocused: isFocused, addToConversation: addToConversation)
                    .onChange(of: review.path, initial: true) { reportVisible(review.path) }
            } else if let reason = store.changesError {
                EmptyState(title: "Comparison unavailable", machine: reason) {
                    Button("Retry") { Task { await store.refreshChanges() } }
                        .accessibilityIdentifier("files-comparison-retry")
                }
            } else { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
        } else if let document = store.selectedDocument, let snapshot = document.snapshot {
            if snapshot.state == .text, store.mode == .read {
                TaskFileReadingView(document: document, baseURL: store.query.remoteMachine == nil
                    ? URL(fileURLWithPath: store.cwd).appendingPathComponent(document.path) : nil,
                    interacted: store.noteInteraction, ready: { reportVisible(document.path) })
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
    var interacted: () -> Void = {}
    var ready: () -> Void = {}

    var body: some View {
        switch (document.path as NSString).pathExtension.lowercased() {
        case "html":
            TaskHTMLView(html: document.text, baseURL: baseURL, interacted: interacted, ready: ready)
                .accessibilityIdentifier("files-rendered-html")
        case "md":
            ScrollView {
                MarkdownBlocks(source: document.text).equatable()
                    .frame(maxWidth: 720, alignment: .leading)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(Spacing.md).textSelection(.enabled)
            }
            .onScrollPhaseChange { _, phase in if phase == .interacting { interacted() } }
            .accessibilityIdentifier("files-rendered-markdown")
            .onAppear(perform: ready)
        default:
            TaskCodeReader(path: document.path, text: document.text, ready: ready)
                .onScrollPhaseChange { _, phase in if phase == .interacting { interacted() } }
        }
    }
}

/// Pages run their scripts but never reach the network; links open in the browser.
/// A rewrite reloads in place at the same scroll position.
struct TaskHTMLView: View {
    let html: String
    let baseURL: URL?
    let interacted: () -> Void
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
                        offset = new
                        if !loading { interacted() }
                    }
                    .simultaneousGesture(TapGesture().onEnded { interacted() })
            } else {
                Color.clear
            }
        }
        .task(id: html) { await show() }
    }

    private func show() async {
        if page == nil {
            guard let rules = await TaskHTMLRules.offline() else {
                let blank = WebPage()
                page = blank
                await Self.load(blank, html: "<p>Page unavailable: network blocking could not be set up.</p>", baseURL: nil)
                return
            }
            var configuration = WebPage.Configuration()
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

/// Files' Diff mode: one file through the review stream, compared with the
/// unsaved draft while there is one. Next/previous steps the file's changes,
/// then opens the next or previous changed file here.
private struct TaskFileDiffView: View {
    @Bindable var store: TaskFilesStore
    let review: TaskFilesStore.FileReview
    let isFocused: Bool
    let addToConversation: ((String) -> Bool)?
    @Environment(\.palette) private var palette
    @State private var width: CGFloat = 0

    private var reader: TaskFilesStore.ReviewReader { .file(review.path) }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: Spacing.sm) {
                ReviewStepper(store: store, reader: reader, isFocused: isFocused)
                Text((store.comparisonDescription ?? store.revision) + (review.draft ? " Against the unsaved draft." : ""))
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
                    ReviewStream(store: store, reader: reader, files: review.diff.files, split: store.layout(width: width) == .split,
                                 addToConversation: addToConversation)
                }
                .scrollTargetLayout()
            }
            .scrollPosition(id: store.anchor(reader), anchor: .top)
            .onScrollPhaseChange { _, phase in if phase == .interacting { store.noteInteraction() } }
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
