// Commit information for git log display.

import Foundation

struct CommitInfo: Identifiable, Equatable {
    let id: String  // Full SHA
    let shortSHA: String
    let message: String
    let author: String
    let date: Date

    var relativeTime: String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}
