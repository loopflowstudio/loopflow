import Foundation

public enum LoggingService {
    public enum Category: String {
        case worktrees
        case lfd
        case general
        case ui       // User interactions: button clicks, selections
        case model    // Data model changes: waves, state updates
    }

    nonisolated(unsafe) private static let dateFormatter: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()

    public static func append(_ message: String, category: Category = .worktrees) {
        let timestamp = dateFormatter.string(from: Date())
        let line = "\(timestamp) [\(category.rawValue.uppercased())] \(message.trimmingCharacters(in: .whitespacesAndNewlines))\n"

        // Also print to console for immediate feedback during development
        print(line, terminator: "")

        do {
            let url = logURL(for: category)
            try ensureLogDirectory(for: url)
            let data = line.data(using: .utf8) ?? Data()
            if FileManager.default.fileExists(atPath: url.path()) {
                let handle = try FileHandle(forWritingTo: url)
                try handle.seekToEnd()
                try handle.write(contentsOf: data)
                try handle.close()
            } else {
                try data.write(to: url, options: .atomic)
            }
        } catch {
            // Avoid throwing from logging.
        }
    }

    // Convenience methods for common logging patterns
    public static func ui(_ message: String) {
        append(message, category: .ui)
    }

    public static func model(_ message: String) {
        append(message, category: .model)
    }

    public static func lfd(_ message: String) {
        append(message, category: .lfd)
    }

    public static func read(category: Category = .worktrees) -> String {
        guard let data = try? Data(contentsOf: logURL(for: category)) else {
            return ""
        }
        return String(data: data, encoding: .utf8) ?? ""
    }

    public static func logPath(category: Category = .worktrees) -> String {
        logURL(for: category).path()
    }

    public static func logDirectory() -> URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library")
            .appendingPathComponent("Logs")
            .appendingPathComponent("Concerto")
    }

    private static func logURL(for category: Category) -> URL {
        logDirectory().appendingPathComponent("\(category.rawValue).log")
    }

    private static func ensureLogDirectory(for url: URL) throws {
        let dir = url.deletingLastPathComponent()
        if !FileManager.default.fileExists(atPath: dir.path()) {
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        }
    }
}
