import Foundation

/// How a repository's plan is kept, as `lf repo tracker --json` answers it:
/// its tracker, or without one the ways the plan reaches other Machines.
public struct RepositoryPlan: Decodable, Sendable, Equatable {
    public let tracker: TrackerKind?
    public let plan: PlanCarriers?

    enum CodingKeys: String, CodingKey { case tracker, plan }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        tracker = try values.decode(TrackerKind?.self, forKey: .tracker)
        plan = try values.decode(PlanCarriers?.self, forKey: .plan)
    }

    /// The line `lf repo tracker` starts with: `Tracker: linear.` or
    /// `Plan: this Machine.`
    public var line: String {
        if let tracker { return "Tracker: \(tracker.rawValue)." }
        return "Plan: this Machine."
    }
}

/// The ways a plan without a tracker reaches other Machines. Every Machine
/// holds the whole plan; with none of these it stays on this one.
public struct PlanCarriers: Decodable, Sendable, Equatable {
    /// The Machine `lf machine hub` names; nil without one.
    public let hub: HubCarrier?

    enum CodingKeys: String, CodingKey { case hub }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        hub = try values.decode(HubCarrier?.self, forKey: .hub)
    }
}

/// The hub's line: `Hub mini, unreachable since 14:02, 2 unsent.`
public struct HubCarrier: Decodable, Sendable, Equatable {
    public let machine: String
    /// How many records changed here the hub has not taken.
    public let unsent: Int
    /// The first of them in change order, by name.
    public let records: [String]
    public let error: String?
    /// Unix seconds since the hub stopped answering.
    public let failingSince: Int?

    enum CodingKeys: String, CodingKey {
        case machine, unsent, records, error
        case failingSince = "failing_since"
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        machine = try values.decode(String.self, forKey: .machine)
        unsent = try values.decode(Int.self, forKey: .unsent)
        records = try values.decode([String].self, forKey: .records)
        error = try values.decode(String?.self, forKey: .error)
        failingSince = try values.decode(Int?.self, forKey: .failingSince)
    }

    public var line: String {
        var line = "Hub \(machine)"
        if let failingSince {
            line += ", unreachable since \(Date(timeIntervalSince1970: TimeInterval(failingSince)).formatted(date: .omitted, time: .shortened))"
        }
        if unsent > 0 { line += ", \(unsent) unsent" }
        return line + "."
    }
}
