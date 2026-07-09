#if os(macOS)
import SwiftUI
import Loopflow

/// The wave detail pane: a header over the local wave plan and live WaveChat
/// transcript. The wave still runs in its own `lf loop` process; Loopflow frames
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
            HSplitView {
                WavePlanView(
                    plan: wave.plan ?? WavePlan(objective: ""),
                    wave: wave,
                    repoPath: repoPath
                )
                .frame(minWidth: 230, idealWidth: 320, maxWidth: 440, maxHeight: .infinity)

                WaveChatView(repoPath: repoPath, waveName: wave.name)
                    .frame(minWidth: 340, maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }

    private var header: some View {
        HStack(spacing: Spacing.sm) {
            Image(systemName: wave.statusIndicator.icon)
                .foregroundStyle(wave.statusIndicator.color)
                .accessibilityLabel(wave.statusText)
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
    let wave: WaveViewModel
    let repoPath: String

    @Environment(\.palette) private var palette
    @State private var runs: [Run] = []
    @State private var backlog: [BacklogItem] = []
    @State private var backlogUnavailable = false

    private var identity: String { "\(repoPath)|\(wave.id)" }

    private var activeRuns: [Run] {
        runs.filter {
            switch $0.status {
            case .pending, .running, .waiting: true
            default: false
            }
        }
    }

    private var openPullRequests: [Run] {
        activeRuns.filter { $0.pr != nil }
    }

    private var filedBacklog: [BacklogItem] {
        let running = Set(activeRuns.compactMap { $0.task?.normalizedTaskTitle })
        return backlog.filter { !running.contains($0.name.normalizedTaskTitle) }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.xl) {
                objective
                projects
                openPRs
                sessions
                backlogSection
            }
            .padding(Spacing.xl)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(palette.background)
        .task(id: identity) {
            while !Task.isCancelled {
                await refreshRuns()
                try? await Task.sleep(for: .seconds(5))
            }
        }
        .task(id: "\(identity)|backlog") {
            while !Task.isCancelled {
                await refreshBacklog()
                try? await Task.sleep(for: .seconds(60))
            }
        }
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

    private var openPRs: some View {
        operationalSection(title: "Open PRs", count: openPullRequests.count) {
            if openPullRequests.isEmpty {
                emptyOperationalRow("No open PRs.")
            } else {
                ForEach(openPullRequests) { run in
                    if let pr = run.pr {
                        Link(destination: pr.url) {
                            operationalRow(
                                icon: "arrow.triangle.pull",
                                title: pr.title ?? run.task ?? run.branch ?? run.flow,
                                detail: run.branch ?? pr.url.absoluteString
                            )
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
    }

    private var sessions: some View {
        operationalSection(title: "Active sessions", count: activeRuns.count) {
            if activeRuns.isEmpty {
                emptyOperationalRow("No hands running.")
            } else {
                ForEach(activeRuns) { run in
                    operationalRow(
                        icon: "hand.raised",
                        title: run.task ?? run.flow,
                        detail: [run.flow, run.loopPassDetail, run.worktree]
                            .compactMap { $0 }
                            .joined(separator: " · ")
                    )
                }
            }
        }
    }

    private var backlogSection: some View {
        operationalSection(title: "Backlog", count: filedBacklog.count) {
            if backlogUnavailable {
                emptyOperationalRow("Backlog unavailable — connect Linear for this wave.")
            } else if filedBacklog.isEmpty {
                emptyOperationalRow("No filed work waiting.")
            } else {
                ForEach(filedBacklog) { item in
                    operationalRow(
                        icon: "tray",
                        title: item.name,
                        detail: item.labels.filter { $0.hasPrefix("project:") }.joined(separator: " · ")
                    )
                }
            }
        }
    }

    private func operationalSection<Content: View>(
        title: String,
        count: Int,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                Text(title)
                    .font(Typography.caption(10))
                    .fontWeight(.medium)
                    .foregroundStyle(palette.textSecondary)
                Text("\(count)")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xxs)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
            }
            VStack(alignment: .leading, spacing: Spacing.xs) {
                content()
            }
        }
    }

    private func operationalRow(icon: String, title: String, detail: String) -> some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Image(systemName: icon)
                .font(Typography.caption(11))
                .foregroundStyle(palette.accent)
                .frame(width: 14)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                Text(title)
                    .font(Typography.caption(12))
                    .foregroundStyle(palette.text)
                    .lineLimit(2)
                if !detail.isEmpty {
                    Text(detail)
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Spacing.sm)
        .background(palette.surfaceMuted.opacity(0.65))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
        .accessibilityElement(children: .combine)
    }

    private func emptyOperationalRow(_ text: String) -> some View {
        Text(text)
            .font(Typography.caption())
            .foregroundStyle(palette.textSecondary)
    }

    private func refreshRuns() async {
        if let snapshot = try? await RegistryQueryLocal.shared.status(
            wave: wave.name,
            waveId: wave.id,
            cwd: repoPath
        ) {
            runs = snapshot.runs
        }
    }

    private func refreshBacklog() async {
        do {
            backlog = try await RegistryQueryLocal.shared.backlog(wave: wave.name, cwd: repoPath)
            backlogUnavailable = false
        } catch {
            backlogUnavailable = true
        }
    }
}

private extension String {
    var normalizedTaskTitle: String {
        trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    }
}

private extension Run {
    var loopPassDetail: String? {
        stepIndex > 0 ? "pass \(stepIndex)" : nil
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
                    ForEach(project.krs) { kr in
                        HStack(alignment: .top, spacing: Spacing.sm) {
                            Image(systemName: kr.proof == .holds ? "checkmark.circle.fill" : "circle")
                                .font(Typography.caption(11))
                                .foregroundStyle(kr.proof == .holds ? palette.accent : palette.textSecondary)
                                .frame(width: 14)
                                .accessibilityHidden(true)

                            Text(kr.text)
                                .font(Typography.caption(12))
                                .foregroundStyle(palette.text)
                                .lineSpacing(2)
                                .textSelection(.enabled)
                        }
                        .accessibilityElement(children: .combine)
                        .accessibilityLabel(kr.text)
                        .accessibilityValue(kr.proof == .holds ? "Holds" : "Open")
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
