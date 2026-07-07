import Foundation

public struct WaveProject: Sendable, Identifiable, Hashable {
    public let id: String
    public var title: String
    public var summary: String?
    public var krs: [String]

    public init(id: String, title: String, summary: String? = nil, krs: [String] = []) {
        self.id = id
        self.title = title
        self.summary = summary
        self.krs = krs
    }
}

public struct WavePlan: Sendable, Hashable {
    public var objective: String
    public var projects: [WaveProject]

    public init(objective: String, projects: [WaveProject] = []) {
        self.objective = objective
        self.projects = projects
    }
}
