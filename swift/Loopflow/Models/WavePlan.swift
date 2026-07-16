import Foundation

public enum KeyResultProof: String, Sendable, Hashable {
    case open
    case holds
}

public struct WaveKeyResult: Sendable, Identifiable, Hashable {
    public let id: String
    public let claimId: String?
    public var text: String
    public var proof: KeyResultProof
    public var receipts: [Receipt]

    public init(
        text: String,
        claimId: String? = nil,
        proof: KeyResultProof = .open,
        receipts: [Receipt] = []
    ) {
        self.id = text
        self.claimId = claimId
        self.text = text
        self.proof = proof
        self.receipts = receipts
    }
}

public struct WaveProject: Sendable, Identifiable, Hashable {
    public let id: String
    public let claimId: String?
    public var title: String
    public var definition: String?
    public var krs: [WaveKeyResult]
    public var receipts: [Receipt]

    public init(
        id: String,
        claimId: String? = nil,
        title: String,
        definition: String? = nil,
        krs: [WaveKeyResult] = [],
        receipts: [Receipt] = []
    ) {
        self.id = id
        self.claimId = claimId
        self.title = title
        self.definition = definition
        self.krs = krs
        self.receipts = receipts
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
