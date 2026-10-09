import Foundation

public enum ContextFact: Codable, Equatable, Sendable {
    case bound(value: String, source: String)
    case unbound
    case unavailable(reason: String)

    private enum CodingKeys: String, CodingKey { case state, value, source, reason }
    private enum State: String, Codable { case bound, unbound, unavailable }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(State.self, forKey: .state) {
        case .bound:
            self = .bound(value: try values.decode(String.self, forKey: .value),
                          source: try values.decode(String.self, forKey: .source))
        case .unbound: self = .unbound
        case .unavailable: self = .unavailable(reason: try values.decode(String.self, forKey: .reason))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .bound(let value, let source):
            try values.encode(State.bound, forKey: .state)
            try values.encode(value, forKey: .value)
            try values.encode(source, forKey: .source)
        case .unbound: try values.encode(State.unbound, forKey: .state)
        case .unavailable(let reason):
            try values.encode(State.unavailable, forKey: .state)
            try values.encode(reason, forKey: .reason)
        }
    }
}

public struct ContextExplanation: Codable, Equatable, Sendable {
    public let observedAt: Int64
    public let machine: ContextFact
    public let repository: ContextFact
    public let repositoryPath: ContextFact
    public let checkout: ContextFact
    public let executionMachine: ContextFact
    public let wave: ContextFact
    public let task: ContextFact
    public let session: ContextFact
    public let process: ContextFact
    public let planningObservedAt: Int64?

    enum CodingKeys: String, CodingKey {
        case observedAt = "observed_at", repositoryPath = "repository_path"
        case executionMachine = "execution_machine", planningObservedAt = "planning_observed_at"
        case machine, repository, checkout, wave, task, session, process
    }
}

public enum TaskRunAction: Codable, Equatable, Sendable {
    case edge(workflow: String, takeUp: Bool, from: String, to: String, flow: String?)
    case flow(String)

    private enum CodingKeys: String, CodingKey {
        case kind, workflow, from, to, flow
        case takeUp = "take_up"
    }
    private enum Kind: String, Codable { case edge, flow }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(Kind.self, forKey: .kind) {
        case .edge:
            self = .edge(workflow: try values.decode(String.self, forKey: .workflow),
                         takeUp: try values.decode(Bool.self, forKey: .takeUp),
                         from: try values.decode(String.self, forKey: .from),
                         to: try values.decode(String.self, forKey: .to),
                         flow: try values.decodeIfPresent(String.self, forKey: .flow))
        case .flow: self = .flow(try values.decode(String.self, forKey: .flow))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .edge(let workflow, let takeUp, let from, let to, let flow):
            try values.encode(Kind.edge, forKey: .kind)
            try values.encode(workflow, forKey: .workflow)
            try values.encode(takeUp, forKey: .takeUp)
            try values.encode(from, forKey: .from)
            try values.encode(to, forKey: .to)
            try values.encode(flow, forKey: .flow)
        case .flow(let flow):
            try values.encode(Kind.flow, forKey: .kind)
            try values.encode(flow, forKey: .flow)
        }
    }
}

public struct TaskRunExplanation: Codable, Equatable, Sendable {
    public let resolution: ContextExplanation
    public let action: TaskRunAction?
    public let impediments: [String]
    public let unavailable: [String]
}

public struct DesktopOpenExplanation: Codable, Equatable, Sendable {
    public let resolution: ContextExplanation
    public let url: String?
    public let impediments: [String]
    public let unavailable: [String]
}
