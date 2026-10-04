// WorkspaceCache — the last workspace a refresh showed, kept inside the Home it
// describes so a returning launch renders before any `lf` read.
//
// It holds the wire text of successful reads, restored through the decoder
// live reads use, so an incompatible file fails to decode instead of drifting.
// Saved text carries no Flow execution state, no Session state and no legal
// action beyond opening: a stale file cannot claim a process is live or
// authorize a mutation. It is display evidence, never authority; the first
// successful read of each part replaces it.

import Foundation
import Loopflow

struct WorkspaceSnapshot: Codable, Equatable, Sendable {
    struct Saved: Codable, Equatable, Sendable {
        var text: String
        var savedAt: Date
    }

    struct Repository: Codable, Equatable, Sendable {
        /// The default interactive Session listing, one wire page each.
        var sessionPages: [String]?
        var selection: WorkReference?
        var savedAt: Date
    }

    /// The Home these reads came from. A different Home invalidates everything.
    var homeId: String?
    var roadmap: Saved?
    var waves: Saved?
    /// Keyed by the repository path the window was scoped to.
    var repositories: [String: Repository] = [:]
}

final class WorkspaceCache: @unchecked Sendable {
    static let version = 1
    /// Most-recently saved repositories kept; older ones are dropped.
    static let maxRepositories = 8
    /// One read larger than this is not kept; the launch reads it instead.
    static let maxTextBytes = 8 * 1024 * 1024
    static let savedReason = "Saved from the last launch; updating…"

    private struct Envelope: Codable {
        var version: Int
        var snapshot: WorkspaceSnapshot
    }

    private let url: URL
    private let queue = DispatchQueue(label: "studio.loopflow.workspace-cache", qos: .utility)
    private var snapshot = WorkspaceSnapshot()
    /// Hashes of the last raw text per part, so an unchanged poll writes nothing.
    private var lastInput: [String: Int] = [:]

    init(directory: URL) {
        url = directory.appendingPathComponent("workspace.json", isDirectory: false)
    }

    /// The cache of the Home this process's `lf` reads resolve to.
    static func forHome(environment: [String: String] = ProcessInfo.processInfo.environment) -> WorkspaceCache {
        let home = environment["LF_HOME"].flatMap { $0.isEmpty ? nil : URL(fileURLWithPath: $0) }
            ?? URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent(".lf", isDirectory: true)
        return WorkspaceCache(directory: home.appendingPathComponent("desktop-cache", isDirectory: true))
    }

    /// The saved workspace, or `nil` when absent, corrupt or from another version.
    /// An unusable file is removed so the next save starts clean.
    func load() -> WorkspaceSnapshot? {
        queue.sync {
            guard let data = try? Data(contentsOf: url) else { return nil }
            guard let envelope = try? JSONDecoder().decode(Envelope.self, from: data),
                  envelope.version == Self.version else {
                try? FileManager.default.removeItem(at: url)
                return nil
            }
            snapshot = envelope.snapshot
            return snapshot
        }
    }

    func saveRoadmap(_ text: String, at date: Date = Date()) {
        update("roadmap", input: text) { $0.roadmap = Self.quietRoadmap(text).map { .init(text: $0, savedAt: date) } }
    }

    func saveWaves(_ text: String, at date: Date = Date()) {
        update("waves", input: text) { $0.waves = Self.bounded(text).map { .init(text: $0, savedAt: date) } }
    }

    func saveSessions(_ pages: [String], repo: String, at date: Date = Date()) {
        update("sessions:\(repo)", input: pages.joined(separator: "\n")) { snapshot in
            let quiet = pages.compactMap(Self.quietSessionPage)
            var entry = snapshot.repositories[repo] ?? .init(savedAt: date)
            entry.sessionPages = quiet.count == pages.count ? quiet : nil
            entry.savedAt = date
            snapshot.repositories[repo] = entry
        }
    }

    func saveSelection(_ selection: WorkReference?, repo: String, at date: Date = Date()) {
        update("selection:\(repo)", input: selection.map { "\($0.kind):\($0.id)" } ?? "") { snapshot in
            var entry = snapshot.repositories[repo] ?? .init(savedAt: date)
            entry.selection = selection
            entry.savedAt = date
            snapshot.repositories[repo] = entry
        }
    }

    /// Record the Home fresh reads come from. Text saved under another Home is discarded.
    func confirmHome(_ id: String) {
        queue.async { [self] in
            guard snapshot.homeId != id else { return }
            if snapshot.homeId != nil {
                snapshot = WorkspaceSnapshot()
                lastInput = [:]
            }
            snapshot.homeId = id
            write()
        }
    }

    /// Wait for queued saves; tests and orderly shutdown only.
    func flush() { queue.sync {} }

    private func update(_ part: String, input: String, _ change: @escaping @Sendable (inout WorkspaceSnapshot) -> Void) {
        queue.async { [self] in
            let hash = input.hashValue
            guard lastInput[part] != hash else { return }
            lastInput[part] = hash
            let previous = snapshot
            change(&snapshot)
            let kept = snapshot.repositories.sorted { $0.value.savedAt > $1.value.savedAt }.prefix(Self.maxRepositories)
            snapshot.repositories = Dictionary(uniqueKeysWithValues: kept.map { ($0.key, $0.value) })
            if snapshot != previous { write() }
        }
    }

    private func write() {
        do {
            let data = try JSONEncoder().encode(Envelope(version: Self.version, snapshot: snapshot))
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try data.write(to: url, options: .atomic)
        } catch {
            // A cache that cannot be written is only a slower next launch.
            try? FileManager.default.removeItem(at: url)
        }
    }

    // MARK: - Saved text carries no liveness or mutation authority

    private static func bounded(_ text: String) -> String? {
        text.utf8.count <= maxTextBytes ? text : nil
    }

    private static func rewrite(_ text: String, _ transform: (Any) -> Any) -> String? {
        guard bounded(text) != nil, let data = text.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data),
              let quiet = try? JSONSerialization.data(withJSONObject: transform(object)) else { return nil }
        return String(decoding: quiet, as: UTF8.self)
    }

    /// Every pinned Flow becomes `unknown` and every Flow control unavailable.
    static func quietRoadmap(_ text: String) -> String? {
        func quiet(_ value: Any) -> Any {
            if let array = value as? [Any] { return array.map(quiet) }
            guard var object = value as? [String: Any] else { return value }
            if object["kind"] as? String == "pinned", object["execution"] != nil {
                object["execution"] = TaskFlowExecution.unknown.rawValue
                object["reason"] = savedReason
            }
            if let controls = object["controls"] as? [[String: Any]] {
                object["controls"] = controls.map { control in
                    var control = control
                    control["unavailable"] = savedReason
                    return control
                }
            }
            return object.mapValues(quiet)
        }
        return rewrite(text, quiet)
    }

    /// Every Session becomes `unknown` and keeps only its open action; `lf`
    /// decides whether opening is still possible.
    static func quietSessionPage(_ text: String) -> String? {
        rewrite(text) { value in
            guard var page = value as? [String: Any], let entries = page["entries"] as? [[String: Any]] else { return value }
            page["entries"] = entries.map { entry in
                var entry = entry
                entry["state"] = SessionState.unknown.rawValue
                entry["actions"] = (entry["actions"] as? [[String: Any]] ?? []).filter {
                    $0["kind"] as? String == SessionActionKind.open.rawValue
                }
                return entry
            }
            return page
        }
    }
}

extension WorkspaceSnapshot.Repository {
    fileprivate init(savedAt: Date) { self.init(sessionPages: nil, selection: nil, savedAt: savedAt) }
}

/// Collects the wire text of the reads one refresh performs.
final class WireCapture: @unchecked Sendable {
    private let lock = NSLock()
    private var pages: [String] = []

    func record(_ args: [String], _ stdout: String) {
        lock.lock(); defer { lock.unlock() }
        pages.append(stdout)
    }

    var texts: [String] {
        lock.lock(); defer { lock.unlock() }
        return pages
    }
}
