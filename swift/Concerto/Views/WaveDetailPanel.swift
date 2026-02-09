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
    @Environment(\.colorScheme) private var colorScheme

    @State private var actionError: String?
    @State private var showingActionError = false
    @State private var showingStopConfirmation = false
    @State private var editingName: String = ""
    @State private var isEditingName = false
    @State private var currentTime = Date()
    @State private var selectedTab: DetailTab = .current
    @State private var waveRuns: [WaveRun] = []
    @State private var isLoadingRuns = false
    @FocusState private var isNameFocused: Bool

    private let terminalLauncher = TerminalLauncher()
    private let waveService = LocalWaveService()
    private let elapsedTimeTimer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    private var ideApp: IDEApp { .cursor }
    private var terminalApp: TerminalApp { .warp }
    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

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
            if selectedTab == .runs {
                Task { await fetchRuns() }
            }
        }
        .onDisappear {
            outputBuffer.stopStreaming(waveId: wave.id)
        }
        .onChange(of: wave.id) { oldId, newId in
            outputBuffer.stopStreaming(waveId: oldId)
            outputBuffer.startStreaming(waveId: newId)
            if selectedTab == .runs {
                Task { await fetchRuns() }
            }
        }
        .onChange(of: selectedTab) { _, tab in
            if tab == .runs {
                Task { await fetchRuns() }
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
                    if selectedTab == .current {
                        if wave.status == .idle {
                            StepRunner(wave: wave)

                            if !wave.commits.isEmpty {
                                commitLogSection
                            }

                            if let stat = wave.diffStat {
                                diffStatSection(stat)
                            }

                            if !wave.commits.isEmpty || wave.prURL != nil {
                                opsActionsBar
                            } else if !wave.recentSteps.isEmpty {
                                Divider()
                                NextActionsBar(wave: wave)
                            }
                        } else if wave.status == .failed {
                            failedRunDetail
                            StepRunner(wave: wave)
                            if !outputBuffer.output(for: wave.id).isEmpty {
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
                        if isLoadingRuns && waveRuns.isEmpty {
                            ProgressView("Loading runs…")
                                .frame(maxWidth: .infinity, alignment: .center)
                        }

                        WaveRunsTab(
                            wave: wave,
                            runs: waveRuns,
                            onCollapse: collapsePRs,
                            onAbsorb: absorbIntoPR(_:),
                            onRefresh: refreshRunsAndWaves
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
                            .onTapGesture {
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

                    if !waveRuns.isEmpty {
                        IterationTimeline(runs: waveRuns, currentIteration: wave.iteration)
                    }
                }

                Text(wave.statusText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

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
        case .merged: .purple
        case .closed: .statusError
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

    // MARK: - Wave Config Summary (read-only, shown when running)

    private var waveConfigSummary: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.lg) {
                if !wave.areaDisplay.isEmpty {
                    Label {
                        Text(wave.areaDisplay)
                            .font(.caption)
                    } icon: {
                        Image(systemName: "folder")
                            .font(.caption2)
                    }
                    .foregroundStyle(.secondary)
                }

                if !wave.directionDisplay.isEmpty {
                    Label {
                        Text(wave.directionDisplay)
                            .font(.caption)
                    } icon: {
                        Image(systemName: "target")
                            .font(.caption2)
                    }
                    .foregroundStyle(.secondary)
                }

                Label {
                    Text(wave.flowDisplay)
                        .font(.caption)
                } icon: {
                    Image(systemName: "arrow.triangle.branch")
                        .font(.caption2)
                }
                .foregroundStyle(.secondary)
            }
        }
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
                    .font(.headline)

                // Status description with progress
                if wave.status == .running {
                    FlowProgressPills(
                        steps: wave.flowSteps ?? [wave.flow],
                        currentIndex: wave.stepIndex,
                        startedAt: wave.activeRun?.startedAt ?? wave.runStartedAt
                    )
                    .font(.subheadline)
                }

                // Activity summary (recent output line)
                if wave.status == .running,
                   let recentOutput = outputBuffer.recentOutput(for: wave.id) {
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
                        .font(.caption)
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.primary)
                        .textSelection(.enabled)
                }
                .padding(12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.statusError.opacity(0.08))
                .clipShape(RoundedRectangle(cornerRadius: 8))
            } else {
                Text("No error details available.")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }

            // Failed step context
            if let step = wave.activeRun?.currentStep {
                Text("Failed during: \(step)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            // Retry button
            Button {
                retryWave()
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "arrow.counterclockwise")
                        .font(.caption)
                    Text("Retry")
                }
            }
            .buttonStyle(DarkButtonStyle())
        }
    }

    private var liveOutputSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(wave.status == .running ? "Live Output" : "Output")
                .font(.caption)
                .fontWeight(.medium)
                .foregroundStyle(.secondary)

            let output = outputBuffer.output(for: wave.id)
            if output.isEmpty {
                Text("Waiting for output…")
                    .font(.caption)
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
                .font(.caption)
                .fontWeight(.medium)
                .foregroundStyle(.secondary)

            VStack(spacing: 0) {
                ForEach(wave.commits) { entry in
                    HStack(spacing: Spacing.sm) {
                        Text(entry.sha)
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(.secondary)
                            .frame(width: 60, alignment: .leading)

                        Text(entry.message)
                            .font(.caption)
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
                .font(.caption)
                .fontWeight(.medium)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(stat.components(separatedBy: "\n").enumerated()), id: \.offset) { _, line in
                    coloredDiffStatLine(line)
                }
            }
            .font(.system(.caption2, design: .monospaced))
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

    @State private var isLanding = false
    @State private var isNexting = false

    private var opsActionsBar: some View {
        HStack(spacing: Spacing.lg) {
            // Commit count
            HStack(spacing: Spacing.sm) {
                Image(systemName: "point.3.filled.connected.trianglepath.dotted")
                    .foregroundStyle(.secondary)
                Text("\(wave.commits.count) commit\(wave.commits.count == 1 ? "" : "s")")
                    .font(.subheadline)
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
                            .font(.caption)
                        Text("View PR")
                    }
                }
                .buttonStyle(DarkButtonStyle())
            }

            // Land
            Button {
                landWave()
            } label: {
                HStack(spacing: Spacing.xs) {
                    if isLanding {
                        ProgressView()
                            .scaleEffect(0.6)
                    } else {
                        Image(systemName: "arrow.merge")
                            .font(.caption)
                    }
                    Text("Land")
                }
            }
            .buttonStyle(.borderedProminent)
            .disabled(isLanding || isNexting)

            // Next
            Button {
                nextWave()
            } label: {
                HStack(spacing: Spacing.xs) {
                    if isNexting {
                        ProgressView()
                            .scaleEffect(0.6)
                    } else {
                        Image(systemName: "arrow.forward.circle")
                            .font(.caption)
                    }
                    Text("Next")
                }
            }
            .buttonStyle(DarkButtonStyle())
            .disabled(isLanding || isNexting)
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.md)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }

    // MARK: - Actions

    private func landWave() {
        isLanding = true
        Task {
            do {
                try await repoState.landWave(wave)
            } catch {
                await MainActor.run {
                    actionError = "Failed to land: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
            await MainActor.run {
                isLanding = false
            }
        }
    }

    private func nextWave() {
        isNexting = true
        Task {
            do {
                try await repoState.nextWave(wave)
            } catch {
                await MainActor.run {
                    actionError = "Failed to start next: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
            await MainActor.run {
                isNexting = false
            }
        }
    }

    private func launchInteractive() {
        guard let path = wave.worktreePath else { return }
        outputBuffer.launchInteractiveSession(
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

    private func fetchRuns() async {
        isLoadingRuns = true
        defer { isLoadingRuns = false }
        do {
            waveRuns = try await waveService.listWaveRuns(waveId: wave.id)
        } catch {
            waveRuns = []
        }
    }

    private func refreshRunsAndWaves() async {
        await fetchRuns()
        await repoState.refreshWaves()
    }

    private func collapsePRs() async throws -> CollapsePRsResult {
        let result = try await waveService.collapsePRs(wave.id)
        await refreshRunsAndWaves()
        return result
    }

    private func absorbIntoPR(_ prNumber: Int) async throws -> AbsorbIntoPRResult {
        let result = try await waveService.absorbIntoPR(wave.id, prNumber: prNumber)
        await refreshRunsAndWaves()
        return result
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
