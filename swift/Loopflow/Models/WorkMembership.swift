import Foundation

/// A finite program compiled by Rust: identity, scope, occurrence paths and
/// ordering are already resolved. Only selection-state predicates remain.
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
        public let parent: ParentFact
        public var lifecycle: StateFact
        public var activity: StateFact
        public var activityKnown: Bool { activity.known }
        // The query overlay mutates a copy, never the confirmed fields.
        public var states: [WorkSelectionState] {
            get { (lifecycle.states + activity.states).sorted { $0.rawValue < $1.rawValue } }
            set {
                let activityStates: Set<WorkSelectionState> = [.available, .busy, .blocked]
                lifecycle = StateFact(states: newValue.filter { !activityStates.contains($0) }, known: lifecycle.known, support: nil)
                activity = StateFact(states: newValue.filter { activityStates.contains($0) }, known: activity.known, support: nil)
            }
        }
    }
    public indirect enum Node: Codable, Equatable, Sendable {
        case members([WorkMemberRef])
        case state(WorkSelectionState, [WorkMemberRef])
        case compose([Node])
        case intersect([Node])

        private enum Keys: String, CodingKey { case kind, members, state, items }
        private enum Kind: String, Codable { case members, state, compose, intersect }
        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: Keys.self)
            switch try c.decode(Kind.self, forKey: .kind) {
            case .members: self = .members(try c.decode([WorkMemberRef].self, forKey: .members))
            case .state: self = .state(try c.decode(WorkSelectionState.self, forKey: .state), try c.decode([WorkMemberRef].self, forKey: .members))
            case .compose: self = .compose(try c.decode([Node].self, forKey: .items))
            case .intersect: self = .intersect(try c.decode([Node].self, forKey: .items))
            }
        }
        public func encode(to encoder: any Encoder) throws {
            var c = encoder.container(keyedBy: Keys.self)
            switch self {
            case .members(let members):
                try c.encode(Kind.members, forKey: .kind); try c.encode(members, forKey: .members)
            case .state(let state, let members):
                try c.encode(Kind.state, forKey: .kind); try c.encode(state, forKey: .state); try c.encode(members, forKey: .members)
            case .compose(let items):
                try c.encode(Kind.compose, forKey: .kind); try c.encode(items, forKey: .items)
            case .intersect(let items):
                try c.encode(Kind.intersect, forKey: .kind); try c.encode(items, forKey: .items)
            }
        }

        fileprivate func evaluate(_ facts: [WorkWorkspaceKey: Facts], knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>], includeUnknown: Bool = false, unavailable: inout [String]) -> [WorkMemberRef] {
            switch self {
            case .members(let members): return members
            case .state(let state, let members):
                return members.filter { member in
                    guard let fact = facts[member.work] else {
                        unavailable.append("Work \(member.work.id) facts are unavailable"); return includeUnknown
                    }
                    if fact.states.contains(state) { return true }
                    if knownAbsent[member.work]?.contains(state) == true { return false }
                    if !fact.activityKnown && [.available, .busy, .blocked].contains(state) {
                        unavailable.append("Task \(fact.name) activity is unavailable")
                        return includeUnknown
                    }
                    return false
                }
            case .compose(let items): return items.flatMap { $0.evaluate(facts, knownAbsent: knownAbsent, includeUnknown: includeUnknown, unavailable: &unavailable) }
            case .intersect(let items):
                var result: [WorkMemberRef]?
                for item in items {
                    let members = item.evaluate(facts, knownAbsent: knownAbsent, includeUnknown: includeUnknown, unavailable: &unavailable)
                    if let previous = result {
                        let keys = Set(members.map(\.work))
                        result = previous.filter { keys.contains($0.work) }
                    } else { result = members }
                }
                return result ?? []
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
        let facts = Dictionary(facts.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first })
        return Set(root.evaluate(facts, knownAbsent: knownAbsent, includeUnknown: true, unavailable: &reasons))
    }

    public func resolve(knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>] = [:]) -> WorkSelectionResolution {
        var unavailable = unavailable
        let members = root.evaluate(Dictionary(facts.map { ($0.work, $0) }, uniquingKeysWith: { first, _ in first }), knownAbsent: knownAbsent, unavailable: &unavailable)
        let selected = Set(members.map(\.work))
        let attention = facts.filter { selected.contains($0.work) && ($0.states.contains(.available) || $0.states.contains(.blocked)) }.map(\.work)
        return WorkSelectionResolution(attention: attention.sorted { ($0.repository, $0.kind.rawValue, $0.id) < ($1.repository, $1.kind.rawValue, $1.id) }, members: members, unavailable: unavailable)
    }
}

/// Presentation data, independent of its immutable confirmed wire body.
@dynamicMemberLookup
public struct ProjectedWorkPlan: Equatable, Sendable {
    private let membership: WorkMembershipPlan
    private let knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>]
    public let confirmed: WorkViewPlan
    public let resolution: WorkSelectionResolution
    public let arrangement: WorkArrangement
    public let projection: WorkArrangementProjection
    public let states: [WorkWorkspaceKey: [WorkSelectionState]]
    public subscript<T>(dynamicMember keyPath: KeyPath<WorkViewPlan, T>) -> T { confirmed[keyPath: keyPath] }
    /// Unknown membership cannot produce an exact total. Occurrences of the
    /// same Task count once, regardless of arrangement or hidden tiles.
    public var taskCount: Int? {
        guard resolution.unavailable.isEmpty else { return nil }
        return Set(resolution.members.filter { $0.work.kind == .task }.map(\.work)).count
    }

    public func reordering(_ task: String, to target: String) -> Self {
        var members = resolution.members
        guard let from = members.firstIndex(where: { $0.work.kind == .task && $0.work.id == task }),
              let to = members.firstIndex(where: { $0.work.kind == .task && $0.work.id == target }) else { return self }
        members.insert(members.remove(at: from), at: to)
        return Self(confirmed, membership: membership, knownAbsent: knownAbsent, memberOrder: members)
    }

    public init(_ confirmed: WorkViewPlan, membership: WorkMembershipPlan? = nil, knownAbsent: [WorkWorkspaceKey: Set<WorkSelectionState>] = [:], memberOrder: [WorkMemberRef]? = nil) {
        self.confirmed = confirmed
        let membership = membership ?? confirmed.membership
        self.membership = membership
        self.knownAbsent = knownAbsent
        var resolution = membership.resolve(knownAbsent: knownAbsent)
        let contentKeys = Set(confirmed.contents.map(\.work))
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
        states = Dictionary(membership.facts.map { ($0.work, $0.states) }, uniquingKeysWith: { first, _ in first })
        let selected = Set(resolution.members)
        let departures: Set<WorkMemberRef>
        if !resolution.unavailable.isEmpty, !knownAbsent.isEmpty,
           let previous = confirmed.membership.possibleMembers(),
           let current = membership.possibleMembers(knownAbsent: knownAbsent) {
            departures = previous.subtracting(current)
        } else { departures = [] }
        var order = (memberOrder != nil || confirmed.arrangement.mode == .auto) && resolution.unavailable.isEmpty
            ? resolution.members : confirmed.arrangement.order.filter { (!resolution.unavailable.isEmpty && !departures.contains($0)) || selected.contains($0) }
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
