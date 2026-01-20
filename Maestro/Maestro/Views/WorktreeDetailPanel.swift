// Worktree detail panel showing dashboard with quick actions, history, and launcher.

import SwiftUI

struct WorktreeDetailPanel: View {
    @Bindable var appState: AppState
    let worktree: Worktree

    @AppStorage("historyExpanded") private var historyExpanded = true
    @AppStorage("commitsExpanded") private var commitsExpanded = false
    @AppStorage("diffExpanded") private var diffExpanded = false
    @AppStorage("launcherExpanded") private var launcherExpanded = false

    @State private var showingDiffSheet = false
    @State private var commits: [CommitInfo] = []
    @State private var diff: String = ""
    @State private var isLoadingCommits = false
    @State private var isLoadingDiff = false
    @State private var actionError: String?
    @State private var showingActionError = false

    private let terminalLauncher = TerminalLauncher()
    private let worktreeService = WorktreeService()

    var body: some View {
        VStack(spacing: 0) {
            header

            Divider()

            quickActionsBar

            Divider()

            ScrollView {
                VStack(spacing: 0) {
                    historySection
                    commitsSection
                    diffSection
                }
            }

            Divider()

            launcherSection
        }
        .onChange(of: worktree.id) {
            // Reset cached data when switching worktrees
            commits = []
            diff = ""
        }
        .sheet(isPresented: $showingDiffSheet) {
            DiffSheet(
                worktree: worktree,
                diffContent: diff.isEmpty ? nil : diff,
                isLoading: isLoadingDiff,
                onOpenWeb: {
                    if let repoURL = appState.currentRepo {
                        Task {
                            if let url = try? await worktreeService.getGitHubCompareURL(
                                branch: worktree.branch,
                                in: repoURL
                            ) {
                                terminalLauncher.openURL(url)
                            }
                        }
                    }
                }
            )
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
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 8) {
                    Text(worktree.branch)
                        .font(.title2)
                        .fontWeight(.semibold)

                    statusBadge
                }

                HStack(spacing: 8) {
                    if let prNumber = worktree.prNumber, let prState = worktree.prState {
                        prBadge(number: prNumber, state: prState)
                    }

                    if worktree.aheadMain > 0 {
                        Text("\(worktree.aheadMain) ahead")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }

            Spacer()
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 16)
    }

    private var statusBadge: some View {
        HStack(spacing: 4) {
            Circle()
                .fill(statusColor)
                .frame(width: 8, height: 8)
            Text(statusText)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    private var statusColor: Color {
        if appState.activeWorktreePaths.contains(worktree.path) {
            return .green
        } else if worktree.isDirty {
            return .orange
        }
        return .gray
    }

    private var statusText: String {
        if appState.activeWorktreePaths.contains(worktree.path) {
            return "Running"
        } else if worktree.isDirty {
            return "Modified"
        }
        return "Clean"
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

    // MARK: - Quick Actions Bar

    private var quickActionsBar: some View {
        HStack(spacing: 12) {
            if worktree.prURL != nil {
                Button {
                    if let url = worktree.prURL {
                        terminalLauncher.openURL(url)
                    }
                } label: {
                    Label("PR", systemImage: "arrow.up.right.square")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .help("Jump to PR")
            }

            Button {
                openInCursor()
            } label: {
                Label("Cursor", systemImage: "cursorarrow.rays")
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            .help("Open in Cursor")

            Button {
                openInTerminal()
            } label: {
                Label("Warp", systemImage: "terminal")
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            .help("Open in Warp")

            Button {
                loadFullDiff()
            } label: {
                Label("Diff", systemImage: "doc.text.magnifyingglass")
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            .help("View diff")

            Spacer()
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 12)
    }

    // MARK: - History Section

    private var historySection: some View {
        DisclosureGroup(isExpanded: $historyExpanded) {
            if worktree.recentTasks.isEmpty {
                emptyHistoryView
            } else {
                VStack(spacing: 4) {
                    ForEach(worktree.recentTasks.prefix(10)) { session in
                        historyRow(session)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
            }
        } label: {
            HStack {
                Image(systemName: "clock")
                    .font(.caption)
                Text("History")
                    .font(.caption)
                    .fontWeight(.medium)
                Spacer()
            }
            .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
    }

    private var emptyHistoryView: some View {
        VStack(spacing: 8) {
            Text("No history yet")
                .font(.caption)
                .foregroundStyle(.tertiary)
            Text("Run a task to see it here.")
                .font(.caption2)
                .foregroundStyle(.quaternary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 16)
    }

    private func historyRow(_ session: TaskSession) -> some View {
        HStack(spacing: 8) {
            statusDot(for: session)

            Text(session.task)
                .font(.caption)
                .fontWeight(.medium)

            Spacer()

            Text(session.relativeTime)
                .font(.caption2)
                .foregroundStyle(.tertiary)

            sessionStatusBadge(session)
        }
        .padding(.vertical, 4)
        .padding(.horizontal, 8)
        .background(
            RoundedRectangle(cornerRadius: 4)
                .fill(Color.primary.opacity(0.03))
        )
    }

    private func statusDot(for session: TaskSession) -> some View {
        Circle()
            .fill(colorForTask(session.task))
            .frame(width: 8, height: 8)
    }

    private func colorForTask(_ task: String) -> Color {
        switch task.lowercased() {
        case "design": return .blue
        case "implement": return .green
        case "review": return .orange
        case "polish": return .purple
        default: return .gray
        }
    }

    private func sessionStatusBadge(_ session: TaskSession) -> some View {
        Group {
            if session.isRunning {
                Text("running")
                    .foregroundStyle(.green)
            } else if session.isCompleted {
                Text("completed")
                    .foregroundStyle(.secondary)
            } else if session.isError {
                Text("error")
                    .foregroundStyle(.red)
            } else {
                Text(session.status)
                    .foregroundStyle(.secondary)
            }
        }
        .font(.caption2)
    }

    // MARK: - Commits Section

    private var commitsSection: some View {
        DisclosureGroup(isExpanded: $commitsExpanded) {
            if isLoadingCommits {
                ProgressView()
                    .scaleEffect(0.7)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 16)
            } else if commits.isEmpty {
                Text("No commits on this branch yet.")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 16)
            } else {
                VStack(spacing: 4) {
                    ForEach(commits.prefix(10)) { commit in
                        commitRow(commit)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
            }
        } label: {
            HStack {
                Image(systemName: "arrow.triangle.branch")
                    .font(.caption)
                Text("Commits")
                    .font(.caption)
                    .fontWeight(.medium)
                Spacer()
            }
            .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .onChange(of: commitsExpanded) { _, expanded in
            if expanded && commits.isEmpty {
                loadCommits()
            }
        }
    }

    private func commitRow(_ commit: CommitInfo) -> some View {
        HStack(spacing: 8) {
            Text(commit.shortSHA)
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.blue)

            Text(commit.message)
                .font(.caption)
                .lineLimit(1)
                .truncationMode(.tail)

            Spacer()

            Text(commit.relativeTime)
                .font(.caption2)
                .foregroundStyle(.tertiary)
        }
        .padding(.vertical, 4)
        .padding(.horizontal, 8)
        .background(
            RoundedRectangle(cornerRadius: 4)
                .fill(Color.primary.opacity(0.03))
        )
    }

    // MARK: - Diff Preview Section

    private var diffSection: some View {
        DisclosureGroup(isExpanded: $diffExpanded) {
            if isLoadingDiff {
                ProgressView()
                    .scaleEffect(0.7)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 16)
            } else if diff.isEmpty {
                Text("No changes on this branch.")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 16)
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    ScrollView(.horizontal, showsIndicators: false) {
                        Text(truncatedDiff)
                            .font(.system(.caption2, design: .monospaced))
                            .foregroundStyle(.primary)
                            .textSelection(.enabled)
                    }
                    .frame(maxHeight: 200)

                    if diff.count > 2000 {
                        Button("Open Full Diff") {
                            loadFullDiff()
                        }
                        .buttonStyle(.link)
                        .font(.caption)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
            }
        } label: {
            HStack {
                Image(systemName: "plus.forwardslash.minus")
                    .font(.caption)
                Text("Diff Preview")
                    .font(.caption)
                    .fontWeight(.medium)
                Spacer()
            }
            .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .onChange(of: diffExpanded) { _, expanded in
            if expanded && diff.isEmpty {
                loadDiff()
            }
        }
    }

    private var truncatedDiff: String {
        if diff.count > 2000 {
            return String(diff.prefix(2000)) + "\n..."
        }
        return diff
    }

    // MARK: - Launcher Section

    private var launcherSection: some View {
        VStack(spacing: 0) {
            Button {
                withAnimation(.easeInOut(duration: 0.2)) {
                    launcherExpanded.toggle()
                }
            } label: {
                HStack {
                    Image(systemName: "play.fill")
                        .font(.caption)
                    Text("Launcher")
                        .font(.caption)
                        .fontWeight(.medium)
                    Spacer()
                    Image(systemName: launcherExpanded ? "chevron.down" : "chevron.up")
                        .font(.caption2)
                }
                .foregroundStyle(.secondary)
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
            }
            .buttonStyle(.plain)
            .background(.bar)

            if launcherExpanded {
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

    private func openInCursor() {
        let ide = appState.config?.ideApp ?? .cursor
        do {
            try terminalLauncher.openInIDE(ide, at: URL(fileURLWithPath: worktree.path))
        } catch {
            actionError = "Failed to open IDE: \(error.localizedDescription)"
            showingActionError = true
        }
    }

    private func loadCommits() {
        isLoadingCommits = true
        Task {
            do {
                let base = worktree.baseBranch ?? "main"
                commits = try await worktreeService.getCommits(for: worktree, since: base)
            } catch {
                // Silently fail - might be a new branch with no commits
                commits = []
            }
            isLoadingCommits = false
        }
    }

    private func loadDiff() {
        isLoadingDiff = true
        Task {
            do {
                let worktreeURL = URL(fileURLWithPath: worktree.path)
                let base = worktree.baseBranch ?? "main"
                diff = try await worktreeService.getDiff("\(base)...HEAD", in: worktreeURL)
            } catch {
                diff = ""
            }
            isLoadingDiff = false
        }
    }

    private func loadFullDiff() {
        showingDiffSheet = true
        if diff.isEmpty {
            loadDiff()
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
