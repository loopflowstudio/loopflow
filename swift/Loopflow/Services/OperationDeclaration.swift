import Foundation

/// Semantic effects understood by the shared client. An unimplemented command
/// remains inspectable; a finished transport result cannot invent its contract.
struct OperationDeclaration {
    let stages: Set<String>
    let requiresPresentation: Bool

    init?(kind: String) {
        requiresPresentation = kind == "task.create" || kind == "session.new"
        switch kind {
        case "task.create": stages = ["task.filed"]
        case "session.new": stages = ["session.input_replaced"]
        case "task.complete": stages = ["task.completed"]
        case "task.reopen": stages = ["task.reopened"]
        case "task.update": stages = ["task.updated"]
        case "task.move": stages = ["task.membership_changed"]
        default: return nil
        }
    }

    func resolutions(in receipt: OperationReceipt) -> [OperationCommit]? {
        let resolutions = receipt.commits.filter { stages.contains($0.stage) }
        guard stages.isSubset(of: Set(resolutions.map(\.stage))) else { return nil }
        return resolutions
    }
}
