import Foundation

/// A confirmed Work query and its provenance. Projection never rewrites this
/// body; each query incorporates a semantic effect independently.
public struct ClientWorkQuery: Codable, Sendable {
    public let stamp: ReadingStamp
    public let plans: [WorkViewPlan]
}

extension ReadingStamp {
    /// Work plans depend on all five fact groups. Incomparable replies cannot
    /// replace a known base; arrival order is not freshness.
    func covers(_ other: ReadingStamp) -> Bool {
        complete && machine == other.machine && epoch == other.epoch && schema == other.schema
            && query == other.query
            && revisions.planning >= other.revisions.planning
            && revisions.sessions >= other.revisions.sessions
            && revisions.graphs >= other.revisions.graphs
            && revisions.processes >= other.revisions.processes
            && revisions.usage >= other.revisions.usage
    }
}
