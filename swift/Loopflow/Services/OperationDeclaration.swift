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
        }
    }

    public static let completionProgress = "Completing…"

    public var progress: String? {
        switch kind {
        case .create, .newSession: "Starting conversation…"
        case .complete: Self.completionProgress
        case .reopen, .update, .move, .abandon, .claim, .unassign, .rename: nil
        }
    }

    var changesMembership: Bool { kind == .complete || kind == .abandon || kind == .move }

    /// The field written by the semantic stage, independent of query scope.
    var field: WorkField? {
        switch kind {
        case .complete, .abandon, .reopen: .lifecycle
        case .move: .parent
        default: nil
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
        }
    }

    func resolutions(in receipt: OperationReceipt) -> [OperationCommit]? {
        let resolutions = receipt.commits.filter { stages.contains($0.stage) }
        guard stages.isSubset(of: Set(resolutions.map(\.stage))) else { return nil }
        return resolutions
    }
}
