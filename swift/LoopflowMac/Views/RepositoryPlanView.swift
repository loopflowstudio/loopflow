import Loopflow
import SwiftUI

/// How the repository's plan travels, as `lf repo tracker` prints it: a line
/// for Git and one for the hub, each with the records it has yet to take.
struct RepositoryPlanView: View {
    let reading: Reading<RepositoryPlan>

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            if let plan = reading.value {
                Text(plan.line)
                if let git = plan.plan?.git { standing(git.standing) }
                if let hub = plan.plan?.hub {
                    Text(hub.line)
                    standing(hub.standing)
                }
            } else if reading.isLoading {
                Text("Reading how the plan travels…")
            }
        }
        .font(Typography.text)
        .foregroundStyle(palette.textSecondary)
        .textSelection(.enabled)
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityIdentifier("repository-plan")
    }

    /// A carrier's unsent records, why it did not take them, and why this
    /// Machine could not write what it brought.
    @ViewBuilder
    private func standing(_ standing: CarrierStanding) -> some View {
        ForEach(standing.records, id: \.self) { Text($0) }
        if standing.unsent > standing.records.count {
            Text("and \(standing.unsent - standing.records.count) more")
        }
        if let error = standing.error { Text(error) }
        if let notTaken = standing.notTaken { Text(notTaken.reason) }
    }
}
