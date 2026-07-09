import Foundation

public enum KeyResultProof: String, Sendable, Hashable {
    case open
    case holds
}

public struct WaveKeyResult: Sendable, Identifiable, Hashable {
    public let id: String
    public var text: String
    public var proof: KeyResultProof

    public init(text: String, proof: KeyResultProof = .open) {
        self.id = text
        self.text = text
        self.proof = proof
    }
}

public struct WaveProject: Sendable, Identifiable, Hashable {
    public let id: String
    public var title: String
    public var summary: String?
    public var krs: [WaveKeyResult]

    public init(id: String, title: String, summary: String? = nil, krs: [WaveKeyResult] = []) {
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
