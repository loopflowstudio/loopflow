#if os(macOS)
import Loopflow
import SwiftUI

/// Both Wave surfaces render the Rust reading; none of these labels mutate readiness.
struct ProjectReadinessView: View {
    let readiness: ProjectReadiness?
    let isPending: Bool
    let transportError: String?
    let retry: () -> Void

    private var message: String? {
        if let transportError { return transportError }
        guard let readiness else { return nil }
        if let error = readiness.activation?.error { return error }
        if let activation = readiness.activation {
            if activation.completedAt == nil { return "Project activation has no recorded outcome yet." }
            if activation.outcome != "succeeded" { return "Project activation \(activation.outcome ?? "outcome unknown")." }
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
            } else if let message {
                Text(message).foregroundStyle(Color.statusWarning).textSelection(.enabled)
                if readiness?.state != .terminal { Button("Retry Project preparation", action: retry) }
            }
            if let successor = readiness?.pendingSuccessor {
                Text("Project transition remains unresolved: \(successor)")
                    .font(Typography.caption()).textSelection(.enabled)
            }
        }
        .accessibilityIdentifier("project-readiness")
    }
}
#endif
