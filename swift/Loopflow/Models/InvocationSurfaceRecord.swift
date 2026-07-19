import Foundation

public struct InvocationRouteRecord: Codable, Sendable, Hashable {
    public let provider: String
    public let model: String?
    public let accountId: String?

    enum CodingKeys: String, CodingKey {
        case provider, model
        case accountId = "account_id"
    }
}

public enum RunContainmentRecord: Codable, Sendable, Hashable {
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

public enum RunLifecycleStateRecord: String, Codable, Sendable, Hashable {
    case reserved, active, stopping, ended

    public var isActive: Bool { self != .ended }
}

public enum RunTriggerRecord: Codable, Sendable, Hashable {
    case input(basis: WorkBasis)
    case time(scheduledAt: String)
    case event(WorkEventReference)
    case child(WorkReference)
    case ciIncident(id: String)
    case recovery(priorRunId: String)
    case user

    private enum CodingKeys: String, CodingKey {
        case kind, basis, event, work
        case scheduledAt = "scheduled_at"
        case incidentId = "incident_id"
        case priorRunId = "prior_run_id"
    }

    private enum Kind: String, Codable {
        case input, time, event, child
        case ciIncident = "ci_incident"
        case recovery, user
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(Kind.self, forKey: .kind) {
        case .input:
            self = .input(basis: try values.decode(WorkBasis.self, forKey: .basis))
        case .time:
            self = .time(scheduledAt: try values.decode(String.self, forKey: .scheduledAt))
        case .event:
            self = .event(try values.decode(WorkEventReference.self, forKey: .event))
        case .child:
            self = .child(try values.decode(WorkReference.self, forKey: .work))
        case .ciIncident:
            self = .ciIncident(id: try values.decode(String.self, forKey: .incidentId))
        case .recovery:
            self = .recovery(priorRunId: try values.decode(String.self, forKey: .priorRunId))
        case .user:
            self = .user
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .input(let basis):
            try values.encode(Kind.input, forKey: .kind)
            try values.encode(basis, forKey: .basis)
        case .time(let scheduledAt):
            try values.encode(Kind.time, forKey: .kind)
            try values.encode(scheduledAt, forKey: .scheduledAt)
        case .event(let event):
            try values.encode(Kind.event, forKey: .kind)
            try values.encode(event, forKey: .event)
        case .child(let work):
            try values.encode(Kind.child, forKey: .kind)
            try values.encode(work, forKey: .work)
        case .ciIncident(let id):
            try values.encode(Kind.ciIncident, forKey: .kind)
            try values.encode(id, forKey: .incidentId)
        case .recovery(let priorRunId):
            try values.encode(Kind.recovery, forKey: .kind)
            try values.encode(priorRunId, forKey: .priorRunId)
        case .user:
            try values.encode(Kind.user, forKey: .kind)
        }
    }
}

public struct RunRecord: Codable, Sendable, Hashable {
    public let id: String
    public let work: WorkReference
    public let epochId: String
    public let homeId: String
    public let state: RunLifecycleStateRecord
    public let trigger: RunTriggerRecord
    public let retryOf: String?
    public let containment: RunContainmentRecord?
    public let cwd: String?
    public let createdAt: String
    public let startedAt: String?
    public let endedAt: String?

    enum CodingKeys: String, CodingKey {
        case id, work, state, trigger, containment, cwd
        case epochId = "epoch_id"
        case homeId = "home_id"
        case retryOf = "retry_of"
        case createdAt = "created_at"
        case startedAt = "started_at"
        case endedAt = "ended_at"
    }
}

public enum InvocationBoundaryStateRecord: String, Codable, Sendable, Hashable {
    case starting, active, succeeded, failed, interrupted, unknown
}

public struct AgentInvocationRecord: Codable, Sendable, Hashable {
    public let id: String
    public let supervisingRunId: String?
    public let route: InvocationRouteRecord
    public let surface: String
    public let resumeToken: String?
    public let startedAt: String
    public let endedAt: String?

    enum CodingKeys: String, CodingKey {
        case id, route, surface
        case supervisingRunId = "supervising_run_id"
        case resumeToken = "resume_token"
        case startedAt = "started_at"
        case endedAt = "ended_at"
    }
}

public struct InvocationAttentionRecord: Codable, Sendable, Hashable {
    public let kind: String
    public let work: WorkReference?
}

/// `lf invocation list|attach --json`. Reopening reads this projection and does
/// not mutate Run liveness; explicit handback remains a temporary Slice 1 API.
public struct InvocationSurfaceRecord: Codable, Sendable, Hashable, Identifiable {
    public let invocation: AgentInvocationRecord
    public let run: RunRecord
    public let work: WorkReference
    public let waveId: String
    public let homeRoute: String
    public let attention: InvocationAttentionRecord?
    public let attentionAt: String?
    public let handback: InvocationBoundaryStateRecord?
    public let attachArgv: [String]?

    public var id: String { invocation.id }
    public var invocationId: String { invocation.id }
    public var parentKind: String { work.kind.rawValue }
    public var parentId: String { work.id }
    public var status: RunLifecycleStateRecord { run.state }
    public var provider: String { invocation.route.provider }
    public var providerSessionId: String? { invocation.resumeToken }
    public var home: String { homeRoute }
    public var host: String { homeRoute }
    public var cwd: String { run.cwd ?? "" }
    public var reason: String {
        attention?.kind == "user" && attentionAt != nil ? "User attention" : invocation.surface
    }
    public var createdAt: String { run.createdAt }
    public var updatedAt: String { run.endedAt ?? invocation.endedAt ?? invocation.startedAt }
    public var ageSecs: Int? { nil }
    public var environment: [String: String] { [:] }
    public var argv: [String] { attachArgv ?? [] }

    enum CodingKeys: String, CodingKey {
        case invocation, run, work, attention, handback
        case waveId = "wave_id"
        case homeRoute = "home_route"
        case attentionAt = "attention_at"
        case attachArgv = "attach_argv"
    }
}
