// Agent detail panel showing status, actions, and flow picker for idle agents.

import SwiftUI
import LoopflowCore

struct AgentDetailPanel: View {
    @Bindable var appState: AppState
    let agent: Agent

    @Environment(\.colorScheme) private var colorScheme
    @AppStorage("changedFilesExpanded") private var changedFilesExpanded = true

    @State private var commits: [CommitInfo] = []
    @State private var fileStats: [FileDiffStat] = []
    @State private var isLoadingCommits = false
    @State private var isLoadingFiles = false
    @State private var actionError: String?
    @State private var showingActionError = false
    @State private var showingStopConfirmation = false

    private let terminalLauncher = TerminalLauncher()
    private let worktreeService = WorktreeService()

    private var ideApp: IDEApp { appState.config?.ideApp ?? .cursor }
    private var terminalApp: TerminalApp { appState.config?.terminalApp ?? .warp }
    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    private var worktree: Worktree? {
        guard let path = agent.worktreePath else { return nil }
        return appState.worktrees.first { $0.path == path }
    }

    var body: some View {
        VStack(spacing: 0) {
            header

            Divider()

            quickActionsBar

            Divider()

            ScrollView {
                VStack(spacing: 0) {
                    if agent.status == .idle {
                        // Show flow picker for idle agents
                        FlowPicker(agent: agent, appState: appState)
                            .padding(20)
                    } else {
                        // Show run progress for active agents
                        runProgressSection
                    }

                    if agent.worktreePath != nil {
                        changedFilesSection
                    }
                }
            }
        }
        .background(palette.background)
        .onAppear {
            loadData()
        }
        .onChange(of: agent.id) {
            loadData()
        }
        .alert("Error", isPresented: $showingActionError) {
            Button("OK") { actionError = nil }
        } message: {
            Text(actionError ?? "An error occurred")
        }
        .confirmationDialog(
            "Stop Agent",
            isPresented: $showingStopConfirmation
        ) {
            Button("Stop", role: .destructive) {
                stopAgent()
            }
        } message: {
            Text("Stop '\(agent.displayName)'? It can be restarted later.")
        }
    }

    // MARK: - Header

    private var header: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 8) {
                    // Status indicator
                    Image(systemName: agent.statusIndicator.icon)
                        .font(.system(size: 14))
                        .foregroundStyle(agent.statusIndicator.color)

                    Text(agent.displayName)
                        .font(.title2)
                        .fontWeight(.semibold)
                }

                HStack(spacing: 8) {
                    Text(agent.areaDisplay)
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    Text("•")
                        .font(.caption)
                        .foregroundStyle(.tertiary)

                    Text(agent.flowDisplay)
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    Text("•")
                        .font(.caption)
                        .foregroundStyle(.tertiary)

                    Text(agent.stimulusText)
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    if agent.iteration > 0 {
                        Text("•")
                            .font(.caption)
                            .foregroundStyle(.tertiary)

                        Text("iter \(agent.iteration)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }

            Spacer()

            // PR badge if available
            if let wt = worktree, let prNumber = wt.prNumber, let prState = wt.prState {
                prBadge(number: prNumber, state: prState, url: wt.prURL)
            }
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 16)
    }

    private func prBadge(number: Int, state: PRState, url: URL?) -> some View {
        let color: Color = switch state {
        case .open: .green
        case .merged: .purple
        case .closed: .red
        case .draft: .orange
        }

        return Button {
            if let url {
                terminalLauncher.openURL(url)
            }
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "arrow.up.right.square")
                    .font(.caption2)
                Text("PR #\(number)")
                    .font(.caption)
                    .fontWeight(.medium)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
        }
        .buttonStyle(.plain)
        .help("View PR on GitHub")
    }

    // MARK: - Quick Actions Bar

    private var quickActionsBar: some View {
        HStack(spacing: 12) {
            if let path = agent.worktreePath {
                Button {
                    openInTerminal(path: path)
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "terminal")
                            .font(.system(size: 14))
                        Text(terminalApp.displayName)
                            .fontWeight(.medium)
                    }
                    .frame(minWidth: 70)
                }
                .buttonStyle(DarkButtonStyle())
                .help("Open in \(terminalApp.displayName)")

                Button {
                    openInIDE(path: path)
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "curlybraces")
                            .font(.system(size: 14))
                        Text(ideApp.displayName)
                            .fontWeight(.medium)
                    }
                    .frame(minWidth: 70)
                }
                .buttonStyle(DarkButtonStyle())
                .help("Open in \(ideApp.displayName)")

                if let wt = worktree, wt.hasDiff {
                    Button {
                        landBranch()
                    } label: {
                        HStack(spacing: 8) {
                            Image(systemName: "airplane.arrival")
                                .font(.system(size: 14))
                            Text("Land")
                                .fontWeight(.medium)
                        }
                        .frame(minWidth: 70)
                    }
                    .buttonStyle(DarkButtonStyle())
                    .help("Land branch (creates PR if needed)")
                }
            }

            Spacer()

            if agent.status == .running || agent.status == .waiting {
                Button {
                    showingStopConfirmation = true
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "stop.fill")
                            .font(.system(size: 14))
                        Text("Stop")
                            .fontWeight(.medium)
                    }
                    .frame(minWidth: 70)
                }
                .buttonStyle(DarkButtonStyle())
                .help("Stop agent")
            }
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 16)
        .background(palette.surface)
    }

    // MARK: - Run Progress Section

    private var runProgressSection: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Status description
            HStack(spacing: 8) {
                switch agent.status {
                case .running:
                    ProgressView()
                        .scaleEffect(0.8)
                    Text("Running \(agent.flowDisplay) flow...")
                        .foregroundStyle(.secondary)

                case .waiting:
                    Image(systemName: "pause.circle.fill")
                        .foregroundStyle(.yellow)
                    Text("Waiting (PR limit reached)")
                        .foregroundStyle(.secondary)

                case .completed:
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(.green)
                    Text("Completed")
                        .foregroundStyle(.secondary)

                case .error:
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.red)
                    Text("Error occurred")
                        .foregroundStyle(.secondary)

                case .idle:
                    EmptyView()
                }
            }
            .font(.subheadline)

            // Live output
            if agent.status == .running {
                liveOutputSection
            }

            // Iteration history
            if agent.iteration > 0 {
                iterationHistory
            }
        }
        .padding(20)
    }

    private var liveOutputSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Live Output")
                .font(.caption)
                .fontWeight(.medium)
                .foregroundStyle(.secondary)

            let output = appState.liveOutput(for: agent)
            if output.isEmpty {
                Text("Waiting for output...")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(12)
                    .background(palette.surface)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            } else {
                LoopLiveOutput(lines: output)
                    .frame(height: 120)
            }
        }
    }

    private var iterationHistory: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("History")
                .font(.caption)
                .fontWeight(.medium)
                .foregroundStyle(.secondary)

            VStack(spacing: 4) {
                ForEach(1...min(agent.iteration, 5), id: \.self) { i in
                    let iter = agent.iteration - i + 1
                    HStack {
                        Image(systemName: "checkmark.circle")
                            .font(.caption)
                            .foregroundStyle(.green)
                        Text("Iteration \(iter)")
                            .font(.caption)
                        Spacer()
                    }
                    .padding(.vertical, 4)
                }
            }
            .padding(12)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
    }

    // MARK: - Changed Files Section

    private var changedFilesSection: some View {
        DisclosureGroup(isExpanded: $changedFilesExpanded) {
            if isLoadingFiles {
                ProgressView()
                    .scaleEffect(0.8)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 20)
            } else if fileStats.isEmpty {
                VStack(spacing: 8) {
                    Image(systemName: "doc.text")
                        .font(.title2)
                        .foregroundStyle(.tertiary)
                    Text("No changes")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 24)
            } else {
                VStack(spacing: 6) {
                    fileSummaryBar

                    ForEach(fileStats) { file in
                        fileRow(file)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
            }
        } label: {
            HStack {
                Image(systemName: "doc.text.fill")
                    .font(.subheadline)
                Text("Changed Files")
                    .font(.subheadline)
                    .fontWeight(.semibold)
                if !fileStats.isEmpty {
                    Text("(\(fileStats.count))")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
                Spacer()
            }
            .foregroundStyle(.primary)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    private var fileSummaryBar: some View {
        let totalAdditions = fileStats.reduce(0) { $0 + $1.additions }
        let totalDeletions = fileStats.reduce(0) { $0 + $1.deletions }

        return HStack(spacing: 16) {
            HStack(spacing: 4) {
                Text("+\(totalAdditions)")
                    .font(.system(.caption, design: .monospaced))
                    .fontWeight(.medium)
                    .foregroundStyle(.green)
            }

            HStack(spacing: 4) {
                Text("-\(totalDeletions)")
                    .font(.system(.caption, design: .monospaced))
                    .fontWeight(.medium)
                    .foregroundStyle(.red)
            }

            Spacer()

            Text("\(fileStats.count) files changed")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(palette.surface)
        )
    }

    private func fileRow(_ file: FileDiffStat) -> some View {
        HStack(spacing: 8) {
            fileIcon(for: file.fileExtension)
                .frame(width: 16)

            VStack(alignment: .leading, spacing: 1) {
                Text(file.filename)
                    .font(.subheadline)
                    .fontWeight(.medium)
                    .lineLimit(1)

                if !file.directory.isEmpty {
                    Text(file.directory)
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                }
            }

            Spacer()

            changeBar(additions: file.additions, deletions: file.deletions)

            HStack(spacing: 8) {
                Text("+\(file.additions)")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundStyle(.green)

                Text("-\(file.deletions)")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundStyle(.red)
            }
            .frame(width: 70, alignment: .trailing)
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 10)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(palette.surface)
        )
    }

    private func fileIcon(for ext: String) -> some View {
        let (icon, color): (String, Color) = switch ext {
        case "swift": ("swift", .orange)
        case "py": ("text.page", .blue)
        case "ts", "tsx", "js", "jsx": ("curlybraces", .yellow)
        case "md": ("doc.richtext", .purple)
        case "yaml", "yml", "json": ("gearshape", .gray)
        case "css", "scss": ("paintbrush", .pink)
        case "html": ("chevron.left.forwardslash.chevron.right", .orange)
        default: ("doc", .gray)
        }

        return Image(systemName: icon)
            .font(.caption)
            .foregroundStyle(color)
    }

    private func changeBar(additions: Int, deletions: Int) -> some View {
        let total = additions + deletions
        let maxBlocks = 5
        let addBlocks: Int
        if total > 0 && additions > 0 {
            addBlocks = max(1, Int(round(Double(additions) / Double(total) * Double(maxBlocks))))
        } else {
            addBlocks = 0
        }
        let delBlocks = total > 0 ? maxBlocks - addBlocks : 0

        return HStack(spacing: 1) {
            ForEach(0..<addBlocks, id: \.self) { _ in
                Rectangle()
                    .fill(Color.green)
                    .frame(width: 6, height: 10)
            }
            ForEach(0..<delBlocks, id: \.self) { _ in
                Rectangle()
                    .fill(Color.red)
                    .frame(width: 6, height: 10)
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 2))
    }

    // MARK: - Actions

    private func loadData() {
        guard let path = agent.worktreePath else { return }
        loadFileStats(worktreePath: path)
    }

    private func loadFileStats(worktreePath: String) {
        isLoadingFiles = true
        Task {
            do {
                let worktreeURL = URL(fileURLWithPath: worktreePath)
                let base = worktree?.baseBranch ?? "main"
                fileStats = try await worktreeService.getDiffStats("\(base)...HEAD", in: worktreeURL)
            } catch {
                fileStats = []
            }
            isLoadingFiles = false
        }
    }

    private func openInTerminal(path: String) {
        do {
            try terminalLauncher.launchTerminal(terminalApp, at: URL(fileURLWithPath: path))
        } catch {
            actionError = "Failed to open terminal: \(error.localizedDescription)"
            showingActionError = true
        }
    }

    private func openInIDE(path: String) {
        do {
            try terminalLauncher.openInIDE(ideApp, at: URL(fileURLWithPath: path))
        } catch {
            actionError = "Failed to open \(ideApp.displayName): \(error.localizedDescription)"
            showingActionError = true
        }
    }

    private func landBranch() {
        guard let wt = worktree else { return }
        Task {
            do {
                try await appState.landBranch(for: wt)
            } catch {
                await MainActor.run {
                    actionError = "Failed to land branch: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }

    private func stopAgent() {
        Task {
            do {
                try await appState.stopAgent(agent)
            } catch {
                await MainActor.run {
                    actionError = "Failed to stop agent: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }
}

#Preview {
    let state = AppState()
    state.configureMockAgents()
    let agent = state.agents.first!
    return AgentDetailPanel(appState: state, agent: agent)
        .frame(width: 600, height: 600)
}
