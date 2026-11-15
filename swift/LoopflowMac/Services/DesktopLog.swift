// DesktopLog — Desktop's diagnostics, in the same `logs/lf.log` every lf process
// writes, in lf's line shape, so one file answers "what happened around then":
//
//   2026-10-29T14:27:25.606Z  WARN loopflow::desktop: <message> k=v … pid=52901 desktop=0.15.1 rev=b11130060
//
// Exit facts live in the launch journal; this file carries what explains them:
// stacks, vitals, hangs and failed reads. Each line is one O_APPEND write on a
// utility queue. A write that fails is dropped; logging never takes the app down.

import Foundation

final class DesktopLog: @unchecked Sendable {
    enum Level: String {
        case info = " INFO"
        case warn = " WARN"
        case error = "ERROR"
    }

    let url: URL
    private let queue = DispatchQueue(label: "studio.loopflow.desktop-log", qos: .utility)
    private let identity: String
    private var started = false

    init(url: URL, app: String = LaunchJournal.appVersion, revision: String = DesktopLog.sourceRevision) {
        self.url = url
        identity = "pid=\(getpid()) desktop=\(app) rev=\(revision)"
    }

    /// `logs/lf.log` in `LF_HOME`, else `~/.lf`: where this process's `lf` reads log.
    /// It writes nothing until the app calls `begin`, so tests and fixture runs stay silent.
    static let home = DesktopLog(url: WorkCache.homeDirectory.deletingLastPathComponent()
        .appendingPathComponent("logs", isDirectory: true)
        .appendingPathComponent("lf.log", isDirectory: false))

    /// The short commit this bundle was built from, stamped by `scripts/bundle_version.py`.
    static var sourceRevision: String {
        String((Bundle.main.object(forInfoDictionaryKey: "LoopflowSourceRevision") as? String ?? "unknown").prefix(9))
    }

    func begin() { queue.sync { started = true } }

    func info(_ message: String, _ fields: KeyValuePairs<String, String> = [:]) { write(.info, message, fields) }
    func warn(_ message: String, _ fields: KeyValuePairs<String, String> = [:]) { write(.warn, message, fields) }
    func error(_ message: String, _ fields: KeyValuePairs<String, String> = [:]) { write(.error, message, fields) }

    func write(_ level: Level, _ message: String, _ fields: KeyValuePairs<String, String>) {
        let line = format(level, message, fields, at: Date())
        queue.async { [self] in if started { Self.append(line, to: url.path) } }
    }

    /// Write before returning; for the paths that end the process.
    func writeNow(_ level: Level, _ message: String, _ fields: KeyValuePairs<String, String> = [:]) {
        let line = format(level, message, fields, at: Date())
        queue.sync { if started { Self.append(line, to: url.path) } }
    }

    /// Wait for queued lines; tests only.
    func flush() { queue.sync {} }

    func format(_ level: Level, _ message: String, _ fields: KeyValuePairs<String, String>, at date: Date) -> String {
        var line = "\(Self.timestamp(date)) \(level.rawValue) loopflow::desktop: \(Self.oneLine(message))"
        for (key, value) in fields { line += " \(key)=\(Self.quoted(value))" }
        return line + " \(identity)\n"
    }

    private static func timestamp(_ date: Date) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter.string(from: date)
    }

    private static func oneLine(_ text: String) -> String {
        text.replacingOccurrences(of: "\n", with: " ⏎ ")
    }

    /// Bare when it reads as one token, else a quoted string as tracing writes it.
    private static func quoted(_ value: String) -> String {
        if !value.isEmpty, !value.contains(where: { $0.isWhitespace || $0 == "\"" || $0 == "=" }) { return value }
        let escaped = value.replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
            .replacingOccurrences(of: "\n", with: "\\n")
        return "\"\(escaped)\""
    }

    private static func append(_ line: String, to path: String) {
        let directory = (path as NSString).deletingLastPathComponent
        try? FileManager.default.createDirectory(atPath: directory, withIntermediateDirectories: true)
        let fd = open(path, O_WRONLY | O_APPEND | O_CREAT | O_CLOEXEC, 0o644)
        guard fd >= 0 else { return }
        defer { close(fd) }
        let bytes = Array(line.utf8)
        _ = bytes.withUnsafeBytes { Darwin.write(fd, $0.baseAddress, $0.count) }
    }
}
