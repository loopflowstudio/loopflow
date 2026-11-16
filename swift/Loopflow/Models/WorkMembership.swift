import Foundation

/// Authored predicates and occurrence paths over a captured repository manifest.
public struct WorkMembershipPlan: Codable, Equatable, Sendable {
    public struct FieldSupport: Codable, Equatable, Sendable {
        public enum Observations: Codable, Equatable, Sendable {
            case sqlite
            case captured(ReadingStamp.Observations)
            case unavailable
            private enum Keys: String, CodingKey { case kind, evidence }
            private enum Kind: String, Codable { case sqlite, captured, unavailable }
            public init(from decoder: any Decoder) throws {
                let fields = try decoder.container(keyedBy: Keys.self)
                switch try fields.decode(Kind.self, forKey: .kind) {
                case .sqlite: self = .sqlite
                case .captured: self = .captured(try fields.decode(ReadingStamp.Observations.self, forKey: .evidence))
                case .unavailable: self = .unavailable
                }
            }
            public func encode(to encoder: any Encoder) throws {
                var fields = encoder.container(keyedBy: Keys.self)
                switch self {
                case .sqlite: try fields.encode(Kind.sqlite, forKey: .kind)
                case .captured(let evidence):
                    try fields.encode(Kind.captured, forKey: .kind)
                    try fields.encode(evidence, forKey: .evidence)
                case .unavailable: try fields.encode(Kind.unavailable, forKey: .kind)
                }
            }
        }
        public let machine: String
        public let epoch: String
        public let schema: UInt32
        public let derivation: String
        public let revisions: [String: Int64]
        public let observations: Observations
    }
    public struct ParentFact: Codable, Equatable, Sendable {
        public let wave: String?
        public let support: FieldSupport?
        private enum CodingKeys: String, CodingKey { case wave, support }
        public init(wave: String?, support: FieldSupport?) { self.wave = wave; self.support = support }
        public init(from decoder: any Decoder) throws {
            let fields = try decoder.container(keyedBy: CodingKeys.self)
            wave = try fields.decode(String?.self, forKey: .wave)
            support = try fields.decode(FieldSupport?.self, forKey: .support)
        }
        public func encode(to encoder: any Encoder) throws {
            var fields = encoder.container(keyedBy: CodingKeys.self)
            try fields.encode(wave, forKey: .wave)
            try fields.encode(support, forKey: .support)
        }
    }
    public struct StateFact: Codable, Equatable, Sendable {
        public var states: [WorkSelectionState]
        public let known: Bool
        public let support: FieldSupport?
        private enum CodingKeys: String, CodingKey { case states, known, support }
        public init(states: [WorkSelectionState], known: Bool, support: FieldSupport?) {
            self.states = states; self.known = known; self.support = support
        }
        public init(from decoder: any Decoder) throws {
            let fields = try decoder.container(keyedBy: CodingKeys.self)
            states = try fields.decode([WorkSelectionState].self, forKey: .states)
            known = try fields.decode(Bool.self, forKey: .known)
            support = try fields.decode(FieldSupport?.self, forKey: .support)
        }
        public func encode(to encoder: any Encoder) throws {
            var fields = encoder.container(keyedBy: CodingKeys.self)
            try fields.encode(states, forKey: .states)
            try fields.encode(known, forKey: .known)
            try fields.encode(support, forKey: .support)
        }
    }
    /// Derived display value, never an authoritative wire or cached field.
    public struct LifecycleFact: Equatable, Sendable {
        public let completedAt: String?
        public var states: [WorkSelectionState]
        public let known: Bool
        public let support: FieldSupport?
        public init(completedAt: String?, states: [WorkSelectionState], known: Bool, support: FieldSupport?) {
            self.completedAt = completedAt; self.states = states; self.known = known; self.support = support
        }
        /// Nil means this dated answer is incomplete. Local intent has no Machine time.
        public func completed(inLast days: Int, now: Date) -> Bool? {
            guard known else { return nil }
            guard states.contains(.done) else { return false }
            if days == 0 { return true }
            guard let support, support.observations != .unavailable,
                  let completedAt, let date = ProjectedTaskHistory.completionDate(completedAt) else { return nil }
            return date <= now && date >= now.addingTimeInterval(-Double(days) * 86_400)
        }
    }
    public struct ValueFact<Value: Codable & Equatable & Sendable>: Codable, Equatable, Sendable {
        public var value: Value
        public var known: Bool
        public var support: FieldSupport?
        public init(value: Value, known: Bool, support: FieldSupport?) {
            self.value = value; self.known = known; self.support = support
        }
        private enum CodingKeys: String, CodingKey { case value, known, support }
        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            value = try c.decode(Value.self, forKey: .value)
            known = try c.decode(Bool.self, forKey: .known)
            support = try c.decode(FieldSupport?.self, forKey: .support)
        }
        public func encode(to encoder: any Encoder) throws {
            var c = encoder.container(keyedBy: CodingKeys.self)
            try c.encode(value, forKey: .value); try c.encode(known, forKey: .known); try c.encode(support, forKey: .support)
        }
    }
    public struct Disposition: Codable, Equatable, Sendable {
        public var completed: Bool
        public var abandoned: Bool
        public var completedAt: String?
        public var planningState: String?
        public var completionRequest: Int64?
        private enum CodingKeys: String, CodingKey {
            case completed, abandoned, completedAt = "completed_at", planningState = "planning_state", completionRequest = "completion_request"
        }
        public init(completed: Bool, abandoned: Bool, completedAt: String?, planningState: String?, completionRequest: Int64?) {
            self.completed = completed; self.abandoned = abandoned; self.completedAt = completedAt
            self.planningState = planningState; self.completionRequest = completionRequest
        }
        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            completed = try c.decode(Bool.self, forKey: .completed); abandoned = try c.decode(Bool.self, forKey: .abandoned)
            completedAt = try c.decode(String?.self, forKey: .completedAt)
            planningState = try c.decode(String?.self, forKey: .planningState)
            completionRequest = try c.decode(Int64?.self, forKey: .completionRequest)
        }
        public func encode(to encoder: any Encoder) throws {
            var c = encoder.container(keyedBy: CodingKeys.self)
            try c.encode(completed, forKey: .completed); try c.encode(abandoned, forKey: .abandoned)
            try c.encode(completedAt, forKey: .completedAt); try c.encode(planningState, forKey: .planningState)
            try c.encode(completionRequest, forKey: .completionRequest)
        }
    }
    public struct ExecutionInputs: Codable, Equatable, Sendable {
        public var checkout: Bool
        public var graphRunning: Bool
        public var carrierRunning: Bool
        public var held: Bool
        public var approved: Bool
        private enum CodingKeys: String, CodingKey { case checkout, graphRunning = "graph_running", carrierRunning = "carrier_running", held, approved }
        public init(checkout: Bool, graphRunning: Bool, carrierRunning: Bool, held: Bool, approved: Bool) {
            self.checkout = checkout; self.graphRunning = graphRunning; self.carrierRunning = carrierRunning
            self.held = held; self.approved = approved
        }
        var state: WorkSelectionState {
            if graphRunning || carrierRunning { return approved ? .shipping : .moving }
            if held && !approved { return .active }
            return checkout ? .idle : .unstarted
        }
    }
    public struct Facts: Codable, Equatable, Sendable {
        public let work: WorkWorkspaceKey
        public let name: String
        public var parent: ParentFact
        public var disposition: ValueFact<Disposition>
        public var ownership: ValueFact<TaskOwner?>
        public var publication: ValueFact<Bool>
        public var execution: ValueFact<ExecutionInputs>
        public var activity: StateFact
        /// A local claim projects ownership without inventing a person or Machine.
        var requestedOwnership: Bool?
        private enum CodingKeys: String, CodingKey { case work, name, parent, disposition, ownership, publication, execution, activity }
        public init(work: WorkWorkspaceKey, name: String, parent: ParentFact, disposition: ValueFact<Disposition>,
                    ownership: ValueFact<TaskOwner?>, publication: ValueFact<Bool>, execution: ValueFact<ExecutionInputs>, activity: StateFact) {
            self.work = work; self.name = name; self.parent = parent; self.disposition = disposition
            self.ownership = ownership; self.publication = publication; self.execution = execution; self.activity = activity
        }
        public var owned: Bool? { requestedOwnership ?? (ownership.known ? ownership.value != nil : nil) }
        public var ownerLabel: String? {
            if let requestedOwnership { return requestedOwnership ? "Claiming…" : "Unassigned" }
            guard ownership.known else { return "Ownership unavailable" }
            return ownership.value?.summary
        }
        public var activityKnown: Bool { knows(.done) && states.contains(.done) || activity.known }
        /// Derived presentation only: no aggregate lifecycle is read or persisted.
        public var lifecycle: LifecycleFact {
            guard work.kind == .task else { return .init(completedAt: nil, states: [], known: true, support: nil) }
            guard disposition.known else { return .init(completedAt: nil, states: [], known: false, support: nil) }
            let d = disposition.value
            if !d.abandoned && !d.completed && !execution.known { return .init(completedAt: nil, states: [], known: false, support: nil) }
            if d.abandoned || (!d.completed && execution.value.state == .unstarted && ["canceled", "duplicate"].contains(d.planningState)) {
                return .init(completedAt: nil, states: [], known: true, support: disposition.support)
            }
            var state = d.completed ? WorkSelectionState.done : execution.value.state
            if state == .unstarted && ["completed", "canceled", "duplicate"].contains(d.planningState) { state = .done }
            if state == .done {
                // Planning can withdraw never-started work. That dated answer
                // also needs supported execution, unlike explicit completion.
                let supported = d.completed || execution.support.map { $0.observations != .unavailable } == true
                return .init(completedAt: d.completedAt, states: [.done], known: true, support: supported ? disposition.support : nil)
            }
            var states: Set<WorkSelectionState> = [state]
            if state == .unstarted { states.insert(.backlog) }
            else if let owned { states.insert(!owned ? .backlog : state == .shipping ? .autonomous : .working) }
            if owned == true || (publication.known && publication.value) { states.insert(.open) }
            return .init(completedAt: nil, states: states.sorted { $0.rawValue < $1.rawValue }, known: state == .unstarted || owned != nil, support: nil)
        }
        func knows(_ state: WorkSelectionState) -> Bool {
            guard disposition.known else { return false }
            if disposition.value.completed || disposition.value.abandoned { return true }
            guard execution.known else { return false }
            if lifecycle.known && (lifecycle.states.isEmpty || lifecycle.states.contains(.done)) { return true }
            if state.isActivity { return activity.known }
            if state == .open { return owned == true || (publication.known && publication.value) || (owned != nil && publication.known) }
            if [.working, .backlog, .autonomous].contains(state) { return execution.value.state == .unstarted || owned != nil }
            return true
        }
        public var states: [WorkSelectionState] {
            let lifecycle = lifecycle
            if work.kind == .task && lifecycle.known && lifecycle.states.isEmpty { return [.done] }
            return (lifecycle.states + (lifecycle.states.contains(.done) ? [] : activity.states)).sorted { $0.rawValue < $1.rawValue }
        }
    }
    public enum Predicate: Codable, Equatable, Sendable {
        case `default`(wave: String?)
        case descendants(wave: String)
        case kind(WorkWorkspaceKey.Kind)
        case state(WorkSelectionState)
        private enum Keys: String, CodingKey { case kind, wave, workKind = "work_kind", state }
        private enum Kind: String, Codable { case `default`, descendants, kind, state }
        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: Keys.self)
            switch try c.decode(Kind.self, forKey: .kind) {
            case .default: self = .default(wave: try c.decode(String?.self, forKey: .wave))
            case .descendants: self = .descendants(wave: try c.decode(String.self, forKey: .wave))
            case .kind: self = .kind(try c.decode(WorkWorkspaceKey.Kind.self, forKey: .workKind))
            case .state: self = .state(try c.decode(WorkSelectionState.self, forKey: .state))
            }
        }
        public func encode(to encoder: any Encoder) throws {
            var c = encoder.container(keyedBy: Keys.self)
            switch self {
            case .default(let wave): try c.encode(Kind.default, forKey: .kind); try c.encode(wave, forKey: .wave)
            case .descendants(let wave): try c.encode(Kind.descendants, forKey: .kind); try c.encode(wave, forKey: .wave)
            case .kind(let kind): try c.encode(Kind.kind, forKey: .kind); try c.encode(kind, forKey: .workKind)
            case .state(let state): try c.encode(Kind.state, forKey: .kind); try c.encode(state, forKey: .state)
            }
        }

        func includes(_ fact: Facts, facts: [WorkWorkspaceKey: Facts]) -> Bool {
            var unavailable: [String] = []
            return includes(fact, facts: facts, unavailable: &unavailable)
        }

        fileprivate func includes(_ fact: Facts, facts: [WorkWorkspaceKey: Facts], unavailable: inout [String]) -> Bool {
            func descendant(_ wave: String) -> Bool {
                var parent = fact.parent.wave
                var seen = Set<String>()
                while let id = parent {
                    if id == wave { return true }
                    guard seen.insert(id).inserted else {
                        unavailable.append("Wave \(id) ancestry is cyclic"); return false
                    }
                    guard let ancestor = facts[WorkWorkspaceKey(repository: fact.work.repository, kind: .wave, id: id)] else {
                        unavailable.append("Wave \(id) ancestry is unavailable"); return false
                    }
                    parent = ancestor.parent.wave
                }
                return false
            }
            switch self {
            case .default(let wave):
                switch fact.work.kind {
                case .repository: return false
                case .wave: return fact.parent.wave == wave
                case .task: return wave.map(descendant) ?? true
                }
            case .descendants(let wave): return descendant(wave)
            case .kind(let kind): return fact.work.kind == kind
            case .state: return fact.work.kind == .task
            }
        }
        fileprivate func state(for fact: Facts) -> WorkSelectionState? {
            switch self {
            case .state(let state): state
            case .default where fact.work.kind == .task: .working
            default: nil
            }
        }
    }
    public indirect enum Node: Codable, Equatable, Sendable {
        case members([WorkMemberRef])
        case select(source: [String], predicate: Predicate)
        case compose([Node])
        case intersect([Node])

        private enum Keys: String, CodingKey { case kind, members, source, predicate, items }
        private enum Kind: String, Codable { case members, select, compose, intersect }
        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: Keys.self)
            switch try c.decode(Kind.self, forKey: .kind) {
            case .members: self = .members(try c.decode([WorkMemberRef].self, forKey: .members))
            case .select: self = .select(source: try c.decode([String].self, forKey: .source), predicate: try c.decode(Predicate.self, forKey: .predicate))
            case .compose: self = .compose(try c.decode([Node].self, forKey: .items))
            case .intersect: self = .intersect(try c.decode([Node].self, forKey: .items))
            }
        }
        public func encode(to encoder: any Encoder) throws {
            var c = encoder.container(keyedBy: Keys.self)
            switch self {
            case .members(let members):
                try c.encode(Kind.members, forKey: .kind); try c.encode(members, forKey: .members)
            case .select(let source, let predicate):
                try c.encode(Kind.select, forKey: .kind); try c.encode(source, forKey: .source); try c.encode(predicate, forKey: .predicate)
            case .compose(let items):
                try c.encode(Kind.compose, forKey: .kind); try c.encode(items, forKey: .items)
            case .intersect(let items):
                try c.encode(Kind.intersect, forKey: .kind); try c.encode(items, forKey: .items)
            }
        }

        fileprivate func evaluate(_ facts: [WorkWorkspaceKey: Facts], order: [WorkWorkspaceKey], includeUnknown: Bool = false, only: WorkWorkspaceKey? = nil, unavailable: inout [String]) -> [WorkMemberRef] {
            switch self {
            case .members(let members): return only.map { key in members.filter { $0.work == key } } ?? members
            case .select(let source, let predicate):
                return order.compactMap { key in
                    guard let fact = facts[key], predicate.includes(fact, facts: facts, unavailable: &unavailable) else { return nil }
                    let member = WorkMemberRef(source: source, work: key)
                    guard let state = predicate.state(for: fact) else { return member }
                    if fact.knows(state) { return fact.states.contains(state) ? member : nil }
                    unavailable.append("Task \(fact.name) \(state.isActivity ? "activity" : "state") is unavailable")
                    return includeUnknown || fact.states.contains(state) ? member : nil
                }
            case .compose(let items): return items.flatMap { $0.evaluate(facts, order: order, includeUnknown: includeUnknown, only: only, unavailable: &unavailable) }
            case .intersect(let items):
                var result: [WorkMemberRef]?
                for item in items {
                    let members = item.evaluate(facts, order: order, includeUnknown: includeUnknown, only: only, unavailable: &unavailable)
                    if let previous = result {
                        let keys = Set(members.map(\.work))
                        result = previous.filter { keys.contains($0.work) }
                    } else { result = members }
                }
                return result ?? []
            }
        }
        func resolveMember(_ key: WorkWorkspaceKey, facts: [WorkWorkspaceKey: Facts]) -> WorkSelectionResolution {
            var unavailable: [String] = []
            let members = evaluate(facts, order: [key], only: key, unavailable: &unavailable)
            return .init(attention: [], members: members, unavailable: unavailable)
        }

        func potentialMembers(facts: [Facts]) -> [WorkMemberRef] {
            switch self {
            case .members(let members): members
            case .select(let source, _): facts.map { .init(source: source, work: $0.work) }
            case .compose(let nodes): nodes.flatMap { $0.potentialMembers(facts: facts) }
            case .intersect(let nodes): nodes.first?.potentialMembers(facts: facts) ?? []
            }
        }
    }
    public struct StateEffect: Codable, Equatable, Sendable {
        public let stage: String
        public let domains: [String]
    }
    public let domains: [String]
    public let effects: [String: StateEffect]
    public let root: Node
    public var facts: [Facts]
    public let unavailable: [String]

    fileprivate func possibleMembers() -> Set<WorkMemberRef>? {
        // An unavailable named query has an unknown universe. Unknown activity
        // inside a compiled node still has finite, identifiable candidates.
        guard unavailable.isEmpty else { return nil }
        var reasons: [String] = []
        let order = facts.map(\.work)
        let facts = Dictionary(facts.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first })
        return Set(root.evaluate(facts, order: order, includeUnknown: true, unavailable: &reasons))
    }

    public func resolve() -> WorkSelectionResolution {
        var unavailable = unavailable
        let members = root.evaluate(Dictionary(facts.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first }), order: facts.map(\.work), unavailable: &unavailable)
        let selected = Set(members.map(\.work))
        let attention = facts.filter { selected.contains($0.work) && ($0.states.contains(.available) || $0.states.contains(.blocked)) }.map(\.work)
        return WorkSelectionResolution(attention: attention.sorted { ($0.repository, $0.kind.rawValue, $0.id) < ($1.repository, $1.kind.rawValue, $1.id) }, members: members, unavailable: unavailable)
    }
}

/// Presentation data, independent of its immutable confirmed wire body.
@dynamicMemberLookup
public struct ProjectedWorkPlan: Equatable, Sendable {
    public let membership: WorkMembershipPlan
    public let contents: [WorkViewPlan.Content]
    public let confirmed: WorkViewPlan
    public let resolution: WorkSelectionResolution
    public let arrangement: WorkArrangement
    public let projection: WorkArrangementProjection
    public let states: [WorkWorkspaceKey: [WorkSelectionState]]
    public let facts: [WorkWorkspaceKey: WorkMembershipPlan.Facts]
    let coverage: WorkAnswerCoverage?
    let inventoryDependencies: [WorkAnswerDependency]
    public subscript<T>(dynamicMember keyPath: KeyPath<WorkViewPlan, T>) -> T { confirmed[keyPath: keyPath] }
    /// Unknown membership cannot produce an exact total. Occurrences of the
    /// same Task count once, regardless of arrangement or hidden tiles.
    public var taskCount: Int? {
        guard resolution.unavailable.isEmpty, coverage?.current != false else { return nil }
        return Set(resolution.members.filter { $0.work.kind == .task }.map(\.work)).count
    }

    /// Planning content joins these fields without becoming another membership authority.
    public func inventory(wave: String? = nil) -> ProjectedWorkInventory {
        let tasks = WorkMembershipPlan.Node.select(source: ["inventory"], predicate: .kind(.task))
        let root = wave.map { WorkMembershipPlan.Node.intersect([tasks,
            .select(source: ["scope"], predicate: .descendants(wave: $0))]) } ?? tasks
        let scoped = WorkMembershipPlan(domains: membership.domains, effects: membership.effects,
            root: root, facts: membership.facts, unavailable: membership.unavailable)
        var resolution = scoped.resolve()
        var reasons = inventoryDependencies.filter { !$0.current }.map {
            "Work \($0.work.id) \($0.field.rawValue) awaits confirmation"
        }
        if coverage?.candidatesCurrent == false { reasons.append("Work candidates await confirmation") }
        reasons += membership.facts.filter { $0.work.kind == .task && !$0.lifecycle.known }.map {
            "Work \($0.work.id) lifecycle awaits confirmation"
        }
        resolution = .init(attention: resolution.attention, members: resolution.members,
            unavailable: resolution.unavailable + reasons)
        return ProjectedWorkInventory(membership: scoped, resolution: resolution)
    }

    public func reordering(_ task: String, to target: String) -> Self {
        var members = resolution.members
        guard let from = members.firstIndex(where: { $0.work.kind == .task && $0.work.id == task }),
              let to = members.firstIndex(where: { $0.work.kind == .task && $0.work.id == target }) else { return self }
        members.insert(members.remove(at: from), at: to)
        return Self(confirmed, membership: membership, memberOrder: members, coverage: coverage, contents: contents, inventoryDependencies: inventoryDependencies)
    }

    public init(_ confirmed: WorkViewPlan, membership: WorkMembershipPlan? = nil, memberOrder: [WorkMemberRef]? = nil) {
        self.init(confirmed, membership: membership, memberOrder: memberOrder, coverage: nil)
    }

    init(_ confirmed: WorkViewPlan, membership: WorkMembershipPlan? = nil, memberOrder: [WorkMemberRef]? = nil, coverage: WorkAnswerCoverage?, contents: [WorkViewPlan.Content]? = nil,
         resolution: WorkSelectionResolution? = nil, states: [WorkWorkspaceKey: [WorkSelectionState]]? = nil, inventoryDependencies: [WorkAnswerDependency] = []) {
        self.confirmed = confirmed
        self.inventoryDependencies = inventoryDependencies
        self.contents = contents ?? confirmed.contents
        let membership = membership ?? confirmed.membership
        self.membership = membership
        facts = Dictionary(membership.facts.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first })
        self.coverage = coverage
        var resolution = resolution ?? membership.resolve()
        let membershipIncomplete = !resolution.unavailable.isEmpty
        if let coverage, !coverage.current {
            var reasons = coverage.dependencies.filter { !$0.current }.map {
                "Work \($0.work.id) \($0.field.rawValue) awaits confirmation"
            }
            if !coverage.candidatesCurrent { reasons.append("Work candidates await confirmation") }
            if !coverage.contentCurrent { reasons.append("Work content awaits confirmation") }
            resolution = WorkSelectionResolution(attention: resolution.attention, members: resolution.members,
                unavailable: resolution.unavailable + reasons)
        }
        let contentKeys = Set(self.contents.map(\.work))
        let missing = Set(resolution.members.map(\.work)).subtracting(contentKeys)
        if !missing.isEmpty {
            resolution = WorkSelectionResolution(attention: resolution.attention, members: resolution.members,
                unavailable: resolution.unavailable + missing.map { "Work \($0.id) content awaits confirmation" }.sorted())
        }
        if let memberOrder {
            let selected = Set(resolution.members)
            var ordered = memberOrder.filter { selected.contains($0) }
            let present = Set(ordered)
            ordered.append(contentsOf: resolution.members.filter { !present.contains($0) })
            resolution.members = ordered
        }
        self.resolution = resolution
        self.states = states ?? Dictionary(membership.facts.map { ($0.work, $0.states) }, uniquingKeysWith: { first, _ in first })
        let selected = Set(resolution.members)
        let departures: Set<WorkMemberRef>
        if !resolution.unavailable.isEmpty,
           let previous = confirmed.membership.possibleMembers(),
           let current = membership.possibleMembers() {
            let removed = previous.subtracting(current)
            let gone = Set(removed.map(\.work)).subtracting(current.map(\.work))
            departures = removed.union(confirmed.arrangement.order.filter { gone.contains($0.work) })
        } else { departures = [] }
        // Losing live coverage keeps the last evaluated members. It must not
        // resurrect occurrences removed by a newer normalized field merely
        // because the immutable Machine body still contains them.
        var order = (memberOrder != nil || confirmed.arrangement.mode == .auto) && !membershipIncomplete
            ? resolution.members : confirmed.arrangement.order.filter { (membershipIncomplete && !departures.contains($0)) || selected.contains($0) }
        var present = Set(order)
        for member in resolution.members where present.insert(member).inserted { order.append(member) }
        let hidden = confirmed.arrangement.hidden.filter { present.contains($0) }
        arrangement = WorkArrangement(order: order, hidden: hidden, mode: confirmed.arrangement.mode)
        let hiddenSet = Set(hidden)
        var winners: [WorkWorkspaceKey: WorkMemberRef] = [:]
        var visible: [WorkMemberRef] = []
        var suppressed: [WorkArrangementProjection.Suppressed] = []
        for member in order where !hiddenSet.contains(member) {
            if let winner = winners[member.work] { suppressed.append(.init(member: member, winner: winner)) }
            else { winners[member.work] = member; visible.append(member) }
        }
        projection = WorkArrangementProjection(visible: visible, suppressed: suppressed)
    }
}

/// Projected Task fields in captured order; hosts join their real presentation content.
public struct ProjectedWorkInventory: Equatable, Sendable {
    public let membership: WorkMembershipPlan
    public let resolution: WorkSelectionResolution
}

/// Real detail/history content joined to the query owner's current fields.
/// Unstamped pages never contribute parent or lifecycle facts.
public struct ProjectedTaskHistory: Sendable {
    public struct Row: Identifiable, Sendable {
        public var id: String { task.id }
        public let task: RoadmapTask
        public let lifecycle: WorkMembershipPlan.LifecycleFact?
        public let ownerLabel: String?
        public let locationKnown: Bool

        public var status: String {
            guard locationKnown, let lifecycle, lifecycle.known else { return "Updating…" }
            if lifecycle.states.contains(.done), lifecycle.support == nil || lifecycle.completedAt == nil { return "Done · date unavailable" }
            return TaskSection(states: lifecycle.states).rawValue.capitalized
        }
    }
    public let rows: [Row]
    /// Includes retained rows outside the filter, for an already-selected inspector.
    public let retained: [Row]
    public let count: Int?
    public let incomplete: Bool

    public init(plans: [ProjectedWorkPlan], repository: String, wave: String,
                content: [RoadmapTask], showCompleted: Bool, days: Int, now: Date,
                contentComplete: Bool, historyComplete: Bool) {
        let plans = plans.filter { $0.repository == repository }
        let fields = Dictionary(plans.flatMap { $0.membership.facts }.map { ($0.work, $0) },
            uniquingKeysWith: { first, _ in first })
        let scoped = plans.map { $0.inventory(wave: wave) }
        var incomplete = plans.isEmpty || !contentComplete || (showCompleted && !historyComplete)
            || scoped.contains { !$0.resolution.unavailable.isEmpty }
        var rows: [Row] = []
        var retained: [Row] = []
        var seen = Set<String>()
        for task in content where task.task.repositoryId == repository && seen.insert(task.id).inserted {
            let key = WorkWorkspaceKey(repository: repository, kind: .task, id: task.id)
            let fact = fields[key]
            var reasons: [String] = []
            let belongs = fact.map {
                WorkMembershipPlan.Predicate.descendants(wave: wave).includes($0, facts: fields, unavailable: &reasons)
            }
            let locationKnown = belongs != nil && reasons.isEmpty
            let row = Row(task: task, lifecycle: fact?.lifecycle, ownerLabel: fact?.ownerLabel, locationKnown: locationKnown)
            retained.append(row)
            if locationKnown && belongs == false { continue }
            guard locationKnown, let lifecycle = fact?.lifecycle, lifecycle.known else {
                incomplete = true
                rows.append(row)
                continue
            }
            // Lifecycle alone cannot certify the end of unresolved execution.
            if task.condition.unresolvedExecution { rows.append(row); continue }
            let section = TaskSection(states: lifecycle.states)
            if section != .done && section != .abandoned { rows.append(row); continue }
            guard section == .done, showCompleted else { continue }
            if days == 0 { rows.append(row); continue }
            guard let included = lifecycle.completed(inLast: days, now: now) else {
                incomplete = true
                continue
            }
            if included { rows.append(row) }
        }
        let loaded = Set(retained.map { $0.task.id })
        if scoped.flatMap({ $0.resolution.members }).contains(where: { !loaded.contains($0.work.id) }) { incomplete = true }
        self.rows = rows
        self.retained = retained
        self.incomplete = incomplete
        count = incomplete ? nil : rows.count
    }

    public static func completionDate(_ timestamp: String) -> Date? {
        if let date = try? Date.ISO8601FormatStyle(includingFractionalSeconds: true).parse(timestamp) { return date }
        return try? Date.ISO8601FormatStyle().parse(timestamp)
    }
}
