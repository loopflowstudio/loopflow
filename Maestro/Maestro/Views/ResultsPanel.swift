// Results panel showing session outcomes.

import SwiftUI

struct ResultsPanel: View {
    @Bindable var appState: AppState
    @State private var isExpanded = true
    @State private var expandedFiles: Set<Int> = []
    @State private var elapsedTime: TimeInterval = 0
    @State private var timer: Timer?
    @State private var showingDiffSheet = false
    @State private var diffContent: String?
    @State private var diffLoading = false
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var palette: LoopflowPalette {
        LoopflowPalette.make(for: colorScheme)
    }

    private let terminalLauncher = TerminalLauncher()

    var body: some View {
        VStack(spacing: 0) {
            if let result = appState.currentSessionResult {
                resultView(result)
            } else if !appState.liveOutputBySession.isEmpty && appState.showResultsLog {
                // Fallback to legacy output view if toggled
                legacyOutputView
            } else {
                // Empty state when no tasks have been run
                emptyStateView
            }
        }
        .sheet(isPresented: $showingDiffSheet) {
            if let result = appState.currentSessionResult {
                ResultsDiffSheet(
                    task: result.task,
                    diffContent: diffContent,
                    isLoading: diffLoading,
                    onDismiss: { showingDiffSheet = false }
                )
            }
        }
    }

    // MARK: - Result View

    @ViewBuilder
    private func resultView(_ result: SessionResult) -> some View {
        VStack(spacing: 0) {
            resultHeader(result)

            if isExpanded && result.status != .running {
                resultContent(result)
            }
        }
        .onAppear {
            if result.status == .running {
                startTimer(from: result.startedAt)
            }
        }
        .onDisappear {
            stopTimer()
        }
        .onChange(of: result.status) { _, newStatus in
            if newStatus != .running {
                stopTimer()
            }
        }
    }

    private func resultHeader(_ result: SessionResult) -> some View {
        HStack(spacing: 8) {
            // Status indicator
            statusIcon(result.status)

            // Task name and status
            Text(result.status == .running ? "Running \(result.task)..." : "\(result.task) \(result.status.rawValue)")
                .font(.caption)
                .fontWeight(.medium)

            Spacer()

            // Duration
            Text(formatDuration(result.status == .running ? elapsedTime : result.duration))
                .font(.caption)
                .foregroundStyle(.secondary)
                .monospacedDigit()

            // Overflow menu
            Menu {
                if result.status != .running {
                    Button {
                        appState.showResultsLog.toggle()
                    } label: {
                        Label(
                            appState.showResultsLog ? "Show Results" : "Show Log",
                            systemImage: appState.showResultsLog ? "list.bullet.rectangle" : "doc.text"
                        )
                    }

                    Button {
                        copyOutput(result)
                    } label: {
                        Label("Copy Output", systemImage: "doc.on.doc")
                    }

                    Divider()

                    Button {
                        appState.clearCompletedResults()
                    } label: {
                        Label("Clear", systemImage: "xmark")
                    }
                }

                Button {
                    withAnimation(DesignAnimation.standard(reduceMotion)) {
                        isExpanded.toggle()
                    }
                } label: {
                    Label(isExpanded ? "Collapse" : "Expand", systemImage: isExpanded ? "chevron.up" : "chevron.down")
                }
            } label: {
                Image(systemName: "ellipsis")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(width: 20, height: 20)
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(palette.surface)
    }

    private func copyOutput(_ result: SessionResult) {
        var output = ""
        if let lines = appState.liveOutputBySession[result.id] {
            output = lines.map { $0.text }.joined(separator: "\n")
        }
        if !output.isEmpty {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(output, forType: .string)
        }
    }

    @ViewBuilder
    private func statusIcon(_ status: SessionResultStatus) -> some View {
        switch status {
        case .running:
            ProgressView()
                .scaleEffect(0.5)
                .frame(width: 16, height: 16)
        case .completed:
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(.green)
                .font(.caption)
        case .error:
            Image(systemName: "xmark.circle.fill")
                .foregroundStyle(.red)
                .font(.caption)
        }
    }

    @ViewBuilder
    private func resultContent(_ result: SessionResult) -> some View {
        if appState.showResultsLog {
            // Show legacy streaming log
            if let lines = appState.liveOutputBySession[result.id] {
                logView(lines: lines)
            }
        } else {
            // Show results summary
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    // Files changed section
                    if !result.filesChanged.isEmpty {
                        filesChangedSection(result)
                    } else if result.status == .completed {
                        noChangesView
                    }

                    // Commits section
                    if !result.newCommits.isEmpty {
                        commitsSection(result.newCommits)
                    }

                    // Error state
                    if result.status == .error {
                        errorBanner
                    }

                    // Action buttons
                    actionButtons(result)
                }
                .padding(16)
            }
            .frame(maxHeight: 280)
            .background(palette.surface)
        }
    }

    // MARK: - Files Changed

    private func filesChangedSection(_ result: SessionResult) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("\(result.filesChanged.count) file\(result.filesChanged.count == 1 ? "" : "s") changed")
                    .font(.caption)
                    .fontWeight(.medium)
                    .foregroundStyle(.secondary)

                Spacer()

                Text("+\(result.totalLinesAdded) -\(result.totalLinesRemoved)")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }

            ForEach(Array(result.filesChanged.enumerated()), id: \.element.id) { index, file in
                fileRow(file, index: index, result: result)
            }
        }
    }

    private func fileRow(_ file: FileChange, index: Int, result: SessionResult) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            // File header - clickable to expand
            Button {
                withAnimation(DesignAnimation.fast(reduceMotion)) {
                    if expandedFiles.contains(index) {
                        expandedFiles.remove(index)
                    } else {
                        expandedFiles.insert(index)
                        // Load diff preview if needed
                        Task {
                            await appState.loadDiffPreview(for: result.id, fileIndex: index)
                        }
                    }
                }
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: expandedFiles.contains(index) ? "chevron.down" : "chevron.right")
                        .font(.caption2)
                        .frame(width: 10)

                    fileKindIcon(file.kind)

                    Text(file.path)
                        .font(.caption)
                        .lineLimit(1)
                        .truncationMode(.middle)

                    Spacer()

                    lineCountBadge(added: file.linesAdded, removed: file.linesRemoved)
                }
                .padding(.vertical, 4)
                .padding(.horizontal, 8)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .background(
                RoundedRectangle(cornerRadius: 4)
                    .fill(Color.primary.opacity(0.03))
            )

            // Expanded diff preview
            if expandedFiles.contains(index) {
                if let preview = file.diffPreview {
                    diffPreview(preview)
                } else {
                    HStack {
                        Spacer()
                        ProgressView()
                            .scaleEffect(0.6)
                        Text("Loading diff...")
                            .font(.caption)
                            .foregroundStyle(.tertiary)
                        Spacer()
                    }
                    .padding(.vertical, 8)
                }
            }
        }
    }

    @ViewBuilder
    private func fileKindIcon(_ kind: FileChangeKind) -> some View {
        switch kind {
        case .added:
            Image(systemName: "plus.circle.fill")
                .foregroundStyle(.green)
                .font(.caption2)
        case .modified:
            Image(systemName: "pencil.circle.fill")
                .foregroundStyle(.orange)
                .font(.caption2)
        case .deleted:
            Image(systemName: "minus.circle.fill")
                .foregroundStyle(.red)
                .font(.caption2)
        case .renamed:
            Image(systemName: "arrow.right.circle.fill")
                .foregroundStyle(.blue)
                .font(.caption2)
        }
    }

    private func lineCountBadge(added: Int, removed: Int) -> some View {
        HStack(spacing: 4) {
            if added > 0 {
                Text("+\(added)")
                    .font(.caption2)
                    .foregroundStyle(.green)
            }
            if removed > 0 {
                Text("-\(removed)")
                    .font(.caption2)
                    .foregroundStyle(.red)
            }
        }
    }

    private func diffPreview(_ content: String) -> some View {
        // Truncate to first 20 lines for preview
        let truncated = content.split(separator: "\n", omittingEmptySubsequences: false)
            .prefix(20)
            .joined(separator: "\n")

        return DiffContentView(content: truncated)
            .font(.system(.caption2, design: .monospaced))
            .padding(8)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: 4))
            .overlay(
                RoundedRectangle(cornerRadius: 4)
                    .stroke(Color.primary.opacity(0.1), lineWidth: 1)
            )
            .padding(.leading, 16)
    }

    // MARK: - Commits Section

    private func commitsSection(_ commits: [String]) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("\(commits.count) new commit\(commits.count == 1 ? "" : "s")")
                .font(.caption)
                .fontWeight(.medium)
                .foregroundStyle(.secondary)

            ForEach(commits, id: \.self) { message in
                HStack(spacing: 6) {
                    Image(systemName: "arrow.turn.down.right")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                    Text(message)
                        .font(.caption)
                        .lineLimit(2)
                }
            }
        }
    }

    // MARK: - States

    private var emptyStateView: some View {
        VStack(spacing: 12) {
            Image(systemName: "terminal.fill")
                .font(.system(size: 28))
                .foregroundStyle(.tertiary)

            VStack(spacing: 4) {
                Text("Ready to run")
                    .fontWeight(.medium)
                    .foregroundStyle(.secondary)
                Text("Results will appear here after you run a task.")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .multilineTextAlignment(.center)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 32)
        .padding(.horizontal, 16)
    }

    private var noChangesView: some View {
        VStack(spacing: 8) {
            Image(systemName: "checkmark.circle")
                .font(.title2)
                .foregroundStyle(.green)
            Text("Task completed")
                .font(.caption)
                .fontWeight(.medium)
            Text("No file changes were made")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 20)
    }

    private var errorBanner: some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(.orange)
            Text("Task ended with an error. Check the terminal for details.")
                .font(.caption)
        }
        .padding(12)
        .background(Color.orange.opacity(0.1))
        .clipShape(RoundedRectangle(cornerRadius: 6))
    }

    // MARK: - Actions

    private func actionButtons(_ result: SessionResult) -> some View {
        HStack(spacing: 12) {
            Button {
                openInTerminal(result.worktree)
            } label: {
                Label("Open Terminal", systemImage: "terminal")
                    .font(.caption)
            }
            .buttonStyle(.bordered)

            if result.hasChanges {
                Button {
                    viewFullDiff(result)
                } label: {
                    Label("View Full Diff", systemImage: "doc.text.magnifyingglass")
                        .font(.caption)
                }
                .buttonStyle(.bordered)
            }

            Spacer()
        }
    }

    private func openInTerminal(_ worktreePath: String) {
        let terminal = appState.config?.terminalApp ?? .warp
        try? terminalLauncher.launchTerminal(terminal, at: URL(fileURLWithPath: worktreePath))
    }

    private func viewFullDiff(_ result: SessionResult) {
        diffContent = nil
        diffLoading = true
        showingDiffSheet = true

        Task {
            let worktreeURL = URL(fileURLWithPath: result.worktree)
            let content = await loadFullDiff(from: result.baselineSHA, in: worktreeURL)
            await MainActor.run {
                diffContent = content
                diffLoading = false
            }
        }
    }

    private func loadFullDiff(from baselineSHA: String, in worktree: URL) async -> String? {
        guard !baselineSHA.isEmpty else { return nil }

        return await withCheckedContinuation { continuation in
            let process = Process()
            let pipe = Pipe()

            process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
            process.arguments = ["diff", "\(baselineSHA)..HEAD"]
            process.currentDirectoryURL = worktree
            process.standardOutput = pipe
            process.standardError = FileHandle.nullDevice

            do {
                try process.run()
                process.waitUntilExit()

                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                let output = String(data: data, encoding: .utf8)

                if process.terminationStatus == 0 {
                    continuation.resume(returning: output)
                } else {
                    continuation.resume(returning: nil)
                }
            } catch {
                continuation.resume(returning: nil)
            }
        }
    }

    // MARK: - Legacy Log View

    private var legacyOutputView: some View {
        VStack(spacing: 0) {
            legacyHeader

            if isExpanded, let sessionId = appState.activeSessionIds.first ?? appState.liveOutputBySession.keys.first,
               let lines = appState.liveOutputBySession[sessionId] {
                logView(lines: lines)
            }
        }
    }

    private var legacyHeader: some View {
        HStack {
            if !appState.activeSessionIds.isEmpty {
                Circle()
                    .fill(.green)
                    .frame(width: 8, height: 8)
                Text("\(appState.activeSessionIds.count) running")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                Circle()
                    .fill(.gray)
                    .frame(width: 8, height: 8)
                Text("idle")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            Button {
                appState.showResultsLog.toggle()
            } label: {
                Image(systemName: "list.bullet.rectangle")
                    .font(.caption)
            }
            .buttonStyle(.plain)
            .help("Show results view")

            Button {
                withAnimation(DesignAnimation.standard(reduceMotion)) { isExpanded.toggle() }
            } label: {
                Image(systemName: isExpanded ? "chevron.down" : "chevron.up")
                    .font(.caption)
            }
            .buttonStyle(.plain)
            .accessibleButton("Toggle results panel", hint: isExpanded ? "Collapse" : "Expand")
            .minHitTarget()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(palette.surface)
    }

    private func logView(lines: [OutputLine]) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    ForEach(lines) { line in
                        Text(line.text)
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(logLineColor(line.text))
                            .textSelection(.enabled)
                            .id(line.id)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .frame(height: 200)
            .background(palette.surface)
            .onChange(of: lines.count) { _, _ in
                if let lastLine = lines.last {
                    withAnimation(DesignAnimation.fast(reduceMotion)) { proxy.scrollTo(lastLine.id, anchor: .bottom) }
                }
            }
        }
    }

    private func logLineColor(_ text: String) -> SwiftUI.Color {
        if text.hasPrefix("→") { return .blue }
        if text.hasPrefix("✓") { return .green }
        if text.hasPrefix("✗") { return .red }
        return .primary
    }

    // MARK: - Timer

    private func startTimer(from startDate: Date) {
        elapsedTime = Date().timeIntervalSince(startDate)
        timer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [self] _ in
            Task { @MainActor in
                self.elapsedTime = Date().timeIntervalSince(startDate)
            }
        }
    }

    private func stopTimer() {
        timer?.invalidate()
        timer = nil
    }

    private func formatDuration(_ interval: TimeInterval) -> String {
        let minutes = Int(interval) / 60
        let seconds = Int(interval) % 60
        return String(format: "%d:%02d", minutes, seconds)
    }
}

// MARK: - Diff Sheet

struct ResultsDiffSheet: View {
    let task: String
    let diffContent: String?
    let isLoading: Bool
    let onDismiss: () -> Void

    var body: some View {
        DiffSheetView(
            title: "Full Diff",
            subtitle: "Changes from \(task)",
            diffContent: diffContent,
            isLoading: isLoading
        )
    }
}

#Preview {
    let state = AppState()
    return ResultsPanel(appState: state)
        .frame(width: 600)
}
