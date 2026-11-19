#if os(macOS)
import Foundation

/// What a worker says about reading its change: which files come first and
/// which are not worth opening, each with an optional reason. Read from
/// `scratch/diff-order.yaml` in the Task's checkout:
///
///     first:
///       - path: swift/LoopflowMac/TaskFileDocument.swift
///         why: A change over 1 MB no longer loses its last files
///     hide:
///       - path: docs/
///         why: Path literal renamed, nothing else
struct DiffOrder: Equatable, Sendable {
    struct Entry: Equatable, Sendable {
        /// A file, or a directory and everything under it.
        var path: String
        var why: String

        func matches(_ file: String) -> Bool {
            file == path || file.hasPrefix(path.hasSuffix("/") ? path : path + "/")
        }
    }

    static let path = "scratch/diff-order.yaml"

    var first: [Entry] = []
    var hide: [Entry] = []

    /// The two lists of this one shape; anything else in the file is ignored.
    static func parse(_ yaml: String) -> DiffOrder {
        var order = DiffOrder()
        var section: WritableKeyPath<DiffOrder, [Entry]>?
        func value(_ text: Substring) -> String {
            var text = text.trimmingCharacters(in: .whitespaces)
            if text.count >= 2, let quote = text.first, quote == "\"" || quote == "'", text.last == quote {
                text = String(text.dropFirst().dropLast())
            }
            return text
        }
        for raw in yaml.split(whereSeparator: \.isNewline) {
            let line = raw.trimmingCharacters(in: .whitespaces)
            guard !line.isEmpty, !line.hasPrefix("#") else { continue }
            if raw.first?.isWhitespace == false, !line.hasPrefix("-") {
                section = line == "first:" ? \.first : line == "hide:" ? \.hide : nil
                continue
            }
            guard let section else { continue }
            var field = line[...]
            if field.hasPrefix("-") {
                order[keyPath: section].append(Entry(path: "", why: ""))
                field = field.dropFirst()
            }
            guard let last = order[keyPath: section].indices.last else { continue }
            let text = field.trimmingCharacters(in: .whitespaces)
            if text.hasPrefix("path:") {
                order[keyPath: section][last].path = value(text.dropFirst("path:".count))
            } else if text.hasPrefix("why:") {
                order[keyPath: section][last].why = value(text.dropFirst("why:".count))
            } else if !text.isEmpty, order[keyPath: section][last].path.isEmpty {
                order[keyPath: section][last].path = value(text[...])
            }
        }
        func named(_ entries: [Entry]) -> [Entry] {
            entries.compactMap { entry in
                let path = entry.path.hasPrefix("./") ? String(entry.path.dropFirst(2)) : entry.path
                return path.isEmpty ? nil : Entry(path: path, why: entry.why)
            }
        }
        return DiffOrder(first: named(order.first), hide: named(order.hide))
    }
}

/// How the Diff orders its files.
enum ReviewOrdering: String, CaseIterable, Sendable {
    /// The worker's files first, in its order; the rest by kind.
    case tour = "Tour"
    /// Source, tests, docs, then generated and lock files; largest change first.
    case kind = "By kind"
    case path = "By path"
}

/// What a changed file is, from its path alone.
enum ReviewKind: Int, CaseIterable, Sendable {
    case source, tests, docs, generated

    var title: String {
        switch self {
        case .source: "Source"
        case .tests: "Tests"
        case .docs: "Docs"
        case .generated: "Generated and lock files"
        }
    }

    init(_ file: ReviewFile) {
        let path = file.path.lowercased()
        let name = (path as NSString).lastPathComponent
        let folders = path.split(separator: "/").dropLast()
        let type = (name as NSString).pathExtension
        if file.status == .binary || type == "lock" || type == "snap" || type == "pbxproj"
            || name.hasSuffix("-lock.json") || name.hasSuffix(".lock.json") || name.contains(".min.")
            || folders.contains(where: { ["generated", "__generated__", "__snapshots__", "vendor"].contains($0) }) {
            self = .generated
        } else if folders.contains(where: { ["test", "tests", "__tests__", "spec", "fixtures"].contains($0) || $0.hasSuffix("tests") })
                    || name.hasPrefix("test_") || name.contains("_test.") || name.contains(".test.")
                    || name.contains(".spec.") || name.contains("tests.") {
            self = .tests
        } else if ["md", "mdx", "rst", "txt", "adoc"].contains(type) || folders.first == "docs" {
            self = .docs
        } else {
            self = .source
        }
    }
}

/// Files the Diff shows together under one heading.
struct ReviewGroup: Equatable, Sendable, Identifiable {
    let id: String
    let title: String
    /// The title is the worker's reason for reading these files first.
    let isTour: Bool
    let files: [ReviewFile]

    /// Files in reading order, grouped. A tour names files; a file it does not
    /// name, or one added after it was written, follows by kind, so the order
    /// can never lose a file.
    static func arrange(_ files: [ReviewFile], ordering: ReviewOrdering, order: DiffOrder?) -> [ReviewGroup] {
        switch ordering {
        case .path:
            let folders = Dictionary(grouping: files) { file in
                file.path.contains("/") ? String(file.path.prefix { $0 != "/" }) + "/" : ""
            }
            return folders.sorted { $0.key < $1.key }.map {
                ReviewGroup(id: "path:\($0.key)", title: $0.key.isEmpty ? "Top level" : $0.key, isTour: false,
                            files: $0.value.sorted { $0.path < $1.path })
            }
        case .kind:
            return byKind(files)
        case .tour:
            var rest = files
            var groups: [ReviewGroup] = []
            for (index, entry) in (order?.first ?? []).enumerated() {
                let named = byKind(rest.filter { entry.matches($0.path) }).flatMap(\.files)
                guard !named.isEmpty else { continue }
                rest.removeAll { entry.matches($0.path) }
                // An entry without a reason reads on under the one above it.
                if entry.why.isEmpty, let last = groups.last {
                    groups[groups.count - 1] = ReviewGroup(id: last.id, title: last.title, isTour: true, files: last.files + named)
                } else {
                    groups.append(ReviewGroup(id: "tour:\(index)", title: entry.why, isTour: true, files: named))
                }
            }
            return groups + byKind(rest)
        }
    }

    private static func byKind(_ files: [ReviewFile]) -> [ReviewGroup] {
        let kinds = Dictionary(grouping: files) { ReviewKind($0) }
        return ReviewKind.allCases.compactMap { kind in
            guard let files = kinds[kind] else { return nil }
            return ReviewGroup(id: "kind:\(kind.rawValue)", title: kind.title, isTour: false, files: files.sorted {
                ($1.additions + $1.deletions, $0.path) < ($0.additions + $0.deletions, $1.path)
            })
        }
    }
}
#endif
