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

    static func isRenderablePage(_ path: String) -> Bool {
        ["md", "html"].contains((path as NSString).pathExtension.lowercased())
    }

    private static func showsRendered(_ path: String, mode: Mode, visit: FileVisit?) -> Bool {
        mode == .read && isRenderablePage(path) && visit?.showsSource != true
    }

    func isShowingRendered(_ path: String) -> Bool {
        Self.showsRendered(path, mode: mode, visit: visit)
    }

    /// History traversal has no origin to record. A path without a source visit opens rendered.
    func reveal(_ path: String, at visit: FileVisit?, mode: Mode = .read, from origin: FilePosition? = nil) {
        let line = visit?.line ?? 1
        let destinationLine: Int? = Self.showsRendered(path, mode: mode, visit: visit) ? nil : line
        jumps.record(FilePosition(path: path, line: destinationLine), from: origin)
        jumps.dismiss()
        browser.open(path, automatically: true)
        page = .files
        self.mode = mode
        jumps.target = nil
        jumps.readingLine = line
        browser.visit = visit
    }

    func openMatch(_ hit: TaskGrepHit) {
        reveal(hit.path, at: FileVisit(line: hit.line, showsSource: true, highlight: .hit(hit)), from: jumps.position(in: self))
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
        path.hasPrefix("scratch/") && isRenderablePage(path)
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
        let visit = request.lines.map { FileVisit(line: $0.lowerBound, showsSource: true, highlight: .lines($0)) }
        reveal(request.path, at: visit, mode: request.mode, from: jumps.position(in: self))
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

    var changeBadges: [String: CheckoutChange] { workingTree.changeBadges }

    enum Mode: String, CaseIterable {
        case read = "Read"
        case edit = "Edit"
        case diff = "Diff"
    }

    var measurementRepository: String?
    let issue: String
    /// The checkout on the Machine that runs the Task, as last read.
    private(set) var cwd: String { didSet { diffReview.cwd = cwd } }
    /// That Machine when it is not this one; nil while it is this Mac.
    private(set) var machine: String? { didSet { diffReview.machine = machine } }
    /// What the Files header calls that Machine: its label when the plan names
    /// one, else its ID.
    var machineName: String? { machine.map { machineLabel ?? $0 } }
    @ObservationIgnored private var machineLabel: String?
    /// This Mac, which watches its own checkouts directly.
    @ObservationIgnored private let localMachineID: String?
    /// Documents to read where the Task has moved, before their next ordinary read.
    @ObservationIgnored private var arriving: Set<String> = []
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

    /// A draft belongs to the Task, its path and its base revision; saving it
    /// wherever the Task runs now succeeds only while that file is at its base.
    func restoreDrafts(_ drafts: [WorkFileDraft]) {
        for draft in drafts {
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
    /// The Task has no checkout on its Machine yet: Files and Changes say
    /// **Not started** until a read finds one.
    var notStarted: Bool { watchNotStarted || workingTree.notStarted || tree.notStarted }
    private var watchNotStarted = false
    private var fileGeneration = 0
    private var savedPreference = true
    private var preferenceChanges: AnyCancellable?
    private var observation: TaskFileObservation?
    @ObservationIgnored private var servedObservation: Task<Void, Never>?
    /// What the served watch last answered, so a repeat after a reconnect reads nothing.
    @ObservationIgnored private var servedRevision: String?
    /// Last seen metadata stamps for `scratch/` pages on a peer Machine, retained across reconnects.
    @ObservationIgnored private var seenScratchStamps: [String: String]?

    private var invalidation: Task<Void, Never>?
    private var pendingPaths: Set<String> = []
    private var pendingComparison = false
    private var ignoreFailureLogged = false
    private var autosaves: [String: Task<Void, Never>] = [:]
    var autosave = true {
        didSet { for document in documents.values { scheduleSave(document) } }
    }

    deinit { servedObservation?.cancel() }

    /// This Mac's checkout through FSEvents; any other Machine's through the
    /// server's `task.files.watch`, which follows the Task when it moves.
    func observeFiles() {
        if machine != nil {
            observeServedFiles()
            return
        }
        guard observation == nil else { return }
        do {
            observation = try TaskFileObservation(path: cwd) { [weak self] paths in
                self?.invalidate(paths)
            }
        } catch { self.error = "Live file observation unavailable: \(error.localizedDescription)" }
    }

    /// The Task's reading names another Machine or checkout: watch that one and
    /// re-read everything open there. Documents and their drafts stay.
    func relocate(machine: String?, label: String? = nil, cwd: String) {
        if machine == self.machine, let label { machineLabel = label }
        guard machine != self.machine || cwd != self.cwd else { return }
        machineLabel = label
        let observing = observation != nil || servedObservation != nil
        observation = nil
        servedObservation?.cancel()
        servedObservation = nil
        servedRevision = nil
        seenScratchStamps = nil
        self.machine = machine
        self.cwd = cwd
        arriving = Set(documents.keys)
        if observing { observeFiles() }
        invalidate([""])
    }

    /// The served watch follows the Task: a reading with no checkout is **Not
    /// started**, and the same watch goes live once one exists. A lost local
    /// server is subscribed to again with a growing pause.
    private func observeServedFiles() {
        guard servedObservation == nil else { return }
        servedObservation = Task { [weak self, query, issue] in
            var delay = Duration.seconds(1)
            // A watch that ends lets the next `observeFiles` start another;
            // a cancelled one was already replaced.
            defer { if !Task.isCancelled { self?.servedObservation = nil } }
            while !Task.isCancelled {
                do {
                    let subscription = try query.watchTaskFiles(issue: issue)
                    defer { Task { await subscription.cancel() } }
                    for try await line in subscription.lines {
                        try Task.checkCancellation()
                        let frame = try JSONDecoder().decode(SubscriptionFrame<TaskFilesRevision>.self, from: line)
                        guard let self else { return }
                        delay = .seconds(1)
                        guard let revision = frame.body else {
                            self.error = frame.unavailable ?? "Live file observation unavailable"
                            continue
                        }
                        self.error = nil
                        // Back on this Mac, its own checkout is watched directly.
                        if revision.machineID == self.localMachineID, let checkout = revision.checkout {
                            self.relocate(machine: nil, cwd: checkout)
                            return
                        }
                        self.receive(revision)
                    }
                    // The local server went away mid-stream; subscribe again.
                    guard let self, !Task.isCancelled else { return }
                    self.error = "Live file observation unavailable: the Loopflow server stopped answering"
                } catch is CancellationError {
                    return
                } catch {
                    guard let self, !Task.isCancelled else { return }
                    self.invalidation?.cancel()
                    self.error = "Live file observation unavailable: \(error.localizedDescription)"
                }
                do { try await Task.sleep(for: delay) } catch { return }
                delay = min(delay * 2, .seconds(30))
            }
        }
    }

    /// Apply a served reading. Working-tree/Git changes move `revision` and rescan
    /// the checkout; ignored `scratch/` page writes move `scratchPages` alone so
    /// Desktop badges Open & Scratch and reloads open scratch pages without `git diff`.
    private func receive(_ revision: TaskFilesRevision) {
        let wasNotStarted = watchNotStarted
        watchNotStarted = revision.notStarted
        if revision.machineID != machine {
            arriving = Set(documents.keys)
            machine = revision.machineID
            seenScratchStamps = nil
        }
        machineLabel = revision.machineLabel
        let checkoutChanged = revision.checkout != nil && revision.checkout != cwd
        if checkoutChanged, let checkout = revision.checkout {
            cwd = checkout
            seenScratchStamps = nil
        }
        guard !revision.notStarted else {
            servedRevision = nil
            seenScratchStamps = nil
            return
        }

        let first = servedRevision == nil
        let treeChanged = checkoutChanged || (!first && revision.revision != servedRevision)
        servedRevision = revision.revision

        let incomingStamps = Dictionary(uniqueKeysWithValues: revision.scratchPages.map { ($0.path, $0.stamp) })
        if wasNotStarted {
            seenScratchStamps = incomingStamps
            // The checkout appeared: read what Not started hid.
            Task { await retry() }
            return
        }

        let previousStamps = seenScratchStamps
        seenScratchStamps = incomingStamps
        var touchedScratch = Set<String>()
        if let previousStamps {
            let removed = Set(previousStamps.keys).subtracting(incomingStamps.keys)
            let changed = Set(incomingStamps.compactMap { path, stamp in previousStamps[path] != stamp ? path : nil })
            touchedScratch = removed.union(changed)
            if !touchedScratch.isEmpty {
                scratchPaths = Set(scratchPaths).subtracting(removed).union(changed).sorted()
                browser.noteWrites(touchedScratch, present: changed)
            }
        }

        if treeChanged {
            invalidate([""])
        } else if !touchedScratch.isEmpty {
            let paths = touchedScratch
            Task { [weak self] in
                await self?.applyPeerScratchWrites(paths)
            }
        }
    }

    private func applyPeerScratchWrites(_ paths: Set<String>) async {
        if tree.showIgnored {
            await tree.rereadDirectories(holding: paths)
            guard !Task.isCancelled else { return }
        }
        await rereadDocuments(matching: paths)
    }

    private func rereadDocuments(matching changed: Set<String>) async {
        for (path, document) in documents where Self.touches(changed, path) {
            do {
                let revision = document.snapshot?.revision
                let next = try await query.taskFile(issue: issue, path: path)
                guard !Task.isCancelled else { return }
                guard revision == document.snapshot?.revision else {
                    invalidate([path])
                    continue
                }
                if arriving.remove(path) != nil {
                    await document.arrive(next)
                } else {
                    await document.reconcile(next)
                }
            } catch is CancellationError {
                return
            } catch { self.error = error.localizedDescription }
        }
    }

    func invalidate(_ paths: [String]) {
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

    private var hasPendingWrites: Bool { !pendingPaths.isEmpty || pendingComparison }

    private func applyWrites() async {
        let changed = pendingPaths
        let committed = pendingComparison
        pendingPaths.removeAll()
        pendingComparison = false
        // Every lf read journals under the checkout and builds write ignored
        // output; those writes never move the comparison. A peer filters its own.
        let ignored = machine == nil ? await ignoredPaths(changed) : []
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
        await rereadDocuments(matching: changed)
        guard !Task.isCancelled, comparisonChanged else { return }
        await workingTree.refresh(afterWrite: true)
        if page == .changes, diffReview.reviewReading !== workingTree { await diffReview.reviewReading.refresh() }
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

    init(issue: String, cwd: String, machine: String? = nil, machineLabel: String? = nil,
         localMachineID: String? = nil, query: RegistryQuery = RegistryQueryLocal.shared) {
        workingTree = ComparisonReading(issue: issue, query: query, comparison: .workingTree(base: "parent"))
        diffReview = DiffReviewState(cwd: cwd, machine: machine, workingTree: workingTree, browser: browser, pullRequest: pullRequest, comments: comments)
        tree = CheckoutTree(issue: issue, query: query)
        self.issue = issue
        self.cwd = cwd
        self.machine = machine
        self.machineLabel = machineLabel
        self.localMachineID = localMachineID.map { StructuredIdentity.prefixed($0, "home_") }
        self.query = query
        autosave = UserDefaults.standard.savesTaskFilesAutomatically
        savedPreference = autosave
        preferenceChanges = NotificationCenter.default.publisher(for: UserDefaults.didChangeNotification)
            .sink { [weak self] _ in
                Task { @MainActor [weak self] in
                    guard let self else { return }
                    let enabled = UserDefaults.standard.savesTaskFilesAutomatically
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
        if machine != nil { observeFiles() }
        await workingTree.refresh()
        if page == .changes, diffReview.reviewReading !== workingTree { await diffReview.reviewReading.refresh() }
    }

    /// Try the Machine again after an outage or before a checkout existed:
    /// watch, re-read the open directories, the selected file and the comparison.
    func retry() async {
        error = nil
        observeFiles()
        await refresh()
        await refreshScratch()
        for directory in Set(tree.listings.keys).union([""]).sorted() {
            await tree.readDirectory(directory, more: false)
        }
        await loadFile()
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
