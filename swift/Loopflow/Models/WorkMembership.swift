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
    public struct Facts: Codable, Equatable, Sendable {
        public let work: WorkWorkspaceKey
        public let name: String
        public var parent: ParentFact
        public var lifecycle: StateFact
        public var activity: StateFact
        public var activityKnown: Bool { activity.known }
        public var states: [WorkSelectionState] {
            (lifecycle.states + activity.states).sorted { $0.rawValue < $1.rawValue }
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

        fileprivate func evaluate(_ facts: [WorkWorkspaceKey: Facts], order: [WorkWorkspaceKey], knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>], includeUnknown: Bool = false, only: WorkWorkspaceKey? = nil, unavailable: inout [String]) -> [WorkMemberRef] {
            switch self {
            case .members(let members): return only.map { key in members.filter { $0.work == key } } ?? members
            case .select(let source, let predicate):
                return order.compactMap { key in
                    guard let fact = facts[key], predicate.includes(fact, facts: facts, unavailable: &unavailable) else { return nil }
                    let member = WorkMemberRef(source: source, work: key)
                    guard let state = predicate.state(for: fact) else { return member }
                    if fact.states.contains(state) { return member }
                    if knownAbsent[key]?.contains(state) == true { return nil }
                    if !fact.activityKnown && state.isActivity {
                        unavailable.append("Task \(fact.name) activity is unavailable")
                        return includeUnknown ? member : nil
                    }
                    return nil
                }
            case .compose(let items): return items.flatMap { $0.evaluate(facts, order: order, knownAbsent: knownAbsent, includeUnknown: includeUnknown, only: only, unavailable: &unavailable) }
            case .intersect(let items):
                var result: [WorkMemberRef]?
                for item in items {
                    let members = item.evaluate(facts, order: order, knownAbsent: knownAbsent, includeUnknown: includeUnknown, only: only, unavailable: &unavailable)
                    if let previous = result {
                        let keys = Set(members.map(\.work))
                        result = previous.filter { keys.contains($0.work) }
                    } else { result = members }
                }
                return result ?? []
            }
        }
        func resolveMember(_ key: WorkWorkspaceKey, facts: [WorkWorkspaceKey: Facts],
                           knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>]) -> WorkSelectionResolution {
            var unavailable: [String] = []
            let members = evaluate(facts, order: [key], knownAbsent: knownAbsent, only: key, unavailable: &unavailable)
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
        public let remove: [WorkSelectionState]
        public let insert: [WorkSelectionState]
    }
    public let domains: [String]
    public let effects: [String: StateEffect]
    public let root: Node
    public var facts: [Facts]
    public let unavailable: [String]

    fileprivate func possibleMembers(knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>] = [:]) -> Set<WorkMemberRef>? {
        // An unavailable named query has an unknown universe. Unknown activity
        // inside a compiled node still has finite, identifiable candidates.
        guard unavailable.isEmpty else { return nil }
        var reasons: [String] = []
        let order = facts.map(\.work)
        let facts = Dictionary(facts.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first })
        return Set(root.evaluate(facts, order: order, knownAbsent: knownAbsent, includeUnknown: true, unavailable: &reasons))
    }

    public func resolve(knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>] = [:]) -> WorkSelectionResolution {
        var unavailable = unavailable
        let members = root.evaluate(Dictionary(facts.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first }), order: facts.map(\.work), knownAbsent: knownAbsent, unavailable: &unavailable)
        let selected = Set(members.map(\.work))
        let attention = facts.filter { selected.contains($0.work) && ($0.states.contains(.available) || $0.states.contains(.blocked)) }.map(\.work)
        return WorkSelectionResolution(attention: attention.sorted { ($0.repository, $0.kind.rawValue, $0.id) < ($1.repository, $1.kind.rawValue, $1.id) }, members: members, unavailable: unavailable)
    }
}

/// Presentation data, independent of its immutable confirmed wire body.
@dynamicMemberLookup
public struct ProjectedWorkPlan: Equatable, Sendable {
    public let membership: WorkMembershipPlan
    private let knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>]
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
        var resolution = scoped.resolve(knownAbsent: knownAbsent)
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
        return Self(confirmed, membership: membership, knownAbsent: knownAbsent, memberOrder: members, coverage: coverage, contents: contents, inventoryDependencies: inventoryDependencies)
    }

    public init(_ confirmed: WorkViewPlan, membership: WorkMembershipPlan? = nil, knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>] = [:], memberOrder: [WorkMemberRef]? = nil) {
        self.init(confirmed, membership: membership, knownAbsent: knownAbsent, memberOrder: memberOrder, coverage: nil)
    }

    init(_ confirmed: WorkViewPlan, membership: WorkMembershipPlan? = nil, knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>] = [:], memberOrder: [WorkMemberRef]? = nil, coverage: WorkAnswerCoverage?, contents: [WorkViewPlan.Content]? = nil,
         resolution: WorkSelectionResolution? = nil, states: [WorkWorkspaceKey: [WorkSelectionState]]? = nil, inventoryDependencies: [WorkAnswerDependency] = []) {
        self.confirmed = confirmed
        self.inventoryDependencies = inventoryDependencies
        self.contents = contents ?? confirmed.contents
        let membership = membership ?? confirmed.membership
        self.membership = membership
        facts = Dictionary(membership.facts.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first })
        self.knownAbsent = knownAbsent
        self.coverage = coverage
        var resolution = resolution ?? membership.resolve(knownAbsent: knownAbsent)
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
        if !resolution.unavailable.isEmpty, !knownAbsent.isEmpty,
           let previous = confirmed.membership.possibleMembers(),
           let current = membership.possibleMembers(knownAbsent: knownAbsent) {
            departures = previous.subtracting(current)
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
        public let lifecycle: WorkMembershipPlan.StateFact?
        public let locationKnown: Bool

        public var status: String {
            guard locationKnown, let lifecycle, lifecycle.known else { return "Updating…" }
            if lifecycle.states.contains(.done), task.task.completedAt == nil { return "Done · date unavailable" }
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
            let row = Row(task: task, lifecycle: fact?.lifecycle, locationKnown: locationKnown)
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
            // Pages carry dates as display content, without field provenance.
            // They can filter retained content, but cannot certify a dated total.
            incomplete = true
            guard let timestamp = task.task.completedAt,
                  let date = Self.completionDate(timestamp), task.lifecycle == .done else {
                incomplete = true
                continue
            }
            if date <= now && date >= now.addingTimeInterval(-Double(days) * 86_400) { rows.append(row) }
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
