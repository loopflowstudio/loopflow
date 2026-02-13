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
                VStack(spacing: 0) {
                    if selectedTab == .current {
                        if wave.status == .idle || wave.status == .failed {
                            if wave.status == .failed {
                                failedRunDetail
                                    .padding(.bottom, Spacing.lg)
                            }

                            StepRunner(wave: wave)

                            if !wave.commits.isEmpty {
                                commitLogSection
                                    .padding(.top, Spacing.lg)
                            }

                            if let stat = wave.diffStat {
                                diffStatSection(stat)
                                    .padding(.top, Spacing.sm)
                            }

                            if !wave.commits.isEmpty || wave.prURL != nil {
                                opsActionsBar
                                    .padding(.top, Spacing.xl)
                            } else if wave.status == .idle && !wave.recentSteps.isEmpty {
                                Divider()
                                    .padding(.top, Spacing.lg)
                                NextActionsBar(wave: wave)
                                    .padding(.top, Spacing.lg)
                            }

                            if wave.status == .failed && !outputBuffer.output(for: wave.id).isEmpty {
                                liveOutputSection
                                    .padding(.top, Spacing.lg)
                            }
                        } else {
                            waveConfigSummary
                            runProgressSection
                                .padding(.top, Spacing.lg)
                        }

                        if wave.worktreePath != nil {
                            quickActionsBar
                                .padding(.top, Spacing.lg)
                        }
                    } else {
                        WaveRunsTab(
                            runs: waveRuns,
                            onCombine: combinePRs
                        )
                    }
                }
                .padding(Spacing.xl)
            }
        }
    }

    // MARK: - Header

    private var header: some View {
        HStack(spacing: Spacing.md) {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                HStack(spacing: Spacing.sm) {
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
                    Label("Stop", systemImage: "stop.fill")
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
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.lg)
    }

    private func prBadge(number: Int, state: PRState, url: URL?) -> some View {
        let color: Color = switch state {
        case .open: .statusSuccess
        case .merged: .statusInfo
        case .closed: .statusError
        case .draft: .statusWarning
        }

        return Button {
            if let url { terminalLauncher.openURL(url) }
        } label: {
            Label("PR #\(number)", systemImage: "arrow.up.right.square")
                .font(Typography.caption())
                .fontWeight(.medium)
                .padding(.horizontal, Spacing.sm)
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
            .padding(.horizontal, Spacing.sm)
            .padding(.vertical, 3)
            .background(Color.statusWarning.opacity(0.14))
            .foregroundStyle(Color.statusWarning)
            .clipShape(Capsule())
            .help("\(wave.effectiveOpenPRCount) open PRs in this stack")
    }

    // MARK: - Wave Config Summary (read-only, shown when running)

    private var waveConfigSummary: some View {
        HStack(spacing: Spacing.lg) {
            configLabel("folder", wave.areaDisplay)
            configLabel("target", wave.directionDisplay)
            configLabel("arrow.triangle.branch", wave.flow)
        }
        .padding(Spacing.md)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }

    @ViewBuilder
    private func configLabel(_ icon: String, _ text: String) -> some View {
        if !text.isEmpty {
            Label(text, systemImage: icon)
                .font(Typography.caption())
                .foregroundStyle(.secondary)
        }
    }

    // MARK: - Quick Actions Bar

    private var quickActionsBar: some View {
        HStack(spacing: Spacing.md) {
            if let path = wave.worktreePath {
                let hasWorktree = FileManager.default.fileExists(atPath: path)

                Button { openInTerminal(path: path) } label: {
                    Label(terminalApp.displayName, systemImage: "terminal")
                }
                .buttonStyle(GhostButtonStyle())
                .disabled(!hasWorktree)
                .help(hasWorktree ? "Open in \(terminalApp.displayName)" : "Worktree path no longer exists")

                Button { openInIDE(path: path) } label: {
                    Label(ideApp.displayName, systemImage: "curlybraces")
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
            VStack(alignment: .leading, spacing: Spacing.lg) {
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
                        .padding(.horizontal, Spacing.sm)
                        .padding(.vertical, Spacing.xs)
                        .background(palette.background)
                        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
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
            .padding(Spacing.lg)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        }
    }

    private var failedRunDetail: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            // Error message from the run
            if let error = wave.activeRun?.error {
                HStack(alignment: .top, spacing: Spacing.sm) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(Color.statusError)
                        .font(Typography.caption())
                    Text(error)
                        .font(Typography.caption())
                        .foregroundStyle(.primary)
                        .textSelection(.enabled)
                }
                .padding(Spacing.md)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.statusError.opacity(0.08))
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
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
            Button { retryWave() } label: {
                Label("Retry", systemImage: "arrow.counterclockwise")
            }
            .buttonStyle(DarkButtonStyle())
        }
    }

    private var liveOutputSection: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(wave.status == .running ? "Live Output" : "Output")
                .font(Typography.caption(10))
                .foregroundStyle(.tertiary)
                .textCase(.uppercase)
                .tracking(0.5)

            let output = outputBuffer.output(for: wave.id)
            if output.isEmpty {
                Text("Waiting for output…")
                    .font(Typography.caption())
                    .foregroundStyle(.tertiary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(Spacing.md)
                    .background(palette.background)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
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
                .font(Typography.caption(10))
                .foregroundStyle(.tertiary)
                .textCase(.uppercase)
                .tracking(0.5)

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
                    .padding(.vertical, Spacing.xs)
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
                .font(Typography.caption(10))
                .foregroundStyle(.tertiary)
                .textCase(.uppercase)
                .tracking(0.5)

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
                Button { terminalLauncher.openURL(prURL) } label: {
                    Label("View PR", systemImage: "arrow.up.right.square")
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
            Button { nextWave() } label: {
                Label("Next", systemImage: "arrow.forward.circle")
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

    private func perform(_ label: String, _ action: @escaping () async throws -> Void) {
        Task {
            do {
                try await action()
            } catch {
                await MainActor.run {
                    actionError = "Failed to \(label): \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }

    private func landWave() { perform("land") { try await repoState.landWave(wave) } }
    private func nextWave() { perform("start next") { try await repoState.nextWave(wave) } }
    private func stopWave() { perform("stop wave") { try await repoState.stopWave(wave) } }

    private func retryWave() {
        perform("retry wave") {
            outputBuffer.clearOutput(for: wave.id)
            try await repoState.runWave(wave: wave)
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

    private func combinePRs() async throws -> CombinePRsResult {
        try await repoState.combinePRs(wave.id)
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
