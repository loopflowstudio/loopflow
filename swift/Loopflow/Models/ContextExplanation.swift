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

/// What `lf task run` would do: run `graph` from `from` until the interactive
/// step `stopsAt` (or to the end), put the Task at the interactive step `step`
/// running nothing, or reach `end` when nothing remains.
public enum TaskRunAction: Codable, Equatable, Sendable {
    case run(graph: String, takeUp: Bool, from: String, stopsAt: String?)
    case wait(graph: String, takeUp: Bool, step: String)
    case end(graph: String, takeUp: Bool)

    private enum CodingKeys: String, CodingKey {
        case kind, graph, from, step
        case takeUp = "take_up"
        case stopsAt = "stops_at"
    }
    private enum Kind: String, Codable { case run, wait, end }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        let graph = try values.decode(String.self, forKey: .graph)
        let takeUp = try values.decode(Bool.self, forKey: .takeUp)
        switch try values.decode(Kind.self, forKey: .kind) {
        case .run:
            self = .run(graph: graph, takeUp: takeUp,
                        from: try values.decode(String.self, forKey: .from),
                        stopsAt: try values.decode(String?.self, forKey: .stopsAt))
        case .wait:
            self = .wait(graph: graph, takeUp: takeUp, step: try values.decode(String.self, forKey: .step))
        case .end:
            self = .end(graph: graph, takeUp: takeUp)
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .run(let graph, let takeUp, let from, let stopsAt):
            try values.encode(Kind.run, forKey: .kind)
            try values.encode(graph, forKey: .graph)
            try values.encode(takeUp, forKey: .takeUp)
            try values.encode(from, forKey: .from)
            try values.encode(stopsAt, forKey: .stopsAt)
        case .wait(let graph, let takeUp, let step):
            try values.encode(Kind.wait, forKey: .kind)
            try values.encode(graph, forKey: .graph)
            try values.encode(takeUp, forKey: .takeUp)
            try values.encode(step, forKey: .step)
        case .end(let graph, let takeUp):
            try values.encode(Kind.end, forKey: .kind)
            try values.encode(graph, forKey: .graph)
            try values.encode(takeUp, forKey: .takeUp)
        }
    }
}

public struct TaskRunExplanation: Codable, Equatable, Sendable {
    public let resolution: ContextExplanation
    public let action: TaskRunAction?
    public let impediments: [String]
    public let unavailable: [String]
}

/// `lf open --link --json`: a readable Work link resolved by Rust's shared owners.
public struct WorkLink: Codable, Equatable, Sendable {
    public enum Relation: String, Codable, Equatable, Sendable {
        case currentConversation = "current_conversation"
        case otherConversation = "other_conversation"
        case unknown
    }

    public enum Host: String, Codable, Equatable, Sendable {
        case auto, desktop, cmux, herdr, rex, terminal
    }

    public struct Target: Codable, Equatable, Sendable {
        public let kind: WorkWorkspaceKey.Kind
        public let repositoryPath: String
        public let repositoryId: String?
        public let task: String?
        public let taskId: String?
        public let wave: String?
        public let session: String?
        public let diff: Bool
        public let executionMachine: String?

        enum CodingKeys: String, CodingKey {
            case kind, task, wave, session, diff
            case repositoryPath = "repository_path", repositoryId = "repository_id"
            case taskId = "task_id", executionMachine = "execution_machine"
        }
    }

    public struct Route: Codable, Equatable, Sendable {
        public let hosts: [Host]
        public let reason: String
    }

    public let label: String
    public let url: String
    public let markdown: String
    public let fallbackArgv: [String]
    public let target: Target
    public let conversation: Relation
    public let conversationReason: String?
    public let route: Route

    enum CodingKeys: String, CodingKey {
        case label, url, markdown, target, conversation, route
        case fallbackArgv = "fallback_argv", conversationReason = "conversation_reason"
    }
}

public enum SessionResumeIntent: String, Codable, Equatable, Sendable {
    case start, resume
    case takeOver = "take_over"
    case newThread = "new_thread"
}

public struct SessionResumeAction: Codable, Equatable, Sendable {
    public let intent: SessionResumeIntent
    /// The selected agent, else the conversation's recorded provider.
    public let agent: String?
    /// A new conversation continues from this one; its holder keeps it.
    public let fork: Bool
    public let prepareOnly: Bool

    enum CodingKeys: String, CodingKey {
        case intent, agent, fork
        case prepareOnly = "prepare_only"
    }
}

public struct SessionResumeExplanation: Codable, Equatable, Sendable {
    public let resolution: ContextExplanation
    public let action: SessionResumeAction?
    public let state: SessionState?
    public let actions: [SessionAction]
    public let impediments: [String]
    public let unavailable: [String]
}

public enum TaskPlanningAction: Codable, Equatable, Sendable {
    case create(title: String, description: String, project: String?)
    case edit(revision: UInt64, fields: [String])
    case comment(message: String?, steer: Bool, refresh: Bool)
    case move(wave: String?, project: String?, previousProject: String?)
    case save(path: String, revision: String, draftBytes: UInt64)

    private enum CodingKeys: String, CodingKey {
        case kind, title, description, project, revision, fields, message, steer, refresh, wave, path
        case previousProject = "previous_project"
        case draftBytes = "draft_bytes"
    }
    private enum Kind: String, Codable { case create, edit, comment, move, save }

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
        case .move:
            self = .move(wave: try values.decodeIfPresent(String.self, forKey: .wave),
                           project: try values.decodeIfPresent(String.self, forKey: .project),
                           previousProject: try values.decodeIfPresent(String.self, forKey: .previousProject))
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
        case .move(let wave, let project, let previousProject):
            try values.encode(Kind.move, forKey: .kind)
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
