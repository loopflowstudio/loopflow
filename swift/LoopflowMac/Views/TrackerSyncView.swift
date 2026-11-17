import Loopflow
import SwiftUI

struct TrackerSyncView: View {
    let sync: TrackerSync

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            ForEach(sync.actions) { action in
                Text(action.text)
                    .font(Typography.body(12))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("tracker-sync-\(action.id)")
            }
        }
        .accessibilityIdentifier("tracker-sync")
    }
}
