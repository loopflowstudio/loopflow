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

public enum SessionConnectIntent: String, Codable, Equatable, Sendable {
    case start, resume
    case connectOrResume = "connect_or_resume"
}

public enum SessionOpenMode: String, Codable, Equatable, Sendable {
    case refuse, replace, `try`
}

public struct SessionConnectAction: Codable, Equatable, Sendable {
    public let intent: SessionConnectIntent
    public let mode: SessionOpenMode
    public let prepareOnly: Bool

    enum CodingKeys: String, CodingKey {
        case intent, mode
        case prepareOnly = "prepare_only"
    }
}

public struct SessionConnectExplanation: Codable, Equatable, Sendable {
    public let resolution: ContextExplanation
    public let action: SessionConnectAction?
    public let state: SessionState?
    public let actions: [SessionAction]
    public let impediments: [String]
    public let unavailable: [String]
}

public struct TaskMoveAction: Codable, Equatable, Sendable {
    public let workflow: String?
    public let from: Workflow.Position?
    public let to: String
    public let reason: String?
    public let force: Bool
}

public struct TaskMoveExplanation: Codable, Equatable, Sendable {
    public let resolution: ContextExplanation
    public let action: TaskMoveAction?
    public let impediments: [String]
    public let unavailable: [String]
}

public enum TaskPlanningAction: Codable, Equatable, Sendable {
    case create(title: String, description: String, project: String?)
    case edit(revision: UInt64, fields: [String])
    case comment(message: String?, steer: Bool, refresh: Bool)
    case refile(wave: String, project: String, previousProject: String)
    case save(path: String, revision: String, draftBytes: UInt64)

    private enum CodingKeys: String, CodingKey {
        case kind, title, description, project, revision, fields, message, steer, refresh, wave, path
        case previousProject = "previous_project"
        case draftBytes = "draft_bytes"
    }
    private enum Kind: String, Codable { case create, edit, comment, refile, save }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(Kind.self, forKey: .kind) {
        case .create:
            self = .create(title: try values.decode(String.self, forKey: .title),
                           description: try values.decode(String.self, forKey: .description),
                           project: try values.decodeIfPresent(String.self, forKey: .project))
        case .edit:
            self = .edit(revision: try values.decode(UInt64.self, forKey: .revision),
                         fields: try values.decode([String].self, forKey: .fields))
        case .refile:
            self = .refile(wave: try values.decode(String.self, forKey: .wave),
                           project: try values.decode(String.self, forKey: .project),
                           previousProject: try values.decode(String.self, forKey: .previousProject))
        case .save:
            self = .save(path: try values.decode(String.self, forKey: .path),
                         revision: try values.decode(String.self, forKey: .revision),
                         draftBytes: try values.decode(UInt64.self, forKey: .draftBytes))
        case .comment:
            self = .comment(message: try values.decodeIfPresent(String.self, forKey: .message),
                            steer: try values.decode(Bool.self, forKey: .steer),
                            refresh: try values.decode(Bool.self, forKey: .refresh))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .create(let title, let description, let project):
            try values.encode(Kind.create, forKey: .kind)
            try values.encode(title, forKey: .title)
            try values.encode(description, forKey: .description)
            try values.encode(project, forKey: .project)
        case .edit(let revision, let fields):
            try values.encode(Kind.edit, forKey: .kind)
            try values.encode(revision, forKey: .revision)
            try values.encode(fields, forKey: .fields)
        case .refile(let wave, let project, let previousProject):
            try values.encode(Kind.refile, forKey: .kind)
            try values.encode(wave, forKey: .wave)
            try values.encode(project, forKey: .project)
            try values.encode(previousProject, forKey: .previousProject)
        case .save(let path, let revision, let draftBytes):
            try values.encode(Kind.save, forKey: .kind)
            try values.encode(path, forKey: .path)
            try values.encode(revision, forKey: .revision)
            try values.encode(draftBytes, forKey: .draftBytes)
        case .comment(let message, let steer, let refresh):
            try values.encode(Kind.comment, forKey: .kind)
            try values.encode(message, forKey: .message)
            try values.encode(steer, forKey: .steer)
            try values.encode(refresh, forKey: .refresh)
        }
    }
}

public struct TaskPlanningExplanation: Codable, Equatable, Sendable {
    public let resolution: ContextExplanation
    public let action: TaskPlanningAction?
    public let effects: [String]
    public let impediments: [String]
    public let unavailable: [String]
}

public struct TaskCheckoutAction: Codable, Equatable, Sendable {
    public enum Behavior: String, Codable, Sendable { case prepare, reuse, restore }
    public let behavior: Behavior
    public let path: String
    public let branch: String
    public let stackOn: String?

    enum CodingKeys: String, CodingKey {
        case behavior, path, branch
        case stackOn = "stack_on"
    }
}

public struct TaskCheckoutExplanation: Codable, Equatable, Sendable {
    public let resolution: ContextExplanation
    public let action: TaskCheckoutAction?
    public let impediments: [String]
    public let unavailable: [String]
}
