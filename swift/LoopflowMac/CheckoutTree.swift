#if os(macOS)
import Foundation
import Loopflow

/// A Task checkout as you browse it: directories read one at a time when
/// opened, in pages `lf task files` returns. Nothing under a closed directory
/// is read, so a `node_modules`-sized checkout costs only what you open.
struct CheckoutTree: Equatable {
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
    mutating func begin(_ directory: String, more: Bool) -> Read? {
        var listing = listings[directory] ?? Listing()
        guard !listing.loading else { return nil }
        if more, listing.nextCursor == nil { return nil }
        listing.loading = true
        listing.error = nil
        listings[directory] = listing
        return Read(directory: directory, cursor: more ? listing.nextCursor : nil)
    }

    mutating func receive(_ page: TaskDirectory, for directory: String, more: Bool) {
        var listing = listings[directory] ?? Listing()
        listing.entries = more ? listing.entries + page.entries : page.entries
        listing.pages = more ? listing.pages + 1 : 1
        listing.nextCursor = page.nextCursor
        listing.loading = false
        listing.error = nil
        listings[directory] = listing
    }

    mutating func fail(_ directory: String, reason: String) {
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

    mutating func setExpanded(_ directory: String, _ open: Bool) {
        if open { expanded.insert(directory) } else { expanded.remove(directory) }
    }

    func isExpanded(_ directory: String) -> Bool { expanded.contains(directory) }

    // MARK: Rows

    /// Every visible row, top to bottom. With a search, loaded entries and
    /// `also` (changed files you may not have opened) whose path matches, flat;
    /// directories you never opened are not searched.
    func rows(matching search: String = "", also: [String] = []) -> [Row] {
        if !search.isEmpty {
            var matches: [String: TaskFileEntry] = [:]
            for entry in listings.values.flatMap(\.entries) { matches[entry.path] = entry }
            for path in also where matches[path] == nil { matches[path] = TaskFileEntry(path: path, kind: .file) }
            return matches.values
                .filter { $0.path.localizedCaseInsensitiveContains(search) }
                .sorted { $0.path < $1.path }
                .map { .entry($0, depth: 0) }
        }
        var rows: [Row] = []
        append("", depth: 0, to: &rows)
        return rows
    }

    private func append(_ directory: String, depth: Int, to rows: inout [Row]) {
        let listing = listings[directory] ?? Listing()
        for entry in listing.entries {
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
