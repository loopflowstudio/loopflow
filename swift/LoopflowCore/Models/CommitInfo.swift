// Commit information for git log display.

import Foundation

public struct CommitInfo: Sendable, Identifiable, Equatable {
    public let id: String  // Full SHA
    public let shortSHA: String
    public let message: String
    public let author: String
    public let date: Date

    public var relativeTime: String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}
