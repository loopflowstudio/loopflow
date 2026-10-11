import Loopflow
import SwiftUI

struct PlanningSyncView: View {
    let sync: PlanningSyncStatus

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            ForEach(sync.changes) { change in
                Text(change.text)
                    .font(Typography.body(12))
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("planning-sync-\(change.id)")
            }
        }
        .accessibilityIdentifier("planning-sync")
    }
}
