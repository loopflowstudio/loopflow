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
    @AppStorage("changedFilesExpanded") private var changedFilesExpanded = true

    @State private var fileStats: [FileDiffStat] = []
    @State private var isLoadingFiles = false
    @State private var actionError: String?
    @State private var showingActionError = false
    @State private var showingStopConfirmation = false
    @State private var isCloning = false
    @State private var editingName: String = ""
    @State private var isEditingName = false
    @State private var currentTime = Date()
    @FocusState private var isNameFocused: Bool

    private let terminalLauncher = TerminalLauncher()
    private let elapsedTimeTimer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()
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
        .onReceive(NotificationCenter.default.publisher(for: .editWaveName)) { _ in
            // Only respond if this wave is selected
            if repoState.selectedWave?.id == wave.id {
                startNameEdit()
            }
        }
        .onReceive(elapsedTimeTimer) { time in
            // Update current time for elapsed time display (only when running)
            if wave.status == .running {
                currentTime = time
            }
        }
    }

    // MARK: - Blended View (header + context + actions)

    private var blendedView: some View {
        VStack(spacing: 0) {
            header

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
                        quickActionsBar
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

            // Clone button (prominent)
            Button {
                cloneWave()
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
            .help("Create a copy of this wave")

            // PR badge if available (from Wave fields)
            if let prNumber = wave.prNumber, let prState = wave.prState {
                prBadge(number: prNumber, state: prState, url: wave.prURL)
            }

            // Stop button when running
            if wave.status == .running || wave.status == .waiting {
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

    // MARK: - Quick Actions Bar

    private var quickActionsBar: some View {
        HStack(spacing: 12) {
            if let path = wave.worktreePath {
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

                if wave.hasDiff {
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

    @ViewBuilder
    private var runProgressSection: some View {
        if wave.status == .waiting {
            WaitingStateCard(wave: wave)
        } else {
            VStack(alignment: .leading, spacing: 16) {
                HStack {
                    Text("Progress")
                        .font(.headline)
                    Spacer()

                    // Connect button for running waves
                    if wave.status == .running, let path = wave.worktreePath {
                        Button {
                            connectToRunning(path: path)
                        } label: {
                            HStack(spacing: 4) {
                                Image(systemName: "terminal")
                                    .font(.caption)
                                Text("Connect")
                                    .font(.caption)
                            }
                        }
                        .buttonStyle(DarkButtonStyle())
                        .help("Connect to running terminal")
                    }
                }

                // Status description with progress
                HStack(spacing: 8) {
                    switch wave.status {
                    case .running:
                        ProgressView()
                            .scaleEffect(0.8)
                        Text(wave.progressDisplay(now: currentTime))
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

                    case .idle, .waiting:
                        EmptyView()
                    }
                }
                .font(.subheadline)

                // Activity summary (recent output line)
                if wave.status == .running,
                   let recentOutput = sessionState.recentOutput(for: wave.id) {
                    Text(recentOutput)
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(palette.background)
                        .clipShape(RoundedRectangle(cornerRadius: 4))
                }

                // Live output
                if wave.status == .running {
                    liveOutputSection
                }
            }
            .padding(16)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
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
        guard let path = wave.worktreePath else { return }
        loadFileStats(worktreePath: path)
    }

    private func loadFileStats(worktreePath: String) {
        isLoadingFiles = true
        Task {
            do {
                let worktreeURL = URL(fileURLWithPath: worktreePath)
                fileStats = try await worktreeService.getDiffStats("main...HEAD", in: worktreeURL)
            } catch {
                fileStats = []
            }
            isLoadingFiles = false
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

    private func connectToRunning(path: String) {
        // Open a shell terminal in the worktree directory
        // This lets users inspect/interact while the wave runs
        do {
            try terminalLauncher.launchTerminal(terminalApp, at: URL(fileURLWithPath: path))
        } catch {
            actionError = "Failed to connect: \(error.localizedDescription)"
            showingActionError = true
        }
    }

    private func cloneWave() {
        isCloning = true
        Task {
            do {
                _ = try await repoState.cloneWave(wave)
            } catch {
                await MainActor.run {
                    actionError = "Failed to clone wave: \(error.localizedDescription)"
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
