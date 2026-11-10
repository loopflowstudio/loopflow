import Foundation

/// Complete payloads for screen reads. Typed clients request them without `select`.
public struct PlanBoard: Decodable, Sendable, Hashable {
    public let tasks: WorkEvidence<PlanBoardTask>
    public let generatedAt: String

    enum CodingKeys: String, CodingKey {
        case tasks
        case generatedAt = "generated_at"
    }
}

public struct PlanBoardTask: Decodable, Sendable, Hashable, Identifiable {
    public let id: String
    public let identifier: String
    public let title: String
    public let section: TaskSection
    public let state: TaskState
    public let pr: BoardPr?
    public let waitingOn: WorkNextMove?

    enum CodingKeys: String, CodingKey {
        case id, identifier, title, section, state, pr
        case waitingOn = "waiting_on"
    }

    public init(from decoder: Decoder) throws {
        let box = try decoder.container(keyedBy: CodingKeys.self)
        id = try box.decode(String.self, forKey: .id)
        identifier = try box.decode(String.self, forKey: .identifier)
        title = try box.decode(String.self, forKey: .title)
        section = try box.decode(TaskSection.self, forKey: .section)
        state = try box.decode(TaskState.self, forKey: .state)
        pr = try box.decode(BoardPr?.self, forKey: .pr)
        waitingOn = try box.decode(WorkNextMove?.self, forKey: .waitingOn)
    }
}

public struct BoardPr: Decodable, Sendable, Hashable {
    public let url: URL
}

public struct TaskDetail: Decodable, Sendable, Hashable {
    public let task: TaskPlanningSnapshot
    public let section: TaskSection
    public let state: TaskState
    public let pr: PrSnapshot?
    public let checks: TaskChecks?
    public let graphProcesses: [GraphProcessDetail]
    public let comments: TaskComments?
    public let currentSession: SessionRecord?

    enum CodingKeys: String, CodingKey {
        case task, section, state, pr, checks, comments
        case graphProcesses = "graph_processes"
        case currentSession = "current_session"
    }

    public init(from decoder: Decoder) throws {
        let box = try decoder.container(keyedBy: CodingKeys.self)
        task = try box.decode(TaskPlanningSnapshot.self, forKey: .task)
        section = try box.decode(TaskSection.self, forKey: .section)
        state = try box.decode(TaskState.self, forKey: .state)
        pr = try box.decode(PrSnapshot?.self, forKey: .pr)
        checks = try box.decode(TaskChecks?.self, forKey: .checks)
        graphProcesses = try box.decode([GraphProcessDetail].self, forKey: .graphProcesses)
        comments = try box.decode(TaskComments?.self, forKey: .comments)
        currentSession = try box.decode(SessionRecord?.self, forKey: .currentSession)
    }
}

public struct TaskChecks: Decodable, Sendable, Hashable {
    public enum State: String, Decodable, Sendable, Hashable {
        case pending, passing, failing
    }

    public struct Check: Decodable, Sendable, Hashable {
        public let name: String
        public let url: URL?

        enum CodingKeys: String, CodingKey { case name, url }

        public init(from decoder: Decoder) throws {
            let box = try decoder.container(keyedBy: CodingKeys.self)
            name = try box.decode(String.self, forKey: .name)
            url = try box.decode(URL?.self, forKey: .url)
        }
    }

    public let headSha: String
    public let state: State
    public let failingChecks: [Check]
    public let observedAt: String

    enum CodingKeys: String, CodingKey {
        case state
        case headSha = "head_sha"
        case failingChecks = "failing_checks"
        case observedAt = "observed_at"
    }
}

public struct SessionCard: Decodable, Sendable, Hashable {
    public enum Liveness: String, Decodable, Sendable, Hashable {
        case live, dead, unknown
    }

    public let session: SessionRecord
    public let liveness: Liveness
    public let controlHolder: String?
    public let lastLine: String?

    enum CodingKeys: String, CodingKey {
        case session, liveness
        case controlHolder = "control_holder"
        case lastLine = "last_line"
    }

    public init(from decoder: Decoder) throws {
        let box = try decoder.container(keyedBy: CodingKeys.self)
        session = try box.decode(SessionRecord.self, forKey: .session)
        liveness = try box.decode(Liveness.self, forKey: .liveness)
        controlHolder = try box.decode(String?.self, forKey: .controlHolder)
        lastLine = try box.decode(String?.self, forKey: .lastLine)
    }
}
