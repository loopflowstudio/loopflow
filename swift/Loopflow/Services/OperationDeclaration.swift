import Foundation

/// Semantic effects understood by the shared client. An unimplemented command
/// remains inspectable; a finished transport result cannot invent its contract.
struct OperationDeclaration {
    let stages: Set<String>
    let requiresPresentation: Bool
    let kind: Kind

    enum Kind: String {
        case create = "task.create", newSession = "session.new", complete = "task.complete", reopen = "task.reopen"
        case update = "task.update", move = "task.move", abandon = "task.abandon", claim = "task.claim"
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
        }
    }

    var changesMembership: Bool { kind == .complete || kind == .abandon }

    func effect(id: String, sequence: Int64, directory: String, issue: String, receipt: OperationReceipt?) -> WorkQueryEffect? {
        guard changesMembership else { return nil }
        return WorkQueryEffect(operationID: id, sequence: sequence, directory: directory, kind: kind.rawValue, issue: issue,
            resolution: receipt?.commits.first { stages.contains($0.stage) })
    }

    init?(kind: String) {
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
        }
    }

    func resolutions(in receipt: OperationReceipt) -> [OperationCommit]? {
        let resolutions = receipt.commits.filter { stages.contains($0.stage) }
        guard stages.isSubset(of: Set(resolutions.map(\.stage))) else { return nil }
        return resolutions
    }
}
