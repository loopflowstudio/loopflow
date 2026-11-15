import Foundation

/// A confirmed field is independent of the query that happened to carry it.
/// Unavailable captures never replace its last-known value or its incorporation.
struct WorkFieldFact<Value: Codable & Equatable & Sendable>: Codable, Equatable, Sendable {
    let value: Value
    let support: WorkMembershipPlan.FieldSupport
    var needsReconciliation: Bool
    var conflictingSupports: [WorkMembershipPlan.FieldSupport] = []

    mutating func reconcile(_ support: WorkMembershipPlan.FieldSupport) {
        needsReconciliation = true
        if conflictingSupports.contains(where: { $0.covers(support) }) { return }
        conflictingSupports.removeAll { support.covers($0) }
        conflictingSupports.append(support)
    }

    /// Revision coverage is independent of answer completeness and delivery.
    /// A captured unknown still orders older captures; it supplies no known state.
    func covers(_ fence: CommitFence) -> Bool {
        support.machine == fence.machine && support.epoch == fence.epoch && support.schema == fence.schema
            && !fence.revisions.isEmpty
            && fence.revisions.allSatisfy { domain, revision in
                support.revisions[domain].map { $0 >= revision } == true
            }
    }
}

struct WorkFieldStates: Codable, Equatable, Sendable {
    let states: Set<WorkSelectionState>
    let known: Bool
    init(_ fact: WorkMembershipPlan.StateFact) { states = Set(fact.states); known = fact.known }
}

struct WorkFieldFacts: Codable, Equatable, Sendable {
    let machine: String
    let work: WorkWorkspaceKey
    var parent: WorkFieldFact<String?>?
    var lifecycle: WorkFieldFact<WorkFieldStates>?
    var activity: WorkFieldFact<WorkFieldStates>?
}

/// This is local field coverage, never a replacement Machine query stamp.
struct WorkFieldReading: Sendable {
    let facts: WorkFieldFacts
    let parentCurrent: Bool
    let lifecycleCurrent: Bool
    let activityCurrent: Bool
}

extension WorkMembershipPlan.FieldSupport {
    func belongs(to stamp: ReadingStamp) -> Bool {
        let vector = ["planning": stamp.revisions.planning, "sessions": stamp.revisions.sessions,
            "graphs": stamp.revisions.graphs, "processes": stamp.revisions.processes, "usage": stamp.revisions.usage]
        return machine == stamp.machine && epoch == stamp.epoch && schema == stamp.schema
            && !derivation.isEmpty && !revisions.isEmpty
            && revisions.allSatisfy { vector[$0.key] == $0.value }
    }

    func isAvailable(on connection: MachineConnection.ObservationConnection?) -> Bool {
        switch observations {
        case .sqlite: true
        case .unavailable: false
        case .captured(let evidence):
            connection?.source == evidence.source
                && Set(evidence.dependencies.map(\.fact)).count == evidence.dependencies.count
                && evidence.dependencies.allSatisfy { $0.token <= evidence.validatedAt }
        }
    }

    /// Compare complete producer-declared supports. Crossed vectors and changed
    /// derivations are incomparable; taking maxima would invent a computation.
    func covers(_ previous: Self) -> Bool {
        guard machine == previous.machine, epoch == previous.epoch, schema == previous.schema,
              derivation == previous.derivation, revisions.keys.sorted() == previous.revisions.keys.sorted() else { return false }
        guard previous.revisions.allSatisfy({ domain, revision in revisions[domain].map { $0 >= revision } == true }) else { return false }
        switch (observations, previous.observations) {
        case (.sqlite, .sqlite): return true
        case (.captured(let current), .captured(let old)):
            // The connection, not a reading, has already elected the new source.
            if current.source != old.source { return true }
            guard current.validatedAt >= old.validatedAt else { return false }
            if current.validatedAt == old.validatedAt { return current == old }
            let tokens = Dictionary(current.dependencies.map { ($0.fact, $0.token) }, uniquingKeysWith: { first, _ in first })
            return old.dependencies.allSatisfy { old in tokens[old.fact].map { $0 >= old.token } ?? true }
        default: return false
        }
    }
}
