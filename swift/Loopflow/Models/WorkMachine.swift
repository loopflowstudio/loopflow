import Foundation

/// `lf task place --json` / `lf wave place --json`: where Work executes, why,
/// and the Machines placement may choose. Mirrors Rust `durable::WorkMachine`.
public struct WorkMachine: Codable, Equatable, Sendable {
    public let work: Ref
    /// Absent only for a Task that started before its Machine was recorded.
    public let machineID: String?
    /// What fixes a Task's Machine; absent for Waves.
    public let execution: Execution?
    public let placement: Placement
    public let localMachineID: String
    /// The local Machine, then saved Machines by label.
    public let machines: [Machine]

    public struct Ref: Codable, Equatable, Hashable, Sendable {
        public let kind: String
        public let id: String
    }

    public enum Execution: String, Codable, Sendable {
        case recordedCheckout = "recorded_checkout"
        case admission
        case effectiveDelegation = "effective_delegation"
    }

    public struct Placement: Codable, Equatable, Sendable {
        public let work: Ref
        /// The Work (or ancestor) supplying the assignment.
        public let source: Ref
        public let provenance: Provenance
        public let machineID: String
        public let placedAt: String

        enum CodingKeys: String, CodingKey {
            case work, source, provenance
            case machineID = "machine_id", placedAt = "placed_at"
        }
    }

    public enum Provenance: String, Codable, Sendable {
        case explicit, legacy
        case localDefault = "local_default"
    }

    public struct Machine: Codable, Equatable, Identifiable, Sendable {
        public let label: String?
        public let repo: String?
        public let id: String
        public let route: String
        public let createdAt: String
        public let observedAt: String

        enum CodingKeys: String, CodingKey {
            case label, repo, id, route
            case createdAt = "created_at", observedAt = "observed_at"
        }
    }

    enum CodingKeys: String, CodingKey {
        case work, execution, placement, machines
        case machineID = "machine_id", localMachineID = "local_machine_id"
    }

    /// Only placement moves unstarted work; a started or admitted Task stays
    /// where it is, and moving it is explicit takeover.
    public var followsPlacement: Bool { execution == nil || execution == .effectiveDelegation }

    /// The Work's own assignment, as opposed to one inherited from an ancestor.
    public var placedHere: Bool { placement.source == work && placement.provenance != .localDefault }

    public func name(of id: String) -> String {
        if let label = machines.first(where: { $0.id == id })?.label { return label }
        return id == localMachineID ? "This Mac" : id
    }
}

/// The Work a Machine control reads and places.
public enum WorkMachineTarget: Hashable, Sendable {
    case task(issue: String)
    case wave(id: String)
}

public enum WorkMachinePlacement: Hashable, Sendable {
    case machine(String)
    /// Clear a Task's own Machine so it follows its Wave again.
    case inherit
}
