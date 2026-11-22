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
                if let task { TaskBriefView(store: store, task: task) }
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

    /// Open documents and their controls; only the Files page has them.
    private var documentRow: some View {
        HStack(spacing: 4) {
            ScrollView(.horizontal) {
                HStack(spacing: 12) {
                    ForEach(store.tabs) { tab in
                        HStack(spacing: 4) {
                            Button((tab.path as NSString).lastPathComponent) { store.selectFileFromInput(tab.path, mode: .read) }
                                .fontWeight(store.selection == tab.path ? .semibold : .regular)
                                .help(tab.path)
                            if store.documents[tab.path]?.dirty == true { Text("•").help("Unsaved draft") }
                            Button { store.closeTab(tab.id); store.noteInteraction() } label: { Image(systemName: "xmark") }
                                .accessibilityLabel("Close \(tab.path)")
                        }
                    }
                }.buttonStyle(.plain).font(.caption).padding(8)
            }
            navigationBar
        }
    }

    /// Where the selected file lives: Machine, checkout, directory.
    private var location: some View {
        let machine = store.query.remoteMachine ?? "this Mac"
        let checkout = URL(fileURLWithPath: store.cwd).lastPathComponent
        let directory = store.selection.map { ($0 as NSString).deletingLastPathComponent } ?? ""
        return Text([machine, checkout + (directory.isEmpty ? "" : "/" + directory)].joined(separator: " · "))
            .font(Typography.code(10)).foregroundStyle(palette.textSecondary)
            .lineLimit(1).truncationMode(.head)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 10).padding(.vertical, 3)
            .help(store.cwd)
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
                                .clipShape(RoundedRectangle(cornerRadius: 10))
                                .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(palette.border))
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
        if let editor = store.selectedDocument?.editor, editor.window != nil {
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
            }.padding(10)
            Divider()
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 2) {
                        sectionHeader("scratch", expanded: $store.showsScratch)
                        if store.showsScratch {
                            ForEach(store.scratchPaths, id: \.self) { path in
                                scratchRow(path)
                            }
                            if store.changes?.scratchTruncated == true {
                                Text("Scratch list truncated").font(.caption).foregroundStyle(palette.textSecondary)
                            }
                        }
                        Text(URL(fileURLWithPath: store.cwd).lastPathComponent)
                            .font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                            .padding(.top, 12).padding(.bottom, 2)
                            .help(store.cwd)
                        let badges = store.changeBadges
                        ForEach(store.treeRows) { row in treeRow(row, badges: badges) }
                        if let reason = store.changesError {
                            Text(reason).font(.caption).foregroundStyle(palette.warning)
                        } else if !store.search.isEmpty, store.scratchPaths.isEmpty, store.treeRows.isEmpty {
                            Text("No matching files").font(.caption).foregroundStyle(palette.textSecondary)
                        }
                    }.padding(10)
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
                HStack(spacing: 4) {
                    Image(systemName: entry.kind == .directory
                          ? (store.tree.isExpanded(entry.path) ? "chevron.down" : "chevron.right")
                          : entry.kind == .symlink ? "arrow.turn.up.right" : "doc")
                        .font(.system(size: 9, weight: .semibold)).frame(width: 10)
                        .foregroundStyle(palette.textSecondary)
                    Text(label)
                        .font(.system(size: 11, design: .monospaced)).lineLimit(1).truncationMode(.middle)
                    if store.documents[entry.path]?.dirty == true { Text("•").help("Unsaved draft") }
                    Spacer(minLength: 4)
                    if let badge = badges[entry.path] {
                        Text(badge.rawValue).font(Typography.code(10))
                            .foregroundStyle(badge == .untracked ? palette.successTone.ink : palette.actionInk)
                            .help(badge.help)
                    }
                }
                .padding(.leading, indent + 2).padding(.trailing, 4).padding(.vertical, 2)
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
                .buttonStyle(.link).font(.caption).padding(.leading, indent + 16)
                .accessibilityIdentifier("files-load-more")
        case .failed(let directory, let reason, _):
            HStack(spacing: 6) {
                Text(reason).font(.caption).foregroundStyle(palette.warning).lineLimit(2)
                Button("Retry") { Task { await store.retryDirectory(directory) } }
                    .buttonStyle(.link).font(.caption)
            }.padding(.leading, indent + 16)
        }
    }

    private var navigationBar: some View {
        HStack(spacing: 8) {
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
                Text("Diff").tag(TaskFilesStore.Mode.diff)
                    .disabled(store.changesError != nil)
            }.pickerStyle(.menu).labelsHidden()
                .accessibilityLabel("View").frame(width: 70).fixedSize()
        }
        .font(Typography.caption(11)).padding(.horizontal, 10)
        .frame(height: TaskFileChrome.headerHeight)
        .fixedSize(horizontal: true, vertical: false)
    }

    private var documentPane: some View {
        VStack(spacing: 0) {
            if let error = store.error {
                Text(error).font(.caption).foregroundStyle(palette.warning).padding(8)
            }
            if let reason = store.selectedDocument?.readOnlyReason {
                Text(reason).font(.caption).padding(8)
            }
            if let document = store.selectedDocument, let external = document.external {
                Text("\(external.state.title) · local draft retained; saving unavailable")
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
        .simultaneousGesture(TapGesture().onEnded { store.noteInteraction() })
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

    /// Open a file the navigator offers; a dropdown has done its job.
    private func read(_ path: String) {
        let wasDropdown = store.navigator == .dropdown
        store.selectFileFromInput(path, mode: .read)
        if wasDropdown { dismissNavigator() }
    }

    private func scratchRow(_ path: String) -> some View {
        Button {
            read(path)
        } label: {
            HStack {
                Text(path.dropFirst("scratch/".count))
                    .font(.system(size: 11, design: .monospaced)).lineLimit(1).truncationMode(.middle)
                if store.documents[path]?.dirty == true { Text("•") }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 4).padding(.vertical, 2)
            .workSelection(store.selection == path)
            .contentShape(Rectangle())
        }.buttonStyle(.plain).help(path)
            .accessibilityLabel(path)
    }

    @ViewBuilder private var content: some View {
        if store.selection == nil {
            ContentUnavailableView("Select a file", systemImage: "doc.text")
        } else if store.mode == .diff {
            if let review = store.fileReview {
                TaskFileDiffView(store: store, review: review, isFocused: isFocused, addToConversation: addToConversation)
            } else if let reason = store.changesError {
                ContentUnavailableView("Comparison unavailable", systemImage: "doc", description: Text(reason))
            } else { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
        } else if let document = store.selectedDocument, let snapshot = document.snapshot {
            if snapshot.state == .text, store.rendersSelection {
                TaskFileReadingView(document: document, baseURL: store.query.remoteMachine == nil
                    ? URL(fileURLWithPath: store.cwd).appendingPathComponent(document.path) : nil,
                    interacted: store.noteInteraction)
            } else if snapshot.state == .text {
                TaskDocumentView(document: document)
            } else {
                ContentUnavailableView(snapshot.state.title, systemImage: "doc")
            }
        } else { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity) }
    }
}

/// A document as you read it: Markdown through `MarkdownBlocks`, HTML as a live page.
struct TaskFileReadingView: View {
    let document: TaskFileDocument
    /// Where relative references resolve; `nil` for a peer's checkout.
    let baseURL: URL?
    var interacted: () -> Void = {}

    var body: some View {
        if (document.path as NSString).pathExtension.lowercased() == "html" {
            TaskHTMLView(html: document.text, baseURL: baseURL, interacted: interacted)
                .accessibilityIdentifier("files-rendered-html")
        } else {
            ScrollView {
                MarkdownBlocks(source: document.text).equatable()
                    .frame(maxWidth: 720, alignment: .leading)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(14).textSelection(.enabled)
            }
            .onScrollPhaseChange { _, phase in if phase == .interacting { interacted() } }
            .accessibilityIdentifier("files-rendered-markdown")
        }
    }
}

/// Pages run their scripts but never reach the network; links open in the browser.
/// A rewrite reloads in place at the same scroll position.
private struct TaskHTMLView: View {
    let html: String
    let baseURL: URL?
    let interacted: () -> Void

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
    @Environment(\.palette) private var palette
    @Environment(\.theme) private var theme
    func makeNSView(context: Context) -> NSView { NSView() }
    func updateNSView(_ host: NSView, context: Context) {
        if document.scroll.superview !== host {
            host.subviews.forEach { $0.removeFromSuperview() }
            document.scroll.removeFromSuperview()
            document.scroll.frame = host.bounds
            document.scroll.autoresizingMask = [.width, .height]
            host.addSubview(document.scroll)
        }
        document.applyTheme(theme)
        document.editor.recordReadiness()
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
            HStack(spacing: 8) {
                ReviewStepper(store: store, reader: reader, isFocused: isFocused)
                Text(store.revision + (review.draft ? " · unsaved draft" : ""))
                    .font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                    .lineLimit(1).truncationMode(.middle)
                    .accessibilityIdentifier("files-diff-comparison")
                Spacer(minLength: 4)
                ReviewLayoutPicker(store: store, width: width)
            }
            .buttonStyle(.plain)
            .padding(.horizontal, 10)
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
