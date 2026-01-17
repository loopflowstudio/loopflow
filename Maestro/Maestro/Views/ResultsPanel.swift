// Results panel showing session outcomes instead of streaming logs.

import SwiftUI

struct ResultsPanel: View {
    @Bindable var appState: AppState
    @State private var isExpanded = true
    @State private var expandedFiles: Set<Int> = []
    @State private var elapsedTime: TimeInterval = 0
    @State private var timer: Timer?

    private let terminalLauncher = TerminalLauncher()

    var body: some View {
        VStack(spacing: 0) {
            if let result = appState.currentSessionResult {
                resultView(result)
            } else if !appState.liveOutputBySession.isEmpty && appState.showResultsLog {
                // Fallback to legacy output view if toggled
                legacyOutputView
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

            // Toggle for log view (when not running)
            if result.status != .running {
                Button {
                    appState.showResultsLog.toggle()
                } label: {
                    Image(systemName: appState.showResultsLog ? "list.bullet.rectangle" : "doc.text")
                        .font(.caption)
                }
                .buttonStyle(.plain)
                .help(appState.showResultsLog ? "Show results" : "Show log")
            }

            // Clear button
            if result.status != .running {
                Button {
                    appState.clearCompletedResults()
                } label: {
                    Image(systemName: "xmark")
                        .font(.caption)
                }
                .buttonStyle(.plain)
                .help("Clear results")
            }

            // Expand/collapse
            Button {
                withAnimation(.easeInOut(duration: 0.2)) {
                    isExpanded.toggle()
                }
            } label: {
                Image(systemName: isExpanded ? "chevron.down" : "chevron.up")
                    .font(.caption)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(.bar)
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
            .background(Color(.textBackgroundColor))
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
                withAnimation(.easeInOut(duration: 0.15)) {
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
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(content.split(separator: "\n", omittingEmptySubsequences: false).prefix(20).enumerated()), id: \.offset) { _, line in
                let lineStr = String(line)
                Text(lineStr)
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundStyle(diffLineColor(lineStr))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 4)
                    .background(diffLineBackground(lineStr))
            }
        }
        .padding(8)
        .background(Color(.textBackgroundColor))
        .clipShape(RoundedRectangle(cornerRadius: 4))
        .overlay(
            RoundedRectangle(cornerRadius: 4)
                .stroke(Color.primary.opacity(0.1), lineWidth: 1)
        )
        .padding(.leading, 16)
    }

    private func diffLineColor(_ line: String) -> Color {
        if line.hasPrefix("+") && !line.hasPrefix("+++") { return .green }
        if line.hasPrefix("-") && !line.hasPrefix("---") { return .red }
        if line.hasPrefix("@@") { return .cyan }
        return .primary.opacity(0.7)
    }

    private func diffLineBackground(_ line: String) -> Color {
        if line.hasPrefix("+") && !line.hasPrefix("+++") { return .green.opacity(0.1) }
        if line.hasPrefix("-") && !line.hasPrefix("---") { return .red.opacity(0.1) }
        return .clear
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
        // This would open the diff sheet - for now just opens terminal
        openInTerminal(result.worktree)
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
                withAnimation { isExpanded.toggle() }
            } label: {
                Image(systemName: isExpanded ? "chevron.down" : "chevron.up")
                    .font(.caption)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(.bar)
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
            .background(Color(.textBackgroundColor))
            .onChange(of: lines.count) { _, _ in
                if let lastLine = lines.last {
                    withAnimation { proxy.scrollTo(lastLine.id, anchor: .bottom) }
                }
            }
        }
    }

    private func logLineColor(_ text: String) -> Color {
        if text.hasPrefix("→") { return .blue }
        if text.hasPrefix("✓") { return .green }
        if text.hasPrefix("✗") { return .red }
        return .primary
    }

    // MARK: - Timer

    private func startTimer(from startDate: Date) {
        elapsedTime = Date().timeIntervalSince(startDate)
        timer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { _ in
            elapsedTime = Date().timeIntervalSince(startDate)
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

#Preview {
    let state = AppState()
    return ResultsPanel(appState: state)
        .frame(width: 600)
}
