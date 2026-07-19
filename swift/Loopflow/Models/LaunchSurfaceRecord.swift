import Foundation

public struct LaunchRouteRecord: Codable, Sendable, Hashable {
    public let provider: String
    public let model: String?
    public let accountId: String?

    enum CodingKeys: String, CodingKey {
        case provider, model
        case accountId = "account_id"
    }
}

public enum LaunchContainmentRecord: Codable, Sendable, Hashable {
    case processGroup(id: Int64)
    case tmux(name: String)

    private enum CodingKeys: String, CodingKey { case kind, id, name }
    private enum Kind: String, Codable { case processGroup = "process_group", tmux }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(Kind.self, forKey: .kind) {
        case .processGroup:
            self = .processGroup(id: try values.decode(Int64.self, forKey: .id))
        case .tmux:
            self = .tmux(name: try values.decode(String.self, forKey: .name))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .processGroup(let id):
            try values.encode(Kind.processGroup, forKey: .kind)
            try values.encode(id, forKey: .id)
        case .tmux(let name):
            try values.encode(Kind.tmux, forKey: .kind)
            try values.encode(name, forKey: .name)
        }
    }
}

public enum LaunchLifecycleState: String, Codable, Sendable, Hashable {
    case starting, live, stopping, ended

    public var isActive: Bool { self != .ended }
}

public enum LaunchBoundaryState: String, Codable, Sendable, Hashable {
    case starting, active, succeeded, failed, interrupted, unknown
}

public struct LaunchRecord: Codable, Sendable, Hashable {
    public let id: String
    public let runId: String
    public let homeId: String
    public let route: LaunchRouteRecord
    public let cwd: String
    public let surface: String
    public let state: LaunchLifecycleState
    public let containment: LaunchContainmentRecord
    public let opaqueBasis: WorkBasis?
    public let resumeToken: String?
    public let startedAt: String
    public let endedAt: String?

    enum CodingKeys: String, CodingKey {
        case id, route, cwd, surface, state, containment
        case runId = "run_id"
        case homeId = "home_id"
        case opaqueBasis = "opaque_basis"
        case resumeToken = "resume_token"
        case startedAt = "started_at"
        case endedAt = "ended_at"
    }
}

public struct LaunchAttentionRecord: Codable, Sendable, Hashable {
    public let kind: String
    public let work: WorkReference?
}

/// `lf launch list|attach --json`. Reopening reads this projection and does not
/// mutate Launch liveness; explicit handback is a separate control.
public struct LaunchSurfaceRecord: Codable, Sendable, Hashable, Identifiable {
    public let launch: LaunchRecord
    public let work: WorkReference
    public let waveId: String
    public let homeRoute: String
    public let attention: LaunchAttentionRecord?
    public let attentionAt: String?
    public let handback: LaunchBoundaryState?
    public let attachArgv: [String]?

    public var id: String { launch.id }
    public var sessionId: String { launch.id }
    public var parentKind: String { work.kind.rawValue }
    public var parentId: String { work.id }
    public var status: LaunchLifecycleState { launch.state }
    public var provider: String { launch.route.provider }
    public var providerSessionId: String? { launch.resumeToken }
    public var home: String { homeRoute }
    public var host: String { homeRoute }
    public var cwd: String { launch.cwd }
    public var reason: String {
        attention?.kind == "user" && attentionAt != nil ? "User attention" : launch.surface
    }
    public var createdAt: String { launch.startedAt }
    public var updatedAt: String { launch.endedAt ?? launch.startedAt }
    public var ageSecs: Int? { nil }
    public var environment: [String: String] { [:] }
    public var argv: [String] { attachArgv ?? [] }

    enum CodingKeys: String, CodingKey {
        case launch, work, attention, handback
        case waveId = "wave_id"
        case homeRoute = "home_route"
        case attentionAt = "attention_at"
        case attachArgv = "attach_argv"
    }
}
