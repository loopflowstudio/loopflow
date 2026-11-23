#if os(macOS)
import Loopflow
import SwiftUI

/// Both Wave surfaces render the Rust reading; none of these labels mutate readiness.
struct ProjectReadinessView: View {
    @Environment(\.palette) private var palette
    let readiness: ProjectReadiness?
    let isPending: Bool
    let transportError: String?
    let retry: () -> Void

    /// What the Machine said when preparation failed.
    private var failure: String? { transportError ?? readiness?.activation?.error }

    /// Where the Project stands when nothing failed: a sentence, not a message.
    private var standing: String? {
        guard let readiness else { return nil }
        if let activation = readiness.activation {
            // An activation whose lf died unended reads `unknown` with no end.
            if let outcome = activation.outcome, outcome != "succeeded" { return "Project activation \(outcome)." }
            if activation.completedAt == nil { return "Project activation has no recorded outcome yet." }
        }
        switch readiness.state {
        case .ready: return nil
        case .unconfigured: return "No Project selected yet."
        case .unavailable: return "Selected Project evidence is unavailable. Retained planning is shown below."
        case .inactive: return "Selected Project is not In Progress."
        case .terminal: return "Selected Project is completed or canceled. Choose its successor explicitly."
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            if isPending {
                ProgressView("Preparing Project…")
            } else if let failure {
                FailureLine("Project not prepared.", machine: failure) { retryButton }
            } else if let standing {
                Text(standing).foregroundStyle(palette.warning).textSelection(.enabled)
                retryButton
            }
            if let successor = readiness?.pendingSuccessor {
                Text("Project transition remains unresolved: \(successor)")
                    .font(Typography.meta).textSelection(.enabled)
            }
        }
        .accessibilityIdentifier("project-readiness")
    }

    @ViewBuilder private var retryButton: some View {
        if readiness?.state != .terminal { Button("Retry Project preparation", action: retry).buttonStyle(LoopflowButtonStyle()) }
    }
}
#endif
