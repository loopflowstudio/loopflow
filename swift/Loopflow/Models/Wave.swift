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
///
/// Beyond identity and lifecycle status, the row carries the few shared facts
/// the Mac surface spends space on: whether a body is `live`, how much work is
/// active, and the `parentWaveId` that future ancestry will indent under. All
/// come straight from `WaveSnapshot` (`lf ls --json`); this is an app model, not
/// a wire DTO, so the defaults keep non-registry call sites terse.
public struct Wave: Sendable, Identifiable, Hashable {
    public let id: String
    public let name: String
    public let repo: String
    public let status: WaveStatus
    public let live: Bool
    public let activeTasks: Int
    public let activeProjects: Int
    public let parentWaveId: String?

    public init(
        id: String,
        name: String,
        repo: String,
        status: WaveStatus,
        live: Bool = false,
        activeTasks: Int = 0,
        activeProjects: Int = 0,
        parentWaveId: String? = nil
    ) {
        self.id = id
        self.name = name
        self.repo = repo
        self.status = status
        self.live = live
        self.activeTasks = activeTasks
        self.activeProjects = activeProjects
        self.parentWaveId = parentWaveId
    }
}
