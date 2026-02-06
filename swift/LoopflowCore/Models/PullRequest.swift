// PullRequest - metadata for a single PR, typically attached to a WaveRun.

import Foundation

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
