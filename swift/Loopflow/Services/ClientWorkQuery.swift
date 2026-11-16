import Foundation

/// A confirmed Work query and its provenance. Projection never rewrites this
/// body. Fields incorporate stages; each derived answer retains its own inputs.
public struct ClientWorkQuery: Codable, Sendable {
    public let stamp: ReadingStamp
    public let plans: [WorkViewPlan]
    // Retired effects and derived answers belong to this cache entry, never its
    // immutable Machine body. Recompute coverage after every field/link change.
    var effects: [WorkQueryEffect]? { didSet { answer = nil } }
    var reconciledRequirements: [WorkQueryRequirement]?
    var answer: WorkQueryAnswer? { didSet { rendered = nil } }
    private var fields: [WorkWorkspaceKey: WorkFieldReading] = [:]
    private var contents: [WorkWorkspaceKey: WorkViewPlan.Content] = [:]
    private var unavailableContent: Set<WorkWorkspaceKey> = []
    private var rendered: [ProjectedWorkPlan]?
    private struct PreparedPlan: Sendable {
        let facts: [WorkWorkspaceKey: WorkMembershipPlan.Facts]
        let positions: [WorkWorkspaceKey: Int]
        let targets: [String: Set<WorkWorkspaceKey>]
        let contents: [WorkWorkspaceKey: WorkViewPlan.Content]
        let ranks: [WorkMemberRef: Int]
        let resolution: WorkSelectionResolution
    }
    private var prepared: [PreparedPlan]?
    private enum CodingKeys: String, CodingKey { case stamp, plans, effects, answer, reconciledRequirements }

    init(stamp: ReadingStamp, plans: [WorkViewPlan]) {
        self.stamp = stamp
        self.plans = plans
    }

    mutating func prepareProjection(fields: [WorkWorkspaceKey: WorkFieldReading] = [:], contents: [WorkWorkspaceKey: WorkViewPlan.Content] = [:], unavailableContent: Set<WorkWorkspaceKey> = []) {
        self.contents = contents
        self.unavailableContent = unavailableContent
        self.fields = fields
        let answer = deriveAnswer()
        self.answer = answer
        rendered = projection
        prepared = zip(plans, answer.plans).map { plan, answer in
            let facts = answer.membership.facts
            var targets: [String: Set<WorkWorkspaceKey>] = [:]
            for fact in facts {
                targets[fact.work.id, default: []].insert(fact.work)
                targets[fact.name, default: []].insert(fact.work)
            }
            let contents = availableContents(for: plan)
            for content in contents {
                if let identifier = content.identifier { targets[identifier, default: []].insert(content.work) }
            }
            let potential = answer.membership.root.potentialMembers(facts: facts)
            return PreparedPlan(facts: Dictionary(facts.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first }),
                positions: Dictionary(facts.enumerated().map { ($0.element.work, $0.offset) }, uniquingKeysWith: { first, _ in first }),
                targets: targets, contents: Dictionary(contents.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first }),
                ranks: Dictionary(potential.enumerated().map { ($0.element, $0.offset) }, uniquingKeysWith: min),
                resolution: answer.membership.resolve())
        }
    }

    public var projection: [ProjectedWorkPlan] {
        if let rendered { return rendered }
        let answer = answer ?? deriveAnswer()
        return zip(plans, answer.plans).map { plan, derived in
            ProjectedWorkPlan(plan, membership: derived.membership,
                coverage: derived.coverage, contents: availableContents(for: plan),
                inventoryDependencies: inventoryDependencies(derived))
        }
    }

    private func inventoryDependencies(_ answer: WorkQueryAnswer.Plan) -> [WorkAnswerDependency] {
        let projected = Dictionary(grouping: answer.coverage.dependencies.filter(\.projected), by: \.work)
        return answer.membership.facts.flatMap { fact in
            let local = Set((projected[fact.work] ?? []).map(\.field))
            var inputs: [WorkField] = [.parent, .disposition]
            if !fact.disposition.known || (!fact.disposition.value.completed && !fact.disposition.value.abandoned) {
                inputs += [.execution]
                if !fact.execution.known || fact.execution.value.state != .unstarted { inputs += [.ownership] }
            }
            return inputs.compactMap { field -> WorkAnswerDependency? in
                guard fact.work.kind == .task || field == .parent else { return nil }
                return dependency(fact.work, field: field, fact: fact, projected: local)
            }
        }
    }

    private func availableContents(for plan: WorkViewPlan) -> [WorkViewPlan.Content] {
        var available = Dictionary(plan.contents.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first })
        for fact in plan.membership.facts {
            if let content = contents[fact.work] { available[fact.work] = content }
        }
        var seen = Set<WorkWorkspaceKey>()
        return (plan.contents.map(\.work) + plan.membership.facts.map(\.work)).compactMap {
            seen.insert($0).inserted ? available[$0] : nil
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
                directory: intent.directory, kind: intent.kind, issue: intent.issue, wave: intent.wave, work: intent.work, resolution: intent.resolution))
        }
        result.effects = layers
        if let prepared, let answer, let rendered,
           let changed = project(intents.filter { intent in !(effects ?? []).contains { $0.operationID == intent.operationID } },
                                 prepared: prepared, answer: answer, rendered: rendered) {
            result.answer = changed.answer
            result.rendered = changed.rendered
        }
        result.prepared = nil
        return result
    }

    private func apply(_ effect: WorkQueryEffect, to fact: inout WorkMembershipPlan.Facts,
                       projected: inout Set<WorkField>) {
        guard fact.work.kind == .task, effect.resolution?.resolution != .definitelyRefused,
              let declaration = OperationDeclaration(kind: effect.kind) else { return }
        func incorporated(_ field: WorkField) -> Bool {
            effect.resolution.map { fields[fact.work]?.facts.incorporates($0, field: field) == true } == true
        }
        if declaration.fields.allSatisfy(incorporated) { return }
        if declaration.kind == .move {
            var wave = effect.wave
            if let commit = effect.resolution, commit.resolution == .committed,
               case .object(let result) = commit.result, let actual = result["wave_id"] {
                if case .string(let id) = actual { wave = id }
                else if actual == .null { wave = nil }
                else { return }
            }
            guard !incorporated(.parent) else { return }
            fact.parent = .init(wave: wave, support: nil)
            projected.insert(.parent)
            return
        }
        switch declaration.kind {
        case .claim, .unassign:
            guard !incorporated(.ownership) else { return }
            if let commit = effect.resolution, commit.resolution == .committed,
               case .object(let result) = commit.result, let owner = result["owner"] {
                fact.requestedOwnership = owner != .null
            } else { fact.requestedOwnership = declaration.kind == .claim }
            if fact.requestedOwnership == false { fact.ownership = .init(value: nil, known: true, support: nil) }
            projected.insert(.ownership)
        case .complete, .abandon:
            guard !incorporated(.disposition) else { return }
            var d = fact.disposition.value
            d.completed = declaration.kind == .complete; d.abandoned = declaration.kind == .abandon
            d.completedAt = nil; d.completionRequest = nil
            fact.disposition = .init(value: d, known: true, support: nil)
            projected.insert(.disposition)
        case .reopen:
            // A committed no-op carries no planning change. It never withdraws approval.
            let committed = effect.resolution?.resolution == .committed
            let changed = effect.resolution.map { $0.planningEffect == .correlated }
            let predicateKnown = fact.disposition.known && (projected.contains(.disposition)
                || fields[fact.work]?.dispositionCurrent != false)
            let predicate: Bool? = committed ? changed : predicateKnown
                ? !fact.disposition.value.abandoned && (fact.disposition.value.completed || fact.disposition.value.completionRequest != nil) : nil
            guard let predicate else {
                fact.disposition.known = false; fact.execution.known = false
                projected.formUnion([.disposition, .execution])
                return
            }
            guard predicate else {
                if committed {
                    // A no-op preserves approval, but its Task result cannot
                    // certify retained inputs that do not cover this stage.
                    if !incorporated(.disposition) { fact.disposition.known = false; projected.insert(.disposition) }
                    if !incorporated(.execution) { fact.execution.known = false; projected.insert(.execution) }
                }
                return
            }
            if !incorporated(.disposition) {
                var d = fact.disposition.value
                d.completed = false; d.completedAt = nil; d.completionRequest = nil; d.planningState = "unstarted"
                fact.disposition = .init(value: d, known: true, support: nil)
                projected.insert(.disposition)
            }
            if !incorporated(.execution) {
                // Withdrawing approval/carrier does not establish checkout or
                // Graph work when their retained reading is inconsistent.
                fact.execution.known = fact.execution.known && fields[fact.work]?.executionCurrent != false
                fact.execution.value.approved = false; fact.execution.value.carrierRunning = false
                fact.execution.support = nil
                projected.insert(.execution)
            }
        default: return
        }
    }

    private func dependency(_ key: WorkWorkspaceKey, field: WorkField, fact: WorkMembershipPlan.Facts?,
                            projected: Set<WorkField>) -> WorkAnswerDependency {
        let reading = fields[key]
        let support: WorkMembershipPlan.FieldSupport?
        let current: Bool?
        let known: Bool
        switch field {
        case .parent: support = fact?.parent.support; current = reading?.parentCurrent; known = fact != nil
        case .disposition: support = fact?.disposition.support; current = reading?.dispositionCurrent; known = fact?.disposition.known == true
        case .ownership: support = fact?.ownership.support; current = reading?.ownershipCurrent; known = fact?.owned != nil
        case .publication: support = fact?.publication.support; current = reading?.publicationCurrent; known = fact?.publication.known == true
        case .execution: support = fact?.execution.support; current = reading?.executionCurrent; known = fact?.execution.known == true
        case .activity: support = fact?.activity.support; current = reading?.activityCurrent; known = fact?.activityKnown == true
        }
        let exact = support == nil && stamp.complete && current != false && known && field != .activity
        let local = projected.contains(field) || (field == .activity && fact?.lifecycle.states.contains(.done) == true)
        return .init(work: key, field: field, support: support, current: known && (local || current == true || exact), projected: local)
    }

    /// Prepared indexes keep gesture work on the changed resources. Full reads,
    /// normalization and program compilation stay on the store actor.
    private func project(_ effects: [WorkQueryEffect], prepared: [PreparedPlan], answer: WorkQueryAnswer,
                         rendered: [ProjectedWorkPlan]) -> (answer: WorkQueryAnswer, rendered: [ProjectedWorkPlan])? {
        var answers = answer.plans
        var renderings = rendered
        for index in plans.indices {
            let plan = plans[index]
            let cached = prepared[index]
            guard cached.resolution.unavailable.isEmpty else { return nil }
            var derived = answers[index]
            var membership = derived.membership
            var facts = cached.facts
            var projected: [WorkWorkspaceKey: Set<WorkField>] = [:]
            var changed = Set<WorkWorkspaceKey>()
            for effect in effects where effect.directory == plan.repositoryPath {
                let keys = effect.work.map { Set([$0]) } ?? cached.targets[effect.issue] ?? []
                for key in keys {
                    guard var fact = facts[key], let position = cached.positions[key] else { continue }
                    var local = projected[key] ?? []
                    apply(effect, to: &fact, projected: &local)
                    guard !local.isEmpty else { continue }
                    facts[key] = fact
                    membership.facts[position] = fact
                    projected[key] = local
                    changed.insert(key)
                }
            }
            guard !changed.isEmpty else { continue }
            var members = cached.resolution.members.filter { !changed.contains($0.work) }
            var attention = Set(cached.resolution.attention).subtracting(changed)
            var dependencies = derived.coverage.dependencies.filter { !changed.contains($0.work) }
            var states = rendered[index].states
            for key in changed {
                guard let fact = facts[key] else { continue }
                let result = membership.root.resolveMember(key, facts: facts)
                guard result.unavailable.isEmpty else { return nil }
                for member in result.members {
                    let rank = cached.ranks[member] ?? Int.max
                    var lower = 0
                    var upper = members.count
                    while lower < upper {
                        let middle = (lower + upper) / 2
                        if (cached.ranks[members[middle]] ?? Int.max) < rank { lower = middle + 1 }
                        else { upper = middle }
                    }
                    members.insert(member, at: lower)
                }
                if !result.members.isEmpty && (fact.states.contains(.available) || fact.states.contains(.blocked)) { attention.insert(key) }
                states[key] = fact.states
                for field in membership.root.fields(for: fact, manifest: facts).sorted(by: { $0.rawValue < $1.rawValue }) {
                    dependencies.append(dependency(key, field: field, fact: fact, projected: projected[key] ?? []))
                }
            }
            let selected = Set(members.map(\.work))
            let previous = derived.coverage
            let coverage = WorkAnswerCoverage(program: previous.program, definitionRevision: previous.definitionRevision,
                root: previous.root, candidates: previous.candidates, content: previous.content, dependencies: dependencies,
                candidatesCurrent: previous.candidatesCurrent,
                contentCurrent: selected.allSatisfy { cached.contents[$0] != nil } && selected.isDisjoint(with: unavailableContent))
            derived = .init(membership: membership, coverage: coverage)
            answers[index] = derived
            let resolution = WorkSelectionResolution(attention: attention.sorted { ($0.repository, $0.kind.rawValue, $0.id) < ($1.repository, $1.kind.rawValue, $1.id) },
                members: members, unavailable: [])
            renderings[index] = ProjectedWorkPlan(plan, membership: membership, coverage: coverage,
                contents: rendered[index].contents, resolution: resolution, states: states,
                inventoryDependencies: inventoryDependencies(derived))
        }
        return (WorkQueryAnswer(plans: answers), renderings)
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
                var next = original
                if let parent = sameEpoch(normalized.parent) { next.parent = .init(wave: parent.value, support: parent.support) }
                if let field = sameEpoch(normalized.disposition) {
                    next.disposition = field.value; next.disposition.support = reading.dispositionCurrent ? field.support : nil
                }
                if let field = sameEpoch(normalized.ownership) {
                    next.ownership = field.value; next.ownership.support = reading.ownershipCurrent ? field.support : nil
                }
                if let field = sameEpoch(normalized.publication) {
                    next.publication = field.value; next.publication.support = reading.publicationCurrent ? field.support : nil
                }
                if let field = sameEpoch(normalized.execution) {
                    next.execution = field.value; next.execution.support = reading.executionCurrent ? field.support : nil
                }
                if let activity = sameEpoch(normalized.activity) {
                    next.activity = .init(states: activity.value.states.sorted { $0.rawValue < $1.rawValue }, known: activity.value.known, support: activity.support)
                }
                membership.facts[index] = next
            }
            var projected: [WorkWorkspaceKey: Set<WorkField>] = [:]
            for effect in (effects ?? []).sorted(by: { $0.sequence < $1.sequence }) {
                if effect.resolution?.resolution == .definitelyRefused { continue }
                guard effect.directory == plan.repositoryPath,
                      OperationDeclaration(kind: effect.kind) != nil else { continue }
                let keys = effect.work.map { Set([$0]) } ?? Set(plan.contents.filter {
                    $0.work.id == effect.issue || $0.identifier == effect.issue
                }.map(\.work))
                for index in membership.facts.indices where keys.contains(membership.facts[index].work)
                    || (effect.work == nil && (membership.facts[index].work.id == effect.issue || membership.facts[index].name == effect.issue)) {
                    let key = membership.facts[index].work
                    var fact = membership.facts[index]
                    var changed = projected[key] ?? []
                    apply(effect, to: &fact, projected: &changed)
                    membership.facts[index] = fact
                    if !changed.isEmpty { projected[key] = changed }
                }
            }
            let dependencies = membership.root.fieldDependencies(facts: membership.facts)
            let facts = Dictionary(membership.facts.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first })
            var consumed: [WorkAnswerDependency] = []
            for key in membership.facts.map(\.work) where dependencies[key] != nil {
                for field in (dependencies[key] ?? []).sorted(by: { $0.rawValue < $1.rawValue }) {
                    consumed.append(dependency(key, field: field, fact: facts[key], projected: projected[key] ?? []))
                }
            }
            let resolution = membership.resolve()
            let selected = Set(resolution.members.map(\.work))
            let content = Set(availableContents(for: plan).map(\.work))
            let missing = selected.subtracting(content)
            let coverage = WorkAnswerCoverage(program: plan.selection, definitionRevision: plan.savedRevision,
                root: plan.membership.root, candidates: stamp,
                content: availableContents(for: plan).map(\.work),
                dependencies: consumed, candidatesCurrent: stamp.complete && membership.unavailable.isEmpty,
                // Missing native input/location readiness is not missing Task
                // content and cannot invalidate an otherwise exact list/count.
                contentCurrent: missing.isEmpty && selected.isDisjoint(with: unavailableContent))
            return WorkQueryAnswer.Plan(membership: membership, coverage: coverage)
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
    let wave: String?
    let work: WorkWorkspaceKey?

    init(operationID: String, sequence: Int64, directory: String, kind: String, issue: String,
         wave: String? = nil, work: WorkWorkspaceKey? = nil, resolution: OperationCommit?) {
        self.operationID = operationID; self.sequence = sequence; self.directory = directory
        self.kind = kind; self.issue = issue; self.wave = wave; self.work = work; self.resolution = resolution
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
        let coverage: WorkAnswerCoverage
        private enum CodingKeys: String, CodingKey { case membership, coverage, ownershipIntents }

        init(membership: WorkMembershipPlan, coverage: WorkAnswerCoverage) {
            self.membership = membership; self.coverage = coverage
        }
        init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            var membership = try c.decode(WorkMembershipPlan.self, forKey: .membership)
            let ownership = try c.decode([WorkWorkspaceKey: Bool].self, forKey: .ownershipIntents)
            for index in membership.facts.indices { membership.facts[index].requestedOwnership = ownership[membership.facts[index].work] }
            self.membership = membership
            coverage = try c.decode(WorkAnswerCoverage.self, forKey: .coverage)
        }
        func encode(to encoder: any Encoder) throws {
            var c = encoder.container(keyedBy: CodingKeys.self)
            try c.encode(membership, forKey: .membership)
            try c.encode(coverage, forKey: .coverage)
            // A derived answer retains its local intent without adding fields to
            // the immutable Machine DTO or fabricating an owner identity.
            let ownership = Dictionary(uniqueKeysWithValues: membership.facts.compactMap { fact in
                fact.requestedOwnership.map { (fact.work, $0) }
            })
            try c.encode(ownership, forKey: .ownershipIntents)
        }
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
    func fields(for fact: WorkMembershipPlan.Facts, manifest: [WorkWorkspaceKey: WorkMembershipPlan.Facts]) -> Set<WorkField> {
        func inputs(_ state: WorkSelectionState) -> Set<WorkField> {
            if fact.disposition.known && (fact.disposition.value.completed || fact.disposition.value.abandoned) { return [.disposition] }
            let runtime: Set<WorkField> = [.disposition, .execution]
            if state.isActivity { return runtime.union([.activity]) }
            if state == .open { return runtime.union([.ownership, .publication]) }
            if [.working, .backlog, .autonomous].contains(state) { return runtime.union([.ownership]) }
            return runtime
        }
        switch self {
        case .members: return []
        case .select(_, let predicate):
            switch predicate {
            case .default:
                return fact.work.kind == .task && predicate.includes(fact, facts: manifest) ? inputs(.working).union([.parent]) : [.parent]
            case .descendants: return [.parent]
            case .state(let state) where fact.work.kind == .task: return inputs(state)
            default: return []
            }
        case .compose(let nodes), .intersect(let nodes):
            return nodes.reduce(into: []) { $0.formUnion($1.fields(for: fact, manifest: manifest)) }
        }
    }
    func fieldDependencies(facts: [WorkMembershipPlan.Facts]) -> [WorkWorkspaceKey: Set<WorkField>] {
        let manifest = Dictionary(facts.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first })
        return Dictionary(facts.compactMap { fact in
            let fields = fields(for: fact, manifest: manifest)
            return fields.isEmpty ? nil : (fact.work, fields)
        }, uniquingKeysWith: { $0.union($1) })
    }
}

/// Acknowledged local invalidations, scoped to one retained base. This never
/// advances the base's manifest stamp or certifies unrelated repository changes.
struct WorkQueryRequirement: Codable {
    var fence: CommitFence
    let directory: String?
    let kind: String?

    func requiredFence(in reading: ClientWorkQuery) -> CommitFence? {
        guard let directory, let kind else { return fence }
        let declarations = reading.plans.filter { $0.repositoryPath == directory }.compactMap { $0.membership.effects[kind] }
        guard !declarations.isEmpty else { return nil }
        return fence.requiring(Array(Set(declarations.flatMap(\.domains)))) ?? fence
    }

    func covers(_ other: WorkQueryRequirement) -> Bool {
        directory == other.directory && kind == other.kind && fence.covers(other.fence)
    }
}
