// Wave detail panel. Adapts to wave state:
// - No area → AreaPicker (pick where to work)
// - Has area, idle → StepRunner (run individual steps)
// - Running/waiting/etc → Progress + actions

import SwiftUI
import LoopflowCore

struct WaveDetailPanel: View {
    let wave: Wave

    @Environment(RepoState.self) private var repoState
    @Environment(SessionState.self) private var sessionState
    @Environment(\.colorScheme) private var colorScheme
    @AppStorage("commitsExpanded") private var commitsExpanded = true
    @AppStorage("changedFilesExpanded") private var changedFilesExpanded = true

    @State private var fileStats: [FileDiffStat] = []
    @State private var isLoadingFiles = false
    @State private var commits: [CommitInfo] = []
    @State private var isLoadingCommits = false
    @State private var actionError: String?
    @State private var showingActionError = false
    @State private var showingStopConfirmation = false
    @State private var showingAbandonConfirmation = false
    @State private var editingName: String = ""
    @State private var isEditingName = false
    @FocusState private var isNameFocused: Bool

    private let terminalLauncher = TerminalLauncher()
    private let worktreeService = WorktreeService()

    private var ideApp: IDEApp { repoState.config?.ideApp ?? .cursor }
    private var terminalApp: TerminalApp { repoState.config?.terminalApp ?? .warp }
    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    var body: some View {
        Group {
            if sessionState.hasActiveSession(for: wave.id),
               let session = sessionState.interactiveSession {
                InteractiveSessionView(session: session)
            } else {
                blendedView
            }
        }
        .background(palette.background)
        .onAppear {
            loadData()
        }
        .onChange(of: wave.id) {
            loadData()
        }
        .alert("Error", isPresented: $showingActionError) {
            Button("OK") { actionError = nil }
        } message: {
            Text(actionError ?? "An error occurred")
        }
        .confirmationDialog(
            "Stop Wave",
            isPresented: $showingStopConfirmation
        ) {
            Button("Stop", role: .destructive) {
                stopWave()
            }
        } message: {
            Text("Stop '\(wave.displayName)'? It can be restarted later.")
        }
        .confirmationDialog(
            "Abandon Wave",
            isPresented: $showingAbandonConfirmation
        ) {
            Button("Abandon", role: .destructive) {
                abandonWave()
            }
        } message: {
            Text("Delete '\(wave.displayName)'? This removes the wave from the daemon.")
        }
        .onReceive(NotificationCenter.default.publisher(for: .editWaveName)) { _ in
            // Only respond if this wave is selected
            if repoState.selectedWave?.id == wave.id {
                startNameEdit()
            }
        }
    }

    // MARK: - Blended View (header + context + actions)

    private var blendedView: some View {
        VStack(spacing: 0) {
            header

            Divider()

            actionBar

            Divider()

            ScrollView {
                VStack(spacing: 16) {
                    if wave.area == nil {
                        AreaPicker(wave: wave)
                    } else if wave.status == .idle {
                        StepRunner(wave: wave)
                        if !wave.recentSteps.isEmpty {
                            Divider()
                            NextActionsBar(wave: wave)
                        }
                    } else {
                        runProgressSection
                    }

                    if wave.worktreePath != nil {
                        commitsSection
                        changedFilesSection
                    }
                }
                .padding(20)
            }
        }
    }

    // MARK: - Header

    private var header: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 8) {
                    // Status indicator
                    Image(systemName: wave.statusIndicator.icon)
                        .font(.system(size: 14))
                        .foregroundStyle(wave.statusIndicator.color)

                    if isEditingName {
                        TextField("Wave name", text: $editingName)
                            .font(.title2)
                            .fontWeight(.semibold)
                            .textFieldStyle(.plain)
                            .focused($isNameFocused)
                            .frame(minWidth: 150)
                            .onSubmit {
                                commitNameChange()
                            }
                            .onExitCommand {
                                cancelNameEdit()
                            }
                    } else {
                        Text(wave.displayName)
                            .font(.title2)
                            .fontWeight(.semibold)
                            .onTapGesture(count: 2) {
                                startNameEdit()
                            }
                    }

                    if wave.iteration > 0 {
                        Text("iter \(wave.iteration)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(palette.surface)
                            .clipShape(Capsule())
                    }
                }

                Text(wave.statusText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 16)
    }

    // MARK: - Idle Wave View

    private var idleWaveView: some View {
        VStack(spacing: 16) {
            // Big start button - prominent green
            Button {
                launchInteractive()
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "play.fill")
                        .font(.title3)
                    Text("Start \(wave.flowDisplay)")
                        .font(.title3)
                        .fontWeight(.semibold)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 16)
                .background(Color.green)
                .foregroundStyle(.white)
                .clipShape(RoundedRectangle(cornerRadius: 10))
            }
            .buttonStyle(.plain)
            .disabled(wave.worktreePath == nil)
            .opacity(wave.worktreePath == nil ? 0.5 : 1)

            // Subtle config summary
            if !wave.areaDisplay.isEmpty || !wave.directionDisplay.isEmpty {
                HStack(spacing: 16) {
                    if !wave.areaDisplay.isEmpty {
                        Label(wave.areaDisplay, systemImage: "folder")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    if !wave.directionDisplay.isEmpty && wave.directionDisplay != "default" {
                        Label(wave.directionDisplay, systemImage: "target")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .padding(16)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    // MARK: - Action Bar

    private var actionBar: some View {
        let hasWorktree = wave.worktreePath != nil

        return HStack(spacing: 12) {
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
            .disabled(!hasWorktree || !wave.hasDiff)
            .help("Land branch (creates PR if needed)")

            Button {
                if let path = wave.worktreePath {
                    openInIDE(path: path)
                }
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "curlybraces")
                        .font(.system(size: 12))
                    Text(ideApp.displayName)
                        .font(.caption)
                }
            }
            .buttonStyle(DarkButtonStyle())
            .disabled(!hasWorktree)
            .help("Open in \(ideApp.displayName)")

            Button {
                if let path = wave.worktreePath {
                    openInTerminal(path: path)
                }
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "terminal")
                        .font(.system(size: 12))
                    Text(terminalApp.displayName)
                        .font(.caption)
                }
            }
            .buttonStyle(DarkButtonStyle())
            .disabled(!hasWorktree)
            .help("Open in \(terminalApp.displayName)")

            Spacer()

            if wave.status == .running || wave.status == .waiting {
                Button {
                    showingStopConfirmation = true
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "stop.fill")
                            .font(.system(size: 12))
                        Text("Stop")
                            .font(.caption)
                    }
                }
                .buttonStyle(OutlineButtonStyle())
                .help("Stop wave")
            }

            Button {
                showingAbandonConfirmation = true
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "trash")
                        .font(.system(size: 12))
                    Text("Abandon")
                        .font(.caption)
                }
            }
            .buttonStyle(OutlineButtonStyle())
            .help("Delete this wave")
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 12)
        .background(palette.surface)
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
                switch wave.status {
                case .running:
                    ProgressView()
                        .scaleEffect(0.8)
                    Text("Running \(wave.flowDisplay) flow...")
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
            if wave.status == .running {
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

            if let path = wave.worktreePath {
                GhosttyTerminalView(workingDirectory: path)
                    .frame(height: 200)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            } else {
                let output = sessionState.output(for: wave.id)
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

    // MARK: - Commits Section

    private var commitsSection: some View {
        DisclosureGroup(isExpanded: $commitsExpanded) {
            if isLoadingCommits {
                ProgressView()
                    .scaleEffect(0.8)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 20)
            } else if commits.isEmpty {
                VStack(spacing: 8) {
                    Image(systemName: "point.topleft.down.curvedto.point.bottomright.up")
                        .font(.title2)
                        .foregroundStyle(.tertiary)
                    Text("No commits yet")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 24)
            } else {
                VStack(spacing: 8) {
                    ForEach(commits) { commit in
                        commitRow(commit)
                    }
                }
                .padding(.vertical, 12)
            }
        } label: {
            HStack {
                Image(systemName: "checkmark.circle")
                    .font(.subheadline)
                Text("Commits")
                    .font(.subheadline)
                    .fontWeight(.semibold)
                if !commits.isEmpty {
                    Text("(\(commits.count))")
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

    private func commitRow(_ commit: CommitInfo) -> some View {
        HStack(spacing: 10) {
            Text(commit.shortSHA)
                .font(.system(.caption2, design: .monospaced))
                .foregroundStyle(.secondary)
                .frame(width: 52, alignment: .leading)

            VStack(alignment: .leading, spacing: 2) {
                Text(commit.message)
                    .font(.subheadline)
                    .fontWeight(.medium)
                    .lineLimit(1)

                Text("\(commit.author) · \(commit.relativeTime)")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }

            Spacer()
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(palette.background)
        )
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
        guard let path = wave.worktreePath else {
            fileStats = []
            commits = []
            isLoadingFiles = false
            isLoadingCommits = false
            return
        }
        loadFileStats(worktreePath: path)
        loadCommits(worktreePath: path)
    }

    private func loadFileStats(worktreePath: String) {
        isLoadingFiles = true
        Task {
            do {
                let worktreeURL = URL(fileURLWithPath: worktreePath)
                let stats = try await worktreeService.getDiffStats("main...HEAD", in: worktreeURL)
                await MainActor.run {
                    fileStats = stats
                }
            } catch {
                await MainActor.run {
                    fileStats = []
                }
            }
            await MainActor.run {
                isLoadingFiles = false
            }
        }
    }

    private func loadCommits(worktreePath: String) {
        isLoadingCommits = true
        Task {
            do {
                let branchName = wave.branch ?? URL(fileURLWithPath: worktreePath).lastPathComponent
                let worktree = Worktree(path: worktreePath, branch: branchName, baseBranch: "main")
                let items = try await worktreeService.getCommits(for: worktree)
                await MainActor.run {
                    commits = items
                }
            } catch {
                await MainActor.run {
                    commits = []
                }
            }
            await MainActor.run {
                isLoadingCommits = false
            }
        }
    }

    private func launchInteractive() {
        guard let path = wave.worktreePath else { return }
        sessionState.launchInteractiveSession(
            waveId: wave.id,
            step: wave.flow,
            worktreePath: path
        )
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
        Task {
            do {
                try await repoState.landBranch(for: wave)
            } catch {
                await MainActor.run {
                    actionError = "Failed to land branch: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }

    private func stopWave() {
        Task {
            do {
                try await repoState.stopWave(wave)
            } catch {
                await MainActor.run {
                    actionError = "Failed to stop wave: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }

    private func abandonWave() {
        Task {
            do {
                try await repoState.deleteWave(wave)
            } catch {
                await MainActor.run {
                    actionError = "Failed to abandon wave: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }

    // MARK: - Name Editing

    private func startNameEdit() {
        editingName = wave.name
        isEditingName = true
        // Small delay to ensure TextField is mounted
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(50))
            isNameFocused = true
        }
    }

    private func cancelNameEdit() {
        isEditingName = false
        isNameFocused = false
    }

    private func commitNameChange() {
        let newName = editingName.trimmingCharacters(in: .whitespacesAndNewlines)
        isEditingName = false
        isNameFocused = false

        guard !newName.isEmpty, newName != wave.name else { return }

        Task {
            do {
                try await repoState.renameWave(wave, to: newName)
            } catch {
                await MainActor.run {
                    actionError = "Failed to rename: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }
}

#Preview {
    let repoState = RepoState()
    repoState.configureMockWaves()
    let wave = repoState.waves.first!
    return WaveDetailPanel(wave: wave)
        .environment(repoState)
        .environment(SessionState())
        .frame(width: 600, height: 700)
}
