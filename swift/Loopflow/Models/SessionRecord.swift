import Foundation

public enum SessionState: String, Codable, Sendable, Hashable {
    /// Metadata has no trustworthy live/closed observation.
    case unknown
    case active
    /// Its agent runs, but no terminal shows it.
    case detached
    case closed
}

public enum SessionAvailability: String, Codable, Sendable, Hashable {
    case available

    public var label: String {
        switch self {
        case .available: "Available"
        }
    }
}

/// Who chose a Session's title. Rust never lets a generated suggestion
/// replace a human-assigned name. `unavailable` means the canonical name lives
/// on another Machine; the title is only a local label.
public enum SessionTitleSource: String, Codable, Sendable, Hashable {
    case generated
    case human
    case unavailable
}

/// Whether a Session is an occurrence of a Graph. Rust owns
/// this from the Graph position or the Session's recorded capture; Swift never
/// infers it from Task, checkout, provider, or skill.
public enum SessionGraphMembership: Codable, Sendable, Hashable {
    case step(graph: String, graphLfProcessId: String, step: String, node: UInt32?, iterations: [[UInt32]]?,
              occurrence: SessionGraphOccurrence)
    case independent
    case unknown(reason: String)

    private enum Kind: String, Codable { case step, independent, unknown }

    enum CodingKeys: String, CodingKey {
        case kind, graph, step, iterations, occurrence, reason
        case graphLfProcessId = "graph_lf_process_id"
        case node
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(Kind.self, forKey: .kind) {
        case .step:
            self = .step(
                graph: try container.decode(String.self, forKey: .graph),
                graphLfProcessId: StructuredIdentity.uuid(try container.decode(String.self, forKey: .graphLfProcessId)),
                step: try container.decode(String.self, forKey: .step),
                node: try container.decodeIfPresent(UInt32.self, forKey: .node),
                iterations: try container.decodeIfPresent([[UInt32]].self, forKey: .iterations),
                occurrence: try container.decode(SessionGraphOccurrence.self, forKey: .occurrence)
            )
        case .independent:
            self = .independent
        case .unknown:
            self = .unknown(reason: try container.decode(String.self, forKey: .reason))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .step(graph, graphLfProcessId, step, node, iterations, occurrence):
            try container.encode(Kind.step, forKey: .kind)
            try container.encode(graph, forKey: .graph)
            try container.encode(graphLfProcessId, forKey: .graphLfProcessId)
            try container.encode(step, forKey: .step)
            try container.encode(node, forKey: .node)
            try container.encode(iterations, forKey: .iterations)
            try container.encode(occurrence, forKey: .occurrence)
        case .independent:
            try container.encode(Kind.independent, forKey: .kind)
        case .unknown(let reason):
            try container.encode(Kind.unknown, forKey: .kind)
            try container.encode(reason, forKey: .reason)
        }
    }

    /// Short human label: "pursue / implement · pass 3", "Independent".
    public var label: String {
        switch self {
        case let .step(graph, _, step, _, iterations, occurrence):
            let base = "\(graph) / \(step)"
            let label: String
            if let iterations {
                label = graphIterationLabel(iterations).map { "\(base) · \($0)" } ?? base
            } else {
                label = "\(base) · pass unavailable"
            }
            switch occurrence {
            case .unknown: return "\(label) · position unavailable"
            case .current: return label
            case .earlier: return "\(label) · earlier"
            case .past: return "\(label) · past run"
            }
        case .independent:
            return "Independent"
        case .unknown:
            return "Graph membership unknown"
        }
    }
}

/// Where a Graph occurrence sits relative to its Graph's current position.
public enum SessionGraphOccurrence: String, Codable, Sendable, Hashable {
    /// Known membership without a recorded selected occurrence.
    case unknown
    /// The invocation's current position.
    case current
    /// An earlier position of the invocation that is still active.
    case earlier
    /// An invocation that has finished or been followed by a newer one.
    case past
}

/// The one thing opening a Session does here: Reconnect, Open here, Take over,
/// Try again or Start conversation.
public struct SessionAction: Codable, Sendable, Hashable {
    public let label: String
    public let help: String
}

public struct SessionWorkspace: Codable, Sendable, Hashable {
    public var identity: WorkspaceIdentity { WorkspaceIdentity(machineId: machineId, worktree: worktree) }
    public let machineId: String
    public let worktree: String
    public let taskId: String?
    public let unavailable: String?

    enum CodingKeys: String, CodingKey {
        case worktree, unavailable
        case machineId = "machine_id"
        case taskId = "task_id"
    }
}

extension SessionWorkspace {
    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        machineId = StructuredIdentity.prefixed(try values.decode(String.self, forKey: .machineId), "home_")
        worktree = try values.decode(String.self, forKey: .worktree)
        taskId = (try values.decodeIfPresent(String.self, forKey: .taskId)).map { StructuredIdentity.prefixed($0, "task_") }
        unavailable = try values.decodeIfPresent(String.self, forKey: .unavailable)
    }
}

/// One resumable Session and the exact command that opens it.
/// Rust owns completion, GraphStep decisions, and provider-client liveness.
public struct SessionPreview: Codable, Hashable, Sendable {
    public let providerThread: String
    public let providerTurn: String
    public let observedAt: Int64
    public let source: String
    public let text: String

    enum CodingKeys: String, CodingKey {
        case providerThread = "provider_thread"
        case providerTurn = "provider_turn"
        case observedAt = "observed_at"
        case source, text
    }
}

public enum ProviderTransitionKind: String, Codable, Sendable, Hashable {
    case retained, resumed, forked, started
}

public struct ProviderTransition: Codable, Sendable, Hashable {
    public let kind: ProviderTransitionKind
    public let fromProviderSession: String?
    public let toProviderSession: String
    public let fromAgentProcess: AgentProcessId?
    public let toAgentProcess: AgentProcessId
    public let originalRunning: Bool?
    public let seed: String?
    public let observedAt: Int64

    enum CodingKeys: String, CodingKey {
        case kind, seed
        case fromProviderSession = "from_provider_session", toProviderSession = "to_provider_session"
        case fromAgentProcess = "from_agent_process", toAgentProcess = "to_agent_process"
        case originalRunning = "original_running", observedAt = "observed_at"
    }
}

public struct SessionRecord: Codable, Sendable, Hashable, Identifiable {
    public let transition: ProviderTransition?
    public let preview: SessionPreview?
    public let primaryScope: String?
    public let availability: SessionAvailability?
    public let programStatus: ProgramStatusRecords?
    /// The provider's current AgentProcess; absent when Loopflow never launched one.
    public let agentProcessId: AgentProcessId?
    /// The Task's primary conversation: its row is marked for its Task.
    public var taskPrimary: Bool { primaryScope == "task" }
    public let taskIds: [String]
    public let id: String
    public let interactive: Bool
    public let work: WorkReference?
    public var workspace: SessionWorkspace?
    public let waveId: String?
    public let workPath: String?
    /// What opening it here does; nil for a Run.
    public let action: SessionAction?
    public var title: String
    public let titleSource: SessionTitleSource
    public let graphMembership: SessionGraphMembership
    public let detail: String
    /// Provider harness recorded on the Session's capture manifest; nil when the
    /// capture is on another Machine or its manifest is unreadable.
    public let provider: String?
    /// Historical attribution: the saved login its conversation last recorded; never the current native login.
    public let account: String?
    /// The model its current input launched with, when one was named.
    public let model: String?
    public let cwd: String
    public let state: SessionState
    public var openArgv: [String]
    public let terminalIds: [String]
    /// Its current or last turn while it is live; nil when nothing says.
    public let turn: SessionTurn?
    /// Open here but never launched by Loopflow; `id` is the provider's conversation ID.
    public let raw: Bool
    /// How its last agent ended, while none runs; nil for a headless Session or before a first start.
    public let ended: SessionEnd?
    /// The server's starts of a Task's conversation since it last finished a turn.
    public let start: SessionStarts?
    /// Whether it comes back while it is down, and what brings it back; nil while it runs.
    public let outlook: SessionOutlook?
    /// Whether it runs, how it ended and its outlook, as the server's one sentence; printed as given.
    public let statusLine: String?

    /// One word for a row: Available for a person wins over what its client is doing.
    public var statusLabel: String {
        if state != .closed, let availability { return availability.label }
        switch state {
        case .active: return "Active"
        case .detached: return "Detached"
        case .closed: return "Closed"
        case .unknown: return interactive ? "Session" : "Run"
        }
    }

    /// Membership is supplied by the shared Task/checkout query.
    public func names(task id: String) -> Bool { taskIds.contains(id) }

    enum CodingKeys: String, CodingKey {
        case id, interactive, work, workspace, title, detail, provider, account, model, cwd, state, availability
        case preview, transition
        case programStatus = "program_status"
        case agentProcessId = "agent_process_id"
        case primaryScope = "primary_scope"
        case waveId = "wave_id"
        case taskIds = "task_ids"
        case action
        case titleSource = "title_source"
        case graphMembership = "graph_membership"
        case workPath = "work_path"
        case openArgv = "open_argv"
        case terminalIds = "terminal_ids"
        case turn, raw, ended, start, outlook
        case statusLine = "status_line"
    }
}

/// A conversation's last agent end.
public struct SessionEnd: Codable, Sendable, Hashable {
    /// RFC 3339.
    public let at: String
    public let exitCode: Int?

    enum CodingKeys: String, CodingKey {
        case at
        case exitCode = "exit_code"
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(at, forKey: .at)
        try values.encode(exitCode, forKey: .exitCode)
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        at = try values.decode(String.self, forKey: .at)
        exitCode = try values.decode(Int?.self, forKey: .exitCode)
    }
}

/// What the server's recovery pass did for a Task conversation.
public struct SessionStarts: Codable, Sendable, Hashable {
    /// Starts since it last finished a turn.
    public let attempts: Int
    /// RFC 3339: the pass's last start.
    public let lastStartedAt: String?

    enum CodingKeys: String, CodingKey {
        case attempts
        case lastStartedAt = "last_started_at"
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(attempts, forKey: .attempts)
        try values.encode(lastStartedAt, forKey: .lastStartedAt)
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        attempts = try values.decode(Int.self, forKey: .attempts)
        lastStartedAt = try values.decode(String?.self, forKey: .lastStartedAt)
    }
}

/// Whether a conversation that is not running comes up, and what brings it.
/// The server works it out, and writes `SessionRecord.statusLine` from it.
public struct SessionOutlook: Codable, Sendable, Hashable {
    public enum Kind: String, Codable, Sendable, Hashable {
        case starting
        case comingBack = "coming_back"
        case waitingForYou = "waiting_for_you"
        case quit
        case couldNotStart = "could_not_start"
    }

    public let kind: Kind
    /// RFC 3339: when the server starts it next; a time already past is a start under way.
    public let at: String?
    /// Ready to print: why it could not start, why its last start failed, or why the server leaves it.
    public let reason: String?
    /// The command that opens it; nil while the server brings it back.
    public let command: String?

    enum CodingKeys: String, CodingKey {
        case kind, at, reason, command
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(kind, forKey: .kind)
        try values.encode(at, forKey: .at)
        try values.encode(reason, forKey: .reason)
        try values.encode(command, forKey: .command)
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        kind = try values.decode(Kind.self, forKey: .kind)
        at = try values.decode(String?.self, forKey: .at)
        reason = try values.decode(String?.self, forKey: .reason)
        command = try values.decode(String?.self, forKey: .command)
    }
}

/// A conversation's current or last turn, as its provider's transcript shows it.
public struct SessionTurn: Codable, Sendable, Hashable {
    public enum State: String, Codable, Sendable, Hashable {
        case running, ended, interrupted
    }

    public struct Usage: Codable, Sendable, Hashable {
        /// Every input token, cached ones included.
        public let inputTokens: Int
        public let cachedInputTokens: Int
        public let outputTokens: Int

        enum CodingKeys: String, CodingKey {
            case inputTokens = "input_tokens"
            case cachedInputTokens = "cached_input_tokens"
            case outputTokens = "output_tokens"
        }
    }

    /// The provider's name for the turn.
    public let id: String?
    public let state: State
    public let startedAt: String?
    public let endedAt: String?
    public let model: String?
    public let usage: Usage?

    enum CodingKeys: String, CodingKey {
        case id, state, model, usage
        case startedAt = "started_at"
        case endedAt = "ended_at"
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(id, forKey: .id)
        try values.encode(state, forKey: .state)
        try values.encode(startedAt, forKey: .startedAt)
        try values.encode(endedAt, forKey: .endedAt)
        try values.encode(model, forKey: .model)
        try values.encode(usage, forKey: .usage)
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decode(String?.self, forKey: .id)
        state = try values.decode(State.self, forKey: .state)
        startedAt = try values.decode(String?.self, forKey: .startedAt)
        endedAt = try values.decode(String?.self, forKey: .endedAt)
        model = try values.decode(String?.self, forKey: .model)
        usage = try values.decode(Usage?.self, forKey: .usage)
    }
}

/// A Task's open conversations as its page orders them: the ones available for a
/// person first, its primary leading either group. Closed ones are not listed.
public struct TaskSessionGroups: Sendable, Equatable {
    public let available: [SessionRecord]
    public let working: [SessionRecord]

    public init(_ sessions: [SessionRecord]) {
        let open = sessions.filter { $0.state != .closed }
        let primaryFirst = open.filter(\.taskPrimary) + open.filter { !$0.taskPrimary }
        available = primaryFirst.filter { $0.availability == .available }
        working = primaryFirst.filter { $0.availability != .available }
    }
}

/// What `lf session move --json` reports: Sessions moved now, Sessions that
/// move after their turn, and Sessions skipped with why.
public struct SessionMoveReport: Codable, Sendable, Hashable {
    public struct Moved: Codable, Sendable, Hashable {
        public let id: String
        public let title: String
        public let how: String
    }

    public struct Skipped: Codable, Sendable, Hashable {
        public let id: String
        public let title: String
        public let reason: String
    }

    public let moved: [Moved]
    public let pending: [Moved]
    public let skipped: [Skipped]

    /// One line per outcome, as the Desktop shows after a move.
    public var summary: String {
        let lines = moved.map { "\($0.title): \($0.how)" } + pending.map { "\($0.title): \($0.how)" }
            + skipped.map { "\($0.title): skipped, \($0.reason)" }
        return lines.isEmpty ? "No blocked Sessions." : lines.joined(separator: "\n")
    }
}

/// A bounded inventory page; nil next marks a complete enumeration.
public struct SessionPage: Codable, Sendable, Hashable {
    public let entries: [SessionRecord]
    public let next: String?
}

extension SessionRecord {
    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(transition, forKey: .transition)
        try values.encode(preview, forKey: .preview)
        try values.encode(primaryScope, forKey: .primaryScope)
        try values.encode(availability, forKey: .availability)
        try values.encode(programStatus, forKey: .programStatus)
        try values.encode(agentProcessId, forKey: .agentProcessId)
        try values.encode(taskIds, forKey: .taskIds)
        try values.encode(id, forKey: .id)
        try values.encode(interactive, forKey: .interactive)
        try values.encode(work, forKey: .work)
        try values.encode(workspace, forKey: .workspace)
        try values.encode(waveId, forKey: .waveId)
        try values.encode(workPath, forKey: .workPath)
        try values.encode(action, forKey: .action)
        try values.encode(title, forKey: .title)
        try values.encode(titleSource, forKey: .titleSource)
        try values.encode(graphMembership, forKey: .graphMembership)
        try values.encode(detail, forKey: .detail)
        try values.encode(provider, forKey: .provider)
        try values.encode(account, forKey: .account)
        try values.encode(model, forKey: .model)
        try values.encode(cwd, forKey: .cwd)
        try values.encode(state, forKey: .state)
        try values.encode(openArgv, forKey: .openArgv)
        try values.encode(terminalIds, forKey: .terminalIds)
        try values.encode(turn, forKey: .turn)
        try values.encode(raw, forKey: .raw)
        try values.encode(ended, forKey: .ended)
        try values.encode(start, forKey: .start)
        try values.encode(outlook, forKey: .outlook)
        try values.encode(statusLine, forKey: .statusLine)
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        transition = try values.decodeIfPresent(ProviderTransition.self, forKey: .transition)
        preview = try values.decodeIfPresent(SessionPreview.self, forKey: .preview)
        primaryScope = try values.decodeIfPresent(String.self, forKey: .primaryScope)
        availability = try values.decodeIfPresent(SessionAvailability.self, forKey: .availability)
        programStatus = try values.decodeIfPresent(ProgramStatusRecords.self, forKey: .programStatus)
        agentProcessId = try values.decodeIfPresent(AgentProcessId.self, forKey: .agentProcessId)
        taskIds = (try values.decode([String].self, forKey: .taskIds)).map { StructuredIdentity.prefixed($0, "task_") }
        id = StructuredIdentity.session(try values.decode(String.self, forKey: .id))
        interactive = try values.decode(Bool.self, forKey: .interactive)
        work = try values.decodeIfPresent(WorkReference.self, forKey: .work)
        workspace = try values.decodeIfPresent(SessionWorkspace.self, forKey: .workspace)
        waveId = (try values.decodeIfPresent(String.self, forKey: .waveId)).map { StructuredIdentity.uuid($0) }
        workPath = try values.decodeIfPresent(String.self, forKey: .workPath)
        action = try values.decode(SessionAction?.self, forKey: .action)
        title = try values.decode(String.self, forKey: .title)
        titleSource = try values.decode(SessionTitleSource.self, forKey: .titleSource)
        graphMembership = try values.decode(SessionGraphMembership.self, forKey: .graphMembership)
        detail = try values.decode(String.self, forKey: .detail)
        provider = try values.decodeIfPresent(String.self, forKey: .provider)
        account = try values.decodeIfPresent(String.self, forKey: .account)
        model = try values.decodeIfPresent(String.self, forKey: .model)
        cwd = try values.decode(String.self, forKey: .cwd)
        state = try values.decode(SessionState.self, forKey: .state)
        openArgv = try values.decode([String].self, forKey: .openArgv)
        terminalIds = try values.decode([String].self, forKey: .terminalIds)
        turn = try values.decode(SessionTurn?.self, forKey: .turn)
        raw = try values.decode(Bool.self, forKey: .raw)
        ended = try values.decode(SessionEnd?.self, forKey: .ended)
        start = try values.decode(SessionStarts?.self, forKey: .start)
        outlook = try values.decode(SessionOutlook?.self, forKey: .outlook)
        statusLine = try values.decode(String?.self, forKey: .statusLine)
    }
}
