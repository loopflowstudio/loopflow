import Foundation

/// Semantic effects understood by the shared client. An unimplemented command
/// remains inspectable; a finished transport result cannot invent its contract.
public struct OperationDeclaration {
    let stages: Set<String>
    let requiresPresentation: Bool
    let kind: Kind

    enum Kind: String {
        case create = "task.create", newSession = "session.new", complete = "task.complete", reopen = "task.reopen"
        case rename = "session.rename"
        case run = "task.run", ship = "task.ship", hold = "task.hold"
        case update = "task.update", move = "task.move", abandon = "task.abandon", claim = "task.claim", unassign = "task.unassign"
    }

    var label: String {
        switch kind {
        case .create: "New Task"
        case .newSession: "New Session"
        case .complete: "Complete Task"
        case .reopen: "Reopen Task"
        case .update: "Update Task"
        case .move: "Move Task"
        case .abandon: "Abandon Task"
        case .claim: "Claim Task"
        case .unassign: "Unassign Task"
        case .rename: "Rename Session"
        case .run: "Start Task"
        case .ship: "Auto-Ship"
        case .hold: "Stop Auto-Ship"
        }
    }

    public static let completionProgress = "Completing…"

    public var progress: String? {
        switch kind {
        case .create, .newSession: "Starting conversation…"
        case .complete: Self.completionProgress
        case .run: "Start requested…"
        case .ship: "Auto-Ship requested…"
        case .hold: "Stop Auto-Ship requested…"
        case .reopen, .update, .move, .abandon, .claim, .unassign, .rename: nil
        }
    }

    public var isExecution: Bool { kind == .run || kind == .ship || kind == .hold }

    var changesMembership: Bool { kind == .complete || kind == .abandon || kind == .move || kind == .claim || kind == .unassign || kind == .reopen }

    /// The field written by the semantic stage, independent of query scope.
    var fields: Set<WorkField> {
        switch kind {
        case .complete, .abandon: [.disposition]
        case .reopen: [.disposition, .execution]
        case .claim, .unassign: [.ownership]
        case .move: [.parent]
        default: []
        }
    }

    func effect(id: String, sequence: Int64, directory: String, issue: String, wave: String? = nil, work: WorkWorkspaceKey? = nil, receipt: OperationReceipt?) -> WorkQueryEffect? {
        guard changesMembership else { return nil }
        return WorkQueryEffect(operationID: id, sequence: sequence, directory: directory, kind: kind.rawValue, issue: issue, wave: wave, work: work,
            resolution: receipt?.commits.first { stages.contains($0.stage) })
    }

    public init?(kind: String) {
        guard let declared = Kind(rawValue: kind) else { return nil }
        self.kind = declared
        requiresPresentation = kind == "task.create" || kind == "session.new"
        switch declared {
        case .create: stages = ["task.filed"]
        case .newSession: stages = ["session.input_replaced"]
        case .complete: stages = ["task.completed"]
        case .reopen: stages = ["task.reopened"]
        case .update: stages = ["task.updated"]
        case .move: stages = ["task.membership_changed"]
        case .abandon: stages = ["task.abandoned"]
        case .claim: stages = ["task.claimed"]
        case .unassign: stages = ["task.unassigned"]
        case .rename: stages = ["session.renamed"]
        case .run, .ship, .hold: stages = []
        }
    }

    func resolutions(in receipt: OperationReceipt) -> [OperationCommit]? {
        // These commands acknowledge dispatch only. No SQLite stage can prove
        // Graph/PR success or Process exit. An unknown dispatch stays held;
        // a finished failure establishes order without proving rollback.
        if isExecution {
            guard receipt.validation == .bound, receipt.delivery == .finished else { return nil }
            return []
        }
        let resolutions = receipt.commits.filter { stages.contains($0.stage) }
        guard stages.isSubset(of: Set(resolutions.map(\.stage))) else { return nil }
        return resolutions
    }
}
