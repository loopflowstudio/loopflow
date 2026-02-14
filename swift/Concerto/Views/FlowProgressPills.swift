// FlowProgressPills - horizontal flow step indicator with current step highlighted and elapsed time.

import SwiftUI
import LoopflowCore

struct FlowProgressPills: View {
    let steps: [String]
    let currentIndex: Int
    let startedAt: Date?
    var onRestartStep: (() -> Void)?

    @Environment(\.palette) private var palette
    @State private var elapsedSeconds: Int = 0

    private let timer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    var body: some View {
        HStack(spacing: Spacing.xs) {
            ForEach(Array(steps.enumerated()), id: \.offset) { index, step in
                if index > 0 {
                    Image(systemName: "chevron.right")
                        .font(Typography.caption(8)).fontWeight(.semibold)
                        .foregroundStyle(.tertiary)
                        .accessibilityHidden(true)
                }

                stepPill(step: step, index: index)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(accessibilityDescription)
        .onReceive(timer) { _ in
            updateElapsedTime()
        }
        .onAppear {
            updateElapsedTime()
        }
    }

    private var accessibilityDescription: String {
        let currentStep = steps.indices.contains(currentIndex) ? formatStepName(steps[currentIndex]) : "unknown"
        let elapsed = formattedElapsedTime ?? "just started"
        return "Step \(currentIndex + 1) of \(steps.count): \(currentStep), \(elapsed)"
    }

    @ViewBuilder
    private func stepPill(step: String, index: Int) -> some View {
        let isCurrent = index == currentIndex
        let isCompleted = index < currentIndex

        HStack(spacing: Spacing.xs) {
            if isCompleted {
                Image(systemName: "checkmark")
                    .font(Typography.caption(9)).fontWeight(.semibold)
            }

            Text(formatStepName(step))
                .font(Typography.body(11)).fontWeight(isCurrent ? .semibold : .regular)

            if isCurrent, let elapsed = formattedElapsedTime {
                Text(elapsed)
                    .font(Typography.caption(10))
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
            }

            if isCurrent, onRestartStep != nil {
                Button {
                    onRestartStep?()
                } label: {
                    Image(systemName: "arrow.counterclockwise")
                        .font(Typography.caption(9))
                        .foregroundStyle(.white.opacity(0.7))
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Restart step")
            }
        }
        .padding(.horizontal, Spacing.sm)
        .padding(.vertical, Spacing.xs)
        .background(
            Capsule()
                .fill(isCurrent ? palette.accent : (isCompleted ? palette.accent.opacity(0.1) : palette.surface))
        )
        .foregroundStyle(isCurrent ? .white : (isCompleted ? palette.accent : .primary))
    }

    private func formatStepName(_ name: String) -> String {
        name.replacingOccurrences(of: "-", with: " ")
    }

    private func updateElapsedTime() {
        guard let startedAt else {
            elapsedSeconds = 0
            return
        }
        elapsedSeconds = max(0, Int(Date().timeIntervalSince(startedAt)))
    }

    private var formattedElapsedTime: String? {
        guard elapsedSeconds > 0 else { return nil }

        let minutes = elapsedSeconds / 60
        let seconds = elapsedSeconds % 60

        if minutes > 0 {
            return "\(minutes)m"
        } else {
            return "\(seconds)s"
        }
    }
}

#Preview("Running - Step 2 of 4") {
    ThemePreview {
        FlowProgressPills(
            steps: ["implement", "compress", "gate", "consolidate"],
            currentIndex: 1,
            startedAt: Date().addingTimeInterval(-125),
            onRestartStep: {}
        )
        .padding()
    }
}

#Preview("Running - First Step") {
    FlowProgressPills(
        steps: ["design", "implement", "ship"],
        currentIndex: 0,
        startedAt: Date().addingTimeInterval(-30) // 30s ago
    )
    .padding()
}

#Preview("Running - Last Step") {
    FlowProgressPills(
        steps: ["implement", "compress", "gate"],
        currentIndex: 2,
        startedAt: Date().addingTimeInterval(-300) // 5m ago
    )
    .padding()
}

#Preview("Single Step Flow") {
    FlowProgressPills(
        steps: ["design"],
        currentIndex: 0,
        startedAt: Date().addingTimeInterval(-60) // 1m ago
    )
    .padding()
}
