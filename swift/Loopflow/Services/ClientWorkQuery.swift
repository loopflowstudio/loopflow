import Foundation

/// A confirmed Work query and its provenance. Projection never rewrites this
/// body. Fields incorporate stages; each derived answer retains its own inputs.
public struct ClientWorkQuery: Codable, Sendable {
    public let stamp: ReadingStamp
    public let plans: [WorkViewPlan]
    // Retired effects and derived answers belong to this cache entry, never its
    // immutable Machine body. Recompute coverage after every field/link change.
    var effects: [WorkQueryEffect]? { didSet { answer = nil } }
    var answer: WorkQueryAnswer? { didSet { rendered = nil } }
    private var fields: [WorkWorkspaceKey: WorkFieldReading] = [:]
    private var rendered: [ProjectedWorkPlan]?
    private enum CodingKeys: String, CodingKey { case stamp, plans, effects, answer }

    init(stamp: ReadingStamp, plans: [WorkViewPlan]) {
        self.stamp = stamp
        self.plans = plans
    }

    mutating func prepareProjection(fields: [WorkWorkspaceKey: WorkFieldReading] = [:]) {
        self.fields = fields
        answer = deriveAnswer()
        rendered = projection
    }

    public var projection: [ProjectedWorkPlan] {
        if let rendered { return rendered }
        let answer = answer ?? deriveAnswer()
        return zip(plans, answer.plans).map { plan, derived in
            ProjectedWorkPlan(plan, membership: derived.membership,
                knownAbsent: derived.knownAbsent, coverage: derived.coverage)
        }
    }

    func applying(_ intents: [WorkQueryEffect]) -> ClientWorkQuery {
        guard !intents.isEmpty else { return self }
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

    private func deriveAnswer() -> WorkQueryAnswer {
        func sameEpoch<Value>(_ fact: WorkFieldFact<Value>?) -> WorkFieldFact<Value>? {
            guard let fact, fact.support.machine == stamp.machine,
                  fact.support.epoch == stamp.epoch, fact.support.schema == stamp.schema else { return nil }
            return fact
        }
        return WorkQueryAnswer(plans: plans.map { plan in
            var membership = plan.membership
            for index in membership.facts.indices {
                let original = membership.facts[index]
                guard let reading = fields[original.work] else { continue }
                let normalized = reading.facts
                membership.facts[index] = .init(work: original.work, name: original.name,
                    parent: sameEpoch(normalized.parent).map { .init(wave: $0.value, support: $0.support) } ?? original.parent,
                    lifecycle: sameEpoch(normalized.lifecycle).map { .init(states: $0.value.states.sorted { $0.rawValue < $1.rawValue },
                        known: $0.value.known, support: $0.support) } ?? original.lifecycle,
                    activity: sameEpoch(normalized.activity).map { .init(states: $0.value.states.sorted { $0.rawValue < $1.rawValue },
                        known: $0.value.known, support: $0.support) } ?? original.activity)
            }
            var knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>] = [:]
            var projected: [WorkWorkspaceKey: Set<WorkField>] = [:]
            for effect in (effects ?? []).sorted(by: { $0.sequence < $1.sequence }) {
                if effect.resolution?.resolution == .definitelyRefused { continue }
                guard effect.directory == plan.repositoryPath,
                      let change = membership.effects[effect.kind] else { continue }
                let keys = Set(plan.contents.filter {
                    $0.work.id == effect.issue || $0.identifier == effect.issue
                }.map(\.work))
                for index in membership.facts.indices where keys.contains(membership.facts[index].work)
                    || membership.facts[index].work.id == effect.issue || membership.facts[index].name == effect.issue {
                    let key = membership.facts[index].work
                    // Incorporation is permanent for this field/stage. A stale
                    // query or unavailable activity cannot resurrect the intent.
                    if let commit = effect.resolution,
                       let field = OperationDeclaration(kind: effect.kind)?.field,
                       fields[key]?.facts.incorporates(commit, field: field) == true { continue }
                    // Effects declare canonical state changes; Swift partitions
                    // fields, without deriving Task lifecycle a second time.
                    let fact = membership.facts[index]
                    membership.facts[index].lifecycle = .init(states: Set(fact.lifecycle.states).subtracting(change.remove)
                        .union(change.insert.filter { !$0.isActivity }).sorted { $0.rawValue < $1.rawValue }, known: true, support: nil)
                    membership.facts[index].activity = .init(states: Set(fact.activity.states).subtracting(change.remove)
                        .union(change.insert.filter(\.isActivity)).sorted { $0.rawValue < $1.rawValue }, known: fact.activity.known, support: nil)
                    projected[key, default: []].insert(.lifecycle)
                    knownAbsent[key] = (knownAbsent[key] ?? []).union(change.remove).subtracting(change.insert)
                }
            }
            let dependencies = membership.root.fieldDependencies
            let facts = Dictionary(membership.facts.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first })
            var consumed: [WorkAnswerDependency] = []
            for key in dependencies.keys.sorted(by: { ($0.repository, $0.kind.rawValue, $0.id) < ($1.repository, $1.kind.rawValue, $1.id) }) {
                for field in (dependencies[key] ?? []).sorted(by: { $0.rawValue < $1.rawValue }) {
                    let reading = fields[key]
                    let support = field == .lifecycle ? facts[key]?.lifecycle.support : facts[key]?.activity.support
                    let current = field == .lifecycle ? reading?.lifecycleCurrent : reading?.activityCurrent
                    // A pure exact-query lifecycle answer can still use its own
                    // snapshot. This grants no cross-query field support and
                    // never certifies externally derived activity.
                    let exact = field == .lifecycle && support == nil
                        && facts[key]?.lifecycle.known == true && stamp.complete
                    let local = projected[key]?.contains(field) == true
                        || (field == .activity && knownAbsent[key]?.isSuperset(of: [.available, .busy, .blocked]) == true)
                    consumed.append(.init(work: key, field: field, support: support, current: local || current == true || exact,
                        projected: local))
                }
            }
            let resolution = membership.resolve(knownAbsent: knownAbsent)
            let selected = Set(resolution.members.map(\.work))
            let content = Set(plan.contents.map(\.work))
            let missing = selected.subtracting(content)
            let coverage = WorkAnswerCoverage(program: plan.selection, definitionRevision: plan.savedRevision,
                root: plan.membership.root, candidates: stamp,
                content: content.sorted { ($0.repository, $0.kind.rawValue, $0.id) < ($1.repository, $1.kind.rawValue, $1.id) },
                dependencies: consumed, candidatesCurrent: stamp.complete && membership.unavailable.isEmpty,
                // Missing native input/location readiness is not missing Task
                // content and cannot invalidate an otherwise exact list/count.
                contentCurrent: missing.isEmpty)
            return WorkQueryAnswer.Plan(membership: membership, knownAbsent: knownAbsent, coverage: coverage)
        })
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

/// A name is one Session field, independent of the lists that mention it.
/// Its sessions-domain fence orders stamped reads and the domain's rename result.
struct SessionNameFact: Codable, Equatable, Sendable {
    let id: String
    let title: String
    let source: SessionTitleSource
    let fence: CommitFence

    func covers(_ other: CommitFence) -> Bool {
        fence.machine == other.machine && fence.epoch == other.epoch && fence.schema == other.schema
            && fence.revisions["sessions"].map { current in
                other.revisions["sessions"].map { current >= $0 } == true
            } == true
    }
}


/// Replaceable client computation, persisted beside its exact-query input.
/// It is never exported as a Machine reading or used to settle delivery.
struct WorkQueryAnswer: Codable, Equatable, Sendable {
    struct Plan: Codable, Equatable, Sendable {
        let membership: WorkMembershipPlan
        let knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>]
        let coverage: WorkAnswerCoverage
    }
    let plans: [Plan]
}

struct WorkAnswerDependency: Codable, Equatable, Sendable {
    let work: WorkWorkspaceKey
    let field: WorkField
    let support: WorkMembershipPlan.FieldSupport?
    let current: Bool
    let projected: Bool
}

struct WorkAnswerCoverage: Codable, Equatable, Sendable {
    let program: WorkSelection
    let definitionRevision: Int64?
    let root: WorkMembershipPlan.Node
    let candidates: ReadingStamp
    let content: [WorkWorkspaceKey]
    let dependencies: [WorkAnswerDependency]
    let candidatesCurrent: Bool
    let contentCurrent: Bool
    var current: Bool { candidatesCurrent && contentCurrent && dependencies.allSatisfy(\.current) }
}

extension WorkSelectionState {
    var isActivity: Bool { self == .available || self == .busy || self == .blocked }
}

extension WorkMembershipPlan.Node {
    var fieldDependencies: [WorkWorkspaceKey: Set<WorkField>] {
        switch self {
        case .members: return [:]
        case .state(let state, let members):
            return Dictionary(members.map { ($0.work, Set([state.isActivity ? WorkField.activity : .lifecycle])) },
                uniquingKeysWith: { $0.union($1) })
        case .compose(let nodes), .intersect(let nodes):
            return nodes.reduce(into: [:]) { result, node in
                result.merge(node.fieldDependencies, uniquingKeysWith: { $0.union($1) })
            }
        }
    }
}
