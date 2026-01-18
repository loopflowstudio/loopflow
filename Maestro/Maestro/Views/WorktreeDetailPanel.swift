// Worktree detail panel showing branch info, output streaming, and launcher.

import SwiftUI

struct WorktreeDetailPanel: View {
    @Bindable var appState: AppState
    let worktree: Worktree

    @State private var isLauncherExpanded = false
    @State private var showingPRSheet = false
    @State private var actionError: String?
    @State private var showingActionError = false

    private let terminalLauncher = TerminalLauncher()

    var body: some View {
        VStack(spacing: 0) {
            // Header
            header

            Divider()

            // Output area
            outputArea

            Divider()

            // Launcher (collapsed by default)
            launcherSection
        }
        .alert("Error", isPresented: $showingActionError) {
            Button("OK") { actionError = nil }
        } message: {
            Text(actionError ?? "An error occurred")
        }
    }

    // MARK: - Header

    private var header: some View {
        HStack(spacing: 12) {
            // Branch name
            VStack(alignment: .leading, spacing: 2) {
                Text(worktree.branch)
                    .font(.title2)
                    .fontWeight(.semibold)

                HStack(spacing: 8) {
                    if let prNumber = worktree.prNumber, let prState = worktree.prState {
                        prBadge(number: prNumber, state: prState)
                    }

                    Text(worktree.commitsText)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            Spacer()

            // Action buttons
            HStack(spacing: 8) {
                if worktree.prURL != nil {
                    if worktree.prState == .open {
                        Button("Land") {
                            landPR()
                        }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.small)
                    }
                } else if worktree.branch != "main" {
                    Button("Create PR") {
                        createPR()
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }

                Button {
                    openInTerminal()
                } label: {
                    Image(systemName: "terminal")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Open in \(appState.config?.terminalApp.displayName ?? "Terminal")")
            }
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 16)
    }

    private func prBadge(number: Int, state: PRState) -> some View {
        let color: Color = switch state {
        case .open: .green
        case .merged: .purple
        case .closed: .red
        case .draft: .orange
        }

        return Button {
            if let url = worktree.prURL {
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

    // MARK: - Output Area

    private var outputArea: some View {
        VStack(spacing: 0) {
            // Status bar
            HStack(spacing: 8) {
                if isWorktreeRunning {
                    Circle()
                        .fill(.green)
                        .frame(width: 8, height: 8)
                    Text("Running")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                } else {
                    Circle()
                        .fill(.gray)
                        .frame(width: 8, height: 8)
                    Text("Idle")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()

                if let lineCount = outputLineCount, lineCount > 0 {
                    Text("\(lineCount) lines")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }

                if hasOutput {
                    Button {
                        clearOutput()
                    } label: {
                        Image(systemName: "trash")
                            .font(.caption)
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(.secondary)
                    .help("Clear output")
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .background(.bar)

            // Output content
            if hasOutput {
                outputContent
            } else if let result = appState.currentSessionResult {
                ResultsSummaryView(appState: appState, result: result)
            } else {
                emptyOutputView
            }
        }
        .frame(maxHeight: .infinity)
    }

    private var isWorktreeRunning: Bool {
        appState.activeWorktreePaths.contains(worktree.path)
    }

    private var sessionIds: [String] {
        appState.liveOutputBySession.keys.filter { sessionId in
            // Match sessions for this worktree
            if let result = appState.sessionResults[sessionId] {
                return result.worktree == worktree.path
            }
            return false
        }.sorted()
    }

    private var hasOutput: Bool {
        !sessionIds.isEmpty && sessionIds.contains { sessionId in
            !(appState.liveOutputBySession[sessionId]?.isEmpty ?? true)
        }
    }

    private var outputLineCount: Int? {
        sessionIds.compactMap { appState.liveOutputBySession[$0]?.count }.reduce(0, +)
    }

    private var outputContent: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    ForEach(sessionIds, id: \.self) { sessionId in
                        ForEach(appState.liveOutputBySession[sessionId] ?? []) { line in
                            Text(line.text)
                                .font(.system(.caption, design: .monospaced))
                                .foregroundStyle(colorFor(line.text))
                                .textSelection(.enabled)
                                .id(line.id)
                        }
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .background(Color(.textBackgroundColor))
            .onChange(of: outputLineCount) { _, _ in
                // Auto-scroll to bottom
                if let sessionId = sessionIds.last,
                   let lastLine = appState.liveOutputBySession[sessionId]?.last {
                    withAnimation {
                        proxy.scrollTo(lastLine.id, anchor: .bottom)
                    }
                }
            }
        }
    }

    private func colorFor(_ text: String) -> Color {
        if text.hasPrefix("→") { return .blue }
        if text.hasPrefix("✓") { return .green }
        if text.hasPrefix("✗") { return .red }
        return .primary
    }

    private func clearOutput() {
        for sessionId in sessionIds {
            if !appState.activeSessionIds.contains(sessionId) {
                appState.liveOutputBySession.removeValue(forKey: sessionId)
            }
        }
    }

    private var emptyOutputView: some View {
        VStack(spacing: 12) {
            Image(systemName: "text.alignleft")
                .font(.system(size: 28))
                .foregroundStyle(.tertiary)

            VStack(spacing: 4) {
                Text("No activity yet")
                    .fontWeight(.medium)
                    .foregroundStyle(.secondary)
                Text("Run a task to see output here.")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    // MARK: - Launcher Section

    private var launcherSection: some View {
        VStack(spacing: 0) {
            // Toggle header
            Button {
                withAnimation(.easeInOut(duration: 0.2)) {
                    isLauncherExpanded.toggle()
                }
            } label: {
                HStack {
                    Image(systemName: "play.fill")
                        .font(.caption)
                    Text("Launcher")
                        .font(.caption)
                        .fontWeight(.medium)
                    Spacer()
                    Image(systemName: isLauncherExpanded ? "chevron.down" : "chevron.up")
                        .font(.caption2)
                }
                .foregroundStyle(.secondary)
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
            }
            .buttonStyle(.plain)
            .background(.bar)

            if isLauncherExpanded {
                CollapsedLauncher(appState: appState, worktree: worktree)
            }
        }
    }

    // MARK: - Actions

    private func openInTerminal() {
        let terminal = appState.config?.terminalApp ?? .warp
        do {
            try terminalLauncher.launchTerminal(terminal, at: URL(fileURLWithPath: worktree.path))
        } catch {
            actionError = "Failed to open terminal: \(error.localizedDescription)"
            showingActionError = true
        }
    }

    private func createPR() {
        Task {
            do {
                try await appState.createPR(for: worktree)
            } catch {
                actionError = error.localizedDescription
                showingActionError = true
            }
        }
    }

    private func landPR() {
        Task {
            do {
                try await appState.landPR(for: worktree)
            } catch {
                actionError = error.localizedDescription
                showingActionError = true
            }
        }
    }
}

// MARK: - Collapsed Launcher

struct CollapsedLauncher: View {
    @Bindable var appState: AppState
    let worktree: Worktree

    @State private var taskSearchText: String = ""
    @State private var argsText: String = ""
    @State private var selectedTask: PromptCard?
    @State private var selectedPipeline: PipelineDef?
    @State private var isShowingDropdown = false
    @State private var launchError: String?
    @State private var showingLaunchError = false
    @FocusState private var isTaskFieldFocused: Bool

    private let terminalLauncher = TerminalLauncher()

    private var filteredTasks: [PromptCard] {
        if taskSearchText.isEmpty {
            return appState.prompts
        }
        return appState.prompts.filter { $0.name.lowercased().contains(taskSearchText.lowercased()) }
    }

    var body: some View {
        VStack(spacing: 12) {
            HStack(spacing: 12) {
                // Task selector
                VStack(alignment: .leading, spacing: 4) {
                    Text("Task")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)

                    ZStack(alignment: .topLeading) {
                        TextField("Select task...", text: $taskSearchText)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 140)
                            .focused($isTaskFieldFocused)
                            .onChange(of: isTaskFieldFocused) { _, focused in
                                isShowingDropdown = focused
                            }
                            .onSubmit {
                                if let task = filteredTasks.first {
                                    selectTask(task)
                                }
                            }

                        if isShowingDropdown && !filteredTasks.isEmpty {
                            taskDropdown
                        }
                    }
                }

                // Args input
                VStack(alignment: .leading, spacing: 4) {
                    Text("Arguments")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)

                    TextField("What to build...", text: $argsText)
                        .textFieldStyle(.roundedBorder)
                }

                // Context toggles
                HStack(spacing: 6) {
                    ContextChip(label: "Docs", isOn: $appState.includeDocs, color: .blue)
                    ContextChip(label: "Files", isOn: $appState.includeDiffFiles, color: .teal)
                    ContextChip(label: "Diff", isOn: $appState.includeDiff, color: .green)
                    ContextChip(label: "Clipboard", isOn: $appState.includePaste, color: .purple)
                }
            }

            HStack(spacing: 12) {
                Spacer()

                Button {
                    runAuto()
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "play.fill")
                            .font(.caption)
                        Text("Run Auto")
                    }
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.return, modifiers: .command)

                Button {
                    runInteractive()
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "terminal")
                            .font(.caption)
                        Text("Run Interactive")
                    }
                }
                .buttonStyle(.bordered)
            }
        }
        .padding(16)
        .background(Color(.controlBackgroundColor))
        .alert("Couldn't Start", isPresented: $showingLaunchError) {
            Button("OK") { launchError = nil }
        } message: {
            Text(launchError ?? "Something went wrong.")
        }
    }

    private var taskDropdown: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(filteredTasks.prefix(8)) { task in
                Button {
                    selectTask(task)
                } label: {
                    HStack {
                        Text(task.displayName)
                            .fontWeight(.medium)
                        Spacer()
                        Text(task.defaultMode == .auto ? "auto" : "interactive")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .background(Color(nsColor: .controlBackgroundColor))
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(Color.primary.opacity(0.1), lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.1), radius: 4, y: 2)
        .frame(width: 200)
        .offset(y: 30)
        .zIndex(100)
    }

    private func selectTask(_ task: PromptCard) {
        selectedTask = task
        taskSearchText = task.displayName
        isShowingDropdown = false
        isTaskFieldFocused = false
    }

    private func buildCommand(interactive: Bool) -> String {
        var parts = ["lf"]

        if let task = selectedTask {
            parts.append(task.name)
        } else if !argsText.isEmpty {
            parts.append(":")
        }

        if !argsText.isEmpty {
            parts.append(shellEscape(argsText))
        }

        parts.append(interactive ? "-i" : "-a")

        if !appState.includeDocs {
            parts.append("--no-docs")
        }
        if appState.includeDiff {
            parts.append("--diff")
        }
        if !appState.includeDiffFiles {
            parts.append("--no-diff-files")
        }
        if appState.includePaste {
            parts.append("--paste")
        }

        return parts.joined(separator: " ")
    }

    private func shellEscape(_ string: String) -> String {
        let needsQuoting = string.contains { char in
            " \t\n'\"\\$`!*?[]{}()<>|&;#~".contains(char)
        }

        if needsQuoting {
            let escaped = string.replacingOccurrences(of: "'", with: "'\\''")
            return "'\(escaped)'"
        }

        return string
    }

    private func runAuto() {
        let command = buildCommand(interactive: false)
        let terminal = appState.config?.terminalApp ?? .warp

        do {
            try terminalLauncher.launchTerminal(
                terminal,
                at: URL(fileURLWithPath: worktree.path),
                command: command
            )
        } catch {
            launchError = error.localizedDescription
            showingLaunchError = true
        }
    }

    private func runInteractive() {
        let command = buildCommand(interactive: true)
        let terminal = appState.config?.terminalApp ?? .warp

        do {
            try terminalLauncher.launchTerminal(
                terminal,
                at: URL(fileURLWithPath: worktree.path),
                command: command
            )
        } catch {
            launchError = error.localizedDescription
            showingLaunchError = true
        }
    }
}

// MARK: - Results Summary View

struct ResultsSummaryView: View {
    @Bindable var appState: AppState
    let result: SessionResult

    @State private var expandedFiles: Set<Int> = []

    private let terminalLauncher = TerminalLauncher()

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                // Status header
                HStack(spacing: 8) {
                    statusIcon(result.status)

                    Text(result.status == .running ? "Running \(result.task)..." : "\(result.task) \(result.status.rawValue)")
                        .font(.caption)
                        .fontWeight(.medium)

                    Spacer()

                    Text(formatDuration(result.duration))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .monospacedDigit()
                }

                // Files changed
                if !result.filesChanged.isEmpty {
                    filesSection
                }

                // Commits
                if !result.newCommits.isEmpty {
                    commitsSection
                }

                // Actions
                if result.status != .running {
                    actionButtons
                }
            }
            .padding(16)
        }
        .background(Color(.textBackgroundColor))
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

    private var filesSection: some View {
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
                fileRow(file, index: index)
            }
        }
    }

    private func fileRow(_ file: FileChange, index: Int) -> some View {
        HStack(spacing: 6) {
            fileKindIcon(file.kind)

            Text(file.path)
                .font(.caption)
                .lineLimit(1)
                .truncationMode(.middle)

            Spacer()

            HStack(spacing: 4) {
                if file.linesAdded > 0 {
                    Text("+\(file.linesAdded)")
                        .font(.caption2)
                        .foregroundStyle(.green)
                }
                if file.linesRemoved > 0 {
                    Text("-\(file.linesRemoved)")
                        .font(.caption2)
                        .foregroundStyle(.red)
                }
            }
        }
        .padding(.vertical, 4)
        .padding(.horizontal, 8)
        .background(
            RoundedRectangle(cornerRadius: 4)
                .fill(Color.primary.opacity(0.03))
        )
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

    private var commitsSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("\(result.newCommits.count) new commit\(result.newCommits.count == 1 ? "" : "s")")
                .font(.caption)
                .fontWeight(.medium)
                .foregroundStyle(.secondary)

            ForEach(result.newCommits, id: \.self) { message in
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

    private var actionButtons: some View {
        HStack(spacing: 12) {
            Button {
                let terminal = appState.config?.terminalApp ?? .warp
                try? terminalLauncher.launchTerminal(terminal, at: URL(fileURLWithPath: result.worktree))
            } label: {
                Label("Open Terminal", systemImage: "terminal")
                    .font(.caption)
            }
            .buttonStyle(.bordered)

            Spacer()
        }
    }

    private func formatDuration(_ interval: TimeInterval) -> String {
        let minutes = Int(interval) / 60
        let seconds = Int(interval) % 60
        return String(format: "%d:%02d", minutes, seconds)
    }
}

#Preview {
    let state = AppState()
    let worktree = Worktree(
        path: "/tmp/test",
        branch: "feature-auth",
        baseBranch: "main",
        isDirty: false,
        aheadMain: 3,
        behindMain: 0,
        aheadRemote: 1,
        behindRemote: 0,
        prURL: URL(string: "https://github.com/test/test/pull/42"),
        prNumber: 42,
        prState: .open,
        hasCodeWorkspace: false,
        isRebasing: false,
        isMerging: false
    )
    return WorktreeDetailPanel(appState: state, worktree: worktree)
        .frame(width: 600, height: 500)
}
