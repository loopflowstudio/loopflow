import Loopflow
import SwiftUI

/// How the repository's plan travels, as `lf repo tracker` prints it: the
/// hub's line and each record it has yet to take.
struct RepositoryPlanView: View {
    let reading: Reading<RepositoryPlan>

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            if let plan = reading.value {
                Text(plan.line)
                if let hub = plan.plan?.hub {
                    Text(hub.line)
                    ForEach(hub.records, id: \.self) { Text($0) }
                    if hub.unsent > hub.records.count {
                        Text("and \(hub.unsent - hub.records.count) more")
                    }
                    if let error = hub.error { Text(error) }
                }
            } else if reading.isLoading {
                Text("Reading how the plan travels…")
            }
        }
        .font(Typography.body(12))
        .foregroundStyle(palette.textSecondary)
        .textSelection(.enabled)
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityIdentifier("repository-plan")
    }
}
