// File-level diff statistics for displaying change summaries.

import Foundation

public struct FileDiffStat: Sendable, Identifiable, Equatable {
    public let id: String  // File path
    public let path: String
    public let additions: Int
    public let deletions: Int

    public var filename: String {
        if let lastSlash = path.lastIndex(of: "/") {
            return String(path[path.index(after: lastSlash)...])
        }
        return path
    }

    public var directory: String {
        if let lastSlash = path.lastIndex(of: "/") {
            return String(path[...lastSlash])
        }
        return ""
    }

    public var totalChanges: Int {
        additions + deletions
    }

    public var fileExtension: String {
        if let lastDot = filename.lastIndex(of: ".") {
            return String(filename[filename.index(after: lastDot)...]).lowercased()
        }
        return ""
    }
}
