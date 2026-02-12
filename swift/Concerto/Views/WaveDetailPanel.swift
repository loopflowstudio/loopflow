// Wave detail panel. Adapts to wave state:
// - Idle → StepRunner (run individual steps)
// - Running/waiting/etc → Progress + actions

import SwiftUI
import LoopflowCore

struct WaveDetailPanel: View {
    private enum DetailTab: String, CaseIterable {
        case current = "Current"
        case runs = "Runs"
    }

    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.palette) private var palette
    @Environment(\.screenshotTab) private var screenshotTab

    @State private var actionError: String?
    @State private var showingActionError = false
    @State private var showingStopConfirmation = false
    @State private var editingName: String = ""
    @State private var isEditingName = false
    @State private var currentTime = Date()
    @State private var selectedTab: DetailTab = .current
    @State private var hasAppliedScreenshotTab = false
    @FocusState private var isNameFocused: Bool

    private let terminalLauncher = TerminalLauncher()
    private let elapsedTimeTimer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    private var waveRuns: [WaveRun] { repoState.runStore.runs(for: wave.id) }

    private var ideApp: IDEApp { .cursor }
    private var terminalApp: TerminalApp { .warp }

    var body: some View {
        Group {
            if outputBuffer.hasActiveSession(for: wave.id),
               let session = outputBuffer.interactiveSession {
                InteractiveSessionView(session: session)
            } else {
                blendedView
            }
        }
        .background(palette.background)
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
        .onChange(of: isNameFocused) { _, focused in
            if !focused && isEditingName {
                commitNameChange()
            }
        }
        .onReceive(elapsedTimeTimer) { time in
            // Update current time for elapsed time display (only when running)
            if wave.status == .running {
                currentTime = time
            }
        }
        .onAppear {
            outputBuffer.startStreaming(waveId: wave.id)
            repoState.loadRuns(for: wave.id)
            if !hasAppliedScreenshotTab, let tab = screenshotTab {
                hasAppliedScreenshotTab = true
                if let match = DetailTab.allCases.first(where: { $0.rawValue.lowercased() == tab.lowercased() }) {
                    selectedTab = match
                }
            }
        }
        .onDisappear {
            outputBuffer.stopStreaming(waveId: wave.id)
        }
        .onChange(of: wave.id) { oldId, newId in
            outputBuffer.stopStreaming(waveId: oldId)
            outputBuffer.startStreaming(waveId: newId)
            repoState.loadRuns(for: newId)
        }
    }

    // MARK: - Blended View (header + context + actions)

    private var blendedView: some View {
        VStack(spacing: 0) {
            header

            Divider()

            ScrollView {
                VStack(spacing: 16) {
                    if selectedTab == .current {
                        if wave.status == .idle || wave.status == .failed {
                            if wave.status == .failed {
                                failedRunDetail
                            }

                            StepRunner(wave: wave)

                            if !wave.commits.isEmpty {
                                commitLogSection
                            }

                            if let stat = wave.diffStat {
                                diffStatSection(stat)
                            }

                            if !wave.commits.isEmpty || wave.prURL != nil {
                                opsActionsBar
                            } else if wave.status == .idle && !wave.recentSteps.isEmpty {
                                Divider()
                                NextActionsBar(wave: wave)
                            }

                            if wave.status == .failed && !outputBuffer.output(for: wave.id).isEmpty {
                                liveOutputSection
                            }
                        } else {
                            waveConfigSummary
                            runProgressSection
                        }

                        if wave.worktreePath != nil {
                            quickActionsBar
                        }
                    } else {
                        WaveRunsTab(
                            wave: wave,
                            runs: waveRuns,
                            onCollapse: collapsePRs,
                            onAbsorb: absorbIntoPR(_:)
                        )
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
                        .font(Typography.body())
                        .foregroundStyle(wave.statusIndicator.color)

                    if isEditingName {
                        TextField("Wave name", text: $editingName)
                            .font(Typography.sectionTitle())
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
                            .font(Typography.sectionTitle())
                            .fontWeight(.semibold)
                            .onTapGesture {
                                startNameEdit()
                            }
                    }

                    if wave.iteration > 0 {
                        Text("iter \(wave.iteration)")
                            .font(Typography.caption())
                            .foregroundStyle(.secondary)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(palette.surface)
                            .clipShape(Capsule())
                    }

                    if !waveRuns.isEmpty {
                        IterationTimeline(runs: waveRuns)
                    }
                }

                Text(wave.statusText)
                    .font(Typography.caption())
                    .foregroundStyle(.secondary)
            }

            Spacer()

            // PR badge if available (from Wave fields)
            if let prNumber = wave.prNumber, let prState = wave.prState {
                prBadge(number: prNumber, state: prState, url: wave.prURL)
            }
            if wave.effectiveOpenPRCount > 1 {
                openPRCountBadge
            }

            // Stop button when running
            if wave.status == .running || wave.status == .waiting {
                Button {
                    showingStopConfirmation = true
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "stop.fill")
                            .font(Typography.caption())
                        Text("Stop")
                    }
                }
                .buttonStyle(DestructiveButtonStyle())
            }

            Picker("", selection: $selectedTab) {
                Text("Current").tag(DetailTab.current)
                Text("Runs (\(waveRuns.count))").tag(DetailTab.runs)
            }
            .pickerStyle(.segmented)
            .frame(maxWidth: 220)
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 16)
    }

    private func prBadge(number: Int, state: PRState, url: URL?) -> some View {
        let color: Color = switch state {
        case .open: .statusSuccess
        case .merged: .statusInfo
        case .closed: .statusError
        case .draft: .statusWarning
        }

        return Button {
            if let url {
                terminalLauncher.openURL(url)
            }
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "arrow.up.right.square")
                    .font(Typography.caption(10))
                Text("PR #\(number)")
                    .font(Typography.caption())
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

    private var openPRCountBadge: some View {
        Label("\(wave.effectiveOpenPRCount) open", systemImage: "arrow.triangle.pull")
            .font(Typography.caption())
            .fontWeight(.medium)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(Color.statusWarning.opacity(0.14))
            .foregroundStyle(Color.statusWarning)
            .clipShape(Capsule())
            .help("\(wave.effectiveOpenPRCount) open PRs in this stack")
    }

    // MARK: - Wave Config Summary (read-only, shown when running)

    private var waveConfigSummary: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.lg) {
                if !wave.areaDisplay.isEmpty {
                    Label {
                        Text(wave.areaDisplay)
                            .font(Typography.caption())
                    } icon: {
                        Image(systemName: "folder")
                            .font(Typography.caption(10))
                    }
                    .foregroundStyle(.secondary)
                }

                if !wave.directionDisplay.isEmpty {
                    Label {
                        Text(wave.directionDisplay)
                            .font(Typography.caption())
                    } icon: {
                        Image(systemName: "target")
                            .font(Typography.caption(10))
                    }
                    .foregroundStyle(.secondary)
                }

                if !wave.flow.isEmpty {
                    Label {
                        Text(wave.flow)
                            .font(Typography.caption())
                    } icon: {
                        Image(systemName: "arrow.triangle.branch")
                            .font(Typography.caption(10))
                    }
                    .foregroundStyle(.secondary)
                }
            }
        }
    }

    // MARK: - Quick Actions Bar

    private var quickActionsBar: some View {
        HStack(spacing: 12) {
            if let path = wave.worktreePath {
                let hasWorktree = FileManager.default.fileExists(atPath: path)

                Button {
                    openInTerminal(path: path)
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "terminal")
                            .font(Typography.caption())
                        Text(terminalApp.displayName)
                            .font(Typography.caption())
                    }
                }
                .buttonStyle(GhostButtonStyle())
                .disabled(!hasWorktree)
                .help(hasWorktree ? "Open in \(terminalApp.displayName)" : "Worktree path no longer exists")

                Button {
                    openInIDE(path: path)
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "curlybraces")
                            .font(Typography.caption())
                        Text(ideApp.displayName)
                            .font(Typography.caption())
                    }
                }
                .buttonStyle(GhostButtonStyle())
                .disabled(!hasWorktree)
                .help(hasWorktree ? "Open in \(ideApp.displayName)" : "Worktree path no longer exists")

                if !hasWorktree {
                    Label("Worktree missing", systemImage: "exclamationmark.triangle")
                        .font(Typography.caption())
                        .foregroundStyle(Color.statusWarning)
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
                Text("Progress")
                    .font(Typography.sectionTitle())

                // Status description with progress
                if wave.status == .running {
                    FlowProgressPills(
                        steps: wave.flowSteps.isEmpty ? [wave.flow] : wave.flowSteps,
                        currentIndex: wave.stepIndex,
                        startedAt: wave.activeRun?.startedAt ?? wave.runStartedAt
                    )
                    .font(Typography.body())
                }

                // Activity summary (recent output line)
                if wave.status == .running,
                   let recentOutput = outputBuffer.recentOutput(for: wave.id) {
                    Text(recentOutput)
                        .font(Typography.caption())
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(palette.background)
                        .clipShape(RoundedRectangle(cornerRadius: 4))
                }

                // Commits and diff while running
                if !wave.commits.isEmpty {
                    commitLogSection
                }

                if let stat = wave.diffStat {
                    diffStatSection(stat)
                }

                // Live output (running or any buffered output)
                if wave.status == .running || !outputBuffer.output(for: wave.id).isEmpty {
                    liveOutputSection
                }
            }
            .padding(16)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
    }

    private var failedRunDetail: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Error message from the run
            if let error = wave.activeRun?.error {
                HStack(alignment: .top, spacing: 8) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(Color.statusError)
                        .font(Typography.caption())
                    Text(error)
                        .font(Typography.caption())
                        .foregroundStyle(.primary)
                        .textSelection(.enabled)
                }
                .padding(12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.statusError.opacity(0.08))
                .clipShape(RoundedRectangle(cornerRadius: 8))
            } else {
                Text("No error details available.")
                    .font(Typography.caption())
                    .foregroundStyle(.tertiary)
            }

            // Failed step context
            if let step = wave.activeRun?.currentStep {
                Text("Failed during: \(step)")
                    .font(Typography.caption())
                    .foregroundStyle(.secondary)
            }

            // Retry button
            Button {
                retryWave()
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "arrow.counterclockwise")
                        .font(Typography.caption())
                    Text("Retry")
                }
            }
            .buttonStyle(DarkButtonStyle())
        }
    }

    private var liveOutputSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(wave.status == .running ? "Live Output" : "Output")
                .font(Typography.caption())
                .fontWeight(.medium)
                .foregroundStyle(.secondary)

            let output = outputBuffer.output(for: wave.id)
            if output.isEmpty {
                Text("Waiting for output…")
                    .font(Typography.caption())
                    .foregroundStyle(.tertiary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(12)
                    .background(palette.background)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            } else {
                LiveOutput(lines: output)
                    .frame(minHeight: 120, maxHeight: 300)
            }
        }
    }

    // MARK: - Git State Sections

    private var commitLogSection: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Commits")
                .font(Typography.caption())
                .fontWeight(.medium)
                .foregroundStyle(.secondary)

            VStack(spacing: 0) {
                ForEach(wave.commits) { entry in
                    HStack(spacing: Spacing.sm) {
                        Text(entry.sha)
                            .font(Typography.code(11))
                            .foregroundStyle(.secondary)
                            .frame(width: 60, alignment: .leading)

                        Text(entry.message)
                            .font(Typography.caption())
                            .lineLimit(1)
                            .truncationMode(.tail)

                        Spacer()
                    }
                    .padding(.vertical, 4)
                    .padding(.horizontal, Spacing.sm)

                    if entry.sha != wave.commits.last?.sha {
                        Divider()
                    }
                }
            }
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        }
    }

    private func diffStatSection(_ stat: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Diff")
                .font(Typography.caption())
                .fontWeight(.medium)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(stat.components(separatedBy: "\n").enumerated()), id: \.offset) { _, line in
                    coloredDiffStatLine(line)
                }
            }
            .font(Typography.code(11))
            .textSelection(.enabled)
            .padding(Spacing.sm)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        }
    }

    private func coloredDiffStatLine(_ line: String) -> Text {
        // Lines with "|" have a stat bar: " src/foo.rs | 10 ++++----"
        guard let pipeRange = line.range(of: " | ") else {
            // Summary line: color insertion/deletion counts
            return coloredDiffSummaryLine(line)
        }

        let prefix = Text(String(line[line.startIndex..<pipeRange.upperBound]))
            .foregroundColor(.secondary)

        let bar = String(line[pipeRange.upperBound...])
        var result = prefix
        for char in bar {
            switch char {
            case "+":
                result = result + Text("+").foregroundColor(Color.statusSuccess)
            case "-":
                result = result + Text("-").foregroundColor(Color.statusError)
            default:
                result = result + Text(String(char)).foregroundColor(.secondary)
            }
        }
        return result
    }

    private func coloredDiffSummaryLine(_ line: String) -> Text {
        // "2 files changed, 8 insertions(+), 7 deletions(-)"
        let parts = line.components(separatedBy: ", ")
        var result = Text("")
        for (i, part) in parts.enumerated() {
            let separator = i > 0 ? Text(", ").foregroundColor(.secondary) : Text("")
            if part.contains("insertion") {
                result = result + separator + Text(part).foregroundColor(Color.statusSuccess)
            } else if part.contains("deletion") {
                result = result + separator + Text(part).foregroundColor(Color.statusError)
            } else {
                result = result + separator + Text(part).foregroundColor(.secondary)
            }
        }
        return result
    }

    private var opsActionsBar: some View {
        HStack(spacing: Spacing.lg) {
            // Commit count
            HStack(spacing: Spacing.sm) {
                Image(systemName: "point.3.filled.connected.trianglepath.dotted")
                    .foregroundStyle(.secondary)
                Text("\(wave.commits.count) commit\(wave.commits.count == 1 ? "" : "s")")
                    .font(Typography.body())
                    .foregroundStyle(.secondary)
            }

            Spacer()

            // View PR
            if let prURL = wave.prURL {
                Button {
                    terminalLauncher.openURL(prURL)
                } label: {
                    HStack(spacing: Spacing.xs) {
                        Image(systemName: "arrow.up.right.square")
                            .font(Typography.caption())
                        Text("View PR")
                    }
                }
                .buttonStyle(GhostButtonStyle())
            }

            // Land
            Button {
                landWave()
            } label: {
                HStack(spacing: Spacing.xs) {
                    if repoState.isActionInFlight(wave.id) {
                        ProgressView()
                            .scaleEffect(0.6)
                    } else {
                        Image(systemName: "arrow.merge")
                            .font(Typography.caption())
                    }
                    Text("Land")
                }
            }
            .buttonStyle(DarkButtonStyle())
            .disabled(repoState.isActionInFlight(wave.id))

            // Next
            Button {
                nextWave()
            } label: {
                HStack(spacing: Spacing.xs) {
                    Image(systemName: "arrow.forward.circle")
                        .font(Typography.caption())
                    Text("Next")
                }
            }
            .buttonStyle(DarkButtonStyle())
            .disabled(repoState.isActionInFlight(wave.id))
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.md)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }

    // MARK: - Actions

    private func landWave() {
        Task {
            do {
                try await repoState.landWave(wave)
            } catch {
                await MainActor.run {
                    actionError = "Failed to land: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }

    private func nextWave() {
        Task {
            do {
                try await repoState.nextWave(wave)
            } catch {
                await MainActor.run {
                    actionError = "Failed to start next: \(error.localizedDescription)"
                    showingActionError = true
                }
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

    private func retryWave() {
        Task {
            do {
                outputBuffer.clearOutput(for: wave.id)
                try await repoState.runWave(wave: wave)
            } catch {
                await MainActor.run {
                    actionError = "Failed to retry wave: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }

    private func collapsePRs() async throws -> CollapsePRsResult {
        try await repoState.collapsePRs(wave.id)
    }

    private func absorbIntoPR(_ prNumber: Int) async throws -> AbsorbIntoPRResult {
        try await repoState.absorbIntoPR(wave.id, prNumber: prNumber)
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
        .environment(OutputBuffer())
        .frame(width: 600, height: 700)
}
