import SwiftUI
import LoopflowCore

struct IterationTimeline: View {
    let runs: [WaveRun]

    var body: some View {
        HStack(spacing: Spacing.xs) {
            ForEach(displayRuns) { run in
                Circle()
                    .fill(dotColor(for: run))
                    .frame(width: 8, height: 8)

                if run.id != displayRuns.last?.id {
                    Rectangle()
                        .fill(Color.secondary.opacity(0.35))
                        .frame(width: 8, height: 1)
                }
            }

            Circle()
                .strokeBorder(Color.secondary, lineWidth: 1.5)
                .frame(width: 10, height: 10)
        }
    }

    private var displayRuns: [WaveRun] {
        runs.sorted { $0.iteration < $1.iteration }
    }

    private func dotColor(for run: WaveRun) -> Color {
        if let pr = run.pr {
            switch pr.state {
            case .merged:
                return .statusSuccess
            case .open, .draft:
                return .statusInfo
            case .closed:
                return .statusError
            case .none:
                return .statusNeutral
            }
        }

        if run.status == .failed { return .statusError }
        return .statusNeutral
    }
}
