// Wave detail panel. Adapts to wave state:
// - Idle → StepRunner (run individual steps)
// - Running/waiting/etc → Progress + actions

import SwiftUI
import LoopflowCore

struct WaveDetailPanel: View {
    private enum DetailTab: String, CaseIterable {
        case current = "Current"
        case runs = "Runs"
        case chat = "Chat"
    }

    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.palette) private var palette
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.screenshotTab) private var screenshotTab
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @State private var actionError: String?
    @State private var showingActionError = false
    @State private var showingStopConfirmation = false
    @State private var editingName: String = ""
    @State private var isEditingName = false
    @State private var currentTime = Date()
    @State private var selectedTab: DetailTab = .current
    @State private var hasAppliedScreenshotTab = false
    @State private var expandedSections: Set<String> = []
    @State private var expandedDiffFiles: Set<String> = []
    @State private var fileDiffs: [String: String] = [:]
    @State private var previousCommitSHAs: Set<String> = []
    @State private var displayedCommits: [CommitEntry] = []
    @State private var highlightedCommitSHAs: Set<String> = []
    @State private var displayedDiffStat: String?
    @State private var diffHeaderPulseActive = false
    @FocusState private var isNameFocused: Bool

    private let terminalLauncher = TerminalLauncher()
    private let elapsedTimeTimer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    private var waveRuns: [WaveRun] { repoState.runStore.runs(for: wave.id) }
    private var isSelectedWave: Bool { repoState.selectedWave?.id == wave.id }
    private var waveContent: WaveContent? { wave.content }
    private var activeSessionState: SessionState? {
        if repoState.shouldShowInteractiveSession(for: wave) {
            if let sessionId = repoState.interactiveSessionId(for: wave.id) {
                return repoState.sessionState(for: wave.id, joinSessionId: sessionId)
            }
            return repoState.sessionState(for: wave.id)
        }
        return nil
    }
    private var isReviewStep: Bool {
        guard let step = wave.activeRun?.currentStep?.lowercased() else { return false }
        return step.contains("review")
    }

    private var ideApp: IDEApp { .cursor }
    private var terminalApp: TerminalApp { .warp }

    var body: some View {
        Group {
            if let sessionState = activeSessionState {
                interactiveSessionView(state: sessionState)
            } else if outputBuffer.hasActiveSession(for: wave.id),
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
            if isSelectedWave {
                startNameEdit()
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .switchToCurrentTab)) { _ in
            guard isSelectedWave else { return }
            selectedTab = .current
        }
        .onReceive(NotificationCenter.default.publisher(for: .switchToRunsTab)) { _ in
            guard isSelectedWave else { return }
            selectedTab = .runs
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
            repoState.loadWaveContent(for: wave.id)
            previousCommitSHAs = Set(wave.commits.map(\.sha))
            displayedCommits = wave.commits
            displayedDiffStat = wave.diffStat
            syncDiffHeaderPulse(for: wave.status)
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
            repoState.loadWaveContent(for: newId)
            previousCommitSHAs = Set(wave.commits.map(\.sha))
            displayedCommits = wave.commits
            highlightedCommitSHAs.removeAll()
            displayedDiffStat = wave.diffStat
            fileDiffs.removeAll()
            expandedDiffFiles.removeAll()
            syncDiffHeaderPulse(for: wave.status)
        }
        .onChange(of: wave.commits) { _, newCommits in
            applyCommitUpdate(newCommits)
        }
        .onChange(of: wave.diffStat) { _, newDiffStat in
            applyDiffStatUpdate(newDiffStat)
        }
        .onChange(of: wave.status) { _, newStatus in
            repoState.loadWaveContent(for: wave.id)
            syncDiffHeaderPulse(for: newStatus)
        }
        .onChange(of: scenePhase) { _, phase in
            guard phase == .active, isSelectedWave else { return }
            repoState.loadWaveContent(for: wave.id)
        }
    }

    // MARK: - Blended View (header + context + actions)

    private func interactiveSessionView(state: SessionState) -> some View {
        VStack(spacing: 0) {
            header
            Divider()
            WaveSessionView(state: state)
        }
    }

    private var blendedView: some View {
        VStack(spacing: 0) {
            header

            Divider()

            if selectedTab == .chat {
                WaveSessionView(state: repoState.sessionState(for: wave.id))
            } else {
                ScrollView {
                    VStack(spacing: Spacing.lg) {
                        if selectedTab == .current {
                            if wave.status == .idle || wave.status == .failed {
                                if wave.status == .idle {
                                    goalsSection
                                }

                                scratchDocSection

                                if wave.status == .failed {
                                    failedRunDetail
                                }

                                StepRunner(wave: wave)

                                if !displayedCommits.isEmpty {
                                    commitLogSection
                                }

                                if let stat = displayedDiffStat {
                                    diffStatSection(stat)
                                }

                                if !displayedCommits.isEmpty || wave.prURL != nil {
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
                                if isReviewStep {
                                    risksSection
                                }
                                runProgressSection
                            }

                            roadmapSection

                            if wave.worktreePath != nil {
                                quickActionsBar
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
                            .help("Edit wave name (E)")
                            .onTapGesture {
                                startNameEdit()
                            }
                    }

                    if wave.iteration > 0 {
                        Text("iter \(wave.iteration)")
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                            .padding(.horizontal, Spacing.sm)
                            .padding(.vertical, Spacing.xxs)
                            .background(palette.surface)
                            .clipShape(Capsule())
                    }

                    if !waveRuns.isEmpty {
                        IterationTimeline(runs: waveRuns)
                    }
                }

                if let vision = waveContent?.vision, !vision.isEmpty {
                    Text(vision)
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(2)
                        .truncationMode(.tail)
                }

                Text(wave.statusText)
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
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
                .help("Stop wave (S)")
            }

            Picker("", selection: $selectedTab) {
                Text("Current").tag(DetailTab.current)
                Text("Runs (\(waveRuns.count))").tag(DetailTab.runs)
                Text("Chat").tag(DetailTab.chat)
            }
            .pickerStyle(.segmented)
            .frame(maxWidth: 320)
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
                .padding(.vertical, Spacing.xxs)
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
            .padding(.vertical, Spacing.xxs)
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
    }

    @ViewBuilder
    private func configLabel(_ icon: String, _ text: String) -> some View {
        if !text.isEmpty {
            Label(text, systemImage: icon)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
        }
    }

    private var goalsSection: some View {
        contentCard(title: "Goals", icon: "target", text: waveContent?.goals)
    }

    private var risksSection: some View {
        contentCard(title: "Risks", icon: "exclamationmark.triangle", text: waveContent?.risks)
    }

    @ViewBuilder
    private var roadmapSection: some View {
        if let roadmapItems = waveContent?.roadmapItems, !roadmapItems.isEmpty {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text("Roadmap")
                    .font(Typography.caption())
                    .fontWeight(.medium)
                    .foregroundStyle(palette.textSecondary)

                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(roadmapItems) { item in
                        roadmapItemRow(item)
                    }
                }
                .padding(.vertical, Spacing.xs)
            }
            .padding(Spacing.md)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        }
    }

    @ViewBuilder
    private func roadmapItemRow(_ item: RoadmapItem) -> some View {
        let hasContent = item.content != nil
        let isExpanded = expandedSections.contains(item.id)

        VStack(alignment: .leading, spacing: Spacing.xs) {
            Button {
                toggleSection(item.id)
            } label: {
                HStack(alignment: .top, spacing: Spacing.sm) {
                    if hasContent {
                        Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                            .font(.system(size: 8, weight: .bold))
                            .foregroundStyle(palette.textSecondary)
                            .frame(width: 12, alignment: .center)
                            .padding(.top, 3)
                    }

                    Image(systemName: item.isShipped ? "checkmark.circle.fill" : "circle")
                        .font(Typography.caption())
                        .foregroundStyle(item.isShipped ? Color.statusSuccess : palette.textSecondary)

                    Text("\(String(format: "%02d", item.number)) · \(item.title)")
                        .font(Typography.caption())
                        .foregroundStyle(palette.text)
                        .lineLimit(1)

                    Spacer()
                }
            }
            .buttonStyle(.plain)
            .disabled(!hasContent)

            if isExpanded, let content = item.content {
                let indent = 12 + Spacing.sm
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    Text(markdownAttributedString(content))
                        .font(Typography.caption())
                        .foregroundStyle(palette.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .textSelection(.enabled)
                        .padding(.leading, indent)

                    if let filePath = item.filePath {
                        openInIDEButton(path: filePath)
                            .padding(.leading, indent)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func contentCard(title: String, icon: String, text: String?) -> some View {
        if let text, !text.isEmpty {
            let sectionKey = title.lowercased()
            let isExpanded = expandedSections.contains(sectionKey)
            let allLines = contentLines(from: text, truncate: false)
            let canExpand = allLines.count > 6

            VStack(alignment: .leading, spacing: Spacing.sm) {
                Label(title, systemImage: icon)
                    .font(Typography.caption())
                    .fontWeight(.medium)
                    .foregroundStyle(palette.textSecondary)

                if isExpanded {
                    Text(markdownAttributedString(text))
                        .font(Typography.caption())
                        .foregroundStyle(palette.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .textSelection(.enabled)
                } else {
                    VStack(alignment: .leading, spacing: Spacing.xs) {
                        ForEach(Array(contentLines(from: text).enumerated()), id: \.offset) { _, line in
                            HStack(alignment: .top, spacing: Spacing.xs) {
                                Circle()
                                    .fill(palette.textSecondary.opacity(0.6))
                                    .frame(width: 4, height: 4)
                                    .padding(.top, 6)

                                Text(line)
                                    .font(Typography.caption())
                                    .foregroundStyle(palette.text)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                        }
                    }
                }

                if canExpand {
                    Button { toggleSection(sectionKey) } label: {
                        Text(isExpanded ? "Show less" : "Show more")
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(Spacing.md)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        }
    }

    private func contentLines(from text: String, truncate: Bool = true) -> [String] {
        let lines = text
            .components(separatedBy: .newlines)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter {
                !$0.isEmpty &&
                    !$0.hasPrefix("## ") &&
                    !$0.hasPrefix("### ")
            }
            .map { line in
                if line.hasPrefix("- ") || line.hasPrefix("* ") {
                    return String(line.dropFirst(2)).trimmingCharacters(in: .whitespaces)
                }
                return line
            }

        return truncate ? Array(lines.prefix(6)) : lines
    }

    private func markdownAttributedString(_ text: String) -> AttributedString {
        (try? AttributedString(
            markdown: text,
            options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
        )) ?? AttributedString(text)
    }

    private func toggleSection(_ key: String) {
        if expandedSections.contains(key) {
            expandedSections.remove(key)
        } else {
            expandedSections.insert(key)
        }
    }

    private func openInIDEButton(path: String) -> some View {
        Button {
            openInIDE(path: path)
        } label: {
            Label("Open in \(ideApp.displayName)", systemImage: "curlybraces")
                .font(Typography.caption())
        }
        .buttonStyle(.plain)
        .foregroundStyle(palette.textSecondary)
    }

    // MARK: - Scratch Doc Section

    @ViewBuilder
    private var scratchDocSection: some View {
        if let scratchDoc = waveContent?.scratchDoc {
            let sectionKey = "design"
            let isExpanded = expandedSections.contains(sectionKey)
            let previewLines = scratchDoc
                .components(separatedBy: .newlines)
                .prefix(5)
                .joined(separator: "\n")
                .trimmingCharacters(in: .whitespacesAndNewlines)

            VStack(alignment: .leading, spacing: Spacing.sm) {
                Button {
                    toggleSection(sectionKey)
                } label: {
                    HStack(spacing: Spacing.sm) {
                        Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                            .font(.system(size: 8, weight: .bold))
                            .foregroundStyle(palette.textSecondary)

                        Label("Design", systemImage: "doc.text")
                            .font(Typography.caption())
                            .fontWeight(.medium)
                            .foregroundStyle(palette.textSecondary)

                        Spacer()
                    }
                }
                .buttonStyle(.plain)

                if isExpanded {
                    Text(markdownAttributedString(scratchDoc))
                        .font(Typography.caption())
                        .foregroundStyle(palette.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .textSelection(.enabled)
                } else {
                    Text(markdownAttributedString(previewLines))
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(5)
                }

                if let scratchDocPath = waveContent?.scratchDocPath {
                    openInIDEButton(path: scratchDocPath)
                }
            }
            .padding(Spacing.md)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        }
    }

    // MARK: - Quick Actions Bar

    private var quickActionsBar: some View {
        HStack(spacing: Spacing.md) {
            if let path = wave.worktreePath {
                let isRemote = repoState.isRemoteTarget
                let remoteHost = repoState.repoTarget?.remoteHost
                let hasWorktree = isRemote || FileManager.default.fileExists(atPath: path)

                Button { openInTerminal(path: path) } label: {
                    Label(terminalApp.displayName, systemImage: "terminal")
                }
                .buttonStyle(GhostButtonStyle())
                .disabled(!hasWorktree)
                .help(hasWorktree ? "Open in \(terminalApp.displayName) (T)" : "Worktree path no longer exists")

                Button { openInIDE(path: path) } label: {
                    Label(ideApp.displayName, systemImage: "curlybraces")
                }
                .buttonStyle(GhostButtonStyle())
                .disabled(!hasWorktree)
                .help(hasWorktree ? "Open in \(ideApp.displayName) (I)" : "Worktree path no longer exists")

                if let host = remoteHost {
                    Button { terminalLauncher.copySSHCommand(host: host, path: path) } label: {
                        Label("Copy SSH", systemImage: "doc.on.doc")
                    }
                    .buttonStyle(GhostButtonStyle())
                    .help("Copy SSH command to clipboard")
                } else if !hasWorktree {
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
                        startedAt: wave.activeRun?.startedAt ?? wave.runStartedAt,
                        stepAgents: wave.stepAgents,
                        onRestartStep: { restartStep() }
                    )
                    .font(Typography.body())
                }

                // Activity summary (recent output line)
                if wave.status == .running,
                   let recentOutput = outputBuffer.recentOutput(for: wave.id) {
                    Text(recentOutput)
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .padding(.horizontal, Spacing.sm)
                        .padding(.vertical, Spacing.xs)
                        .background(palette.background)
                        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                }

                // Commits and diff while running
                if !displayedCommits.isEmpty {
                    commitLogSection
                }

                if let stat = displayedDiffStat {
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
                        .foregroundStyle(palette.text)
                        .textSelection(.enabled)
                }
                .padding(Spacing.md)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.statusError.opacity(0.08))
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
            } else {
                Text("No error details available.")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }

            // Failed step context
            if let step = wave.activeRun?.currentStep {
                Text("Failed during: \(step)")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
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
                .font(Typography.caption())
                .fontWeight(.medium)
                .foregroundStyle(palette.textSecondary)

            let output = outputBuffer.output(for: wave.id)
            if output.isEmpty {
                Text("Waiting for output…")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
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

    private func applyCommitUpdate(_ newCommits: [CommitEntry]) {
        let decision = evaluateCommitFeedUpdate(
            previousCommitSHAs: previousCommitSHAs,
            commits: newCommits,
            isWaveRunning: wave.status == .running
        )

        if decision.shouldInvalidateDiffCache {
            invalidateExpandedDiffCache()
        }

        if decision.shouldAnimateInsertion && !reduceMotion {
            withAnimation(.easeOut(duration: 0.2)) {
                displayedCommits = newCommits
                highlightedCommitSHAs.formUnion(decision.newCommitSHAs)
            }
            let highlighted = decision.newCommitSHAs
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(300))
                withAnimation(DesignAnimation.standard(reduceMotion)) {
                    highlightedCommitSHAs.subtract(highlighted)
                }
            }
        } else {
            displayedCommits = newCommits
            if reduceMotion {
                highlightedCommitSHAs.removeAll()
            }
        }

        previousCommitSHAs = decision.currentCommitSHAs
    }

    private func applyDiffStatUpdate(_ newDiffStat: String?) {
        guard displayedDiffStat != newDiffStat else { return }
        if wave.status == .running && !reduceMotion {
            withAnimation(DesignAnimation.standard(reduceMotion)) {
                displayedDiffStat = newDiffStat
            }
        } else {
            displayedDiffStat = newDiffStat
        }
    }

    private func invalidateExpandedDiffCache() {
        fileDiffs.removeAll()
        expandedDiffFiles.removeAll()
    }

    private func syncDiffHeaderPulse(for status: WaveStatus) {
        guard status == .running else {
            diffHeaderPulseActive = false
            return
        }
        guard !reduceMotion else {
            diffHeaderPulseActive = true
            return
        }
        diffHeaderPulseActive = false
        withAnimation(.easeInOut(duration: 0.9).repeatForever(autoreverses: true)) {
            diffHeaderPulseActive = true
        }
    }

    private var commitLogSection: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Commits")
                .font(Typography.caption())
                .fontWeight(.medium)
                .foregroundStyle(palette.textSecondary)

            VStack(spacing: 0) {
                ForEach(displayedCommits) { entry in
                    HStack(spacing: Spacing.sm) {
                        Text(entry.sha)
                            .font(Typography.code(11))
                            .foregroundStyle(palette.textSecondary)
                            .frame(width: 60, alignment: .leading)

                        Text(entry.message)
                            .font(Typography.caption())
                            .lineLimit(1)
                            .truncationMode(.tail)

                        Spacer()
                    }
                    .padding(.vertical, Spacing.xs)
                    .padding(.horizontal, Spacing.sm)
                    .background(
                        highlightedCommitSHAs.contains(entry.sha) ?
                            Color.loopflowBurgundy.opacity(0.16) : Color.clear
                    )
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                    .transition(
                        reduceMotion
                            ? .identity
                            : .move(edge: .top).combined(with: .opacity)
                    )

                    if entry.sha != displayedCommits.last?.sha {
                        Divider()
                    }
                }
            }
            .animation(
                reduceMotion ? nil : .easeOut(duration: 0.2),
                value: displayedCommits.map(\.sha)
            )
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        }
    }

    private func diffStatSection(_ stat: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.xs) {
                Text("Diff")
                    .font(Typography.caption())
                    .fontWeight(.medium)
                    .foregroundStyle(palette.textSecondary)

                if wave.status == .running {
                    Circle()
                        .fill(Color.loopflowBurgundy)
                        .frame(width: 6, height: 6)
                        .opacity(reduceMotion ? 1 : (diffHeaderPulseActive ? 1 : 0.35))
                        .scaleEffect(reduceMotion ? 1 : (diffHeaderPulseActive ? 1.05 : 0.85))
                        .accessibilityHidden(true)
                }
            }

            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(stat.components(separatedBy: "\n").enumerated()), id: \.offset) { _, line in
                    diffStatFileLine(line)
                }
            }
            .id(stat)
            .transition(.opacity)
            .font(Typography.code(11))
            .textSelection(.enabled)
            .padding(Spacing.sm)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        }
        .animation(DesignAnimation.standard(reduceMotion), value: stat)
        .animation(DesignAnimation.standard(reduceMotion), value: diffHeaderPulseActive)
    }

    @ViewBuilder
    private func diffStatFileLine(_ line: String) -> some View {
        let filePath = extractFilePath(from: line)
        let isFile = filePath != nil
        let isExpanded = filePath.map { expandedDiffFiles.contains($0) } ?? false

        VStack(alignment: .leading, spacing: 0) {
            if isFile, let path = filePath {
                Button {
                    if isExpanded {
                        expandedDiffFiles.remove(path)
                    } else {
                        expandedDiffFiles.insert(path)
                        if fileDiffs[path] == nil {
                            loadFileDiff(path: path)
                        }
                    }
                } label: {
                    HStack(spacing: Spacing.xs) {
                        Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                            .font(.system(size: 7, weight: .bold))
                            .foregroundStyle(palette.textSecondary)
                            .frame(width: 10, alignment: .center)

                        coloredDiffStatLine(line)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)

                if isExpanded {
                    if let diff = fileDiffs[path], !diff.isEmpty {
                        DiffLinesView(diff: diff)
                            .padding(.leading, Spacing.xl)
                            .padding(.vertical, Spacing.xs)
                    } else {
                        ProgressView()
                            .scaleEffect(0.5)
                            .padding(.leading, Spacing.xl)
                            .padding(.vertical, Spacing.xs)
                    }
                }
            } else {
                coloredDiffStatLine(line)
            }
        }
    }

    private func extractFilePath(from line: String) -> String? {
        guard let pipeRange = line.range(of: " | ") else { return nil }
        let path = String(line[line.startIndex..<pipeRange.lowerBound])
            .trimmingCharacters(in: .whitespaces)
        return path.isEmpty ? nil : path
    }

    private func loadFileDiff(path: String) {
        Task {
            do {
                let diff = try await repoState.fileDiff(waveId: wave.id, path: path)
                await MainActor.run {
                    fileDiffs[path] = diff
                }
            } catch {
                await MainActor.run {
                    fileDiffs[path] = nil
                }
            }
        }
    }

    private func coloredDiffStatLine(_ line: String) -> Text {
        // Lines with "|" have a stat bar: " src/foo.rs | 10 ++++----"
        guard let pipeRange = line.range(of: " | ") else {
            // Summary line: color insertion/deletion counts
            return coloredDiffSummaryLine(line)
        }

        let prefix = Text(String(line[line.startIndex..<pipeRange.upperBound]))
            .foregroundColor(palette.textSecondary)

        let bar = String(line[pipeRange.upperBound...])
        var result = prefix
        for char in bar {
            switch char {
            case "+":
                result = result + Text("+").foregroundColor(Color.statusSuccess)
            case "-":
                result = result + Text("-").foregroundColor(Color.statusError)
            default:
                result = result + Text(String(char)).foregroundColor(palette.textSecondary)
            }
        }
        return result
    }

    private func coloredDiffSummaryLine(_ line: String) -> Text {
        // "2 files changed, 8 insertions(+), 7 deletions(-)"
        let parts = line.components(separatedBy: ", ")
        var result = Text("")
        for (i, part) in parts.enumerated() {
            let separator = i > 0 ? Text(", ").foregroundColor(palette.textSecondary) : Text("")
            if part.contains("insertion") {
                result = result + separator + Text(part).foregroundColor(Color.statusSuccess)
            } else if part.contains("deletion") {
                result = result + separator + Text(part).foregroundColor(Color.statusError)
            } else {
                result = result + separator + Text(part).foregroundColor(palette.textSecondary)
            }
        }
        return result
    }

    private var opsActionsBar: some View {
        HStack(spacing: Spacing.lg) {
            // Commit count
            HStack(spacing: Spacing.sm) {
                Image(systemName: "point.3.filled.connected.trianglepath.dotted")
                    .foregroundStyle(palette.textSecondary)
                Text("\(displayedCommits.count) commit\(displayedCommits.count == 1 ? "" : "s")")
                    .font(Typography.body())
                    .foregroundStyle(palette.textSecondary)
            }

            Spacer()

            // View PR
            if let prURL = wave.prURL {
                Button { terminalLauncher.openURL(prURL) } label: {
                    Label("View PR", systemImage: "arrow.up.right.square")
                }
                .buttonStyle(GhostButtonStyle())
                .help("View PR (P)")
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
            .help("Land wave (L)")

            // Next
            Button { nextWave() } label: {
                Label("Next", systemImage: "arrow.forward.circle")
            }
            .buttonStyle(DarkButtonStyle())
            .disabled(repoState.isActionInFlight(wave.id))
            .help("Start next iteration (N)")
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

    private func restartStep() { perform("restart step") { try await repoState.restartStep(wave) } }

    private func retryWave() {
        perform("retry wave") {
            outputBuffer.clearOutput(for: wave.id)
            try await repoState.runWave(wave: wave)
        }
    }

    private func openInTerminal(path: String) {
        do {
            try terminalLauncher.openTerminal(terminalApp, at: path, remoteHost: repoState.repoTarget?.remoteHost)
        } catch {
            actionError = "Failed to open terminal: \(error.localizedDescription)"
            showingActionError = true
        }
    }

    private func openInIDE(path: String) {
        do {
            try terminalLauncher.openInIDE(
                ideApp,
                at: URL(fileURLWithPath: path),
                remoteHost: repoState.repoTarget?.remoteHost
            )
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
