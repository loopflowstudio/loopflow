import SwiftUI
import LoopflowCore

struct WaveRunsTab: View {
    let wave: WaveViewModel
    let runs: [WaveRun]
    let onCollapse: () async throws -> CollapsePRsResult
    let onAbsorb: (Int) async throws -> AbsorbIntoPRResult

    @Environment(\.colorScheme) private var colorScheme

    @State private var showCollapseConfirmation = false
    @State private var isCollapsing = false
    @State private var isAbsorbing = false
    @State private var actionError: String?
    @State private var actionSuccess: String?
    @State private var successDismissTask: Task<Void, Never>?

    private let terminalLauncher = TerminalLauncher()

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    private var openPRRuns: [WaveRun] {
        sortedRuns.filter { $0.pr?.state == .open || $0.pr?.state == .draft }
    }

    private var absorbTargetPR: PullRequest? {
        guard !wave.commits.isEmpty, wave.prURL == nil else { return nil }
        return openPRRuns.first?.pr
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            if openPRRuns.count >= 2 {
                collapseBar
            }

            if let targetPR = absorbTargetPR, let prNumber = targetPR.number {
                absorbBar(prNumber: prNumber)
            }

            if let actionError {
                Text(actionError)
                    .font(.caption)
                    .foregroundStyle(Color.statusError)
            }

            if let actionSuccess {
                Label(actionSuccess, systemImage: "checkmark.circle.fill")
                    .font(.caption)
                    .foregroundStyle(Color.statusSuccess)
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xs)
                    .background(Color.statusSuccess.opacity(0.12))
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                    .transition(.opacity)
            }

            if runs.isEmpty {
                Text("No runs yet")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(sortedRuns) { run in
                    WaveRunRow(run: run)
                }
            }
        }
        .confirmationDialog(
            "Collapse PRs?",
            isPresented: $showCollapseConfirmation,
            titleVisibility: .visible
        ) {
            Button("Collapse into One PR") {
                Task { await collapsePRs() }
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

    private var collapseBar: some View {
        HStack {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("\(openPRRuns.count) open PRs")
                    .font(.subheadline)
                    .fontWeight(.medium)
                Text("Combine into a single PR")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            Button {
                showCollapseConfirmation = true
            } label: {
                if isCollapsing {
                    ProgressView()
                        .controlSize(.small)
                } else {
                    Label("Collapse", systemImage: "arrow.triangle.merge")
                }
            }
            .buttonStyle(.borderedProminent)
            .tint(.loopflowBurgundy)
            .disabled(isCollapsing || isAbsorbing)
        }
        .padding(Spacing.lg)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
    }

    private func absorbBar(prNumber: Int) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("\(wave.commits.count) unpublished commit\(wave.commits.count == 1 ? "" : "s")")
                    .font(.subheadline)
                    .fontWeight(.medium)
                Text("Add to PR #\(prNumber)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            Button {
                Task { await absorbIntoPR(prNumber: prNumber) }
            } label: {
                if isAbsorbing {
                    ProgressView()
                        .controlSize(.small)
                } else {
                    Label("Absorb", systemImage: "arrow.down.to.line")
                }
            }
            .buttonStyle(.bordered)
            .disabled(isAbsorbing || isCollapsing)
        }
        .padding(Spacing.lg)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
    }

    private func collapsePRs() async {
        isCollapsing = true
        actionError = nil
        actionSuccess = nil

        do {
            let result = try await onCollapse()
            showSuccess("Collapsed \(result.closedPRs.count) PR\(result.closedPRs.count == 1 ? "" : "s") into one stack PR")
            if let urlString = result.newPRUrl, let url = URL(string: urlString) {
                terminalLauncher.openURL(url)
            }
        } catch {
            actionError = error.localizedDescription
        }

        isCollapsing = false
    }

    private func absorbIntoPR(prNumber: Int) async {
        isAbsorbing = true
        actionError = nil
        actionSuccess = nil

        do {
            let result = try await onAbsorb(prNumber)
            showSuccess("Absorbed \(result.commitsAbsorbed) commit\(result.commitsAbsorbed == 1 ? "" : "s") into \(result.targetBranch)")
        } catch {
            actionError = error.localizedDescription
        }

        isAbsorbing = false
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

    @State private var isExpanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                Text("#\(run.iteration)")
                    .font(.caption)
                    .fontWeight(.medium)
                    .monospacedDigit()

                Text(run.flow)
                    .font(.caption)
                    .foregroundStyle(.secondary)

                if let pr = run.pr {
                    RunPRBadge(pr: pr)
                }

                Spacer()

                if let duration = run.duration {
                    Text(duration)
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                        .monospacedDigit()
                }

                Text(run.relativeTime)
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
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
                            .font(.caption)
                            .foregroundStyle(Color.statusError)
                    }
                }
                .padding(.top, Spacing.xs)
            }
        }
        .padding(Spacing.md)
        .background(Color.secondary.opacity(0.06))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }

    private func detailLine(label: String, value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
            Text("\(label):")
                .font(.caption2)
                .foregroundStyle(.secondary)
            Text(value)
                .font(.caption2)
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
        case .none: Color.secondary
        }

        return Button {
            terminalLauncher.openURL(pr.url)
        } label: {
            Text(labelText)
                .font(.caption2)
                .fontWeight(.medium)
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
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
