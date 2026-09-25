import Foundation

public struct ChapterHistoryEntry: Decodable, Identifiable, Sendable, Hashable {
    public let id: String
    public let sourceProjectId: String
    public let sourceWorkId: String?
    public let sourceProjectSlug: String
    public let closedAt: Int64?
    public let phase: String

    enum CodingKeys: String, CodingKey {
        case id, phase
        case sourceProjectId = "source_project_id"
        case sourceWorkId = "source_work_id"
        case sourceProjectSlug = "source_project_slug"
        case closedAt = "closed_at"
    }
}

public struct ChapterSnapshot: Decodable, Sendable {
    public struct Content: Decodable, Sendable {
        public let metricTargets: [ChapterMetricTarget]
        public let flows: ProjectFlowPlanSnapshot
        public let krs: [PlanningKeyResult]
        enum CodingKeys: String, CodingKey {
            case flows, krs
            case metricTargets = "metric_targets"
        }
    }
    public struct TaskDisposition: Decodable, Identifiable, Sendable {
        public struct Task: Decodable, Sendable {
            public let id: String
            public let identifier: String
            public let name: String
            public let completed: Bool
        }
        public var id: String { task.id }
        public let task: Task
        public let disposition: String
        public let reason: String
        public let applied: Bool
        public let observedAt: Int64
        public let atBoundary: Bool
        enum CodingKeys: String, CodingKey {
            case task, disposition, reason, applied
            case observedAt = "observed_at"
            case atBoundary = "at_boundary"
        }
    }
    public let id: String
    public let wave: String
    public let metrics: MetricPortfolio
    public let metricsEvaluatedAt: Int64
    public let content: Content
    public let tasks: [TaskDisposition]
    public let observedAt: Int64
    public let closedAt: Int64?

    enum CodingKeys: String, CodingKey {
        case id, wave, content, tasks, metrics
        case metricsEvaluatedAt = "metrics_evaluated_at"
        case observedAt = "observed_at"
        case closedAt = "closed_at"
    }
}
