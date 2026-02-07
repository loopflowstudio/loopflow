// PullRequest - metadata for a single PR, typically attached to a WaveRun.

import Foundation

public enum PRState: String, Sendable, Codable {
    case open
    case merged
    case closed
    case draft

    public var displayText: String {
        switch self {
        case .open: return "Open"
        case .merged: return "Merged"
        case .closed: return "Closed"
        case .draft: return "Draft"
        }
    }
}

public struct PullRequest: Sendable, Hashable, Codable {
    public let url: URL
    public let number: Int?
    public let state: PRState?
    public let title: String?
    public let branch: String?

    public init(
        url: URL,
        number: Int? = nil,
        state: PRState? = nil,
        title: String? = nil,
        branch: String? = nil
    ) {
        self.url = url
        self.number = number
        self.state = state
        self.title = title
        self.branch = branch
    }
}
