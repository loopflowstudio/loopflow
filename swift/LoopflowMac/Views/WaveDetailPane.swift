#if os(macOS)
import SwiftUI
import LoopflowCore

/// The wave detail pane: a header over the local wave plan and live WaveChat
/// transcript. The wave still runs in its own `lf wave` process; Concerto frames
/// the objective and projects around the vendor-owned conversation.
struct WaveDetailPane: View {
    let wave: WaveViewModel
    let repoPath: String
    let onClose: () -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            if let plan = wave.plan {
                HSplitView {
                    WavePlanView(plan: plan)
                        .frame(minWidth: 300, idealWidth: 380, maxWidth: 480, maxHeight: .infinity)

                    WaveChatView(repoPath: repoPath, waveName: wave.name)
                        .frame(minWidth: 420, maxWidth: .infinity, maxHeight: .infinity)
                }
            } else {
                WaveChatView(repoPath: repoPath, waveName: wave.name)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }

    private var header: some View {
        HStack(spacing: Spacing.sm) {
            Image(systemName: wave.statusIndicator.icon)
                .foregroundStyle(wave.statusIndicator.color)
            Text(wave.displayName)
                .font(Typography.sectionTitle())
                .foregroundStyle(palette.text)
            Text("WaveChat")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            Spacer()

            Button {
                onClose()
            } label: {
                Image(systemName: "xmark")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            .buttonStyle(.plain)
            .help("Close wave")
            .accessibilityLabel("Close wave")
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.md)
    }
}

private struct WavePlanView: View {
    let plan: WavePlan

    @Environment(\.palette) private var palette

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.xl) {
                objective
                projects
            }
            .padding(Spacing.xl)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(palette.background)
    }

    private var objective: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Objective")
                .font(Typography.caption(10))
                .fontWeight(.medium)
                .foregroundStyle(palette.textSecondary)

            Text(plan.objective.isEmpty ? "No objective written yet." : plan.objective)
                .font(Typography.body(14))
                .foregroundStyle(palette.text)
                .lineSpacing(3)
                .textSelection(.enabled)
        }
    }

    private var projects: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            HStack(spacing: Spacing.sm) {
                Text("Projects")
                    .font(Typography.caption(10))
                    .fontWeight(.medium)
                    .foregroundStyle(palette.textSecondary)

                Text("\(plan.projects.count)")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xxs)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
            }

            if plan.projects.isEmpty {
                Text("No live projects.")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            } else {
                LazyVStack(alignment: .leading, spacing: Spacing.md) {
                    ForEach(plan.projects) { project in
                        WaveProjectView(project: project)
                    }
                }
            }
        }
    }
}

private struct WaveProjectView: View {
    let project: WaveProject

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(project.title)
                .font(Typography.sectionTitle(17))
                .foregroundStyle(palette.text)

            if let summary = project.summary {
                Text(summary)
                    .font(Typography.body(13))
                    .foregroundStyle(palette.textSecondary)
                    .lineSpacing(2)
                    .textSelection(.enabled)
            }

            if !project.krs.isEmpty {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(project.krs, id: \.self) { kr in
                        HStack(alignment: .top, spacing: Spacing.sm) {
                            Image(systemName: "checkmark.circle")
                                .font(Typography.caption(11))
                                .foregroundStyle(palette.accent)
                                .frame(width: 14)

                            Text(kr)
                                .font(Typography.caption(12))
                                .foregroundStyle(palette.text)
                                .lineSpacing(2)
                                .textSelection(.enabled)
                        }
                    }
                }
                .padding(.top, Spacing.xs)
            }
        }
        .padding(Spacing.md)
        .background(palette.surfaceMuted.opacity(0.65))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }
}

#endif
