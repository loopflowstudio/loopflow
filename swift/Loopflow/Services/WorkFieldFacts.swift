import Foundation

enum WorkField: String, Codable, Hashable, Sendable {
    case parent, lifecycle, activity
}

/// A confirmed field is independent of the query that happened to carry it.
/// Unavailable captures never replace its last-known value or its incorporation.
struct WorkFieldFact<Value: Codable & Equatable & Sendable>: Codable, Equatable, Sendable {
    let value: Value
    let support: WorkMembershipPlan.FieldSupport
    var needsReconciliation: Bool
    var conflictingSupports: [WorkMembershipPlan.FieldSupport] = []
    // Cache-local incorporation, bounded by semantic stage rather than request
    // count. It survives a later conflict or loss of live answer coverage.
    var incorporated: [String: [CommitFence]]?

    mutating func incorporate(_ commit: OperationCommit) {
        guard commit.resolution != .definitelyRefused, !needsReconciliation,
              covers(commit.fence) else { return }
        var stages = incorporated ?? [:]
        var fences = stages[commit.stage] ?? []
        if fences.contains(where: { $0.covers(commit.fence) }) { return }
        fences.removeAll { commit.fence.covers($0) }
        fences.append(commit.fence)
        stages[commit.stage] = fences
        incorporated = stages
    }

    func incorporates(_ commit: OperationCommit) -> Bool {
        commit.resolution != .definitelyRefused
            && incorporated?[commit.stage]?.contains(where: { $0.covers(commit.fence) }) == true
    }

    mutating func reconcile(_ support: WorkMembershipPlan.FieldSupport) {
        needsReconciliation = true
        if conflictingSupports.contains(where: { $0.covers(support) }) { return }
        conflictingSupports.removeAll { support.covers($0) }
        conflictingSupports.append(support)
    }

    /// Revision coverage is independent of answer completeness and delivery.
    /// A captured unknown still orders older captures; it supplies no known state.
    func covers(_ fence: CommitFence) -> Bool {
        CommitFence(machine: support.machine, epoch: support.epoch, schema: support.schema,
            revisions: support.revisions).covers(fence)
    }
}

struct WorkFieldStates: Codable, Equatable, Sendable {
    let states: Set<WorkSelectionState>
    let known: Bool
    init(_ fact: WorkMembershipPlan.StateFact) { states = Set(fact.states); known = fact.known }
}

struct WorkFieldLifecycle: Codable, Equatable, Sendable {
    let states: Set<WorkSelectionState>
    let known: Bool
    let completedAt: String?
    init(_ fact: WorkMembershipPlan.LifecycleFact) {
        states = Set(fact.states); known = fact.known; completedAt = fact.completedAt
    }
    private enum CodingKeys: String, CodingKey { case states, known, completedAt }
    init(from decoder: any Decoder) throws {
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        states = try fields.decode(Set<WorkSelectionState>.self, forKey: .states)
        known = try fields.decode(Bool.self, forKey: .known)
        completedAt = try fields.decode(String?.self, forKey: .completedAt)
    }
    func encode(to encoder: any Encoder) throws {
        var fields = encoder.container(keyedBy: CodingKeys.self)
        try fields.encode(states, forKey: .states)
        try fields.encode(known, forKey: .known)
        try fields.encode(completedAt, forKey: .completedAt)
    }
}

struct WorkFieldFacts: Codable, Equatable, Sendable {
    let machine: String
    let work: WorkWorkspaceKey
    var parent: WorkFieldFact<String?>?
    var lifecycle: WorkFieldFact<WorkFieldLifecycle>?
    var activity: WorkFieldFact<WorkFieldStates>?

    mutating func incorporate(_ commit: OperationCommit, field: WorkField) {
        // The resource and field are part of the address. A coincidentally
        // newer revision on another resource or field cannot absorb this stage.
        guard work.kind == .task, commit.resources.contains(work.id) else { return }
        switch field {
        case .parent: parent?.incorporate(commit)
        case .lifecycle:
            if lifecycle?.value.known == true { lifecycle?.incorporate(commit) }
        case .activity:
            if activity?.value.known == true { activity?.incorporate(commit) }
        }
    }

    func incorporates(_ commit: OperationCommit, field: WorkField) -> Bool {
        guard work.kind == .task, commit.resources.contains(work.id) else { return false }
        switch field {
        case .parent: return parent?.incorporates(commit) == true
        case .lifecycle: return lifecycle?.incorporates(commit) == true
        case .activity: return activity?.incorporates(commit) == true
        }
    }
}

extension CommitFence {
    func covers(_ previous: CommitFence) -> Bool {
        machine == previous.machine && epoch == previous.epoch && schema == previous.schema
            && !previous.revisions.isEmpty
            && previous.revisions.allSatisfy { domain, revision in
                revisions[domain].map { $0 >= revision } == true
            }
    }
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
