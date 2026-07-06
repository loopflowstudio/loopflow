import Foundation

public struct WaveContent: Sendable, Equatable, Hashable {
    public var vision: String?
    public var strategy: String?
    public var goals: String?
    public var risks: String?
    public var metrics: String?
    public var scratchDoc: String?
    public var scratchDocPath: String?

    public init(
        vision: String? = nil,
        strategy: String? = nil,
        goals: String? = nil,
        risks: String? = nil,
        metrics: String? = nil,
        scratchDoc: String? = nil,
        scratchDocPath: String? = nil
    ) {
        self.vision = vision
        self.strategy = strategy
        self.goals = goals
        self.risks = risks
        self.metrics = metrics
        self.scratchDoc = scratchDoc
        self.scratchDocPath = scratchDocPath
    }
}
