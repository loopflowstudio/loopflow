// FlowProgressPills - horizontal flow step indicator with current step highlighted and elapsed time.

import SwiftUI
import LoopflowCore

struct FlowProgressPills: View {
    let steps: [String]
    let currentIndex: Int
    let startedAt: Date?
    var stepAgents: [String: String]? = nil
    var onRestartStep: (() -> Void)?

    @Environment(\.palette) private var palette
    @State private var elapsedSeconds: Int = 0

    private let timer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    private enum PillKind {
        case step
        case ops
        case branch
        case fork
    }

    var body: some View {
        HStack(spacing: Spacing.xs) {
            ForEach(Array(steps.enumerated()), id: \.offset) { index, step in
                if index > 0 {
                    Image(systemName: "chevron.right")
                        .font(Typography.caption(8)).fontWeight(.semibold)
                        .foregroundStyle(palette.textSecondary)
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
        let currentStep = steps.indices.contains(currentIndex) ? displayStepName(steps[currentIndex]) : "unknown"
        let elapsed = formattedElapsedTime ?? "just started"
        return "Step \(currentIndex + 1) of \(steps.count): \(currentStep), \(elapsed)"
    }

    @ViewBuilder
    private func stepPill(step: String, index: Int) -> some View {
        let isCurrent = index == currentIndex
        let isCompleted = index < currentIndex
        let kind = stepKind(step)
        let label = displayStepName(step)

        HStack(spacing: Spacing.xs) {
            if isCompleted {
                Image(systemName: "checkmark")
                    .font(Typography.caption(9)).fontWeight(.semibold)
            }

            if kind == .ops {
                Text("ops")
                    .font(Typography.caption(9))
                    .padding(.horizontal, Spacing.xs)
                    .padding(.vertical, 1)
                    .background(Color.statusInfo.opacity(isCurrent ? 0.28 : 0.18))
                    .clipShape(Capsule())
            }

            Text(label)
                .font(Typography.body(11)).fontWeight(isCurrent ? .semibold : .regular)

            if isCurrent, let elapsed = formattedElapsedTime {
                Text(elapsed)
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .monospacedDigit()
            }

            if let overrideAgent = stepAgents?[step] {
                Text(overrideAgent)
                    .font(Typography.caption(9))
                    .padding(.horizontal, Spacing.xs)
                    .padding(.vertical, 2)
                    .background(palette.surface.opacity(isCurrent ? 0.3 : 1))
                    .clipShape(Capsule())
            }

            if isCurrent, let onRestartStep {
                Button(action: onRestartStep) {
                    Image(systemName: "arrow.counterclockwise")
                        .font(Typography.caption(9))
                        .foregroundStyle(.white.opacity(0.7))
                }
                .buttonStyle(.plain)
                .minHitTarget()
                .accessibilityLabel("Restart step")
            }
        }
        .padding(.horizontal, Spacing.sm)
        .padding(.vertical, Spacing.xs)
        .background(
            Capsule()
                .fill(backgroundColor(isCurrent: isCurrent, isCompleted: isCompleted, kind: kind))
        )
        .foregroundStyle(foregroundColor(isCurrent: isCurrent, isCompleted: isCompleted, kind: kind))
    }

    private func stepKind(_ value: String) -> PillKind {
        if value.hasPrefix("ops:") { return .ops }
        if value == "[branch]" { return .branch }
        if value == "[fork]" { return .fork }
        return .step
    }

    private func displayStepName(_ value: String) -> String {
        let base: String
        if value.hasPrefix("ops:") {
            base = value.split(separator: ":", maxSplits: 1)[1].trimmingCharacters(in: .whitespaces)
        } else {
            base = value
        }
        return base.replacingOccurrences(of: "-", with: " ")
    }

    private func backgroundColor(isCurrent: Bool, isCompleted: Bool, kind: PillKind) -> Color {
        switch kind {
        case .ops:
            if isCurrent { return Color.statusInfo }
            if isCompleted { return Color.statusInfo.opacity(0.14) }
            return palette.surface
        case .step, .branch, .fork:
            if isCurrent { return palette.accent }
            if isCompleted { return palette.accent.opacity(0.1) }
            return palette.surface
        }
    }

    private func foregroundColor(isCurrent: Bool, isCompleted: Bool, kind: PillKind) -> Color {
        switch kind {
        case .ops:
            if isCurrent { return .white }
            if isCompleted { return Color.statusInfo }
            return .primary
        case .step, .branch, .fork:
            if isCurrent { return .white }
            if isCompleted { return palette.accent }
            return .primary
        }
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
            steps: ["implement", "compress", "gate", "update-wave"],
            currentIndex: 1,
            startedAt: Date().addingTimeInterval(-125),
            onRestartStep: {}
        )
        .padding()
    }
}

#Preview("Running - First Step") {
    FlowProgressPills(
        steps: ["design", "build", "review"],
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
