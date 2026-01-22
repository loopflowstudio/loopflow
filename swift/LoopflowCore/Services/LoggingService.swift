import Foundation

public enum LoggingService {
    public enum Category: String {
        case worktrees
        case lfd
        case general
    }

    public static func append(_ message: String, category: Category = .worktrees) {
        do {
            let url = logURL(for: category)
            try ensureLogDirectory(for: url)
            let timestamp = ISO8601DateFormatter().string(from: Date())
            let line = "\(timestamp) \(message.trimmingCharacters(in: .whitespacesAndNewlines))\n"
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
