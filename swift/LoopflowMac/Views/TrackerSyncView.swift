import Loopflow
import SwiftUI

/// A Task's or Project's tracker lines in `repo`. A lost line offers Keep
/// theirs / Use mine wherever the window installs `answerLost`.
struct TrackerSyncView: View {
    let sync: TrackerSync
    let repo: String

    @Environment(\.palette) private var palette
    @Environment(\.answerLost) private var answerLost

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            ForEach(sync.actions) { action in
                HStack(spacing: Spacing.sm) {
                    Text(action.text)
                        .font(Typography.body(12))
                        .foregroundStyle(palette.textSecondary)
                        .textSelection(.enabled)
                        .accessibilityIdentifier("tracker-sync-\(action.id)")
                    if action.phase == .conflicted, let answerLost {
                        Button("Keep \(action.tracker.displayName)'s") { answerLost(repo, action, .keepTheirs) }
                            .accessibilityIdentifier("tracker-keep-theirs-\(action.id)")
                        Button("Use mine") { answerLost(repo, action, .useMine) }
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

extension View {
    /// Every tracker line below answers through `model`.
    func answeringLostLines(_ model: WorkModel) -> some View {
        environment(\.answerLost) { [model] repo, action, answer in
            model.answerLost(repo: repo, action: action, answer: answer)
        }
    }
}

extension EnvironmentValues {
    /// Answers a lost tracker line of a repository: `lf repo tracker answer`.
    @Entry var answerLost: (@MainActor (_ repo: String, TrackerActionStatus, LostAnswer) -> Void)? = nil
}
