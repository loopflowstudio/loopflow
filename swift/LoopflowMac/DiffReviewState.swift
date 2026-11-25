#if os(macOS)
import AppKit
import Loopflow
import Observation

@MainActor @Observable
final class DiffReviewState {
    private var issue: String { workingTree.issue }
    /// The Task's checkout and its Machine (nil for this Mac); Files moves both.
    var cwd: String
    var machine: String?
    private var query: RegistryQuery { workingTree.query }
    let workingTree: ComparisonReading
    let browser: FilesBrowserState
    let pullRequest: PullRequestStore

    let comments: ReviewComments

    init(cwd: String, machine: String?, workingTree: ComparisonReading,
         browser: FilesBrowserState, pullRequest: PullRequestStore, comments: ReviewComments) {
        self.comments = comments
        self.cwd = cwd
        self.machine = machine
        self.taskReading = workingTree
        self.workingTree = workingTree
        self.browser = browser
        self.pullRequest = pullRequest
    }

    private var taskReading: ComparisonReading
    private var stepReading: (step: ReviewStep, reading: ComparisonReading)?
    var reviewReading: ComparisonReading { stepReading?.reading ?? taskReading }
    var comparison: ChangeComparison {
        get { reviewReading.comparison }
        set {
            guard newValue != comparison else { return }
            stepReading = nil
            taskReading = newValue == workingTree.comparison ? workingTree : makeReading(newValue)
            reviewGeneration += 1
            review = nil
        }
    }

    private func makeReading(_ comparison: ChangeComparison) -> ComparisonReading {
        ComparisonReading(issue: issue, query: query, comparison: comparison)
    }

    var changes: TaskChangesSnapshot? { reviewReading.changes }
    var fileDiffError: String?

    /// A file's changes: the count read with the file list, else its own lines'.
    func changeCount(_ file: ReviewFile, in reader: ReviewReader = .changes) -> Int {
        if rendering(of: file, in: reader) != nil { return 1 }
        return reader == .changes ? reviewReading.filesByPath[file.path]?.changes ?? file.stops.count : file.stops.count
    }
    /// Files' Diff mode: the selected file through the review stream.
    private(set) var fileReview: FileReview?
    private var diffGeneration = 0

    // MARK: Review

    /// The whole comparison as one stream. A reload replaces it in place; the
    /// reading anchor and current stop are ids, so new evidence never moves them.
    private(set) var review: ReviewDiff? { didSet { arrange() } }
    /// Changed files whose lines are not read yet, or were read before the
    /// comparison last moved. A hidden file stays here until it is shown.
    private(set) var unread: Set<String> = []
    /// Why a file's lines are missing or cut short, by path.
    private(set) var fileNotes: [String: FileNote] = [:]
    /// Lines past `ReviewLine.readable` that you asked to see whole, by `ReviewLine.id(in:)`.
    var openedLongLines: Set<String> = []
    @ObservationIgnored private var reading: Set<String> = []
    /// The base and head commits the files were last read against.
    @ObservationIgnored private var readCommits: [String] = []
    @ObservationIgnored private var nearPaths: Set<String> = []
    private var reviewGeneration = 0
    /// Each reader's place, so switching paths or pages and back lands where
    /// you were. The row at the top moves on every scrolled row, the current
    /// change only when you step, so they are observed apart: scrolling
    /// redraws what names the top file, never the rows that mark the change.
    private var anchors: [ReviewReader: String] = [:] { didSet { refreshTopPath() } }
    private var currents: [ReviewReader: String] = [:] { didSet { refreshTopPath() } }

    /// The file whose lines are at the top of the Diff: the focused one, else
    /// the one the anchor or the current change is in. While a file's own
    /// header is the top row, the file above it is still the one you are in.
    /// Set only when it changes, so the file bar and the sidebar redraw when
    /// you cross into another file, not on every scrolled row.
    private(set) var topPath: String?

    private func refreshTopPath() {
        let path: String? = {
            if let focusedPath { return focusedPath }
            let files = shownReview?.files ?? []
            guard let id = anchors[.changes] ?? currents[.changes] else { return files.first?.path }
            if id == "footer" { return files.last?.path }
            guard let index = shownReview?.fileIndex(holding: id) else { return files.first?.path }
            return files[id.hasPrefix("file:") ? max(0, index - 1) : index].path
        }()
        if path != topPath { topPath = path }
    }
    /// When next/previous last moved, so the change it landed on can pulse once.
    @ObservationIgnored private(set) var steppedAt = Date.distantPast
    var reviewLayout = ReviewLayout.new
    /// Long lines wrap; off, each line keeps one row and is cut at the pane's edge.
    var wrapsLines = true
    /// Files you marked viewed, by path, with the lines they had then.
    private var viewed: [String: Int] = [:]
    /// Changes whose removed lines you opened while reading new code.
    var openedRemovals: Set<String> = []
    /// Rendered files you switched to their line Diff, by path.
    var sourceFiles: Set<String> = []
    /// One file at a time; next/previous still crosses into the next file.
    var focusedPath: String? { didSet { refreshTopPath() } }
    var expandedMoves: Set<String> = []
    /// New code reads the file as it is now: added lines marked, removed
    /// lines folded to a mark that opens them.
    enum ReviewLayout: String, CaseIterable { case new = "New code", unified = "Unified", split = "Side by side" }

    /// Side by side needs a wide pane; a narrower one reads unified.
    static let splitWidth: CGFloat = 640
    /// The layout the stream reads in at this width.
    func layout(width: CGFloat) -> ReviewLayout { reviewLayout == .split && width < Self.splitWidth ? .unified : reviewLayout }

    // MARK: Steps and viewed files

    /// The Graph steps in the Task's commits, oldest first, from the last read
    /// of the whole Task's comparison.
    var steps: [ReviewStep] { ReviewStep.steps(taskReading.changes?.commits ?? []) }
    var step: ReviewStep? { stepReading?.step }

    func showStep(_ next: ReviewStep?) {
        guard next != step else { return }
        // A branch ref may have moved while its step was open.
        if next == nil, taskReading !== workingTree { taskReading = makeReading(taskReading.comparison) }
        stepReading = next.map { ($0, makeReading(.branch(base: $0.base, head: $0.head))) }
        reviewGeneration += 1
        review = nil
        reviewReading.openedGaps = [:]
        reviewReading.dropSides()
        self[place: .changes] = ReviewPlace()
        windowStart = nil
        windowRows = Self.windowBudget
    }

    /// How the file reads in the Diff instead of its lines; nil once you chose
    /// its Source, and for Files' Diff of an unsaved draft, which the sides
    /// read from disk would not show.
    func rendering(of file: ReviewFile, in reader: ReviewReader) -> ReviewRendering? {
        if showsDraft(reader) { return nil }
        // Before its patch reads, a file `lf task changes` counted no lines in may be a picture.
        let awaitingPatch = reader == .changes && isReading(file) && reviewReading.filesByPath[file.path]?.changes == 0
        guard let rendering = ReviewRendering.of(file, awaitingPatch: awaitingPatch) else { return nil }
        return rendering.hasSource && sourceFiles.contains(file.path) ? nil : rendering
    }

    /// Files' Diff of an unsaved draft, whose lines are not the file on disk.
    private func showsDraft(_ reader: ReviewReader) -> Bool { reader != .changes && fileReview?.draft == true }

    /// A file you marked viewed and whose lines have not changed since.
    func isViewed(_ file: ReviewFile) -> Bool { viewed[file.path] == file.fingerprint && !isReading(file) }

    func setViewed(_ file: ReviewFile, _ on: Bool) {
        viewed[file.path] = on ? file.fingerprint : nil
    }

    /// "12 / 60 viewed" over the files shown.
    var viewedCount: Int { shownReview?.files.count(where: isViewed) ?? 0 }

    /// Read the comparison file by file. `lf task changes` names every
    /// changed file at once; each shown file's lines are then read on their
    /// own, in the stream's order, so no file's size cuts another short. A
    /// reload keeps the lines it had until their replacement arrives, and
    /// rereads only the `changed` paths while the comparison's commits stand.
    func loadReview(changed: Set<String>? = nil) async {
        if changes == nil { await reviewReading.refresh() }
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
        let paths = Set(reviewReading.filesByPath.keys)
        let commits = [changes.baseCommit, changes.headCommit]
        if let changed, commits == readCommits {
            unread = paths.filter { unread.contains($0) || known[$0] == nil || TaskFilesStore.touches(changed, $0) }
        } else {
            unread = paths
            reviewReading.dropSides()
        }
        readCommits = commits
        reading = []
        fileNotes = fileNotes.filter { paths.contains($0.key) }
        for path in reviewReading.openedGaps.keys where !paths.contains(path) { reviewReading.openedGaps[path] = nil }
        for path in Set(comments.items.map(\.selection.path)).subtracting(paths) {
            comments.reconcile(path: path, revision: revision, file: nil)
        }
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
        let files = reviewReading.filesByPath
        let query = self.query, issue = self.issue, base = changes.baseCommit, head = comparison.head
        await withTaskGroup(of: (String, FileReading?).self) { group in
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
                // A read that was called off is no answer: the file stays unread for the next load.
                if let result { accept(result, for: path) }
            }
        }
    }

    /// The next shown file without lines: one near the viewport, else the
    /// first in the stream's order. Hidden files are never read.
    private func nextToRead() -> String? {
        var first: String?
        for file in shownReview?.files ?? [] where unread.contains(file.path) && !reading.contains(file.path) {
            if nearPaths.contains(file.path) { return file.path }
            if first == nil { first = file.path }
        }
        return first
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

    /// Why a file's lines are missing or cut short.
    enum FileNote: Equatable, Sendable {
        case cut
        /// The Machine's message.
        case failed(String)
    }

    /// Read a file again after its read failed.
    func retryFile(_ path: String) async {
        fileNotes[path] = nil
        unread.insert(path)
        await readPatches()
    }

    private struct FileReading: Sendable {
        /// nil when the read failed; the stream keeps the lines it had.
        let file: ReviewFile?
        let note: FileNote?
    }

    /// One file's lines: its patch, which for a new file, untracked or
    /// committed, is its contents.
    private nonisolated static func readFile(_ changed: TaskChangedFile, query: RegistryQuery, issue: String,
                                             base: String, head: String?) async -> FileReading? {
        let path = changed.path
        do {
            let next = try await query.taskDiff(issue: issue, path: path, base: base, head: head)
            let parsed = ReviewDiff.parse(next.patch).files
            let file = parsed.first { $0.path == path } ?? parsed.first
                ?? ReviewFile(path: path, oldPath: changed.oldPath, status: next.binary ? .binary : .modified, blocks: [])
            return FileReading(file: file, note: next.truncated ? .cut : nil)
        } catch is CancellationError {
            return nil
        } catch {
            return FileReading(file: nil, note: .failed(error.localizedDescription))
        }
    }

    private func accept(_ result: FileReading, for path: String) {
        unread.remove(path)
        fileNotes[path] = result.note
        guard let review, let index = review.files.firstIndex(where: { $0.path == path }),
              let file = result.file else { return }
        if file != review.files[index] {
            reviewReading.openedGaps[path] = nil
            reviewReading.dropSides(path)
            var files = review.files
            files[index] = file
            self.review = ReviewDiff(files: files)
        }
        comments.reconcile(path: path, revision: revision, file: opened(file))
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
            refreshTopPath()
            return
        }
        let filter = reviewFilter.trimmingCharacters(in: .whitespaces)
        let files = review.files.filter { file in
            (showsHidden || !hides(file.path)) && tourHides(file.path) == nil
                && (filter.isEmpty || file.path.range(of: filter, options: .caseInsensitive) != nil)
        }
        shownGroups = ReviewGroup.arrange(files, ordering: ordering, tour: diffTour, groups: diffGroups)
        shownReview = ReviewDiff(files: shownGroups.flatMap(\.files))
        refreshTopPath()
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
        guard machine == nil, tourWriting != .launching else { return nil }
        if diffTour == nil { return "Write tour" }
        return tourAge != nil ? "Update tour" : nil
    }

    /// Run the `tour` skill headless in the Task's checkout. The order
    /// switches to Tour when the file it writes lands.
    func writeTour() async {
        guard tourAction != nil else { return }

        tourWriting = .launching
        do {
            tourWriting = .launched(log: try await query.runSkill("tour", checkout: cwd))
        } catch {
            tourWriting = .failed("The tour could not start: \(error.localizedDescription)")
        }
    }

    func reading(for reader: ReviewReader) -> ComparisonReading { reader == .changes ? reviewReading : workingTree }

    // MARK: Syntax colors

    struct HighlightSource: Equatable {
        let fingerprint: Int
        /// Read from both whole files, rather than each hunk's own lines.
        let whole: Bool
    }

    /// Each file's syntax colors, with what they were worked out from.
    /// Parsing runs off the main thread.
    private var highlighted: [String: (source: HighlightSource, highlights: FileHighlights)] = [:]
    @ObservationIgnored private var highlighting: [String: HighlightSource] = [:]

    /// The file's syntax colors: from each hunk's own lines at first, then from
    /// both whole files once they are read, so a hunk that opens inside a
    /// docstring colors as one. Files' Diff of an unsaved draft keeps hunk
    /// colors, since the file on disk is not what it shows. Until new colors
    /// arrive, the ones before, which keep their numbers when skipped lines open.
    func highlights(of file: ReviewFile, in reader: ReviewReader) -> FileHighlights? {
        guard let grammar = Grammar.named(file.path) else { return nil }
        let sides = showsDraft(reader) ? nil : self.reading(for: reader).sides(of: file)
        let source = HighlightSource(fingerprint: file.fingerprint, whole: sides != nil)
        let known = highlighted[file.path]
        if known?.source == source { return known?.highlights }
        if highlighting[file.path] != source {
            highlighting[file.path] = source
            let old = sides?.old?.content, new = sides?.new?.content
            Task { [weak self] in
                let result = await Task.detached(priority: .userInitiated) {
                    FileHighlights.hunks(of: file, grammar: grammar).reading(old: old, new: new, grammar: grammar)
                }.value
                guard let self, self.highlighting[file.path] == source else { return }
                self.highlighting[file.path] = nil
                self.highlighted[file.path] = (source, result)
            }
        }
        return known?.highlights
    }

    // MARK: Skipped lines

    func opened(_ file: ReviewFile, in reader: ReviewReader = .changes) -> ReviewFile {
        self.reading(for: reader).openedGaps[file.path].map { file.opening($0.lines) } ?? file
    }

    /// Show the lines a hunk header skips, from the comparison's newer side:
    /// its head commit, or the working tree. Files' Diff of an unsaved draft
    /// reads the draft.
    func openGap(_ header: String, gap: ReviewGap, file: ReviewFile, reader: ReviewReader) async {
        let reading = self.reading(for: reader)
        let path = file.path
        do {
            let content: String
            if showsDraft(reader), let draft = browser.documents[path]?.text {
                content = draft
            } else {
                let new = try await reading.readSides(file).new
                guard let new, new.state == .text, let text = new.content else {
                    throw RegistryQueryError("\((new?.state ?? .missing).title) cannot be shown as text.")
                }
                content = text
            }
            guard let lines = gap.lines(in: content) else {
                throw RegistryQueryError("the file changed since this comparison was read.")
            }
            reading.openedGaps[path, default: .init()].lines[header] = lines
            reading.openedGaps[path]?.failure = nil
        } catch is CancellationError {
            return
        } catch {
            reading.openedGaps[path, default: .init()].failure = error.localizedDescription
        }
    }

    /// One of the two diff readers: Changes' whole comparison, or Files' Diff of one path.
    enum ReviewReader: Hashable { case changes, file(String) }

    /// Where you are in a reader: the row at its top and the change next/previous moves from.
    struct ReviewPlace: Equatable {
        var anchor: String?
        var current: String?
    }

    /// Setting a place moves to it: in the Diff, the window moves to a row it
    /// does not hold or holds far down.
    subscript(place reader: ReviewReader) -> ReviewPlace {
        get { ReviewPlace(anchor: anchors[reader], current: currents[reader]) }
        set {
            if anchors[reader] != newValue.anchor { anchors[reader] = newValue.anchor }
            if currents[reader] != newValue.current { currents[reader] = newValue.current }
            if reader == .changes { reveal(newValue.anchor) }
        }
    }

    /// The row the scroll view reports at its top as you scroll. Scrolling
    /// stays inside the window, so it never moves it.
    func scrolled(to id: String?, in reader: ReviewReader) {
        if anchors[reader] != id { anchors[reader] = id }
    }

    // MARK: The Diff's window

    /// How many rows the Diff draws from where it starts. A scroll view finds
    /// a row by passing every row above it, so a jump past thousands of rows
    /// takes seconds; the window keeps every jump short.
    static let windowBudget = 1_500

    /// The first file the Diff draws; nil for the first of all.
    private(set) var windowStart: String?
    /// How many rows the window holds from its start: the budget, plus what
    /// scrolling down and showing earlier files added.
    private(set) var windowRows = windowBudget

    /// The shown files the Diff draws, as indices into `shownReview`.
    var window: Range<Int> {
        let files = shownReview?.files ?? []
        let start = windowStart.flatMap { path in files.firstIndex { $0.path == path } } ?? 0
        var end = start, rows = 0
        while end < files.count, end == start || rows < windowRows {
            rows += files[end].rows
            end += 1
        }
        return start..<end
    }

    /// Scrolling reached the window's last row: draw the files after it.
    func extendWindow() { windowRows += Self.windowBudget }

    /// Draw the files above the window, about a budget's worth, keeping the
    /// line at the top where it is.
    func showEarlierFiles() {
        let files = shownReview?.files ?? []
        var start = window.lowerBound, added = 0
        guard start > 0 else { return }
        let top = anchors[.changes] ?? topID(of: files[start], in: .changes)
        while start > 0, added < Self.windowBudget {
            start -= 1
            added += files[start].rows
        }
        windowStart = start == 0 ? nil : files[start].path
        windowRows += added
        anchors[.changes] = top
    }

    /// Start the window at the file holding `id` when the window lacks it or
    /// holds it more than a budget's rows down.
    private func reveal(_ id: String?) {
        guard let id, let files = shownReview?.files, let index = shownReview?.fileIndex(holding: id) else { return }
        let window = self.window
        if window.contains(index), files[window.lowerBound..<index].reduce(0, { $0 + $1.rows }) <= Self.windowBudget {
            return
        }
        windowStart = index == 0 ? nil : files[index].path
        windowRows = Self.windowBudget
    }

    /// The change next/previous moves from, read without the scroll anchor.
    func current(_ reader: ReviewReader) -> String? { currents[reader] }

    /// The diff a reader shows, once it has been read.
    func diff(_ reader: ReviewReader) -> ReviewDiff? {
        switch reader {
        // A viewed file is folded to its header, so stepping passes over it.
        case .changes: shownReview.map { ReviewDiff(files: $0.files.filter { !isViewed($0) }) }
        case .file(let path): fileReview?.path == path ? fileReview?.diff : nil
        }
    }

    /// The changes a reader steps through, as it shows them: a file the Diff
    /// renders is one change, read as a document; with Source on, its hunks.
    func stops(of files: [ReviewFile], in reader: ReviewReader) -> [ReviewStop] {
        files.flatMap { file in
            rendering(of: file, in: reader) != nil
                ? [.rendered(file.path)] : file.stops.map { ReviewStop(id: $0, path: file.path) }
        }
    }

    func stops(_ reader: ReviewReader) -> [ReviewStop] { diff(reader).map { stops(of: $0.files, in: reader) } ?? [] }

    /// The id of the first row the reader draws for a file under its header.
    func topID(of file: ReviewFile, in reader: ReviewReader) -> String {
        rendering(of: file, in: reader) != nil ? ReviewStop.rendered(file.path).id : file.topID
    }

    /// Source totals come with the file list, before the patch reads finish.
    func position(_ reader: ReviewReader) -> String {
        guard let diff = diff(reader) else { return "" }
        let total = diff.files.reduce(0) { $0 + changeCount($1, in: reader) }
        guard total > 0 else { return "No changes" }
        guard let current = current(reader), let file = diff.fileIndex(holding: current),
              let index = stops(of: [diff.files[file]], in: reader).firstIndex(where: { $0.id == current }) else {
            return total == 1 ? "1 change" : "\(total) changes"
        }
        let before = diff.files[..<file].reduce(0) { $0 + changeCount($1, in: reader) }
        return "\(before + index + 1) of \(total)"
    }

    /// Move to the next or previous change in a reader. Changes crosses files
    /// in one stream. Files stops at either end of its own changes.
    @discardableResult
    func step(_ offset: Int, in reader: ReviewReader) -> ReviewStop? {
        steppedAt = .now
        if let stop = stops(reader).stop(after: current(reader), offset: offset) {
            currents[reader] = stop.id
            if reader == .changes, focusedPath != nil { focusedPath = stop.path }
            return stop
        }
        return nil
    }

    /// What the stream compares, named as the PR would: its base branch and
    /// commit, then the head. Footer and conversation references share it.
    /// What is compared, as sentences. Refs and commits are marked as code,
    /// so the page sets them in mono: "Compared with `main` at `2750c9e0fb`.
    /// Includes uncommitted changes."
    var comparisonLine: AttributedString? {
        guard let changes else { return nil }
        func code(_ text: some StringProtocol) -> AttributedString {
            var name = AttributedString(String(text))
            name.inlinePresentationIntent = .code
            return name
        }
        let base = code(changes.baseCommit.prefix(10)), head = code(changes.headCommit.prefix(10))
        if let step { return AttributedString("Step \(step.title): ") + base + AttributedString("..") + head + AttributedString(".") }
        let branch = pullRequest.pullRequest.map { code($0.baseRefName) } ?? AttributedString("the Task's base")
        let compared = AttributedString("Compared with ") + branch + AttributedString(" at ") + base + AttributedString(".")
        return comparison.head == nil ? compared + AttributedString(" Includes uncommitted changes.")
            : compared + AttributedString(" \(comparison.title) at ") + head + AttributedString(".")
    }

    var comparisonDescription: String? { comparisonLine.map { String($0.characters) } }

    /// The comparison's name for a line sent to the conversation, before it is read too.
    func revision(for reader: ReviewReader) -> String {
        if case .file = reader, let fileReview {
            return (fileReview.draft ? "unsaved draft" : "working tree") + " on " + fileReview.baseCommit.prefix(10)
        }
        return revision
    }

    var revision: String {
        guard let changes else { return comparison.title }
        let base = changes.baseCommit.prefix(10)
        if comparison.head == nil { return "working tree on " + base }
        return "\(comparison.title) \(base)..\(changes.headCommit.prefix(10))"
    }

    /// "4 changed files", or why there is nothing to count yet. Hidden files,
    /// yours and the tour's, stay in the total: "4 of 9 changed files". A
    /// comparison that failed shows its error here.
    var changesTitle: String {
        guard let changes else { return "Reading changes…" }
        guard !changes.files.isEmpty else { return "No changed files" }
        let all = changes.files.count
        let noun = all == 1 ? "changed file" : "changed files"
        guard let review, let shown = shownReview, shown.files.count != review.files.count else { return "\(all) \(noun)" }
        return "\(shown.files.count) of \(review.files.count) \(noun)"
    }

    /// "12 of 85 viewed", once one file is.
    var viewedSummary: String? {
        guard viewedCount > 0, let shown = shownReview?.files.count else { return nil }
        return "\(viewedCount) of \(shown) viewed"
    }

    // MARK: One file

    struct FileReview: Equatable {
        let path: String
        let baseCommit: String
        let diff: ReviewDiff
        let truncated: Bool
        let editVersion: Int?
        let marks: [Int: ReviewLine.Kind]
        /// Compared with the unsaved draft rather than the file on disk.
        let draft: Bool
    }

    /// The selected file's diff. A dirty draft is compared as typed, debounced.
    func loadDiff() async {
        if workingTree.changes == nil { await workingTree.refresh() }
        diffGeneration += 1
        let generation = diffGeneration
        // Keep the displayed diff for the same file while its replacement loads.
        if fileReview?.path != browser.selection || fileReview?.baseCommit != workingTree.changes?.baseCommit { fileReview = nil }
        guard let path = browser.selection, let base = workingTree.changes?.baseCommit else { return }
        let document = browser.documents[path]
        let version = document?.editVersion
        let draft = document?.dirty == true ? document?.text : nil
        do {
            // Debounce edits; identity checks also protect transports that finish after cancellation.
            if document?.dirty == true { try await Task.sleep(for: .milliseconds(180)) }
            let next = try await query.taskDiff(issue: issue, path: path, base: base, head: nil, draft: draft)
            let parsed = await Task.detached(priority: .userInitiated) { ReviewDiff.parse(next.patch) }.value
            guard !Task.isCancelled, generation == diffGeneration, browser.selection == path, workingTree.changes?.baseCommit == base,
                  browser.documents[path]?.editVersion == version else { return }
            if let shown = fileReview, shown.diff != parsed {
                workingTree.openedGaps[path] = nil
                // A changed comparison of the file on disk means the disk changed.
                if draft == nil { workingTree.dropSides(path) }
            }
            fileReview = FileReview(path: path, baseCommit: base, diff: parsed, truncated: next.truncated,
                                    editVersion: version, marks: parsed.files.first?.gutterMarks ?? [:], draft: draft != nil)
            comments.reconcile(path: path, revision: revision(for: .file(path)),
                               file: parsed.files.first.map { opened($0, in: .file(path)) })
            fileDiffError = nil
        } catch {
            guard !Task.isCancelled, generation == diffGeneration, browser.selection == path, workingTree.changes?.baseCommit == base else { return }
            fileDiffError = error.localizedDescription
        }
    }
}
#endif
