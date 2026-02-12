// QuickExperimentView - run steps directly on the main repo without creating a wave.
// Used in empty state (sidebar) and placeholder state (detail panel).

import SwiftUI
import LoopflowCore

// Common steps for quick experiments - single source of truth
enum QuickExperiment {
    static let steps: [(name: String, description: String)] = [
        ("design", "Interactive design session"),
        ("review", "Analyze the codebase"),
        ("debug", "Fix an error"),
        ("implement", "Build from a design")
    ]

    static let stepNames: [String] = steps.map(\.name)
}

// MARK: - Sidebar Empty State Variant

/// Compact version for the sidebar empty state with burgundy styling.
struct QuickExperimentSidebarView: View {
    var onLaunchStep: ((String) -> Void)?

    var body: some View {
        VStack(spacing: Spacing.md) {
            VStack(spacing: Spacing.xs) {
                Text("Quick Experiment")
                    .font(.caption)
                    .fontWeight(.semibold)
                    .foregroundStyle(.white.opacity(0.8))

                Text("Run a step on your codebase")
                    .font(.caption2)
                    .foregroundStyle(.white.opacity(0.5))
            }

            // Step buttons in 2x2 grid for sidebar width
            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: Spacing.xs) {
                ForEach(QuickExperiment.stepNames, id: \.self) { step in
                    Button {
                        onLaunchStep?(step)
                    } label: {
                        Text(step)
                            .font(.caption)
                            .fontWeight(.medium)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, Spacing.sm)
                            .background(Color.white.opacity(0.1))
                            .foregroundStyle(.white.opacity(0.9))
                            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                    }
                    .buttonStyle(.plain)
                }
            }
        }
        .padding(.horizontal, Spacing.md)
    }
}

// MARK: - Sidebar Preview

/// Visual preview of what the sidebar looks like when populated with waves.
struct SidebarPreviewView: View {
    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text("Or create a wave for ongoing work")
                .font(.caption2)
                .foregroundStyle(.white.opacity(0.5))
                .padding(.bottom, Spacing.xs)

            // Mock section structure
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                previewSection("Needs Attention", icon: "exclamationmark.triangle.fill", color: .statusWarning)
                previewSection("Open PRs", icon: "arrow.triangle.pull", color: .statusSuccess)
                previewSection("Active", icon: "circle.fill", color: .statusSuccess)
                previewSection("Idle", icon: "circle", color: .white.opacity(0.5))
            }
            .padding(.leading, Spacing.xs)
        }
        .padding(.horizontal, Spacing.md)
    }

    private func previewSection(_ title: String, icon: String, color: Color) -> some View {
        HStack(spacing: Spacing.xs) {
            Image(systemName: icon)
                .font(.system(size: 8))
                .foregroundStyle(color.opacity(0.6))

            Text(title)
                .font(.caption2)
                .foregroundStyle(.white.opacity(0.4))
        }
    }
}

// MARK: - Detail Panel Placeholder

/// Full placeholder view for the detail panel when no wave is selected.
struct QuickExperimentDetailView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(\.colorScheme) private var colorScheme

    var onLaunchStep: ((String) -> Void)?

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    var body: some View {
        VStack(spacing: Spacing.xxl) {
            Spacer()

            // Quick experiment section
            VStack(spacing: Spacing.lg) {
                VStack(spacing: Spacing.sm) {
                    Image(systemName: "bolt.fill")
                        .font(.system(size: 32))
                        .foregroundStyle(palette.accent)

                    Text("Quick Experiment")
                        .font(.title2)
                        .fontWeight(.semibold)

                    Text("Run a step on the entire codebase")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }

                // Step buttons
                HStack(spacing: Spacing.md) {
                    ForEach(QuickExperiment.steps, id: \.name) { step in
                        Button {
                            onLaunchStep?(step.name)
                        } label: {
                            VStack(spacing: Spacing.xs) {
                                Text(step.name)
                                    .font(.headline)
                                    .fontWeight(.semibold)

                                Text(step.description)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            .frame(width: 120)
                            .padding(.vertical, Spacing.lg)
                            .background(palette.surface)
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
                    .font(.caption)
                    .foregroundStyle(.tertiary)

                Rectangle()
                    .fill(Color.secondary.opacity(0.2))
                    .frame(height: 1)
                    .frame(maxWidth: 100)
            }

            // Select wave hint
            VStack(spacing: Spacing.sm) {
                Text("Select a wave from the sidebar")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)

                Text("Waves track ongoing work with branches, PRs, and history")
                    .font(.caption)
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

#Preview("Quick Experiment - Sidebar") {
    ZStack {
        Color.loopflowBurgundy
        QuickExperimentSidebarView { step in
            print("Launch: \(step)")
        }
    }
    .frame(width: 280, height: 200)
}

#Preview("Sidebar Preview") {
    ZStack {
        Color.loopflowBurgundy
        SidebarPreviewView()
    }
    .frame(width: 280, height: 200)
}
