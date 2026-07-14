import SwiftUI

public enum WaveStatus: String, Sendable, Codable {
    case idle
    case running
    case paused

    public var color: Color {
        switch self {
        case .running: .statusSuccess
        case .idle, .paused: .statusNeutral
        }
    }

    public var icon: String {
        switch self {
        case .running: "circle.fill"
        case .paused: "pause.circle"
        case .idle: "circle"
        }
    }
}

/// A durable control plane for one repository. Projects and Tasks carry the
/// shipping state; a Wave itself has no worktree, branch, diff, or PR.
public struct Wave: Sendable, Identifiable, Hashable {
    public let id: String
    public let name: String
    public let repo: String
    public let status: WaveStatus

    public init(
        id: String,
        name: String,
        repo: String,
        status: WaveStatus
    ) {
        self.id = id
        self.name = name
        self.repo = repo
        self.status = status
    }
}
