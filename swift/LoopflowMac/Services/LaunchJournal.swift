// LaunchJournal — timings of real launches and their `lf` reads, kept in the
// Home so a report needs no trace and no staged benchmark.
//
// Two bounded NDJSON files under `<Home>/desktop-cache/timings/`: `launches`
// holds one line per launch milestone, `reads` one line per `lf` read and per
// workspace refresh. Lines carry durations, verbs and the app version; never
// arguments, output or error text. Each append is one write on a utility queue.
// Report: scripts/benchmarks/desktop-performance/timings.py.

import Foundation
import QuartzCore

final class LaunchJournal: @unchecked Sendable {
    /// Each milestone is written once per launch; later windows add nothing.
    enum Milestone: String {
        /// The saved workspace was looked up; `cache` says hit or miss.
        case restored
        /// The first window's content was committed to the render server.
        /// Not on-glass presentation.
        case firstFrame = "first_frame"
        /// Outline rows were observed; `source` says saved or fresh.
        case usable
        /// Every part of the workspace was read by this launch.
        case fresh
        /// A refresh failed before the workspace was fresh; `part` names it.
        case refreshFailed = "refresh_failed"
    }

    /// A file past this size keeps its newer half.
    static let maxBytes = 256 * 1024

    private let directory: URL
    private let queue = DispatchQueue(label: "studio.loopflow.launch-journal", qos: .utility)
    private var launch: String?
    private var app = ""
    private var marked: Set<Milestone> = []
    private var sizes: [String: Int] = [:]

    init(directory: URL) {
        self.directory = directory
    }

    /// The journal of the Home this process's `lf` reads resolve to. It writes
    /// nothing until the app calls `begin`, so tests and fixture runs stay silent.
    static let home = LaunchJournal(
        directory: WorkCache.homeDirectory.appendingPathComponent("timings", isDirectory: true))

    static var appVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "dev"
    }

    /// Start this process's launch record.
    func begin(app: String = LaunchJournal.appVersion, preMainMs: Double? = Perf.millisecondsSinceProcessStart()) {
        let at = ISO8601DateFormatter().string(from: Date())
        queue.async { [self] in
            guard launch == nil else { return }
            let id = UUID().uuidString
            launch = id
            self.app = app
            var line: [String: Any] = ["event": "launch", "launch": id, "app": app, "at": at]
            if let preMainMs { line["ms"] = Self.rounded(preMainMs) }
            append(line, to: "launches")
        }
    }

    /// Milliseconds are measured from kernel process start, so they include
    /// everything before `main`.
    func mark(_ milestone: Milestone, _ detail: [String: String] = [:],
              ms: Double? = Perf.millisecondsSinceProcessStart()) {
        queue.async { [self] in write(milestone, detail, ms: ms) }
    }

    private func write(_ milestone: Milestone, _ detail: [String: String], ms: Double?) {
        guard let launch, let ms, marked.insert(milestone).inserted else { return }
        var line: [String: Any] = ["event": milestone.rawValue, "launch": launch, "app": app, "ms": Self.rounded(ms)]
        for (key, value) in detail { line[key] = value }
        append(line, to: "launches")
    }

    /// Mark once the current Core Animation transaction has been committed.
    @MainActor
    func markAfterCommit(_ milestone: Milestone) {
        CATransaction.setCompletionBlock { [self] in mark(milestone) }
    }

    /// One `lf` read. Only the subcommand words are kept, never flags or values.
    func read(_ args: [String], ms: Double, ok: Bool) {
        let verb = args.prefix(2).filter { !$0.hasPrefix("-") }.joined(separator: " ")
        queue.async { [self] in record("read", "verb", verb, ms: ms, ok: ok) }
    }

    /// One workspace refresh of `part`. A failure before the launch is fresh
    /// is also that launch's `refresh_failed`.
    func refreshed(_ part: String, ms: Double, ok: Bool) {
        let sinceStart = Perf.millisecondsSinceProcessStart()
        queue.async { [self] in
            record("refresh", "part", part, ms: ms, ok: ok)
            if !ok, !marked.contains(.fresh) { write(.refreshFailed, ["part": part], ms: sinceStart) }
        }
    }

    /// Wait for queued lines; tests only.
    func flush() { queue.sync {} }

    private func record(_ event: String, _ key: String, _ value: String, ms: Double, ok: Bool) {
        guard launch != nil else { return }
        append(["event": event, key: value, "ms": Self.rounded(ms), "ok": ok, "app": app], to: "reads")
    }

    private static func rounded(_ ms: Double) -> Double { (ms * 10).rounded() / 10 }

    private func append(_ line: [String: Any], to name: String) {
        guard var data = try? JSONSerialization.data(withJSONObject: line, options: [.sortedKeys]) else { return }
        data.append(0x0A)
        let url = directory.appendingPathComponent("\(name).ndjson", isDirectory: false)
        if sizes[name] == nil {
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            if !FileManager.default.fileExists(atPath: url.path) {
                FileManager.default.createFile(atPath: url.path, contents: nil)
            }
            sizes[name] = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int) ?? 0
        }
        // A journal that cannot be written loses timings, nothing else.
        guard let handle = try? FileHandle(forWritingTo: url) else { return }
        defer { try? handle.close() }
        guard (try? handle.seekToEnd()) != nil, (try? handle.write(contentsOf: data)) != nil else { return }
        sizes[name, default: 0] += data.count
        if sizes[name, default: 0] > Self.maxBytes { trim(url, name: name) }
    }

    private func trim(_ url: URL, name: String) {
        guard let data = try? Data(contentsOf: url) else { return }
        let tail = data.suffix(Self.maxBytes / 2)
        // Start at a whole line.
        let kept = tail.firstIndex(of: 0x0A).map { Data(tail[tail.index(after: $0)...]) } ?? Data()
        guard (try? kept.write(to: url, options: .atomic)) != nil else { return }
        sizes[name] = kept.count
    }
}
