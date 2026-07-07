import Foundation

public enum WaveTaskStatus: String, Sendable, Hashable, Codable {
    case todo
    case inProgress
    case done
}

public struct WaveTask: Sendable, Identifiable, Hashable {
    public let id: String
    public var title: String
    public var status: WaveTaskStatus
    public var pr: URL?

    public init(id: String, title: String, status: WaveTaskStatus, pr: URL? = nil) {
        self.id = id
        self.title = title
        self.status = status
        self.pr = pr
    }
}

public struct WaveProject: Sendable, Identifiable, Hashable {
    public let id: String
    public var title: String
    public var summary: String?
    public var krs: [String]
    public var tasks: [WaveTask]

    public init(
        id: String,
        title: String,
        summary: String? = nil,
        krs: [String] = [],
        tasks: [WaveTask] = []
    ) {
        self.id = id
        self.title = title
        self.summary = summary
        self.krs = krs
        self.tasks = tasks
    }
}

public struct WavePlan: Sendable, Hashable {
    public var objective: String
    public var projects: [WaveProject]
    public var tasks: [WaveTask]
    public var runs: [Run]

    public init(
        objective: String,
        projects: [WaveProject] = [],
        tasks: [WaveTask] = [],
        runs: [Run] = []
    ) {
        self.objective = objective
        self.projects = projects
        self.tasks = tasks
        self.runs = runs
    }
}
