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
        VStack(spacing: 0) {
            HStack(spacing: 4) {
                ScrollView(.horizontal) {
                    HStack(spacing: 12) {
                        if task != nil { Button("Brief") { store.page = .task; store.noteInteraction() } }
                        Button("Changes") {
                            store.page = .files; store.mode = .diff; store.navigator = .pinned; store.noteInteraction()
                        }
                        if prURL != nil { Button("Pull request") { store.page = .pullRequest; store.noteInteraction() } }
                        ForEach(store.tabs) { tab in
                            HStack(spacing: 4) {
                                Button(tab.path) { store.selectFile(tab.path, mode: .read) }
                                    .fontWeight(store.page == .files && store.selection == tab.path ? .semibold : .regular)
                                Button { store.closeTab(tab.id); store.noteInteraction() } label: { Image(systemName: "xmark") }
                                    .accessibilityLabel("Close \(tab.path)")
                            }
                        }
                    }.buttonStyle(.plain).font(.caption).padding(8)
                }
                navigationBar
            }
            Divider()
            if let task, store.page == .task {
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        Text(task.task.identifier).font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                        Text(task.task.name).font(Typography.strong(16))
                        TaskPlanningFactsView(task: task)
                        MarkdownBlocks(source: task.task.description).equatable()
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(14).textSelection(.enabled)
                }
                .accessibilityIdentifier("browser-task-overview")
            } else if task != nil, store.page == .pullRequest {
                TaskPullRequestView(store: store.pullRequest, url: prURL)
            } else {
                VStack(spacing: 0) {
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
            if store.mode != .diff { await store.loadFile() }
        }
        .task(id: diffIdentity) { if store.mode == .diff { await store.loadDiff() } }
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
                TextField("Find in scratch and changes", text: $store.search)
                    .textFieldStyle(.plain).focused($navigatorFocused)
                    .accessibilityIdentifier("files-search")
                    .onSubmit {
                        let paths = store.scratchPaths + store.changedPaths
                        if let path = paths.first {
                            selectFile(path, mode: store.scratchPaths.contains(path) ? .read : .diff)
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
                            row(path, label: String(path.dropFirst("scratch/".count)), mode: .read)
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
                        Text(reason).font(.caption).foregroundStyle(palette.warning)
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
                Image(systemName: "sidebar.left")
                    .frame(width: 24, height: 28).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Show or hide file browser (⌘P)")
            .accessibilityLabel("Show or hide file browser")
            .accessibilityIdentifier("files-toggle-navigator")
            if store.page == .files, let document = store.selectedDocument, document.snapshot?.state == .text {
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
            if store.page == .files {
                Picker("View", selection: Binding(get: { store.mode }, set: { store.mode = $0; store.noteInteraction() })) {
                    Text("Read").tag(TaskFilesStore.Mode.read)
                    Text("Edit").tag(TaskFilesStore.Mode.edit)
                    Text("Diff").tag(TaskFilesStore.Mode.diff)
                        .disabled(store.changesError != nil)
                }.pickerStyle(.menu).labelsHidden()
                    .accessibilityLabel("View").frame(width: 70).fixedSize()
            }
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
            .workSelection(store.selection == path && (store.mode == .diff) == (mode == .diff))
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
            if snapshot.state == .text, store.rendersSelection {
                TaskFileReadingView(document: document, baseURL: store.query.remoteMachine == nil
                    ? URL(fileURLWithPath: store.cwd).appendingPathComponent(document.path) : nil,
                    interacted: store.noteInteraction)
            } else if snapshot.state == .text {
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
    func makeNSView(context: Context) -> NSView { NSView() }
    func updateNSView(_ host: NSView, context: Context) {
        if document.scroll.superview !== host {
            host.subviews.forEach { $0.removeFromSuperview() }
            document.scroll.removeFromSuperview()
            document.scroll.frame = host.bounds
            document.scroll.autoresizingMask = [.width, .height]
            host.addSubview(document.scroll)
        }
        document.editor.applyPalette(palette)
        document.scroll.backgroundColor = NSColor(palette.background)
    }
}

private struct TaskPatchView: NSViewRepresentable {
    let patch: String
    let lines: [TaskPatchLine]
    @Environment(\.palette) private var palette
    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSScrollView()
        let text = TaskPatchTextView(usingTextLayoutManager: true)
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
        guard let text = scroll.documentView as? TaskPatchTextView else { return }
        text.update(patch: patch, lines: lines, palette: palette)
        scroll.backgroundColor = NSColor(palette.background)
    }
}

final class TaskPatchTextView: NSTextView {
    private var renderedPalette: PaletteDefinition?
    private var renderedLines: [TaskPatchLine] = []

    func update(patch: String, lines: [TaskPatchLine], palette: LoopflowPalette) {
        let display = patch.isEmpty ? "No textual changes." : patch
        guard string != display || renderedPalette != palette.definition || renderedLines != lines else { return }
        if string != display { string = display }
        let storage = textStorage
        storage?.beginEditing()
        storage?.setAttributes([
            .font: NSFont.monospacedSystemFont(ofSize: 12, weight: .regular),
            .foregroundColor: NSColor(palette.text),
        ], range: NSRange(location: 0, length: (display as NSString).length))
        for line in lines {
            storage?.addAttribute(.backgroundColor,
                                  value: NSColor(line.added ? palette.successTone.fill : palette.failureTone.fill),
                                  range: line.range)
        }
        storage?.endEditing()
        applyPalette(palette)
        renderedPalette = palette.definition
        renderedLines = lines
    }
}

#endif
