#if os(macOS)
import AppKit
import Combine
import Loopflow
import Observation

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
    /// Where the Brief was scrolled, and the brief it was showing.
    var briefOffset = CGPoint.zero
    var briefShown: Int?

    let pullRequest = PullRequestStore()
    var page = Page.files { didSet { if page != oldValue { jumps.dismiss() } } }

    /// The page a pane shows: Brief needs the Task's row, and a pane without
    /// one reads Files instead.
    func shownPage(hasTask: Bool) -> Page { page == .brief && !hasTask ? .files : page }
    let browser = FilesBrowserState()
    var find: FileFind?
    var filePicker: TaskFilePicker?

    func chooseFile(_ mode: TaskFilePicker.Mode = .files) {
        guard filePicker?.mode != mode else { return }
        jumps.dismiss()
        filePicker = TaskFilePicker(store: self, mode: mode)
        if mode == .grep, let find { filePicker?.text = find.query }
    }
    let performanceOwner = UUID()

    var jumps: FileJumps { browser.jumps }
    var visit: FileVisit? { browser.visit }

    func reveal(_ location: FilePosition, hit: TaskGrepHit? = nil) {
        browser.open(location.path, automatically: true)
        page = .files
        mode = .read
        jumps.target = nil
        jumps.readingLine = location.line
        browser.visit = FileVisit(line: location.line, hit: hit, visitID: UUID())
    }

    func openMatch(_ hit: TaskGrepHit) {
        let destination = FilePosition(path: hit.path, line: hit.line)
        jumps.record(destination, from: jumps.position(in: self))
        reveal(destination, hit: hit)
    }

    func selectFileFromInput(_ path: String) {
        browser.visit = nil
        jumps.dismiss()
        if selection == path, page == .files {
            selectFile(path)
            return
        }
        Perf.userAction(.openFile, scenario: "file_to_visible_reader", id: path,
                        repository: measurementRepository, liveSessions: nil, owner: performanceOwner) {
            selectFile(path)
            Perf.acknowledged(.openFile, id: path, owner: performanceOwner)
        }
    }

    func selectFile(_ path: String, mode: Mode? = nil) {
        page = .files
        browser.open(path, automatically: false)
        if let mode { self.mode = mode }
    }

    static func isScratchPage(_ path: String) -> Bool {
        path.hasPrefix("scratch/") && ["md", "html"].contains((path as NSString).pathExtension.lowercased())
    }

    var openAndScratch: [String] {
        var seen = Set<String>()
        let drafts = documents.values.filter(\.dirty).map(\.path).sorted()
        return (tabs.map(\.path) + drafts + browser.presentations.reversed().map(\.path)
            + browser.updated.reversed() + scratchPaths).filter { seen.insert($0).inserted }
    }

    var holdsPresentation: Bool { page == .changes || (page == .files && mode != .read) }

    func openPresentation(_ request: FilesBrowserState.Presentation) {
        filePicker = nil
        let position = FilePosition(path: request.path, line: request.lines?.lowerBound ?? 1)
        jumps.record(position, from: jumps.position(in: self))
        reveal(position)
        // Without a line address, Markdown and HTML open rendered.
        browser.visit = request.lines.map { FileVisit(line: position.line, hit: nil, visitID: UUID(), lines: $0) }
        mode = request.mode
    }

    private(set) var scratchPaths: [String] = []
    private var latestScratchPage: String?
    private var scratchGeneration = 0
    private(set) var scratchError: String?

    func refreshScratch() async {
        scratchGeneration += 1
        let generation = scratchGeneration
        do {
            let inventory = try await query.taskFind(issue: issue, limit: 1)
            guard !Task.isCancelled, scratchGeneration == generation else { return }
            scratchPaths = inventory.scratch
            latestScratchPage = inventory.latestScratchPage
            scratchError = nil
        } catch {
            guard !Task.isCancelled, scratchGeneration == generation else { return }
            scratchError = error.localizedDescription
        }
    }

    func enterFiles() async {
        async let scratch: Void = refreshScratch()
        await workingTree.refresh()
        await scratch
        guard !Task.isCancelled, page == .files, !browser.hasChosenInitialFile else { return }
        browser.hasChosenInitialFile = true
        if tabs.isEmpty, selection == nil {
            selection = documents.values.filter(\.dirty).map(\.path).sorted().first
                ?? latestScratchPage ?? workingChanges?.files.first?.path
            if selection == nil { chooseFile() }
        }
    }

    // MARK: Checkout tree

    let tree: CheckoutTree

    var changeBadges: [String: CheckoutChange] { CheckoutChange.badges(workingChanges?.files ?? []) }

    func moveTree(_ move: CheckoutTree.Move) async {
        if let file = await tree.move(move) { selectFileFromInput(file) }
    }

    enum Mode: String, CaseIterable {
        case read = "Read"
        case edit = "Edit"
        case diff = "Diff"
    }

    var measurementRepository: String?
    let issue: String
    let cwd: String
    let query: RegistryQuery
    var tabs: [WorkFileTab] { browser.tabs }
    var selection: String? {
        get { browser.selection }
        set { browser.selection = newValue }
    }
    func closeTab(_ id: String) {
        browser.close(id)
        watchOpenFiles()
    }

    private func watchOpenFiles() {
        guard let observation else { return }
        var paths = Set(documents.keys)
        if let selection { paths.insert(selection) }
        observation.watch(paths)
    }

    func restoreTabs(_ retained: [WorkFileTab], selected: String?) {
        browser.tabs = retained
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
    var mode: Mode {
        get { browser.mode }
        set { browser.mode = newValue }
    }

    let workingTree: ComparisonReading
    let diffReview: DiffReviewState
    var workingChanges: TaskChangesSnapshot? { workingTree.changes }
    let comments = ReviewComments()

    var documents: [String: TaskFileDocument] { browser.documents }
    private(set) var error: String?
    private var fileGeneration = 0
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
                    if !frame.paths.isEmpty || !frame.ignoredScratch.isEmpty {
                        self.error = nil
                        self.invalidate(frame.paths, ignoredScratch: frame.ignoredScratch)
                    }
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

    private var pendingIgnoredScratch: Set<String> = []

    func invalidate(_ paths: [String], ignoredScratch: [String] = []) {
        pendingIgnoredScratch.formUnion(ignoredScratch)
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
        guard hasPendingWrites, invalidation == nil else { return }
        invalidation = Task { [weak self] in
            defer { self?.invalidation = nil }
            while self?.hasPendingWrites == true {
                do { try await Task.sleep(for: .milliseconds(150)) } catch { return }
                await self?.applyWrites()
                if Task.isCancelled { return }
            }
        }
    }

    private var hasPendingWrites: Bool { !pendingIgnoredScratch.isEmpty || !pendingPaths.isEmpty || pendingComparison }

    private func applyWrites() async {
        let peerIgnored = pendingIgnoredScratch
        pendingIgnoredScratch.removeAll()
        let changed = pendingPaths.union(peerIgnored)
        let committed = pendingComparison
        pendingPaths.removeAll()
        pendingComparison = false
        // Every lf read journals under the checkout and builds write ignored
        // output; those writes never move the comparison. A peer filters its own.
        let ignored = query.remoteMachine == nil ? await ignoredPaths(changed) : peerIgnored
        guard !Task.isCancelled else { return }
        let visible = changed.filter { !ignored.contains($0) }
        let comparisonChanged = committed || !visible.isEmpty
        await tree.rereadDirectories(holding: tree.showIgnored ? changed : visible)
        guard !Task.isCancelled else { return }
        let pages = changed.filter(Self.isScratchPage)
        if changed.contains("") || visible.subtracting(pages).contains(where: { $0 == "scratch" || $0.hasPrefix("scratch/") }) {
            await refreshScratch()
        }
        if !pages.isEmpty { await reconcileScratchWrites(pages) }
        guard !Task.isCancelled else { return }
        // Re-read retained documents even when another file or terminal is visible.
        for (path, document) in documents where Self.touches(changed, path) {
            do {
                let revision = document.snapshot?.revision
                let next = try await query.taskFile(issue: issue, path: path)
                guard !Task.isCancelled else { return }
                guard revision == document.snapshot?.revision else {
                    invalidate([path])
                    continue
                }
                await document.reconcile(next)
            } catch is CancellationError {
                return
            } catch { self.error = error.localizedDescription }
        }
        guard !Task.isCancelled, comparisonChanged else { return }
        await workingTree.refresh(afterWrite: true)
        if page == .changes, diffReview.comparison != workingTree.comparison { await diffReview.reviewReading.refresh() }
        if !Task.isCancelled, mode == .diff { await loadDiff() }
        if !Task.isCancelled, page == .changes { await diffReview.loadReview(changed: committed ? nil : visible) }
    }

    /// A write can precede the checkout inventory's next observation. Read the
    /// affected directory on its Machine to distinguish the page from a vanished rename sibling.
    private func reconcileScratchWrites(_ paths: Set<String>) async {
        let directories = Dictionary(grouping: paths) { ($0 as NSString).deletingLastPathComponent }
        for (directory, writes) in directories.sorted(by: { $0.key < $1.key }) {
            let wanted = Set(writes)
            var present = Set<String>()
            var cursor: String?
            do {
                repeat {
                    let page = try await query.taskFiles(issue: issue, directory: directory, cursor: cursor, showIgnored: true)
                    guard !Task.isCancelled else { return }
                    for entry in page.entries where entry.kind == .file && wanted.contains(entry.path) {
                        present.insert(entry.path)
                    }
                    cursor = page.nextCursor
                } while cursor != nil && present != wanted
                scratchPaths = Set(scratchPaths).subtracting(wanted).union(present).sorted()
                browser.noteWrites(wanted, present: present)
            } catch {
                guard !Task.isCancelled else { return }
                scratchError = error.localizedDescription
            }
        }
    }

    /// Whether a changed path is `path`, a folder above it, or the whole checkout.
    static func touches(_ changed: Set<String>, _ path: String) -> Bool {
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
        workingTree = ComparisonReading(issue: issue, query: query, comparison: .workingTree(base: "parent"))
        diffReview = DiffReviewState(cwd: cwd, workingTree: workingTree, browser: browser, pullRequest: pullRequest, comments: comments)
        tree = CheckoutTree(issue: issue, query: query)
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
        var anchoredVersion: Int?
        document.onEdit = { [weak self, weak document] in
            guard let self, let document else { return }
            if anchoredVersion != document.editVersion {
                anchoredVersion = document.editVersion
                self.comments.reconcile(document)
            }
            self.scheduleSave(document)
        }
        browser.documents[path] = document
        return document
    }

    /// Refresh navigation only; selection and filesystem events own document reads.
    func refresh() async {
        if query.remoteMachine != nil { observeFiles() }
        await workingTree.refresh()
        if page == .changes, diffReview.comparison != workingTree.comparison { await diffReview.reviewReading.refresh() }
    }

    private var appliedDiffVisit: UUID?

    func loadDiff() async {
        watchOpenFiles()
        await diffReview.loadDiff()
        if let path = selection, let id = visit?.visitID, id != appliedDiffVisit,
           let lines = visit?.lines, let file = diffReview.fileReview?.diff.file(path) {
            diffReview.sourceFiles.insert(path)
            for (index, block) in file.blocks.enumerated() {
                if case .hunk(let header) = block, let gap = file.gaps[index], gap.new.contains(lines.lowerBound) {
                    await diffReview.openGap(header, gap: gap, file: file, reader: .file(path))
                }
            }
            guard selection == path, visit?.visitID == id else { return }
            let shown = diffReview.opened(file, in: .file(path))
            if let row = shown.row(containing: lines.lowerBound) {
                appliedDiffVisit = id
                diffReview[place: .file(path)] = .init(anchor: row, current: nil)
            }
        }
    }

    func loadFile() async {
        fileGeneration += 1
        let generation = fileGeneration
        guard let path = selection else { return }
        watchOpenFiles()
        let revision = documents[path]?.snapshot?.revision
        do {
            let next = try await query.taskFile(issue: issue, path: path)
            guard !Task.isCancelled, selection == path, generation == fileGeneration,
                  documents[path]?.snapshot?.revision == revision else { return }
            await document(path).reconcile(next)
            error = nil
        } catch {
            // A superseded read is not a failure of the file.
            guard !(error is CancellationError), !Task.isCancelled,
                  selection == path, generation == fileGeneration else { return }
            self.error = error.localizedDescription
        }
    }

}
#endif
