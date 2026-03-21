import Foundation

public enum RoadmapPriority: Int, Sendable, CaseIterable, Equatable, Hashable {
    case urgent = 1
    case high = 2
    case medium = 3
    case low = 4

    public var displayName: String {
        switch self {
        case .urgent: return "Urgent"
        case .high: return "High"
        case .medium: return "Medium"
        case .low: return "Low"
        }
    }

    public var filenamePrefix: String {
        String(rawValue)
    }

    public static func from(prefix: String) -> RoadmapPriority? {
        switch prefix.trimmingCharacters(in: .whitespacesAndNewlines) {
        case "1", "01": return .urgent
        case "2", "02": return .high
        case "3", "03": return .medium
        case "4", "04": return .low
        default: return nil
        }
    }
}

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
    public var slug: String
    public var fileName: String
    public var priority: RoadmapPriority
    public var isShipped: Bool
    public var content: String?
    public var filePath: String?

    public init(
        id: String,
        number: Int,
        title: String,
        slug: String,
        fileName: String,
        priority: RoadmapPriority,
        isShipped: Bool,
        content: String? = nil,
        filePath: String? = nil
    ) {
        self.id = id
        self.number = number
        self.title = title
        self.slug = slug
        self.fileName = fileName
        self.priority = priority
        self.isShipped = isShipped
        self.content = content
        self.filePath = filePath
    }
}
