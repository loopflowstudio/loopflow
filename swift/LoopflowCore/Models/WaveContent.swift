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
        parseFilenamePrefix(prefix)?.priority
    }

    static func parseFilenamePrefix(_ prefix: String) -> (priority: Self, isCanonical: Bool)? {
        let normalized = prefix.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let value = Int(normalized), let priority = Self(rawValue: value) else {
            return nil
        }
        return (priority, normalized == priority.filenamePrefix)
    }
}

public struct WaveContent: Sendable, Equatable, Hashable {
    public var vision: String?
    public var strategy: String?
    public var goals: String?
    public var risks: String?
    public var metrics: String?
    public var roadmapTasks: [RoadmapTask]
    public var scratchDoc: String?
    public var scratchDocPath: String?

    public init(
        vision: String? = nil,
        strategy: String? = nil,
        goals: String? = nil,
        risks: String? = nil,
        metrics: String? = nil,
        roadmapTasks: [RoadmapTask] = [],
        scratchDoc: String? = nil,
        scratchDocPath: String? = nil
    ) {
        self.vision = vision
        self.strategy = strategy
        self.goals = goals
        self.risks = risks
        self.metrics = metrics
        self.roadmapTasks = roadmapTasks
        self.scratchDoc = scratchDoc
        self.scratchDocPath = scratchDocPath
    }
}

/// A wave discovered for a repo from its configured PM source.
/// For Asana-backed repos, every project in the configured team is a wave.
/// The sidebar shows these entries whether or not lfd has a store record.
public struct DiscoveredWaveSummary: Sendable, Equatable, Identifiable {
    public var repoPath: String
    public var repoId: String
    public var waveName: String
    public var provider: String
    public var asanaProjectId: String?
    public var managedWaveId: String?

    public var id: String { "\(repoPath)::\(waveName)" }
    public var isManaged: Bool { managedWaveId != nil }

    public init(
        repoPath: String,
        repoId: String,
        waveName: String,
        provider: String,
        asanaProjectId: String?,
        managedWaveId: String?
    ) {
        self.repoPath = repoPath
        self.repoId = repoId
        self.waveName = waveName
        self.provider = provider
        self.asanaProjectId = asanaProjectId
        self.managedWaveId = managedWaveId
    }
}

public struct RoadmapResponse: Sendable, Equatable {
    public var wave: String
    public var provider: String
    public var refreshedAt: Int64
    public var stale: Bool
    public var staleReason: String?
    public var tasks: [RoadmapTask]

    public init(
        wave: String,
        provider: String,
        refreshedAt: Int64,
        stale: Bool,
        staleReason: String?,
        tasks: [RoadmapTask]
    ) {
        self.wave = wave
        self.provider = provider
        self.refreshedAt = refreshedAt
        self.stale = stale
        self.staleReason = staleReason
        self.tasks = tasks
    }
}

public struct RoadmapTask: Sendable, Identifiable, Equatable, Hashable {
    public var id: String
    public var number: Int
    public var title: String
    public var slug: String
    public var fileName: String
    public var priority: RoadmapPriority
    public var isShipped: Bool
    public var content: String?
    public var filePath: String?
    public var asanaId: String?

    public init(
        id: String,
        number: Int,
        title: String,
        slug: String,
        fileName: String,
        priority: RoadmapPriority,
        isShipped: Bool,
        content: String? = nil,
        filePath: String? = nil,
        asanaId: String? = nil
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
        self.asanaId = asanaId
    }
}
