import Foundation

public struct WaveContent: Sendable, Equatable, Hashable {
    public var vision: String?
    public var goals: String?
    public var risks: String?
    public var metrics: String?
    public var roadmapItems: [RoadmapItem]

    public init(
        vision: String? = nil,
        goals: String? = nil,
        risks: String? = nil,
        metrics: String? = nil,
        roadmapItems: [RoadmapItem] = []
    ) {
        self.vision = vision
        self.goals = goals
        self.risks = risks
        self.metrics = metrics
        self.roadmapItems = roadmapItems
    }
}

public struct RoadmapItem: Sendable, Identifiable, Equatable, Hashable {
    public var id: String
    public var number: Int
    public var title: String
    public var isShipped: Bool

    public init(id: String, number: Int, title: String, isShipped: Bool) {
        self.id = id
        self.number = number
        self.title = title
        self.isShipped = isShipped
    }
}
