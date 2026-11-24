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
    public let trackerSyncedAt: Int64?

    enum CodingKeys: String, CodingKey {
        case observedAt = "observed_at", repositoryPath = "repository_path"
        case executionMachine = "execution_machine", trackerSyncedAt = "tracker_synced_at"
        case machine, repository, checkout, wave, task, session, process
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        observedAt = try values.decode(Int64.self, forKey: .observedAt)
        machine = try values.decode(ContextFact.self, forKey: .machine)
        repository = try values.decode(ContextFact.self, forKey: .repository)
        repositoryPath = try values.decode(ContextFact.self, forKey: .repositoryPath)
        checkout = try values.decode(ContextFact.self, forKey: .checkout)
        executionMachine = try values.decode(ContextFact.self, forKey: .executionMachine)
        wave = try values.decode(ContextFact.self, forKey: .wave)
        task = try values.decode(ContextFact.self, forKey: .task)
        session = try values.decode(ContextFact.self, forKey: .session)
        process = try values.decode(ContextFact.self, forKey: .process)
        trackerSyncedAt = try values.decode(Int64?.self, forKey: .trackerSyncedAt)
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(observedAt, forKey: .observedAt)
        try values.encode(machine, forKey: .machine)
        try values.encode(repository, forKey: .repository)
        try values.encode(repositoryPath, forKey: .repositoryPath)
        try values.encode(checkout, forKey: .checkout)
        try values.encode(executionMachine, forKey: .executionMachine)
        try values.encode(wave, forKey: .wave)
        try values.encode(task, forKey: .task)
        try values.encode(session, forKey: .session)
        try values.encode(process, forKey: .process)
        try values.encode(trackerSyncedAt, forKey: .trackerSyncedAt)
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
        public let executionMachine: String?

        enum CodingKeys: String, CodingKey {
            case kind, task, wave, session
            case repositoryPath = "repo_path", repositoryId = "repo_id"
            case taskId = "task_id", executionMachine = "execution_machine"
        }
    }

    public struct Route: Codable, Equatable, Sendable {
        public let host: Host?
        public let reason: String

        enum CodingKeys: String, CodingKey { case host, reason }

        public init(host: Host?, reason: String) {
            self.host = host
            self.reason = reason
        }

        public init(from decoder: Decoder) throws {
            let values = try decoder.container(keyedBy: CodingKeys.self)
            host = try values.decode(Host?.self, forKey: .host)
            reason = try values.decode(String.self, forKey: .reason)
        }

        public func encode(to encoder: Encoder) throws {
            var values = encoder.container(keyedBy: CodingKeys.self)
            try values.encode(host, forKey: .host)
            try values.encode(reason, forKey: .reason)
        }
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
    /// A live held terminal gains a view.
    case view
    /// A provider running in a native client lf can't share; that client ends.
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
    /// What opening it does; nil when it can't open.
    public let opening: SessionAction?
    public let impediments: [String]
    public let unavailable: [String]

    enum CodingKeys: String, CodingKey { case resolution, action, state, opening, impediments, unavailable }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        resolution = try values.decode(ContextExplanation.self, forKey: .resolution)
        action = try values.decode(SessionResumeAction?.self, forKey: .action)
        state = try values.decode(SessionState?.self, forKey: .state)
        opening = try values.decode(SessionAction?.self, forKey: .opening)
        impediments = try values.decode([String].self, forKey: .impediments)
        unavailable = try values.decode([String].self, forKey: .unavailable)
    }
    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(resolution, forKey: .resolution)
        try values.encode(action, forKey: .action)
        try values.encode(state, forKey: .state)
        try values.encode(opening, forKey: .opening)
        try values.encode(impediments, forKey: .impediments)
        try values.encode(unavailable, forKey: .unavailable)
    }
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
