import Foundation

public struct WaveContent: Sendable, Equatable, Hashable {
    public var vision: String?
    public var strategy: String?
    public var goals: String?
    public var risks: String?
    public var metrics: String?
    public var roadmapItems: [RoadmapItem]
    public var scratchDoc: String?
    public var scratchDocPath: String?

    public init(
        vision: String? = nil,
        strategy: String? = nil,
        goals: String? = nil,
        risks: String? = nil,
        metrics: String? = nil,
        roadmapItems: [RoadmapItem] = [],
        scratchDoc: String? = nil,
        scratchDocPath: String? = nil
    ) {
        self.vision = vision
        self.strategy = strategy
        self.goals = goals
        self.risks = risks
        self.metrics = metrics
        self.roadmapItems = roadmapItems
        self.scratchDoc = scratchDoc
        self.scratchDocPath = scratchDocPath
    }
}

public struct RoadmapItem: Sendable, Identifiable, Equatable, Hashable {
    public var id: String
    public var number: Int
    public var title: String
    public var isShipped: Bool
    public var content: String?
    public var filePath: String?

    public init(
        id: String,
        number: Int,
        title: String,
        isShipped: Bool,
        content: String? = nil,
        filePath: String? = nil
    ) {
        self.id = id
        self.number = number
        self.title = title
        self.isShipped = isShipped
        self.content = content
        self.filePath = filePath
    }
}
