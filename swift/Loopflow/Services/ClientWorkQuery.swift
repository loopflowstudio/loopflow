import Foundation

/// A confirmed Work query and its provenance. Projection never rewrites this
/// body; each query incorporates a semantic effect independently.
public struct ClientWorkQuery: Codable, Sendable {
    public let stamp: ReadingStamp
    public let plans: [WorkViewPlan]
    // Cache-local obligations survive operation retirement. Nil reads caches
    // written before per-query incorporation was introduced.
    var effects: [WorkQueryEffect]? { didSet { rendered = nil } }
    private var rendered: [ProjectedWorkPlan]? = nil
    private enum CodingKeys: String, CodingKey { case stamp, plans, effects }

    init(stamp: ReadingStamp, plans: [WorkViewPlan]) {
        self.stamp = stamp
        self.plans = plans
    }

    /// Disk reads prepare this on the store actor, before crossing to a host.
    /// The transient rendering is never encoded as a confirmed query body.
    mutating func prepareProjection() { rendered = makeProjection() }

    public var projection: [ProjectedWorkPlan] { rendered ?? makeProjection() }

    func applying(_ intents: [WorkQueryEffect]) -> ClientWorkQuery {
        var result = self
        var layers = effects ?? []
        var sequence = layers.map(\.sequence).max() ?? 0
        for intent in intents where !layers.contains(where: { $0.operationID == intent.operationID }) {
            sequence += 1
            layers.append(WorkQueryEffect(operationID: intent.operationID, sequence: sequence,
                directory: intent.directory, kind: intent.kind, issue: intent.issue, resolution: intent.resolution))
        }
        result.effects = layers
        return result
    }

    private func makeProjection() -> [ProjectedWorkPlan] {
        plans.map { plan in
            var membership = plan.membership
            var knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>] = [:]
            for effect in (effects ?? []).sorted(by: { $0.sequence < $1.sequence }) {
                if effect.resolution?.resolution == .definitelyRefused { continue }
                guard effect.directory == plan.repositoryPath,
                      let change = membership.effects[effect.kind] else { continue }
                let keys = Set(plan.contents.filter {
                    $0.work.id == effect.issue || $0.identifier == effect.issue
                }.map(\.work))
                for index in membership.facts.indices where keys.contains(membership.facts[index].work)
                    || membership.facts[index].work.id == effect.issue || membership.facts[index].name == effect.issue {
                    let states = Set(membership.facts[index].states).subtracting(change.remove).union(change.insert)
                    membership.facts[index].states = states.sorted { $0.rawValue < $1.rawValue }
                    let key = membership.facts[index].work
                    knownAbsent[key] = (knownAbsent[key] ?? []).union(change.remove).subtracting(change.insert)
                }
            }
            return ProjectedWorkPlan(plan, membership: membership, knownAbsent: knownAbsent)
        }
    }
}

/// An ordered effect, not a second command or mutable confirmed record.
struct WorkQueryEffect: Codable, Sendable {
    let operationID: String
    let sequence: Int64
    let directory: String
    let kind: String
    let issue: String
    let resolution: OperationCommit?

    func covered(by stamp: ReadingStamp, plans: [WorkViewPlan]) -> Bool {
        let declarations = plans.filter { $0.repositoryPath == directory }.compactMap { $0.membership.effects[kind] }
        // Changing a query to an unrelated scope removes its old obligation.
        guard !declarations.isEmpty else { return true }
        guard let resolution, let query = stamp.query else { return false }
        return declarations.allSatisfy {
            guard $0.stage == resolution.stage, let fence = resolution.fence.requiring($0.domains) else { return false }
            return fence.isCovered(by: stamp, query: query)
        }
    }
}

extension ReadingStamp {
    /// Compare only the domains declared by both versions of the query plan.
    /// Incomparable replies retain the known base; arrival order is not freshness.
    func covers(_ other: ReadingStamp, domains: Set<String>) -> Bool {
        guard complete, machine == other.machine, epoch == other.epoch, schema == other.schema,
              let query = other.query, self.query == query else { return false }
        if domains.isEmpty { return true }
        let vector = ["planning": other.revisions.planning, "sessions": other.revisions.sessions,
            "graphs": other.revisions.graphs, "processes": other.revisions.processes, "usage": other.revisions.usage]
        let fence = CommitFence(machine: other.machine, epoch: other.epoch, schema: other.schema, revisions: vector)
        guard let required = fence.requiring(Array(domains)) else { return false }
        return required.isCovered(by: self, query: query)
    }
}

extension CommitFence {
    /// Missing declared domains are unavailable evidence, never revision zero.
    func requiring(_ domains: [String]) -> CommitFence? {
        guard !domains.isEmpty, domains.allSatisfy({ revisions[$0] != nil }) else { return nil }
        let required = Set(domains)
        return CommitFence(machine: machine, epoch: epoch, schema: schema,
            revisions: revisions.filter { required.contains($0.key) })
    }
}
