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

public struct WavePlan: Sendable, Hashable {
    public var objective: String
    public var chapter: ChapterSummary?

    public init(objective: String, chapter: ChapterSummary? = nil) {
        self.objective = objective
        self.chapter = chapter
    }
}
