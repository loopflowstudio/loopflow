// Agent detail panel showing config, stimulus toggle, actions, and flow experimentation.

import SwiftUI
import LoopflowCore

struct AgentDetailPanel: View {
    @Bindable var appState: AppState
    let agent: Agent

    @Environment(\.colorScheme) private var colorScheme
    @AppStorage("changedFilesExpanded") private var changedFilesExpanded = true

    @State private var fileStats: [FileDiffStat] = []
    @State private var isLoadingFiles = false
    @State private var actionError: String?
    @State private var showingActionError = false
    @State private var showingStopConfirmation = false
    @State private var isCloning = false

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

            ScrollView {
                VStack(spacing: 16) {
                    // Config section (always visible)
                    configSection

                    if agent.status == .idle {
                        // Stimulus controls when idle
                        stimulusSection

                        // Try a different flow (experimentation)
                        experimentSection
                    } else {
                        // Show run progress for active agents
                        runProgressSection
                    }

                    if agent.worktreePath != nil {
                        quickActionsBar
                        changedFilesSection
                    }
                }
                .padding(20)
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

                    if agent.iteration > 0 {
                        Text("iter \(agent.iteration)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(palette.surface)
                            .clipShape(Capsule())
                    }
                }

                Text(agent.statusText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            // Clone button (prominent)
            Button {
                cloneAgent()
            } label: {
                HStack(spacing: 4) {
                    if isCloning {
                        ProgressView()
                            .scaleEffect(0.6)
                    } else {
                        Image(systemName: "doc.on.doc")
                            .font(.caption)
                    }
                    Text("Clone")
                }
            }
            .buttonStyle(DarkButtonStyle())
            .disabled(isCloning)
            .help("Create a copy of this agent")

            // PR badge if available
            if let wt = worktree, let prNumber = wt.prNumber, let prState = wt.prState {
                prBadge(number: prNumber, state: prState, url: wt.prURL)
            }

            // Stop button when running
            if agent.status == .running || agent.status == .waiting {
                Button {
                    showingStopConfirmation = true
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "stop.fill")
                            .font(.caption)
                        Text("Stop")
                    }
                }
                .buttonStyle(DarkButtonStyle())
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

    // MARK: - Config Section

    private var configSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Configuration")
                    .font(.headline)
                Spacer()
            }

            // Config display in a grid
            LazyVGrid(columns: [
                GridItem(.flexible(), alignment: .leading),
                GridItem(.flexible(), alignment: .leading)
            ], spacing: 8) {
                configRow(label: "Area", value: agent.areaDisplay.isEmpty ? "Not set" : agent.areaDisplay)
                configRow(label: "Goal", value: agent.goalDisplay.isEmpty ? "None" : agent.goalDisplay)
                configRow(label: "Flow", value: agent.flowDisplay)
                configRow(label: "Stimulus", value: agent.stimulusText)
            }
        }
        .padding(16)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    private func configRow(label: String, value: String) -> some View {
        HStack(spacing: 8) {
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(width: 50, alignment: .trailing)
            Text(value)
                .font(.subheadline)
                .lineLimit(1)
        }
    }

    // MARK: - Stimulus Section

    private var stimulusSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Stimulus")
                    .font(.headline)

                Spacer()

                // Paused toggle
                Toggle(isOn: Binding(
                    get: { !agent.paused },
                    set: { newValue in
                        togglePaused(!newValue)
                    }
                )) {
                    Text(agent.paused ? "Paused" : "Active")
                        .font(.caption)
                        .foregroundColor(agent.paused ? .secondary : .green)
                }
                .toggleStyle(.switch)
                .controlSize(.small)
            }

            // Explanation text
            if agent.paused {
                Text("Agent is paused. Stimulus won't fire automatically. Use Run to execute manually.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                HStack(spacing: 8) {
                    Image(systemName: "bolt.fill")
                        .font(.caption)
                        .foregroundStyle(.green)
                    Text("Stimulus active: \(stimulusDescription)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            // Run button
            if agent.isConfigured {
                Button {
                    runWithCurrentConfig()
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "play.fill")
                            .font(.caption)
                        Text("Run")
                            .fontWeight(.medium)
                    }
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
            } else {
                Text("Configure area to enable running.")
                    .font(.caption)
                    .foregroundStyle(.orange)
            }
        }
        .padding(16)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    private var stimulusDescription: String {
        switch agent.stimulus.kind {
        case .once:
            return "runs once when triggered"
        case .loop:
            return "runs continuously"
        case .watch:
            return "runs when \(agent.areaDisplay) changes"
        case .cron:
            return "runs on schedule (\(agent.stimulus.cron ?? ""))"
        case .manual:
            return "manual triggers only"
        }
    }

    // MARK: - Experiment Section

    private var experimentSection: some View {
        FlowPicker(agent: agent, appState: appState)
    }

    // MARK: - Quick Actions Bar

    private var quickActionsBar: some View {
        HStack(spacing: 12) {
            if let path = agent.worktreePath {
                Button {
                    openInTerminal(path: path)
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "terminal")
                            .font(.system(size: 12))
                        Text(terminalApp.displayName)
                            .font(.caption)
                    }
                }
                .buttonStyle(DarkButtonStyle())
                .help("Open in \(terminalApp.displayName)")

                Button {
                    openInIDE(path: path)
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "curlybraces")
                            .font(.system(size: 12))
                        Text(ideApp.displayName)
                            .font(.caption)
                    }
                }
                .buttonStyle(DarkButtonStyle())
                .help("Open in \(ideApp.displayName)")

                if let wt = worktree, wt.hasDiff {
                    Button {
                        landBranch()
                    } label: {
                        HStack(spacing: 6) {
                            Image(systemName: "airplane.arrival")
                                .font(.system(size: 12))
                            Text("Land")
                                .font(.caption)
                        }
                    }
                    .buttonStyle(DarkButtonStyle())
                    .help("Land branch (creates PR if needed)")
                }

                Spacer()
            }
        }
    }

    // MARK: - Run Progress Section

    private var runProgressSection: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text("Progress")
                    .font(.headline)
                Spacer()
            }

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
        }
        .padding(16)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    private var liveOutputSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Live Output")
                .font(.caption)
                .fontWeight(.medium)
                .foregroundStyle(.secondary)

            if Flags.embeddedTerminal, let path = agent.worktreePath {
                // Use embedded Ghostty terminal when feature flag is enabled
                GhosttyTerminalView(workingDirectory: path)
                    .frame(height: 200)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            } else {
                // Default text-based output
                let output = appState.liveOutput(for: agent)
                if output.isEmpty {
                    Text("Waiting for output...")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(12)
                        .background(palette.background)
                        .clipShape(RoundedRectangle(cornerRadius: 8))
                } else {
                    LoopLiveOutput(lines: output)
                        .frame(height: 120)
                }
            }
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
        .padding(16)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: 12))
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
                .fill(palette.background)
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
                .fill(palette.background)
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

    private func togglePaused(_ paused: Bool) {
        Task {
            do {
                try await appState.updateAgent(agent, paused: paused)
            } catch {
                await MainActor.run {
                    actionError = "Failed to update agent: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }

    private func runWithCurrentConfig() {
        Task {
            do {
                try await appState.runAgent(agent: agent)
            } catch {
                await MainActor.run {
                    actionError = "Failed to run agent: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }

    private func cloneAgent() {
        isCloning = true
        Task {
            do {
                _ = try await appState.cloneAgent(agent)
            } catch {
                await MainActor.run {
                    actionError = "Failed to clone agent: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
            await MainActor.run {
                isCloning = false
            }
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
        .frame(width: 600, height: 700)
}
