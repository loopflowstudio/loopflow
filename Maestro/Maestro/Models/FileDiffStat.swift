// File-level diff statistics for displaying change summaries.

import Foundation

struct FileDiffStat: Identifiable, Equatable {
    let id: String  // File path
    let path: String
    let additions: Int
    let deletions: Int

    var filename: String {
        if let lastSlash = path.lastIndex(of: "/") {
            return String(path[path.index(after: lastSlash)...])
        }
        return path
    }

    var directory: String {
        if let lastSlash = path.lastIndex(of: "/") {
            return String(path[...lastSlash])
        }
        return ""
    }

    var totalChanges: Int {
        additions + deletions
    }

    var fileExtension: String {
        if let lastDot = filename.lastIndex(of: ".") {
            return String(filename[filename.index(after: lastDot)...]).lowercased()
        }
        return ""
    }
}
