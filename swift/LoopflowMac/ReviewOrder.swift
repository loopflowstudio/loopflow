#if os(macOS)
import Foundation
import Loopflow

/// What the tour says about reading a change: which files come first and
/// which are not worth opening, each with an optional reason. Read from
/// `scratch/tour.yaml` in the Task's checkout, written by the `tour`
/// skill at the head commit it read:
///
///     at: 81d3fa5973e63a4394a25357502e635a3baf5d5b
///     first:
///       - path: swift/LoopflowMac/TaskFileDocument.swift
///         why: A change over 1 MB no longer loses its last files
///     hide:
///       - path: docs/
///         why: Path literal renamed, nothing else
struct DiffTour: Equatable, Sendable {
    struct Entry: Equatable, Sendable {
        /// A file, or a directory and everything under it.
        var path: String
        var why: String

        func matches(_ file: String) -> Bool {
            file == path || file.hasPrefix(path.hasSuffix("/") ? path : path + "/")
        }
    }

    static let path = "scratch/tour.yaml"

    /// The head commit the tour was written at; nil when the file does not say.
    var at: String?
    var first: [Entry] = []
    var hide: [Entry] = []

    /// `at:` and the two lists of this one shape; anything else in the file is ignored.
    static func parse(_ yaml: String) -> DiffTour {
        var tour = DiffTour()
        var section: WritableKeyPath<DiffTour, [Entry]>?
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
                if line.hasPrefix("at:") {
                    let commit = value(line.dropFirst("at:".count))
                    tour.at = commit.isEmpty ? nil : commit
                }
                section = line == "first:" ? \.first : line == "hide:" ? \.hide : nil
                continue
            }
            guard let section else { continue }
            var field = line[...]
            if field.hasPrefix("-") {
                tour[keyPath: section].append(Entry(path: "", why: ""))
                field = field.dropFirst()
            }
            guard let last = tour[keyPath: section].indices.last else { continue }
            let text = field.trimmingCharacters(in: .whitespaces)
            if text.hasPrefix("path:") {
                tour[keyPath: section][last].path = value(text.dropFirst("path:".count))
            } else if text.hasPrefix("why:") {
                tour[keyPath: section][last].why = value(text.dropFirst("why:".count))
            } else if !text.isEmpty, tour[keyPath: section][last].path.isEmpty {
                tour[keyPath: section][last].path = value(text[...])
            }
        }
        func named(_ entries: [Entry]) -> [Entry] {
            entries.compactMap { entry in
                let path = entry.path.hasPrefix("./") ? String(entry.path.dropFirst(2)) : entry.path
                return path.isEmpty ? nil : Entry(path: path, why: entry.why)
            }
        }
        return DiffTour(at: tour.at, first: named(tour.first), hide: named(tour.hide))
    }
}

/// How the Diff orders its files.
enum ReviewOrdering: String, CaseIterable, Sendable {
    /// The tour's files first, in its order; the rest by group.
    case tour = "Tour"
    /// Under the `diff_groups` headings, by path within each: an order known
    /// before any file's lines are read, so nothing moves as they arrive.
    case groups = "By group"
    case path = "By path"
}

/// A path pattern from `diff_groups` or `diff_hidden`, matched against `/`
/// and the file's path, ignoring case: contained anywhere (`/tests/`,
/// `scratch/`), or at the end when it starts with `*` (`*.sql`).
enum DiffPattern {
    static func matches(_ pattern: String, _ path: String) -> Bool {
        let path = "/" + path.lowercased(), pattern = pattern.lowercased()
        if pattern.hasPrefix("*") { return path.hasSuffix(pattern.dropFirst()) }
        return path.contains(pattern)
    }
}

/// One heading of the `diff_groups` setting: the files a pattern matches, or,
/// with no patterns, every file no group matches (`- rest: Everything else`).
struct DiffGroupSetting: Equatable, Sendable {
    var title: String
    /// nil for the rest.
    var paths: [String]?

    static let restTitle = "Everything else"
    /// Until the setting is read, and when that read fails, the files stay
    /// under one heading.
    static let unread = [DiffGroupSetting(title: "Changed files", paths: nil)]

    /// The setting's final value as `lf` resolved it; nil when it is not a list of groups.
    static func parse(_ value: JSONValue) -> [DiffGroupSetting]? {
        guard case .array(let entries) = value else { return nil }
        let groups = entries.compactMap { entry -> DiffGroupSetting? in
            guard case .object(let fields) = entry else { return nil }
            if let rest = fields["rest"] {
                if case .string(let title) = rest { return DiffGroupSetting(title: title, paths: nil) }
                return DiffGroupSetting(title: restTitle, paths: nil)
            }
            guard case .string(let title)? = fields["title"], case .array(let paths)? = fields["paths"] else { return nil }
            return DiffGroupSetting(title: title, paths: paths.compactMap { if case .string(let path) = $0 { path } else { nil } })
        }
        return groups.count == entries.count ? groups : nil
    }
}

/// Files the Diff shows together under one heading.
struct ReviewGroup: Equatable, Sendable, Identifiable {
    let id: String
    let title: String
    /// The title is the tour's reason for reading these files first.
    let isTour: Bool
    let files: [ReviewFile]

    /// Files in reading order, grouped. A tour names files; a file it does not
    /// name, or one added after it was written, follows by group, so the order
    /// can never lose a file.
    static func arrange(_ files: [ReviewFile], ordering: ReviewOrdering, tour: DiffTour?,
                        groups: [DiffGroupSetting]) -> [ReviewGroup] {
        switch ordering {
        case .path:
            let folders = Dictionary(grouping: files) { file in
                file.path.contains("/") ? String(file.path.prefix { $0 != "/" }) + "/" : ""
            }
            return folders.sorted { $0.key < $1.key }.map {
                ReviewGroup(id: "path:\($0.key)", title: $0.key.isEmpty ? "Top level" : $0.key, isTour: false,
                            files: $0.value.sorted { $0.path < $1.path })
            }
        case .groups:
            return byGroup(files, groups)
        case .tour:
            var rest = files
            var named: [ReviewGroup] = []
            for (index, entry) in (tour?.first ?? []).enumerated() {
                let files = byGroup(rest.filter { entry.matches($0.path) }, groups).flatMap(\.files)
                guard !files.isEmpty else { continue }
                rest.removeAll { entry.matches($0.path) }
                // An entry without a reason reads on under the one above it.
                if entry.why.isEmpty, let last = named.last {
                    named[named.count - 1] = ReviewGroup(id: last.id, title: last.title, isTour: true, files: last.files + files)
                } else {
                    named.append(ReviewGroup(id: "tour:\(index)", title: entry.why, isTour: true, files: files))
                }
            }
            return named + byGroup(rest, groups)
        }
    }

    /// Each file under the first group a pattern matches, else under the rest;
    /// a list with no rest puts those files last, under "Everything else".
    private static func byGroup(_ files: [ReviewFile], _ groups: [DiffGroupSetting]) -> [ReviewGroup] {
        let rest = groups.firstIndex { $0.paths == nil } ?? groups.count
        let placed = Dictionary(grouping: files) { file in
            groups.firstIndex { $0.paths?.contains { DiffPattern.matches($0, file.path) } == true } ?? rest
        }
        return placed.sorted { $0.key < $1.key }.map { index, files in
            ReviewGroup(id: "group:\(index)", title: groups.indices.contains(index) ? groups[index].title : DiffGroupSetting.restTitle,
                        isTour: false, files: files.sorted { $0.path < $1.path })
        }
    }
}
#endif
