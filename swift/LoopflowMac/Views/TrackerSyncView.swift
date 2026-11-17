import Loopflow
import SwiftUI

/// A Task's or Project's tracker lines. A lost line on a Task offers Keep
/// theirs / Use mine when `onAnswer` is given.
struct TrackerSyncView: View {
    let sync: TrackerSync
    var onAnswer: ((TrackerActionStatus, LostAnswer) -> Void)?

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            ForEach(sync.actions) { action in
                HStack(spacing: Spacing.sm) {
                    Text(action.text)
                        .font(Typography.body(12))
                        .foregroundStyle(palette.textSecondary)
                        .textSelection(.enabled)
                        .accessibilityIdentifier("tracker-sync-\(action.id)")
                    if action.phase == .conflicted, let onAnswer {
                        Button("Keep \(action.tracker.displayName)'s") { onAnswer(action, .keepTheirs) }
                            .accessibilityIdentifier("tracker-keep-theirs-\(action.id)")
                        Button("Use mine") { onAnswer(action, .useMine) }
                            .accessibilityIdentifier("tracker-use-mine-\(action.id)")
                    }
                }
                .buttonStyle(.borderless)
                .font(Typography.body(12))
            }
        }
        .accessibilityIdentifier("tracker-sync")
    }
}
