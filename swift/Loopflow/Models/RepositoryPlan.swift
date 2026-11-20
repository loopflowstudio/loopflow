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

    /// The line `lf repo tracker` starts with: `Tracker: linear.`,
    /// `Plan: Git origin.` or `Plan: this Machine.`
    public var line: String {
        if let tracker { return "Tracker: \(tracker.rawValue)." }
        guard let git = plan?.git else { return "Plan: this Machine." }
        return git.standing.line("Plan: Git \(git.remote)")
    }
}

/// The ways a plan without a tracker reaches other Machines. Every Machine
/// holds the whole plan; with none of these it stays on this one.
public struct PlanCarriers: Decodable, Sendable, Equatable {
    /// The repository's remote; nil without one.
    public let git: GitCarrier?
    /// The Machine `lf machine hub` names; nil without one.
    public let hub: HubCarrier?

    enum CodingKeys: String, CodingKey { case git, hub }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        git = try values.decode(GitCarrier?.self, forKey: .git)
        hub = try values.decode(HubCarrier?.self, forKey: .hub)
    }
}

/// Git's line: `Plan: Git origin, failing since 14:02, 3 unsent.`
public struct GitCarrier: Decodable, Sendable, Equatable {
    public let remote: String
    public let standing: CarrierStanding

    enum CodingKeys: String, CodingKey { case remote }

    public init(from decoder: Decoder) throws {
        remote = try decoder.container(keyedBy: CodingKeys.self).decode(String.self, forKey: .remote)
        standing = try CarrierStanding(from: decoder)
    }
}

/// The hub's line: `Hub mini, failing since 14:02, 2 unsent.`
public struct HubCarrier: Decodable, Sendable, Equatable {
    public let machine: String
    public let standing: CarrierStanding

    enum CodingKeys: String, CodingKey { case machine }

    public init(from decoder: Decoder) throws {
        machine = try decoder.container(keyedBy: CodingKeys.self).decode(String.self, forKey: .machine)
        standing = try CarrierStanding(from: decoder)
    }

    public var line: String { standing.line("Hub \(machine)") }
}

/// What a carrier has yet to take, and why when it cannot.
public struct CarrierStanding: Decodable, Sendable, Equatable {
    /// How many records changed here the carrier has not taken.
    public let unsent: Int
    /// The first of them in change order, by name.
    public let records: [String]
    public let error: String?
    /// Unix seconds since the carrier stopped working.
    public let failingSince: Int?
    /// What the carrier's last read brought that this Machine could not
    /// write.
    public let notTaken: NotTaken?

    public struct NotTaken: Decodable, Sendable, Equatable {
        /// How many records.
        public let count: Int
        /// Why the first was not, and what to do about it.
        public let reason: String
    }

    enum CodingKeys: String, CodingKey {
        case unsent, records, error
        case failingSince = "failing_since"
        case notTaken = "not_taken"
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        unsent = try values.decode(Int.self, forKey: .unsent)
        records = try values.decode([String].self, forKey: .records)
        error = try values.decode(String?.self, forKey: .error)
        failingSince = try values.decode(Int?.self, forKey: .failingSince)
        notTaken = try values.decode(NotTaken?.self, forKey: .notTaken)
    }

    /// `name`, then since when it has not worked, how much is unsent and how
    /// much was not taken.
    public func line(_ name: String) -> String {
        var line = name
        if let failingSince {
            line += ", failing since \(Date(timeIntervalSince1970: TimeInterval(failingSince)).formatted(date: .omitted, time: .shortened))"
        }
        if unsent > 0 { line += ", \(unsent) unsent" }
        if let notTaken { line += ", \(notTaken.count) not taken" }
        return line + "."
    }
}
