#if os(macOS)
import SwiftUI
import LoopflowCore

struct IterationTimeline: View {
    let runs: [WaveRun]

    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.xs) {
            ForEach(displayRuns) { run in
                Circle()
                    .fill(dotColor(for: run))
                    .frame(width: 8, height: 8)

                if run.id != displayRuns.last?.id {
                    Rectangle()
                        .fill(palette.border)
                        .frame(width: 8, height: 1)
                }
            }

            Circle()
                .strokeBorder(palette.border, lineWidth: 1.5)
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
                return .statusInfo
            case .open, .draft:
                return .statusSuccess
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

#endif
