import Foundation

/// Where a repository's plan lives, as `lf repo tracker --json` answers it:
/// its tracker, or without one the Machine that holds the plan.
public struct RepositoryPlan: Decodable, Sendable, Equatable {
    public let tracker: TrackerKind?
    public let plan: PlanHome?

    enum CodingKeys: String, CodingKey { case tracker, plan }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        tracker = try values.decode(TrackerKind?.self, forKey: .tracker)
        plan = try values.decode(PlanHome?.self, forKey: .plan)
    }

    /// The line `lf repo tracker` starts with: `Tracker: linear.`,
    /// `Plan: hub mini. 1 open.`, `Plan: this Machine (hub).`
    public var line: String {
        if let tracker { return "Tracker: \(tracker.rawValue)." }
        guard let plan else { return "Plan: this Machine." }
        guard let hub = plan.hub else { return plan.isHub ? "Plan: this Machine (hub)." : "Plan: this Machine." }
        let place = plan.unreachableSince.map { since in
            "hub \(hub), unreachable since \(Date(timeIntervalSince1970: TimeInterval(since)).formatted(date: .omitted, time: .shortened))"
        } ?? "hub \(hub)"
        return plan.open.isEmpty ? "Plan: \(place). Nothing open." : "Plan: \(place). \(plan.open.count) open."
    }
}

/// The Machine that holds a plan without a tracker.
public struct PlanHome: Decodable, Sendable, Equatable {
    /// The hub's name when the plan is on another Machine; nil on this one.
    public let hub: String?
    public let isHub: Bool
    /// Unix seconds since the hub stopped answering.
    public let unreachableSince: Int?
    public let error: String?
    /// Values saved here the hub has yet to confirm, in send order.
    public let open: [HubOpen]

    enum CodingKeys: String, CodingKey {
        case hub, error, open
        case isHub = "is_hub"
        case unreachableSince = "unreachable_since"
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        hub = try values.decode(String?.self, forKey: .hub)
        isHub = try values.decode(Bool.self, forKey: .isHub)
        unreachableSince = try values.decode(Int?.self, forKey: .unreachableSince)
        error = try values.decode(String?.self, forKey: .error)
        open = try values.decode([HubOpen].self, forKey: .open)
    }
}

/// One value on its way to the hub: `3e1c9a07  title · queued`.
public struct HubOpen: Decodable, Sendable, Equatable, Identifiable {
    public let owner: String
    public let field: String
    public let phase: String

    public var id: String { "\(owner) \(field)" }
}
