import Foundation

enum LoggingService {
    static func append(_ message: String) {
        do {
            let url = logURL()
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

    static func read() -> String {
        guard let data = try? Data(contentsOf: logURL()) else {
            return ""
        }
        return String(data: data, encoding: .utf8) ?? ""
    }

    static func logPath() -> String {
        logURL().path()
    }

    private static func logURL() -> URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library")
            .appendingPathComponent("Logs")
            .appendingPathComponent("Maestro")
            .appendingPathComponent("worktrees.log")
    }

    private static func ensureLogDirectory(for url: URL) throws {
        let dir = url.deletingLastPathComponent()
        if !FileManager.default.fileExists(atPath: dir.path()) {
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        }
    }
}
