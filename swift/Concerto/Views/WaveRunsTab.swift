import SwiftUI
import LoopflowCore

struct WaveRunsTab: View {
    let runs: [WaveRun]
    let onCombine: () async throws -> CombinePRsResult

    @Environment(\.palette) private var palette

    @State private var showCombineConfirmation = false
    @State private var isCombining = false
    @State private var actionError: String?
    @State private var actionSuccess: String?
    @State private var successDismissTask: Task<Void, Never>?

    private let terminalLauncher = TerminalLauncher()

    private var openPRRuns: [WaveRun] {
        sortedRuns.filter { $0.pr?.state == .open || $0.pr?.state == .draft }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            if openPRRuns.count >= 2 {
                combineBar
            }

            if let actionError {
                Text(actionError)
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusError)
            }

            if let actionSuccess {
                Label(actionSuccess, systemImage: "checkmark.circle.fill")
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusSuccess)
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xs)
                    .background(Color.statusSuccess.opacity(0.12))
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                    .transition(.opacity)
            }

            if runs.isEmpty {
                Text("No runs yet")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            } else {
                ForEach(sortedRuns) { run in
                    WaveRunRow(run: run)
                }
            }
        }
        .confirmationDialog(
            "Combine PRs?",
            isPresented: $showCombineConfirmation,
            titleVisibility: .visible
        ) {
            Button("Combine into One PR") {
                Task { await combinePRs() }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This combines open PRs into one PR and closes old PRs.")
        }
        .onDisappear {
            successDismissTask?.cancel()
        }
    }

    private var sortedRuns: [WaveRun] {
        runs.sorted { lhs, rhs in
            if lhs.iteration == rhs.iteration {
                return (lhs.startedAt ?? lhs.createdAt) > (rhs.startedAt ?? rhs.createdAt)
            }
            return lhs.iteration > rhs.iteration
        }
    }

    private var combineBar: some View {
        HStack {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("\(openPRRuns.count) open PRs")
                    .font(Typography.body())
                    .fontWeight(.medium)
                Text("Combine into a single PR")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }

            Spacer()

            Button {
                showCombineConfirmation = true
            } label: {
                if isCombining {
                    ProgressView()
                        .controlSize(.small)
                } else {
                    Label("Combine", systemImage: "arrow.triangle.merge")
                }
            }
            .buttonStyle(.borderedProminent)
            .tint(.loopflowBurgundy)
            .disabled(isCombining)
        }
        .padding(Spacing.lg)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
    }

    private func combinePRs() async {
        isCombining = true
        actionError = nil
        actionSuccess = nil

        do {
            let result = try await onCombine()
            showSuccess("Combined \(result.closedPRs.count) PR\(result.closedPRs.count == 1 ? "" : "s") into one stack PR")
            if let urlString = result.newPRUrl, let url = URL(string: urlString) {
                terminalLauncher.openURL(url)
            }
        } catch {
            actionError = error.localizedDescription
        }

        isCombining = false
    }

    private func showSuccess(_ message: String) {
        successDismissTask?.cancel()
        actionSuccess = message
        successDismissTask = Task {
            try? await Task.sleep(for: .seconds(4))
            guard !Task.isCancelled else { return }
            await MainActor.run {
                actionSuccess = nil
            }
        }
    }
}

private struct WaveRunRow: View {
    let run: WaveRun

    @Environment(\.palette) private var palette
    @State private var isExpanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                Text("#\(run.iteration)")
                    .font(Typography.caption())
                    .fontWeight(.medium)
                    .monospacedDigit()

                Text(run.flow)
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)

                if let pr = run.pr {
                    RunPRBadge(pr: pr)
                }

                Spacer()

                if let duration = run.duration {
                    Text(duration)
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                        .monospacedDigit()
                }

                Text(run.relativeTime)
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
            }
            .contentShape(Rectangle())
            .onTapGesture {
                isExpanded.toggle()
            }

            if isExpanded {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    if let branch = run.branch, !branch.isEmpty {
                        detailLine(label: "Branch", value: branch)
                    }

                    if let worktree = run.worktree, !worktree.isEmpty {
                        detailLine(label: "Worktree", value: worktree)
                    }

                    if let startedAt = run.startedAt {
                        detailLine(label: "Started", value: startedAt.formatted(date: .abbreviated, time: .standard))
                    }

                    if let endedAt = run.endedAt {
                        detailLine(label: "Ended", value: endedAt.formatted(date: .abbreviated, time: .standard))
                    }

                    if let error = run.error, !error.isEmpty {
                        Text(error)
                            .font(Typography.caption())
                            .foregroundStyle(Color.statusError)
                    }
                }
                .padding(.top, Spacing.xs)
            }
        }
        .padding(Spacing.md)
        .background(palette.surfaceMuted)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }

    private func detailLine(label: String, value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
            Text("\(label):")
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
            Text(value)
                .font(Typography.caption(10))
                .textSelection(.enabled)
        }
    }
}

private struct RunPRBadge: View {
    let pr: PullRequest

    @State private var isHovering = false

    private let terminalLauncher = TerminalLauncher()

    var body: some View {
        let state = pr.state
        let color = switch state {
        case .open: Color.statusSuccess
        case .draft: Color.statusWarning
        case .merged: Color.statusInfo
        case .closed: Color.statusError
        case .none: Color.statusNeutral
        }

        return Button {
            terminalLauncher.openURL(pr.url)
        } label: {
            Text(labelText)
                .font(Typography.caption(10))
                .fontWeight(.medium)
                .padding(.horizontal, Spacing.sm)
                .padding(.vertical, Spacing.xxs)
                .background(color.opacity(isHovering ? 0.28 : 0.18))
                .foregroundStyle(color)
                .clipShape(Capsule())
        }
        .buttonStyle(.plain)
        .onHover { isHovering = $0 }
    }

    private var labelText: String {
        let number = pr.number.map { "#\($0)" } ?? "PR"
        let stateText = pr.state?.rawValue ?? "unknown"
        return "\(number) \(stateText)"
    }
}
