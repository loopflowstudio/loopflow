#if os(macOS)
import AppKit
import Combine
import Loopflow
import Observation

/// One native buffer per Task/path. Its view, undo stack and selection outlive navigation.
@MainActor
@Observable
final class TaskFileDocument: NSObject, NSTextViewDelegate, @preconcurrency NSTextStorageDelegate {
    let path: String
    private(set) var snapshot: TaskFileSnapshot?
    private(set) var external: TaskFileSnapshot?
    private(set) var recoveries: [TaskFileRecovery] = []
    private(set) var saveMessage: String?
    private(set) var saving = false
    private(set) var text = ""
    private(set) var editVersion = 0
    let editor = TaskFileTextView(usingTextLayoutManager: true)
    let scroll = NSScrollView()
    private let undo = UndoManager()
    private var incomingGeneration = 0
    private var deferredSnapshot: TaskFileSnapshot?
    private var applyingIncoming = false
    private(set) var saveFailed = false
    private(set) var readOnlyReason: String?
    var onEdit: (() -> Void)?
    /// The palette the editor shows, once it has been on screen.
    @ObservationIgnored private var palette: LoopflowPalette?
    @ObservationIgnored private var coloringScheduled = false

    var canSave: Bool {
        dirty && !saving && external == nil && deferredSnapshot == nil
            && editor.isEditable && !editor.hasMarkedText()
    }

    func reconcile(_ next: TaskFileSnapshot) async {
        defer { editor.recordReadiness() }
        incomingGeneration += 1
        let generation = incomingGeneration
        updateAccess(next)
        // Content reads skip history; keep what the last Save disclosed.
        if !next.recoveries.isEmpty { recoveries = next.recoveries }
        guard !saving, !editor.hasMarkedText() else {
            deferredSnapshot = next
            return
        }
        deferredSnapshot = nil
        guard let base = snapshot?.content, let incoming = next.content, next.state == .text else {
            loadSnapshot(next)
            return
        }
        if next.revision == snapshot?.revision {
            snapshot = next
            external = nil
            onEdit?()
            return
        }
        let local = text, version = editVersion
        let (merged, edits) = await Task.detached(priority: .userInitiated) {
            let merged = TaskTextEdit.merge(base: base, local: local, disk: incoming)
            return (merged, TaskTextEdit.between(local, merged))
        }.value
        guard !Task.isCancelled, generation == incomingGeneration, snapshot?.content == base else { return }
        guard !saving, !editor.hasMarkedText() else {
            deferredSnapshot = next
            return
        }
        if version != editVersion { await reconcile(next); return }
        snapshot = next
        external = nil
        applyIncoming(merged, edits: edits)
        // Native Undo contains offsets into the old disk version. Rebase its
        // boundary to the surviving local draft; Undo must never restore LLM text.
        undo.removeAllActions()
        if merged != incoming { registerLocalUndo(incoming) }
        saveMessage = "Updated from disk"
        onEdit?()
    }

    private func applyIncoming(_ value: String, edits: [TaskTextEdit]? = nil) {
        guard value != text else { return }
        let edits = edits ?? TaskTextEdit.between(text, value)
        var selection = editor.selectedRange()
        func moved(_ position: Int) -> Int {
            var result = position
            for edit in edits {
                if position >= NSMaxRange(edit.range) {
                    result += edit.replacement.utf16.count - edit.range.length
                } else if position > edit.range.location {
                    result += edit.range.location - position
                }
            }
            return max(0, min(value.utf16.count, result))
        }
        let end = moved(NSMaxRange(selection))
        selection.location = moved(selection.location)
        selection.length = max(0, end - selection.location)
        editor.breakUndoCoalescing()
        applyingIncoming = true
        for edit in edits.reversed() {
            editor.textStorage?.replaceCharacters(in: edit.range, with: edit.replacement)
        }
        applyingIncoming = false
        text = value
        editor.setSelectedRange(selection)
    }

    private func registerLocalUndo(_ value: String) {
        undo.registerUndo(withTarget: editor) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.registerLocalUndo(self.text)
                self.applyIncoming(value)
                self.onEdit?()
            }
        }
        undo.setActionName("Local file edits")
    }

    var dirty: Bool { text != snapshot?.content && snapshot?.content != nil }

    init(path: String) {
        self.path = path
        super.init()
        editor.isRichText = false
        editor.allowsUndo = true
        editor.usesFindBar = true
        editor.isAutomaticQuoteSubstitutionEnabled = false
        editor.isAutomaticDashSubstitutionEnabled = false
        editor.isAutomaticTextReplacementEnabled = false
        editor.isAutomaticSpellingCorrectionEnabled = false
        editor.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        editor.textContainerInset = NSSize(width: 12, height: 12)
        editor.isVerticallyResizable = true
        editor.isHorizontallyResizable = false
        editor.autoresizingMask = [.width]
        editor.textContainer?.widthTracksTextView = true
        editor.delegate = self
        editor.compositionEnded = { [weak self] in
            Task { @MainActor [weak self] in
                guard let self else { return }
                if let pending = self.deferredSnapshot { await self.reconcile(pending) }
                self.onEdit?()
            }
        }
        editor.textStorage?.delegate = self
        scroll.hasVerticalScroller = true
        scroll.documentView = editor
    }

    /// The editor's colors, syntax included. Setting the text color clears
    /// every syntax color, so they are applied again after it.
    func applyPalette(_ palette: LoopflowPalette) {
        self.palette = palette
        editor.applyPalette(palette)
        colorSyntax()
    }

    private func colorSyntax() {
        guard let palette, !editor.hasMarkedText() else { return }
        editor.textStorage?.colorSyntax(path: path, palette: palette)
    }

    /// Storage cannot change inside its own editing callback; color after it.
    private func scheduleSyntaxColors() {
        guard palette != nil, !coloringScheduled else { return }
        coloringScheduled = true
        Task { [weak self] in
            await Task.yield()
            self?.coloringScheduled = false
            self?.colorSyntax()
        }
    }

    func restore(_ draft: WorkFileDraft, task: String) {
        guard !dirty else { return }
        snapshot = TaskFileSnapshot(recoveries: [], issueIdentifier: task, taskId: task, path: draft.path,
            content: draft.baseContent, state: .text, revision: draft.baseRevision,
            readOnlyReason: "Recovered draft; checking the current file", sizeBytes: UInt64(draft.baseContent.utf8.count))
        applyIncoming(draft.text)
        editor.isEditable = false
        readOnlyReason = "Recovered draft; checking the current file"
    }

    private func loadSnapshot(_ next: TaskFileSnapshot) {
        guard !dirty, !editor.hasMarkedText() else {
            external = next.revision != snapshot?.revision || next.state != snapshot?.state ? next : nil
            return
        }
        external = nil
        if next.content == snapshot?.content, next.state == snapshot?.state, next.revision == snapshot?.revision {
            snapshot = next
            return
        }
        snapshot = next
        text = next.content ?? ""
        editor.string = text
        editor.undoManager?.removeAllActions()
        editVersion += 1
    }

    private func updateAccess(_ next: TaskFileSnapshot) {
        readOnlyReason = next.readOnlyReason
        editor.isEditable = next.state == .text && next.readOnlyReason == nil
        onEdit?()
    }

    func save(issue: String, query: RegistryQuery) async {
        guard canSave, let revision = snapshot?.revision else { return }
        saving = true
        saveFailed = false
        let submitted = text
        var refreshAfterSave = false
        do {
            let result = try await query.saveTaskFile(issue: issue, path: path, revision: revision,
                                                      content: submitted)
            saveMessage = result.message
            updateAccess(result.file)
            recoveries = result.file.recoveries
            if result.published, result.file.content == submitted {
                // Advance the disk baseline without replacing newer typing, selection or Undo.
                snapshot = result.file
                external = nil
                // A concurrent read may precede or follow publication. Read its
                // current name again instead of applying or dropping uncertain bytes.
                refreshAfterSave = deferredSnapshot != nil
                deferredSnapshot = nil
                if dirty { saveMessage = result.message + " Newer edits remain unsaved." }
            } else {
                // Resolve after the write has settled; never merge against its old baseline.
                deferredSnapshot = result.file
            }
        } catch {
            saveFailed = true
            saveMessage = "Save could not be confirmed: \(error.localizedDescription). Draft retained. Refresh to inspect recovery."
        }
        saving = false
        if refreshAfterSave {
            do { await reconcile(try await query.taskFile(issue: issue, path: path)) }
            catch { saveMessage = "Saved; refresh failed: \(error.localizedDescription)" }
        } else if let pending = deferredSnapshot { await reconcile(pending) }
        onEdit?()
    }

    func undoManager(for view: NSTextView) -> UndoManager? { undo }

    func textStorage(_ storage: NSTextStorage, didProcessEditing mask: NSTextStorageEditActions,
                     range: NSRange, changeInLength: Int) {
        if mask.contains(.editedCharacters) { scheduleSyntaxColors() }
        // Undo can change storage without a textDidChange delegate callback.
        guard mask.contains(.editedCharacters), text != storage.string else { return }
        text = storage.string
        saveMessage = nil
        saveFailed = false
        editVersion += 1
        guard !applyingIncoming else { return }
        // Marked-text status settles after NSTextStorage's callback.
        Task { [weak self] in
            await Task.yield()
            guard let self else { return }
            if let pending = self.deferredSnapshot, !self.editor.hasMarkedText(), !self.saving {
                await self.reconcile(pending)
            }
            self.onEdit?()
        }
    }
}

@MainActor
final class TaskFileTextView: NSTextView {
    var contentsReady: (() -> Void)?
    var compositionEnded: (() -> Void)?

    func recordReadiness() {
        guard window != nil, !isHiddenOrHasHiddenAncestor, !visibleRect.isEmpty,
              isEditable, acceptsFirstResponder else { return }
        contentsReady?()
    }

    override func layout() {
        super.layout()
        recordReadiness()
    }

    override func unmarkText() {
        super.unmarkText()
        compositionEnded?()
    }
}

@MainActor
@Observable
final class TaskFilesStore {
    /// Task details beside the conversation: what was asked, what changed, where.
    enum Page: String, CaseIterable {
        case brief = "Brief"
        case changes = "Diff"
        case files = "Files"

        var symbol: String {
            switch self {
            case .brief: "doc.text"
            case .changes: "plus.forwardslash.minus"
            case .files: "folder"
            }
        }
    }
    var comparison = ChangeComparison.workingTree(base: "parent")
    let pullRequest = PullRequestStore()
    var page = Page.files

    /// The page a pane shows: Brief needs the Task's row, and a pane without
    /// one reads Files instead.
    func shownPage(hasTask: Bool) -> Page { page == .brief && !hasTask ? .files : page }
    enum Navigator { case closed, dropdown, pinned }
    var navigator = Navigator.closed
    var search = ""
    var showsScratch = true

    func toggleNavigator() {
        navigator = navigator == .closed ? .dropdown : .closed
    }

    func togglePinnedNavigator() {
        navigator = navigator == .pinned ? .closed : .pinned
    }

    func dismissNavigator() {
        if navigator == .dropdown { navigator = .closed }
    }

    let performanceOwner = UUID()

    func selectFileFromInput(_ path: String, mode: Mode) {
        Perf.userAction(.openFile, scenario: "file_to_visible_editor", id: path,
                        repository: measurementRepository, liveSessions: nil, owner: performanceOwner) {
            let document = document(path)
            document.editor.contentsReady = { [weak self, weak document] in
                guard let self, let document, self.selection == path,
                      document.snapshot?.state == .text, document.snapshot?.path == path,
                      document.editor.string == document.text else { return }
                Perf.stateObserved(.openFile, id: path, owner: self.performanceOwner)
            }
            selectFile(path, mode: mode)
            Perf.acknowledged(.openFile, id: path, owner: performanceOwner)
            document.editor.recordReadiness()
        }
    }

    func selectFile(_ path: String, mode: Mode) {
        entering = nil
        page = .files
        selection = path
        self.mode = mode
        dismissNavigator()
        noteInteraction()
    }

    // MARK: Following scratch writes

    /// What a scratch page write does to the view.
    enum Follow: Equatable { case reload, select, openTab }

    /// How long Files may go untouched before a write may take the view.
    static let quietInterval: TimeInterval = 120

    /// Pages agents present for review: Markdown and HTML under `scratch/`.
    static func isScratchPage(_ path: String) -> Bool {
        path.hasPrefix("scratch/") && isRenderable(path)
    }

    static func isRenderable(_ path: String) -> Bool {
        ["md", "html"].contains((path as NSString).pathExtension.lowercased())
    }

    /// A write takes the view only from stale attention: nothing selected, a
    /// document that was itself followed, or Files untouched for the quiet
    /// interval. Editing, diffing or a draft keep it.
    static func follow(_ path: String, selection: String?, page: Page, mode: Mode, dirty: Bool,
                       autoSelected: Bool, lastInteraction: Date?, now: Date) -> Follow {
        if selection == path, page == .files { return .reload }
        guard selection != nil else { return .select }
        if mode != .read || dirty { return .openTab }
        guard !autoSelected, let lastInteraction else { return .select }
        return now.timeIntervalSince(lastInteraction) >= quietInterval ? .select : .openTab
    }

    /// The selection came from a write, not from you.
    private(set) var autoSelected = false
    @ObservationIgnored private var lastInteraction: Date?

    /// You selected, scrolled, clicked or switched something in Files.
    func noteInteraction() {
        lastInteraction = .now
        autoSelected = false
    }

    func presentFile(_ path: String) {
        _ = document(path)
        observation?.watch(path)
        follow(path)
    }

    private func follow(_ path: String) {
        let outcome = Self.follow(path, selection: selection, page: page, mode: mode,
            dirty: selectedDocument?.dirty == true, autoSelected: autoSelected,
            lastInteraction: lastInteraction, now: .now)
        switch outcome {
        case .reload: break
        case .select:
            page = .files
            selection = path
            mode = .read
            autoSelected = true
        case .openTab:
            if !tabs.contains(where: { $0.path == path }) {
                tabs.append(WorkFileTab(id: UUID().uuidString, path: path))
            }
        }
    }

    var scratchPaths: [String] {
        (changes?.scratch ?? []).filter(matchesSearch)
    }

    // MARK: Checkout tree

    /// The checkout as you have opened it, kept across page and Task switches.
    var tree = CheckoutTree()
    /// Where the tree was scrolled.
    var treeOffset = CGPoint.zero

    var treeRows: [CheckoutTree.Row] {
        tree.rows(matching: search, also: (changes?.files ?? []).map(\.path))
    }

    var changeBadges: [String: CheckoutChange] {
        CheckoutChange.badges(changes?.files ?? [])
    }

    /// Read the checkout root the first time the tree is shown.
    func openTree() async {
        if tree.needsRead("") { await readDirectory("", more: false) }
    }

    /// One page of `directory` through `lf task files`; a remote Task reads
    /// through its Machine. A failure stays on that directory.
    func readDirectory(_ directory: String, more: Bool) async {
        guard let read = tree.begin(directory, more: more) else { return }
        do {
            let page = try await query.taskFiles(issue: issue, directory: directory, cursor: read.cursor)
            tree.receive(page, for: directory, more: more)
        } catch {
            tree.fail(directory, reason: error.localizedDescription)
        }
    }

    /// Retry a failed read where it stopped: the first page, or the next one.
    func retryDirectory(_ directory: String) async {
        await readDirectory(directory, more: (tree.listings[directory]?.pages ?? 0) > 0)
    }

    func setDirectory(_ directory: String, expanded: Bool) async {
        tree.setExpanded(directory, expanded)
        tree.selection = directory
        noteInteraction()
        if expanded, tree.needsRead(directory) { await readDirectory(directory, more: false) }
    }

    enum TreeMove { case up, down, expand, collapse, open }

    /// The navigator's keys: ↑/↓ move, → opens a directory or enters it, ←
    /// closes it or goes to its parent, Return opens a file or toggles a directory.
    func moveTree(_ move: TreeMove) async {
        let rows = treeRows
        let current = tree.selection.flatMap { path in rows.compactMap(\.entry).first { $0.path == path } }
        switch move {
        case .up, .down:
            tree.selection = tree.neighbor(move == .up ? -1 : 1, in: rows)
            noteInteraction()
        case .expand:
            guard let current, current.kind == .directory else { return }
            if tree.isExpanded(current.path) {
                tree.selection = tree.neighbor(1, in: rows)
            } else {
                await setDirectory(current.path, expanded: true)
            }
        case .collapse:
            guard let current else { return }
            if current.kind == .directory, tree.isExpanded(current.path) {
                await setDirectory(current.path, expanded: false)
            } else {
                let parent = (current.path as NSString).deletingLastPathComponent
                if !parent.isEmpty { tree.selection = parent }
            }
        case .open:
            guard let current else { return }
            if current.kind == .directory {
                await setDirectory(current.path, expanded: !tree.isExpanded(current.path))
            } else {
                selectFile(current.path, mode: .read)
            }
        }
    }

    /// Re-read loaded directories a write may have changed, as many pages as were loaded.
    private func rereadDirectories(holding paths: Set<String>) async {
        for directory in tree.listingsHolding(paths) {
            let pages = tree.listings[directory]?.pages ?? 0
            await readDirectory(directory, more: false)
            while let listing = tree.listings[directory], listing.pages < pages,
                  listing.nextCursor != nil, listing.error == nil, !Task.isCancelled {
                await readDirectory(directory, more: true)
            }
        }
    }

    private func matchesSearch(_ path: String) -> Bool {
        search.isEmpty || path.localizedCaseInsensitiveContains(search)
    }
    enum Mode: String, CaseIterable {
        case read = "Read"
        case edit = "Edit"
        case diff = "Diff"
    }

    /// Read mode shows Markdown and HTML rendered; other text stays in the editor.
    var rendersSelection: Bool {
        mode == .read && selection.map(Self.isRenderable) == true
    }

    var measurementRepository: String?
    let issue: String
    let cwd: String
    let query: RegistryQuery
    private(set) var tabs: [WorkFileTab] = []
    var selection: String? {
        didSet {
            if let path = selection, !tabs.contains(where: { $0.path == path }) {
                tabs.append(WorkFileTab(id: UUID().uuidString, path: path))
            }
        }
    }

    func closeTab(_ id: String) {
        guard let index = tabs.firstIndex(where: { $0.id == id }) else { return }
        let removed = tabs.remove(at: index)
        if selection == removed.path {
            selection = tabs.isEmpty ? nil : tabs[min(index, tabs.count - 1)].path
        }
        // Documents (including unsaved drafts and Undo) remain Task-owned.
    }

    func restoreTabs(_ retained: [WorkFileTab], selected: String?) {
        tabs = retained
        selection = retained.first { $0.id == selected }?.path
    }

    func restoreDrafts(_ drafts: [WorkFileDraft], location: WorkViewPlan.Location) {
        for draft in drafts where draft.location == location {
            document(draft.path).restore(draft, task: issue)
        }
    }

    var browserTab: WorkBrowserTab {
        .files(id: BrowserTab.files.id, tabs: tabs, selected: tabs.first { $0.path == selection }?.id)
    }

    func drafts(location: WorkViewPlan.Location) -> [WorkFileDraft] {
        documents.values.compactMap { document in
            guard document.dirty, let base = document.snapshot?.content,
                  let revision = document.snapshot?.revision else { return nil }
            return WorkFileDraft(location: location, path: document.path,
                                 baseRevision: revision, baseContent: base, text: document.text)
        }.sorted { $0.path < $1.path }
    }
    var mode = Mode.read
    private(set) var changes: TaskChangesSnapshot?
    private(set) var isRefreshingChanges = false
    private(set) var changesError: String?
    private(set) var documents: [String: TaskFileDocument] = [:]
    /// Files' Diff mode: the selected file through the review stream.
    private(set) var fileReview: FileReview?
    private(set) var error: String?
    private var refreshGeneration = 0
    private var fileGeneration = 0
    private var diffGeneration = 0
    private var savedPreference = true
    private var preferenceChanges: AnyCancellable?
    private var observation: TaskFileObservation?
    @ObservationIgnored private var remoteObservation: Task<Void, Never>?

    private var invalidation: Task<Void, Never>?
    private var pendingPaths: Set<String> = []
    private var pendingComparison = false
    private var ignoreFailureLogged = false
    private var autosaves: [String: Task<Void, Never>] = [:]
    var autosave = true {
        didSet { for document in documents.values { scheduleSave(document) } }
    }

    deinit { remoteObservation?.cancel() }

    func observeFiles() {
        if let machine = query.remoteMachine {
            observeRemoteFiles(machine: machine)
            return
        }
        guard observation == nil else { return }
        do {
            observation = try TaskFileObservation(path: cwd) { [weak self] paths in
                self?.invalidate(paths)
            }
        } catch { self.error = "Live file observation unavailable: \(error.localizedDescription)" }
    }

    private func observeRemoteFiles(machine: String) {
        guard remoteObservation == nil else { return }
        let request = UUID().uuidString
        remoteObservation = Task { [weak self, query, issue, cwd] in
            var observation: TaskFilesObservation?
            do {
                let opened = try await query.watchTaskFiles(issue: issue, checkout: cwd, request: request)
                observation = opened
                for try await frame in opened.frames {
                    try Task.checkCancellation()
                    guard frame.request == request, frame.taskID == issue,
                          frame.machineID == machine, frame.checkout == cwd else {
                        throw RegistryQueryError("Stale or mismatched file observation; drafts retained. Reopen the Task.")
                    }
                    guard let self else { break }
                    if !frame.paths.isEmpty { self.error = nil; self.invalidate(frame.paths) }
                }
                if !Task.isCancelled { throw RegistryQueryError("Live file observation disconnected; drafts retained. Refresh to reconnect.") }
            } catch {
                if let self, !Task.isCancelled {
                    self.invalidation?.cancel()
                    self.error = "Live file observation unavailable: \(error.localizedDescription)"
                }
            }
            await observation?.cancel()
            self?.remoteObservation = nil
        }
    }

    private func invalidate(_ paths: [String]) {
        for path in paths {
            if path == ".git" || path.hasPrefix(".git/") {
                // Git and lf write other files here on every read. A commit,
                // checkout or staging moves what the comparison reads.
                let name = (path as NSString).lastPathComponent
                if ["HEAD", "ORIG_HEAD", "index"].contains(name) || path.contains("/refs/") {
                    pendingComparison = true
                }
                continue
            }
            pendingPaths.insert(path)
        }
        guard !pendingPaths.isEmpty || pendingComparison else { return }
        invalidation?.cancel()
        invalidation = Task { [weak self] in
            do { try await Task.sleep(for: .milliseconds(150)) } catch { return }
            guard let self else { return }
            let changed = self.pendingPaths
            var comparisonChanged = self.pendingComparison
            let committed = comparisonChanged
            self.pendingPaths.removeAll()
            self.pendingComparison = false
            defer {
                if Task.isCancelled {
                    self.pendingPaths.formUnion(changed)
                    self.pendingComparison = self.pendingComparison || comparisonChanged
                }
            }
            // Every lf read journals under the checkout and builds write ignored
            // output; those writes never move the comparison. A peer filters its own.
            let ignored = self.query.remoteMachine == nil ? await self.ignoredPaths(changed) : []
            guard !Task.isCancelled else { return }
            let visible = changed.filter { !ignored.contains($0) }
            comparisonChanged = comparisonChanged || !visible.isEmpty
            await self.rereadDirectories(holding: visible)
            guard !Task.isCancelled else { return }
            // A page an agent writes under scratch/ is retained like an open
            // document, so the re-read below brings it to you or into a tab.
            for path in changed where Self.isScratchPage(path) && self.documents[path] == nil {
                _ = self.document(path)
                self.observation?.watch(path)
            }
            // Re-read retained documents even when another file or terminal is visible.
            for (path, document) in self.documents where Self.touches(changed, path) {
                do {
                    let revision = document.snapshot?.revision
                    let next = try await self.query.taskFile(issue: self.issue, path: path)
                    guard !Task.isCancelled else { return }
                    guard revision == document.snapshot?.revision else {
                        self.invalidate([path])
                        return
                    }
                    await document.reconcile(next)
                    if Self.isScratchPage(path), next.state == .text { self.follow(path) }
                } catch { self.error = error.localizedDescription }
            }
            guard !Task.isCancelled, comparisonChanged else { return }
            await self.refreshChanges()
            if !Task.isCancelled, self.mode == .diff { await self.loadDiff() }
            if !Task.isCancelled, self.page == .changes { await self.loadReview(changed: committed ? nil : visible) }
        }
    }

    /// Whether a changed path is `path`, a folder above it, or the whole checkout.
    private static func touches(_ changed: Set<String>, _ path: String) -> Bool {
        changed.contains { $0.isEmpty || $0 == path || path.hasPrefix($0 + "/") }
    }

    /// The paths git ignores in this checkout. When git cannot judge, none are.
    private func ignoredPaths(_ paths: Set<String>) async -> Set<String> {
        let candidates = paths.filter { !$0.isEmpty }
        guard !candidates.isEmpty else { return [] }
        let cwd = self.cwd
        do { return try await Task.detached { try Self.checkIgnore(candidates, in: cwd) }.value } catch {
            if !ignoreFailureLogged {
                ignoreFailureLogged = true
                DesktopLog.home.warn("File observation keeps ignored paths", ["error": error.localizedDescription])
            }
            return []
        }
    }

    private nonisolated static func checkIgnore(_ paths: Set<String>, in checkout: String) throws -> Set<String> {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["git", "-C", checkout, "check-ignore", "--stdin", "-z"]
        let input = Pipe.uninherited(), output = Pipe.uninherited(), errors = Pipe.uninherited()
        process.standardInput = input
        process.standardOutput = output
        process.standardError = errors
        try process.run()
        // Write on another thread so a batch larger than a pipe buffer cannot deadlock.
        let request = Data(paths.joined(separator: "\0").utf8 + [0])
        DispatchQueue.global().async {
            try? input.fileHandleForWriting.write(contentsOf: request)
            try? input.fileHandleForWriting.close()
        }
        let data = output.fileHandleForReading.readDataToEndOfFile()
        let stderr = errors.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        // Status 1 means no path is ignored.
        guard [0, 1].contains(process.terminationStatus) else {
            throw RegistryQueryError(String(decoding: stderr, as: UTF8.self))
        }
        return Set(data.split(separator: 0).map { String(decoding: $0, as: UTF8.self) })
    }

    private func scheduleSave(_ document: TaskFileDocument) {
        autosaves[document.path]?.cancel()
        guard autosave, document.canSave, !document.saveFailed else { return }
        autosaves[document.path] = Task { [weak self, weak document] in
            // One recovery version per settled edit burst, never per keystroke.
            do { try await Task.sleep(for: .seconds(2)) } catch { return }
            guard let self, let document, self.autosave, document.canSave else { return }
            self.autosaves[document.path] = nil
            await document.save(issue: self.issue, query: self.query)
        }
    }

    init(issue: String, cwd: String, query: RegistryQuery = RegistryQueryLocal.shared) {
        self.issue = issue
        self.cwd = cwd
        self.query = query
        autosave = UserDefaults.standard.object(forKey: "taskFilesAutosave") as? Bool ?? true
        savedPreference = autosave
        preferenceChanges = NotificationCenter.default.publisher(for: UserDefaults.didChangeNotification)
            .sink { [weak self] _ in
                Task { @MainActor [weak self] in
                    guard let self else { return }
                    let enabled = UserDefaults.standard.object(forKey: "taskFilesAutosave") as? Bool ?? true
                    if self.savedPreference != enabled {
                        self.savedPreference = enabled
                        self.autosave = enabled
                    }
                }
            }
    }

    var selectedDocument: TaskFileDocument? { selection.flatMap { documents[$0] } }

    func document(_ path: String) -> TaskFileDocument {
        if let document = documents[path] { return document }
        let document = TaskFileDocument(path: path)
        document.onEdit = { [weak self, weak document] in
            guard let self, let document else { return }
            self.scheduleSave(document)
        }
        documents[path] = document
        return document
    }

    /// Refresh navigation only; selection and filesystem events own document reads.
    func refresh() async {
        if query.remoteMachine != nil { observeFiles() }
        await refreshChanges()
    }

    func refreshChanges() async {
        refreshGeneration += 1
        let generation = refreshGeneration
        isRefreshingChanges = true
        defer { if generation == refreshGeneration { isRefreshingChanges = false } }
        do {
            let changes = try await query.taskChanges(issue: issue, base: comparison.base, head: comparison.head)
            guard !Task.isCancelled, generation == refreshGeneration else { return }
            self.changes = changes
            if selection == nil, let first = changes.scratch.first ?? changes.files.first?.path {
                selection = first
                autoSelected = true
            }
            changesError = nil
        } catch {
            guard !Task.isCancelled, generation == refreshGeneration else { return }
            changes = nil
            fileReview = nil
            changesError = error.localizedDescription
        }
    }

    func loadFile() async {
        fileGeneration += 1
        let generation = fileGeneration
        guard let path = selection else { return }
        observation?.watch(path)
        let revision = documents[path]?.snapshot?.revision
        do {
            let next = try await query.taskFile(issue: issue, path: path)
            guard !Task.isCancelled, selection == path, generation == fileGeneration,
                  documents[path]?.snapshot?.revision == revision else { return }
            await document(path).reconcile(next)
            error = nil
        } catch {
            guard !Task.isCancelled, selection == path, generation == fileGeneration else { return }
            self.error = error.localizedDescription
        }
    }

    // MARK: Review

    /// The whole comparison as one stream. A reload replaces it in place; the
    /// reading anchor and current stop are ids, so new evidence never moves them.
    private(set) var review: ReviewDiff? { didSet { arrange() } }
    /// Changed files whose lines are not read yet, or were read before the
    /// comparison last moved. A hidden file stays here until it is shown.
    private(set) var unread: Set<String> = []
    /// Why a file's lines are missing or cut short, by path.
    private(set) var fileNotes: [String: String] = [:]
    /// Files with lines too long to read that you opened anyway.
    var openedLongFiles: Set<String> = []
    @ObservationIgnored private var reading: Set<String> = []
    /// The base and head commits the files were last read against.
    @ObservationIgnored private var readCommits: [String] = []
    @ObservationIgnored private var nearPaths: Set<String> = []
    private var reviewGeneration = 0
    /// Each reader's place, so switching paths or pages and back lands where you were.
    private var places: [ReviewReader: ReviewPlace] = [:]
    /// The file next/previous crossed into, and from which end its first stop is taken.
    @ObservationIgnored private var entering: (path: String, offset: Int)?
    /// When next/previous last moved, so the change it landed on can pulse once.
    @ObservationIgnored private(set) var steppedAt = Date.distantPast
    var reviewLayout = ReviewLayout.unified
    /// One file at a time; next/previous still crosses into the next file.
    var focusedPath: String?
    var expandedMoves: Set<String> = []
    var reviewSelection: ReviewSelection? { didSet { if reviewSelection != oldValue { commentDraft = "" } } }
    /// Where the Brief was scrolled, and the brief it was showing.
    var briefOffset = CGPoint.zero
    var briefShown: Int?

    enum ReviewLayout: String, CaseIterable { case unified = "Unified", split = "Side by side" }

    /// Side by side needs a wide pane; a narrower one reads unified.
    static let splitWidth: CGFloat = 640
    func readsSplit(width: CGFloat) -> Bool { reviewLayout == .split && width >= Self.splitWidth }

    /// Read the comparison file by file. `lf task changes` names every
    /// changed file at once; each shown file's lines are then read on their
    /// own, in the stream's order, so no file's size cuts another short. A
    /// reload keeps the lines it had until their replacement arrives, and
    /// rereads only the `changed` paths while the comparison's commits stand.
    func loadReview(changed: Set<String>? = nil) async {
        if changes == nil { await refreshChanges() }
        reviewGeneration += 1
        let generation = reviewGeneration
        let target = comparison
        guard changes != nil else { return }
        let tour = await readTour()
        let age = await readTourAge(tour)
        guard !Task.isCancelled, generation == reviewGeneration, comparison == target, let changes else { return }
        if tourWriting.isUnderway, tour != diffTour {
            tourWriting = .idle
            orderingChoice = .tour
        }
        diffTour = tour
        tourAge = age
        let known = Dictionary((review?.files ?? []).map { ($0.path, $0) }) { first, _ in first }
        let paths = Set(changes.files.map(\.path))
        let commits = [changes.baseCommit, changes.headCommit]
        if let changed, commits == readCommits {
            unread = paths.filter { unread.contains($0) || known[$0] == nil || Self.touches(changed, $0) }
        } else {
            unread = paths
        }
        readCommits = commits
        reading = []
        fileNotes = fileNotes.filter { paths.contains($0.key) }
        for path in openedGaps.keys where !paths.contains(path) { openedGaps[path] = nil }
        comments = comments.map { paths.contains($0.selection.path) ? $0 : $0.anchored(in: nil) }
        if let focusedPath, !paths.contains(focusedPath) { self.focusedPath = nil }
        review = ReviewDiff(files: changes.files.map {
            known[$0.path] ?? ReviewFile(path: $0.path, oldPath: $0.oldPath, status: .modified, blocks: [])
        })
        await readPatches()
    }

    /// Up to six files at a time, until every shown file has been read.
    private func readPatches() async {
        let generation = reviewGeneration
        guard let changes else { return }
        let files = Dictionary(changes.files.map { ($0.path, $0) }) { first, _ in first }
        let query = self.query, issue = self.issue, base = changes.baseCommit, head = comparison.head
        await withTaskGroup(of: (String, FileReading).self) { group in
            var running = 0
            while true {
                while running < 6, generation == reviewGeneration, !Task.isCancelled,
                      let path = nextToRead(), let file = files[path] {
                    reading.insert(path)
                    running += 1
                    group.addTask {
                        (path, await Self.readFile(file, query: query, issue: issue, base: base, head: head))
                    }
                }
                guard let (path, result) = await group.next() else { return }
                running -= 1
                // A newer reload reads this file again; its answer replaces this one.
                guard generation == reviewGeneration else { continue }
                reading.remove(path)
                accept(result, for: path)
            }
        }
    }

    /// The next shown file without lines: one near the viewport, else the
    /// first in the stream's order. Hidden files are never read.
    private func nextToRead() -> String? {
        let waiting = (shownReview?.files ?? []).map(\.path).filter { unread.contains($0) && !reading.contains($0) }
        return waiting.first(where: nearPaths.contains) ?? waiting.first
    }

    /// A file came into or left the viewport before its lines were read.
    func setNear(_ path: String, _ near: Bool) {
        if near { nearPaths.insert(path) } else { nearPaths.remove(path) }
    }

    /// Whether a file's place in the stream is still waiting for its lines.
    func isReading(_ file: ReviewFile) -> Bool {
        unread.contains(file.path) && file.blocks.isEmpty && fileNotes[file.path] == nil
    }

    /// How many shown files are not read yet.
    var unreadShown: Int { shownReview?.files.count { unread.contains($0.path) } ?? 0 }

    /// A file that becomes shown (unhidden, or no longer filtered out) is read then.
    private func readShown() {
        guard nextToRead() != nil else { return }
        Task { await readPatches() }
    }

    private struct FileReading: Sendable {
        /// nil when the read failed; the stream keeps the lines it had.
        let file: ReviewFile?
        /// Why lines are missing.
        let note: String?
    }

    /// One file's lines. A new file is read as its contents while the
    /// comparison reads the working tree: an untracked one at once, a
    /// committed one when its patch is over the read limit.
    private nonisolated static func readFile(_ changed: TaskChangedFile, query: RegistryQuery, issue: String,
                                             base: String, head: String?) async -> FileReading {
        let path = changed.path
        /// The file as its contents, or why it has none to show.
        func contents() async throws -> FileReading {
            let whole = try await query.taskFile(issue: issue, path: path)
            if whole.state == .text, let content = whole.content {
                return FileReading(file: .added(path, content: content), note: nil)
            }
            let binary = whole.state == .binary
            return FileReading(file: ReviewFile(path: path, oldPath: nil, status: binary ? .binary : .added, blocks: []),
                               note: binary ? nil : whole.state.title)
        }
        do {
            if head == nil, changed.untracked { return try await contents() }
            let next = try await query.taskDiff(issue: issue, path: path, base: base, head: head)
            let parsed = ReviewDiff.parse(next.patch).files
            let file = parsed.first { $0.path == path } ?? parsed.first
                ?? ReviewFile(path: path, oldPath: changed.oldPath, status: next.binary ? .binary : .modified, blocks: [])
            if next.truncated, head == nil, file.status == .added,
               let whole = try? await contents(), whole.note == nil, whole.file?.status == .added {
                return whole
            }
            return FileReading(file: file, note: next.truncated ? "Cut at 1 MB · the rest of this file is not shown" : nil)
        } catch {
            return FileReading(file: nil, note: "Could not be read: \(error.localizedDescription)")
        }
    }

    private func accept(_ result: FileReading, for path: String) {
        unread.remove(path)
        fileNotes[path] = result.note
        guard let review, let index = review.files.firstIndex(where: { $0.path == path }),
              let file = result.file else { return }
        if file != review.files[index] {
            openedGaps[path] = nil
            var files = review.files
            files[index] = file
            self.review = ReviewDiff(files: files)
        }
        comments = comments.map { $0.selection.path == path ? $0.anchored(in: opened(file)) : $0 }
    }

    // MARK: Hidden paths and order

    /// A pattern the Diff leaves out, and whose it is.
    struct HiddenPattern: Equatable, Sendable {
        let pattern: String
        /// Where a pattern you cannot edit here comes from ("Repository");
        /// nil for your own, saved in the repository's `.lf/config.local.yaml`.
        let source: String?
    }

    /// The `diff_hidden` setting as `lf` resolved it, every layer added. Until
    /// the Diff has read it and when that read fails, this copy of its catalog
    /// default (`diff_hidden` in `rust/loopflow-execution/src/config/catalog.rs`).
    var hidden = [HiddenPattern(pattern: "scratch/", source: "Default")] { didSet { arrange(); readShown() } }
    /// The `diff_groups` setting as `lf` resolved it.
    var diffGroups = DiffGroupSetting.unread { didSet { arrange() } }

    /// Your own hidden patterns, the only ones the Diff edits.
    var yourHidden: [String] { hidden.filter { $0.source == nil }.map(\.pattern) }

    func hide(_ pattern: String) {
        let pattern = pattern.trimmingCharacters(in: .whitespaces)
        guard !pattern.isEmpty, !yourHidden.contains(pattern) else { return }
        hidden.append(HiddenPattern(pattern: pattern, source: nil))
    }

    func unhide(_ pattern: String) {
        hidden.removeAll { $0.pattern == pattern && $0.source == nil }
    }

    /// The final value and, for each pattern, the last layer that added it.
    static func hidden(from setting: ConfigurationValue) -> [HiddenPattern]? {
        guard case .array(let values) = setting.value else { return nil }
        let patterns = values.compactMap { if case .string(let pattern) = $0 { pattern } else { nil } }
        var seen = Set<String>()
        return patterns.filter { seen.insert($0).inserted }.map { pattern in
            let candidate = setting.candidates.last { candidate in
                guard case .array(let values) = candidate.value else { return false }
                return values.contains(.string(pattern))
            }
            let source: String? = switch (candidate?.source, candidate?.scope) {
            case ("personal", "repository"): nil
            case ("builtin", _): "Default"
            case ("repository", _): "Repository"
            case ("personal", _): "Personal"
            default: "Override"
            }
            return HiddenPattern(pattern: pattern, source: source)
        }
    }
    /// Read the hidden files too, without forgetting what is hidden.
    var showsHidden = false { didSet { arrange(); readShown() } }
    /// Narrows the stream to paths containing this text.
    var reviewFilter = "" { didSet { arrange(); readShown() } }
    /// The checkout's `scratch/tour.yaml`, when it has one.
    private(set) var diffTour: DiffTour? { didSet { if diffTour != oldValue { arrange() } } }
    /// The order you chose; until then the tour when there is one.
    var orderingChoice: ReviewOrdering? { didSet { arrange() } }
    /// Paths the tour hid that you asked to see.
    var shownTourHides: Set<String> = [] { didSet { arrange(); readShown() } }
    /// The changed files beside the stream: your choice, else open when there is room.
    var showsSidebar: Bool?

    var hasTour: Bool { diffTour?.first.isEmpty == false }

    var ordering: ReviewOrdering {
        let choice = orderingChoice ?? .tour
        return choice == .tour && !hasTour ? .groups : choice
    }

    func hides(_ path: String) -> Bool {
        hidden.contains { DiffPattern.matches($0.pattern, path) }
    }

    /// The tour's reason to leave a path out, until you ask to see it.
    func tourHides(_ path: String) -> DiffTour.Entry? {
        diffTour?.hide.first { $0.matches(path) && !shownTourHides.contains($0.path) }
    }

    /// The comparison as the Diff reads it: your hidden paths and the tour's
    /// left out, then the filter, in the chosen order.
    private(set) var shownGroups: [ReviewGroup] = []
    private(set) var shownReview: ReviewDiff?

    private func arrange() {
        guard let review else {
            shownGroups = []
            shownReview = nil
            return
        }
        let filter = reviewFilter.trimmingCharacters(in: .whitespaces)
        let files = review.files.filter { file in
            (showsHidden || !hides(file.path)) && tourHides(file.path) == nil
                && (filter.isEmpty || file.path.range(of: filter, options: .caseInsensitive) != nil)
        }
        shownGroups = ReviewGroup.arrange(files, ordering: ordering, tour: diffTour, groups: diffGroups)
        shownReview = ReviewDiff(files: shownGroups.flatMap(\.files))
    }

    private func readTour() async -> DiffTour? {
        guard let file = try? await query.taskFile(issue: issue, path: DiffTour.path),
              file.state == .text, let content = file.content else { return nil }
        return DiffTour.parse(content)
    }

    // MARK: Tour

    /// How far the change has moved since its tour was written.
    struct TourAge: Equatable {
        /// The commit the tour names; nil when it names none.
        let at: String?
        /// Files outside `scratch/` that commits since `at` changed; nil when
        /// that could not be read.
        let changedFiles: Int?
    }

    /// Set when the tour is behind the head; nil while it is current or there is none.
    private(set) var tourAge: TourAge?

    /// Nothing reports a tour run ending, so the wait ends when the file
    /// changes; a run that dies leaves `launched` until the next click.
    enum TourWriting: Equatable {
        case idle
        /// The click, until `lf` answers that the `tour` skill started.
        case launching
        /// The skill started headless; `log` is where its output goes.
        case launched(log: String)
        case failed(String)

        var isUnderway: Bool {
            switch self {
            case .launching, .launched: true
            case .idle, .failed: false
            }
        }
    }
    private(set) var tourWriting = TourWriting.idle

    /// A tour is behind when it names no commit, or when a commit after the one
    /// it names changed a file outside `scratch/`. Committing the tour itself
    /// leaves it current.
    private func readTourAge(_ tour: DiffTour?) async -> TourAge? {
        guard let tour, let head = changes?.headCommit else { return nil }
        guard let at = tour.at else { return TourAge(at: nil, changedFiles: nil) }
        if head.hasPrefix(at) || at.hasPrefix(head) { return nil }
        guard let since = try? await query.taskChanges(issue: issue, base: at, head: head) else {
            return TourAge(at: at, changedFiles: nil)
        }
        let changed = since.files.filter { !$0.path.hasPrefix("scratch/") }.count
        return changed == 0 ? nil : TourAge(at: at, changedFiles: changed)
    }

    /// What the Diff says about the tour; nil while it is current or there is none.
    var tourStatus: String? {
        switch tourWriting {
        case .launching: return "Writing the tour…"
        case .launched(let log): return "Writing the tour… · its output: \(log)"
        case .failed(let reason): return reason
        case .idle: break
        }
        guard let age = tourAge else { return nil }
        guard let at = age.at else { return "The tour names no commit, so it may be behind the change" }
        guard let changed = age.changedFiles else { return "The tour was written at \(at.prefix(7)), before the latest commits" }
        return "The tour was written at \(at.prefix(7)) · \(changed == 1 ? "1 file" : "\(changed) files") changed since"
    }

    /// Write tour when there is none, Update tour when it is behind; nil
    /// while it is current, its launch has not answered, or the checkout is on
    /// another Machine. The `tour` skill only runs in a checkout on this
    /// Machine. Offered again once the launch answers: a click during a
    /// healthy run starts a second one that rewrites the same file.
    var tourAction: String? {
        guard query.remoteMachine == nil, tourWriting != .launching else { return nil }
        if diffTour == nil { return "Write tour" }
        return tourAge != nil ? "Update tour" : nil
    }

    /// Run the `tour` skill headless in the Task's checkout. The order
    /// switches to Tour when the file it writes lands.
    func writeTour() async {
        guard tourAction != nil else { return }
        noteInteraction()
        tourWriting = .launching
        do {
            tourWriting = .launched(log: try await query.runSkill("tour", checkout: cwd))
        } catch {
            tourWriting = .failed("The tour could not start: \(error.localizedDescription)")
        }
    }

    // MARK: Skipped lines

    /// Unchanged lines you opened between hunks, by file and hunk header.
    private(set) var openedGaps: [String: [String: [ReviewLine]]] = [:]
    private(set) var gapError: String?

    /// The skipped lines are read from the file as it stands, which is the
    /// comparison's newer side only while it reads the working tree.
    var canOpenGaps: Bool { comparison.head == nil }

    /// Lines too long to read (minified, base64, embedded data) start as one
    /// row with the file's size. Size alone never collapses a file.
    func collapses(_ file: ReviewFile) -> Bool {
        file.longestLine > ReviewFile.readableLine && !openedLongFiles.contains(file.path)
    }

    /// The row that stands for a stop in a reader: the stop itself, or the
    /// summary row of a collapsed file.
    func row(for stop: ReviewStop, in reader: ReviewReader) -> String {
        guard let file = diff(reader)?.file(stop.path), collapses(file) else { return stop.id }
        return file.topID
    }

    func opened(_ file: ReviewFile) -> ReviewFile {
        openedGaps[file.path].map(file.opening) ?? file
    }

    /// Show the lines a hunk header skips. Files' Diff of an unsaved draft
    /// reads the draft; everything else reads the checkout.
    func openGap(_ header: String, gap: ReviewGap, path: String, reader: ReviewReader) async {
        noteInteraction()
        do {
            let content: String
            if case .file = reader, fileReview?.draft == true, let draft = documents[path]?.text {
                content = draft
            } else {
                let file = try await query.taskFile(issue: issue, path: path)
                guard file.state == .text, let text = file.content else {
                    throw RegistryQueryError("\(file.state.title) cannot be shown as text.")
                }
                content = text
            }
            guard let lines = gap.lines(in: content) else {
                throw RegistryQueryError("the file changed since this comparison was read.")
            }
            openedGaps[path, default: [:]][header] = lines
            gapError = nil
        } catch {
            gapError = "Skipped lines of \(path) unavailable: \(error.localizedDescription)"
        }
    }

    // MARK: Comments

    /// Comments waiting to be sent, kept for this window like drafts.
    var comments: [ReviewComment] = []
    /// The comment being written on the selected lines.
    var commentDraft = ""

    /// Leave the draft on the selected lines of `file`.
    func addComment(in file: ReviewFile) {
        guard let selection = reviewSelection, !commentDraft.allSatisfy(\.isWhitespace) else { return }
        comments.append(ReviewComment(selection: selection, text: commentDraft, in: opened(file)))
        reviewSelection = nil
        noteInteraction()
    }

    /// One of the two diff readers: Changes' whole comparison, or Files' Diff of one path.
    enum ReviewReader: Hashable { case changes, file(String) }

    /// Where you are in a reader: the row at its top and the change next/previous moves from.
    struct ReviewPlace: Equatable {
        var anchor: String?
        var current: String?
    }

    subscript(place reader: ReviewReader) -> ReviewPlace {
        get { places[reader] ?? ReviewPlace() }
        set { places[reader] = newValue }
    }

    /// The diff a reader shows, once it has been read.
    func diff(_ reader: ReviewReader) -> ReviewDiff? {
        switch reader {
        case .changes: shownReview
        case .file(let path): fileReview?.path == path ? fileReview?.diff : nil
        }
    }

    /// "2 of 5" once a change is current; the count before.
    func position(_ reader: ReviewReader) -> String {
        guard let diff = diff(reader) else { return "" }
        let stops = diff.stops
        guard !stops.isEmpty else { return "No changes" }
        guard let current = self[place: reader].current, let index = stops.firstIndex(where: { $0.id == current }) else {
            return stops.count == 1 ? "1 change" : "\(stops.count) changes"
        }
        return "\(index + 1) of \(stops.count)"
    }

    /// Move to the next or previous change in a reader. Changes crosses files
    /// in one stream, and a focused file follows the stop. Files' Diff past a
    /// file's last (first) change opens the next (previous) changed file in
    /// Diff mode, landing on its first (last) change once read; that returns nil.
    @discardableResult
    func step(_ offset: Int, in reader: ReviewReader) -> ReviewStop? {
        noteInteraction()
        steppedAt = .now
        if let diff = diff(reader) {
            var current = self[place: reader].current
            while let stop = diff.stop(after: current, offset: offset) {
                current = stop.id
                // A collapsed file is one stop: its first change, shown as its summary row.
                if let file = diff.file(stop.path), collapses(file), file.stops.first != stop.id { continue }
                self[place: reader].current = stop.id
                if reader == .changes, focusedPath != nil { focusedPath = stop.path }
                return stop
            }
        }
        guard case .file(let path) = reader, let next = changedFile(from: path, offset: offset) else { return nil }
        selectFile(next, mode: .diff)
        entering = (next, offset)
        return nil
    }

    /// The changed file `offset` away from `path` in `lf task changes` order.
    private func changedFile(from path: String, offset: Int) -> String? {
        let paths = changes?.files.map(\.path) ?? []
        guard let index = paths.firstIndex(of: path) else {
            return offset > 0 ? paths.first { $0 > path } : paths.last { $0 < path }
        }
        let next = index + (offset > 0 ? 1 : -1)
        return paths.indices.contains(next) ? paths[next] : nil
    }

    /// What the stream compares, named as the PR would: its base branch and
    /// commit, then the head. Footer and conversation references share it.
    var comparisonDescription: String? {
        guard let changes else { return nil }
        let branch = pullRequest.pullRequest?.baseRefName ?? "the Task's base"
        let compared = "Compared with \(branch) at \(changes.baseCommit.prefix(10))"
        return comparison.head == nil ? "\(compared) · includes uncommitted changes"
            : "\(compared) · \(comparison.title) at \(changes.headCommit.prefix(10))"
    }

    /// The comparison's name for a line sent to the conversation, before it is read too.
    var revision: String { comparisonDescription ?? comparison.title }

    // MARK: One file

    struct FileReview: Equatable {
        let path: String
        let baseCommit: String
        let diff: ReviewDiff
        let truncated: Bool
        /// Compared with the unsaved draft rather than the file on disk.
        let draft: Bool
    }

    /// The selected file's diff. A dirty draft is compared as typed, debounced.
    func loadDiff() async {
        if changes == nil { await refreshChanges() }
        diffGeneration += 1
        let generation = diffGeneration
        // Keep the displayed diff for the same file while its replacement loads.
        if fileReview?.path != selection || fileReview?.baseCommit != changes?.baseCommit { fileReview = nil }
        guard let path = selection, let base = changes?.baseCommit else { return }
        observation?.watch(path)
        let document = selectedDocument
        let version = document?.editVersion
        let target = comparison
        let draft = target.head == nil && document?.dirty == true ? document?.text : nil
        do {
            // Debounce edits; identity checks also protect transports that finish after cancellation.
            if document?.dirty == true { try await Task.sleep(for: .milliseconds(180)) }
            let next = try await query.taskDiff(issue: issue, path: path, base: base, head: target.head, draft: draft)
            let parsed = await Task.detached(priority: .userInitiated) { ReviewDiff.parse(next.patch) }.value
            guard !Task.isCancelled, generation == diffGeneration, selection == path, changes?.baseCommit == base, comparison == target,
                  selectedDocument?.editVersion == version else { return }
            if let shown = fileReview, shown.diff != parsed { openedGaps[path] = nil }
            comments = comments.map { $0.selection.path == path ? $0.anchored(in: parsed.files.first.map(opened)) : $0 }
            fileReview = FileReview(path: path, baseCommit: base, diff: parsed, truncated: next.truncated, draft: draft != nil)
            if let entry = entering, entry.path == path {
                entering = nil
                let stop = parsed.stop(after: nil, offset: entry.offset)
                places[.file(path)] = ReviewPlace(anchor: stop?.id, current: stop?.id)
            }
            changesError = nil
        } catch {
            guard !Task.isCancelled, generation == diffGeneration, selection == path, changes?.baseCommit == base, comparison == target else { return }
            changesError = error.localizedDescription
        }
    }
}
#endif
