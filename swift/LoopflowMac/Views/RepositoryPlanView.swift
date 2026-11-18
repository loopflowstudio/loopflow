import Loopflow
import SwiftUI

/// Where the repository's plan lives, the line `lf repo tracker` prints, and
/// each value still on its way to the hub.
struct RepositoryPlanView: View {
    let reading: Reading<RepositoryPlan>

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            if let plan = reading.value {
                Text(plan.line)
                if let home = plan.plan {
                    ForEach(home.open) { open in
                        Text("\(open.owner)  \(open.field) · \(open.phase)")
                    }
                    if let error = home.error { Text(error) }
                }
            } else if reading.isLoading {
                Text("Reading where the plan lives…")
            }
        }
        .font(Typography.body(12))
        .foregroundStyle(palette.textSecondary)
        .textSelection(.enabled)
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityIdentifier("repository-plan")
    }
}
