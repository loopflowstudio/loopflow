// QuickExperimentView - run steps directly on the main repo without creating a wave.
// Used as the detail panel placeholder when no wave is selected.

import SwiftUI
import LoopflowCore

// MARK: - Detail Panel Placeholder

/// Full placeholder view for the detail panel when no wave is selected.
struct QuickExperimentDetailView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    var onLaunchStep: ((String) -> Void)?

    private let steps: [(name: String, description: String)] = [
        ("design", "Interactive design session"),
        ("review", "Analyze the codebase"),
        ("debug", "Fix an error"),
        ("implement", "Build from a design")
    ]

    var body: some View {
        VStack(spacing: Spacing.xxl) {
            Spacer()

            // Quick experiment section
            VStack(spacing: Spacing.lg) {
                VStack(spacing: Spacing.sm) {
                    Image(systemName: "bolt.fill")
                        .font(Typography.heroTitle())
                        .foregroundStyle(palette.accent)

                    Text("Quick Experiment")
                        .font(Typography.sectionTitle())
                        .fontWeight(.semibold)

                    Text("Run a step on the entire codebase")
                        .font(Typography.body())
                        .foregroundStyle(.secondary)
                }

                // Step buttons
                HStack(spacing: Spacing.md) {
                    ForEach(steps, id: \.name) { step in
                        Button {
                            onLaunchStep?(step.name)
                        } label: {
                            VStack(spacing: Spacing.xs) {
                                Text(step.name)
                                    .font(Typography.body())
                                    .fontWeight(.medium)

                                Text(step.description)
                                    .font(Typography.caption())
                                    .foregroundStyle(.secondary)
                            }
                            .frame(width: 100)
                            .padding(.vertical, Spacing.md)
                            .background(palette.surface.opacity(0.7))
                            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
                        }
                        .buttonStyle(.plain)
                    }
                }
            }

            // Divider
            HStack {
                Rectangle()
                    .fill(Color.secondary.opacity(0.2))
                    .frame(height: 1)
                    .frame(maxWidth: 100)

                Text("or")
                    .font(Typography.caption())
                    .foregroundStyle(.tertiary)

                Rectangle()
                    .fill(Color.secondary.opacity(0.2))
                    .frame(height: 1)
                    .frame(maxWidth: 100)
            }

            // Contextual hint
            VStack(spacing: Spacing.sm) {
                if repoState.waves.isEmpty {
                    Text("Or create a wave for ongoing work")
                        .font(Typography.body())
                        .foregroundStyle(.secondary)
                } else {
                    Text("Select a wave from the sidebar")
                        .font(Typography.body())
                        .foregroundStyle(.secondary)
                }

                Text("Waves track ongoing work with branches, PRs, and history")
                    .font(Typography.caption())
                    .foregroundStyle(.tertiary)
            }

            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Previews

#Preview("Quick Experiment - Detail") {
    QuickExperimentDetailView { step in
        print("Launch: \(step)")
    }
    .environment(RepoState())
    .frame(width: 600, height: 500)
}

