#if os(macOS)
import Foundation
import Loopflow
import Observation

/// A Task checkout as you browse it: directories read one at a time when
/// opened, in pages `lf task files` returns. Nothing under a closed directory
/// is read, so a `node_modules`-sized checkout costs only what you open.
@MainActor @Observable
final class CheckoutTree {
    /// What is known about one directory. The root is `""`.
    struct Listing: Equatable {
        var entries: [TaskFileEntry] = []
        var nextCursor: String?
        var pages = 0
        var loading = false
        /// The last read failed; earlier pages stay readable.
        var error: String?
    }

    enum Row: Equatable, Identifiable {
        case entry(TaskFileEntry, depth: Int)
        case loading(directory: String, depth: Int)
        case more(directory: String, depth: Int)
        case failed(directory: String, reason: String, depth: Int)

        var id: String {
            switch self {
            case .entry(let entry, _): "entry:\(entry.path)"
            case .loading(let directory, _): "loading:\(directory)"
            case .more(let directory, _): "more:\(directory)"
            case .failed(let directory, _, _): "failed:\(directory)"
            }
        }

        var depth: Int {
            switch self {
            case .entry(_, let depth), .loading(_, let depth), .more(_, let depth), .failed(_, _, let depth): depth
            }
        }

        var entry: TaskFileEntry? {
            if case .entry(let entry, _) = self { return entry }
            return nil
        }
    }

    private(set) var listings: [String: Listing] = [:]
    private(set) var expanded: Set<String> = []
    /// The row the arrow keys move from; opening a file is a separate act.
    var selection: String?

    // MARK: Reading

    /// One page to read: the directory, and the cursor after its loaded pages.
    struct Read: Equatable {
        let directory: String
        let cursor: String?
    }

    /// Mark a read as started; nil when one is running or `more` has nothing left.
    func begin(_ directory: String, more: Bool) -> Read? {
        var listing = listings[directory] ?? Listing()
        guard !listing.loading else { return nil }
        if more, listing.nextCursor == nil { return nil }
        listing.loading = true
        listing.error = nil
        listings[directory] = listing
        return Read(directory: directory, cursor: more ? listing.nextCursor : nil)
    }

    func receive(_ page: TaskDirectory, for directory: String, more: Bool) {
        var listing = listings[directory] ?? Listing()
        listing.entries = more ? listing.entries + page.entries : page.entries
        listing.pages = more ? listing.pages + 1 : 1
        listing.nextCursor = page.nextCursor
        listing.loading = false
        listing.error = nil
        listings[directory] = listing
    }

    func fail(_ directory: String, reason: String) {
        var listing = listings[directory] ?? Listing()
        listing.loading = false
        listing.error = reason
        listings[directory] = listing
    }

    /// Whether opening `directory` needs a read.
    func needsRead(_ directory: String) -> Bool {
        guard let listing = listings[directory] else { return true }
        return listing.pages == 0 && !listing.loading && listing.error == nil
    }

    /// Loaded directories that hold one of `paths`; a write there may add or remove entries.
    func listingsHolding(_ paths: Set<String>) -> [String] {
        let parents = Set(paths.map { ($0 as NSString).deletingLastPathComponent })
        return listings.keys.filter { parents.contains($0) && listings[$0]?.pages ?? 0 > 0 }.sorted()
    }

    // MARK: Opening and closing

    func setExpanded(_ directory: String, _ open: Bool) {
        if open { expanded.insert(directory) } else { expanded.remove(directory) }
    }

    func isExpanded(_ directory: String) -> Bool { expanded.contains(directory) }

    var showIgnored = false {
        didSet { if showIgnored != oldValue { resetListings() } }
    }
    private var treeGeneration = 0
    private var reads: [String: Task<Void, Never>] = [:]
    var offset = CGPoint.zero
    private let issue: String
    private let query: RegistryQuery

    init(issue: String, query: RegistryQuery) {
        self.issue = issue
        self.query = query
    }

    private func resetListings() {
        treeGeneration += 1
        for read in reads.values { read.cancel() }
        reads = [:]
        listings = [:]
        expanded = []
        selection = nil
    }

    func reveal(_ path: String) async {
        let parent = (path as NSString).deletingLastPathComponent
        guard await revealDirectory(parent), !Task.isCancelled else { return }
        if await find(path, in: parent), !Task.isCancelled { selection = path }
    }

    @discardableResult
    func revealDirectory(_ path: String) async -> Bool {
        let generation = treeGeneration
        var parent = ""
        for part in path.split(separator: "/") {
            let child = parent.isEmpty ? String(part) : parent + "/" + part
            guard await find(child, in: parent), !Task.isCancelled, generation == treeGeneration else { return false }
            parent = child
            setExpanded(child, true)
        }
        if needsRead(parent) { await readDirectory(parent, more: false) }
        return generation == treeGeneration && !Task.isCancelled
    }

    private func find(_ child: String, in parent: String) async -> Bool {
        let generation = treeGeneration
        if needsRead(parent) || reads[parent] != nil { await readDirectory(parent, more: false) }
        while !Task.isCancelled, generation == treeGeneration, let listing = listings[parent] {
            if listing.entries.contains(where: { $0.path == child }) { return true }
            guard listing.error == nil, listing.nextCursor != nil else { break }
            await readDirectory(parent, more: true)
        }
        return false
    }

    /// Read the checkout root the first time the tree is shown.
    func openRoot() async {
        if needsRead("") { await readDirectory("", more: false) }
    }

    /// One page of `directory` through `lf task files`; a remote Task reads
    /// through its Machine. A failure stays on that directory.
    func readDirectory(_ directory: String, more: Bool) async {
        if let pending = reads[directory] { await pending.value; return }
        let generation = treeGeneration
        guard let read = begin(directory, more: more) else { return }
        let pending = Task {
            defer { if treeGeneration == generation { reads[directory] = nil } }
            do {
                let page = try await query.taskFiles(issue: issue, directory: directory, cursor: read.cursor, showIgnored: showIgnored)
                guard treeGeneration == generation else { return }
                receive(page, for: directory, more: more)
            } catch {
                guard treeGeneration == generation else { return }
                fail(directory, reason: error.localizedDescription)
            }
        }
        reads[directory] = pending
        await pending.value
    }

    /// Retry a failed read where it stopped: the first page, or the next one.
    func retryDirectory(_ directory: String) async {
        await readDirectory(directory, more: (listings[directory]?.pages ?? 0) > 0)
    }

    func setDirectory(_ directory: String, expanded: Bool) async {
        setExpanded(directory, expanded)
        selection = directory

        if expanded, needsRead(directory) { await readDirectory(directory, more: false) }
        if selection == directory, let entry = listings[(directory as NSString).deletingLastPathComponent]?.entries.first(where: { $0.path == directory }) {
            selection = compact(entry).path
        }
    }

    enum Move { case up, down, expand, collapse, open }

    /// The navigator's keys: ↑/↓ move, → opens a directory or enters it, ←
    /// closes it or goes to its parent, Return opens a file or toggles a directory.
    func move(_ move: Move) async -> String? {
        let rows = rows()
        let current = selection.flatMap { path in rows.compactMap(\.entry).first { $0.path == path } }
        switch move {
        case .up, .down:
            selection = neighbor(move == .up ? -1 : 1, in: rows)
        case .expand:
            guard let current, current.kind == .directory else { return nil }
            if isExpanded(current.path) {
                selection = neighbor(1, in: rows)
            } else {
                await setDirectory(current.path, expanded: true)
            }
        case .collapse:
            guard let current else { return nil }
            if current.kind == .directory, isExpanded(current.path) {
                await setDirectory(current.path, expanded: false)
            } else {
                if let parent = rows.compactMap(\.entry).last(where: { current.path.hasPrefix($0.path + "/") }) { selection = parent.path }
            }
        case .open:
            guard let current else { return nil }
            if current.kind == .directory {
                await setDirectory(current.path, expanded: !isExpanded(current.path))
            } else {
                return current.path
            }
        }
        return nil
    }

    /// Re-read loaded directories a write may have changed, as many pages as were loaded.
    func rereadDirectories(holding paths: Set<String>) async {
        for directory in listingsHolding(paths) {
            let pages = listings[directory]?.pages ?? 0
            await readDirectory(directory, more: false)
            while let listing = listings[directory], listing.pages < pages,
                  listing.nextCursor != nil, listing.error == nil, !Task.isCancelled {
                await readDirectory(directory, more: true)
            }
        }
    }

    /// Only a complete listing can establish that a directory has one child.
    private func compact(_ start: TaskFileEntry) -> TaskFileEntry {
        var entry = start
        while entry.kind == .directory, isExpanded(entry.path), let listing = listings[entry.path],
              listing.pages > 0, !listing.loading, listing.error == nil, listing.nextCursor == nil,
              listing.entries.count == 1, let child = listing.entries.first, child.kind == .directory {
            entry = child
        }
        return entry
    }

    func label(for entry: TaskFileEntry) -> String {
        var parent = (entry.path as NSString).deletingLastPathComponent
        while !parent.isEmpty, let listing = listings[parent], listing.entries.count == 1,
              listing.pages > 0, !listing.loading, listing.nextCursor == nil, listing.error == nil,
              isExpanded(parent), listing.entries.first?.kind == .directory {
            parent = (parent as NSString).deletingLastPathComponent
        }
        return parent.isEmpty ? entry.path : String(entry.path.dropFirst(parent.count + 1))
    }

    // MARK: Rows

    func rows() -> [Row] {
        var rows: [Row] = []
        append("", depth: 0, to: &rows)
        return rows
    }

    private func append(_ directory: String, depth: Int, to rows: inout [Row]) {
        let listing = listings[directory] ?? Listing()
        for start in listing.entries {
            let entry = compact(start)
            rows.append(.entry(entry, depth: depth))
            if entry.kind == .directory, expanded.contains(entry.path) {
                append(entry.path, depth: depth + 1, to: &rows)
            }
        }
        if let error = listing.error {
            rows.append(.failed(directory: directory, reason: error, depth: depth))
        } else if listing.loading {
            rows.append(.loading(directory: directory, depth: depth))
        } else if listing.nextCursor != nil {
            rows.append(.more(directory: directory, depth: depth))
        }
    }

    /// The entry above or below the selection; the first entry when none is selected.
    func neighbor(_ offset: Int, in rows: [Row]) -> String? {
        let paths = rows.compactMap { $0.entry?.path }
        guard !paths.isEmpty else { return nil }
        guard let selection, let index = paths.firstIndex(of: selection) else { return paths.first }
        return paths[max(0, min(paths.count - 1, index + offset))]
    }
}

/// How a path differs from the Task's base, as one letter beside its name.
enum CheckoutChange: String, Equatable {
    case modified = "M"
    case renamed = "R"
    case untracked = "U"
    /// A directory holding a change.
    case contains = "•"

    var help: String {
        switch self {
        case .modified: "Changed from the Task's base"
        case .renamed: "Renamed from the Task's base"
        case .untracked: "New, not yet tracked"
        case .contains: "Holds changed files"
        }
    }

    static func badges(_ files: [TaskChangedFile]) -> [String: CheckoutChange] {
        var badges: [String: CheckoutChange] = [:]
        for file in files {
            badges[file.path] = file.untracked ? .untracked : file.oldPath == nil ? .modified : .renamed
            var parent = (file.path as NSString).deletingLastPathComponent
            while !parent.isEmpty, badges[parent] == nil {
                badges[parent] = .contains
                parent = (parent as NSString).deletingLastPathComponent
            }
        }
        return badges
    }
}
#endif
