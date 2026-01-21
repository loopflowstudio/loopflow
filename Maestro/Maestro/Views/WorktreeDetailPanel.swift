// Worktree detail panel showing dashboard with commits, changed files, and launcher.

import SwiftUI

struct WorktreeDetailPanel: View {
    @Bindable var appState: AppState
    let worktree: Worktree

    @Environment(\.colorScheme) private var colorScheme
    @AppStorage("commitsExpanded") private var commitsExpanded = true
    @AppStorage("filesExpanded") private var filesExpanded = true
    @AppStorage("launcherExpanded") private var launcherExpanded = false

    @State private var showingDiffSheet = false
    @State private var commits: [CommitInfo] = []
    @State private var fileStats: [FileDiffStat] = []
    @State private var selectedFileDiff: String = ""
    @State private var isLoadingCommits = false
    @State private var isLoadingFiles = false
    @State private var actionError: String?
    @State private var showingActionError = false
    @State private var showingAbandonConfirmation = false

    private let terminalLauncher = TerminalLauncher()
    private let worktreeService = WorktreeService()

    private var ideApp: IDEApp { appState.config?.ideApp ?? .cursor }
    private var terminalApp: TerminalApp { appState.config?.terminalApp ?? .warp }
    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    var body: some View {
        VStack(spacing: 0) {
            header

            Divider()

            quickActionsBar

            Divider()

            ScrollView {
                VStack(spacing: 0) {
                    commitsSection
                    filesSection
                }
            }

            Divider()

            launcherSection
        }
        .background(palette.background)
        .onAppear {
            loadCommits()
            loadFileStats()
        }
        .onChange(of: worktree.id) {
            commits = []
            fileStats = []
            loadCommits()
            loadFileStats()
        }
        .sheet(isPresented: $showingDiffSheet) {
            DiffSheet(
                worktree: worktree,
                diffContent: selectedFileDiff.isEmpty ? nil : selectedFileDiff,
                isLoading: false,
                onOpenWeb: { openPR() }
            )
        }
        .alert("Error", isPresented: $showingActionError) {
            Button("OK") { actionError = nil }
        } message: {
            Text(actionError ?? "An error occurred")
        }
        .confirmationDialog(
            "Abandon Worktree",
            isPresented: $showingAbandonConfirmation
        ) {
            Button("Abandon", role: .destructive) {
                abandonWorktree()
            }
        } message: {
            Text("Are you sure you want to abandon '\(worktree.branch)'? This will remove the worktree and its local branch.")
        }
    }

    // MARK: - Header

    private var header: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text(worktree.branch)
                    .font(.title2)
                    .fontWeight(.semibold)

                HStack(spacing: 8) {
                    if let prNumber = worktree.prNumber, let prState = worktree.prState {
                        prBadge(number: prNumber, state: prState)
                    }

                    if worktree.aheadMain > 0 {
                        Text("\(worktree.aheadMain) commits")
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

    private func prBadge(number: Int, state: PRState) -> some View {
        let color: Color = switch state {
        case .open: .green
        case .merged: .purple
        case .closed: .red
        case .draft: .orange
        }

        return Button {
            openPR()
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
            Button {
                openPR()
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "arrow.triangle.pull")
                        .font(.system(size: 14))
                    Text("PR")
                        .fontWeight(.medium)
                }
                .frame(minWidth: 70)
            }
            .buttonStyle(DarkButtonStyle())
            .help("Open or create PR")

            Button {
                openInIDE()
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "curlybraces")
                        .font(.system(size: 14))
                    Text(ideApp.displayName)
                        .fontWeight(.medium)
                }
                .frame(minWidth: 70)
            }
            .buttonStyle(DarkButtonStyle())
            .help("Open in \(ideApp.displayName)")

            Button {
                openInTerminal()
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "terminal")
                        .font(.system(size: 14))
                    Text(terminalApp.displayName)
                        .fontWeight(.medium)
                }
                .frame(minWidth: 70)
            }
            .buttonStyle(DarkButtonStyle())
            .help("Open in \(terminalApp.displayName)")

            Spacer()

            // Land button - only when PR is open
            if worktree.prState == .open {
                Button {
                    landPR()
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "airplane.arrival")
                            .font(.system(size: 14))
                        Text("Land")
                            .fontWeight(.medium)
                    }
                    .frame(minWidth: 70)
                }
                .buttonStyle(DarkButtonStyle())
                .help("Land PR (merge and clean up)")
            }

            // Abandon button
            Button {
                showingAbandonConfirmation = true
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "trash")
                        .font(.system(size: 14))
                    Text("Abandon")
                        .fontWeight(.medium)
                }
                .frame(minWidth: 70)
            }
            .buttonStyle(DarkButtonStyle())
            .help("Abandon worktree")
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 16)
        .background(palette.surface)
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
                    Image(systemName: "arrow.triangle.branch")
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
                    ForEach(commits.prefix(15)) { commit in
                        commitRow(commit)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
            }
        } label: {
            HStack {
                Image(systemName: "arrow.triangle.branch")
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
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    private func commitRow(_ commit: CommitInfo) -> some View {
        HStack(spacing: 10) {
            Text(commit.shortSHA)
                .font(.system(.caption, design: .monospaced))
                .foregroundStyle(.secondary)
                .frame(width: 60, alignment: .leading)

            Text(commit.message)
                .font(.subheadline)
                .lineLimit(1)
                .truncationMode(.tail)

            Spacer()

            Text(commit.relativeTime)
                .font(.caption)
                .foregroundStyle(.tertiary)
        }
        .padding(.vertical, 6)
        .padding(.horizontal, 10)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(palette.surface)
        )
    }

    // MARK: - Files Section (GitHub-style diff summary)

    private var filesSection: some View {
        DisclosureGroup(isExpanded: $filesExpanded) {
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
                    // Summary bar
                    fileSummaryBar

                    // File list
                    ForEach(fileStats) { file in
                        fileRow(file)
                    }
                }
                .padding(.horizontal, 16)
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
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
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
                .fill(palette.surface)
        )
    }

    private func fileRow(_ file: FileDiffStat) -> some View {
        HStack(spacing: 8) {
            // File icon based on extension
            fileIcon(for: file.fileExtension)
                .frame(width: 16)

            // File path
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

            // Change stats bar (GitHub-style)
            changeBar(additions: file.additions, deletions: file.deletions)

            // Stats
            HStack(spacing: 8) {
                Text("+\(file.additions)")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundStyle(.green)

                Text("-\(file.deletions)")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundStyle(.red)
            }
            .frame(width: 70, alignment: .trailing)

            // Quick actions
            HStack(spacing: 6) {
                Button {
                    openFileInIDE(file.path)
                } label: {
                    AppIconProvider.iconImage(for: AppIdentifier(ide: ideApp), size: 14)
                }
                .buttonStyle(.borderless)
                .help("Open in \(ideApp.displayName)")

                Button {
                    openFileInTerminal(file.path)
                } label: {
                    AppIconProvider.iconImage(for: AppIdentifier(terminal: terminalApp), size: 14)
                }
                .buttonStyle(.borderless)
                .help("Open in \(terminalApp.displayName)")
            }
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 10)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(palette.surface)
        )
        .contentShape(Rectangle())
        .onTapGesture {
            loadFileDiff(file.path)
        }
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
            .background(palette.surface)

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

    private func openInIDE() {
        do {
            try terminalLauncher.openInIDE(ideApp, at: URL(fileURLWithPath: worktree.path))
        } catch {
            actionError = "Failed to open \(ideApp.displayName): \(error.localizedDescription)"
            showingActionError = true
        }
    }

    private func openPR() {
        let worktreeURL = URL(fileURLWithPath: worktree.path)

        // If draft PR exists, upgrade to ready then open
        if worktree.prState == .draft {
            Task {
                do {
                    try await worktreeService.markPRReady(in: worktreeURL)
                    let url = await worktreeService.getExistingPRURL(in: worktreeURL)
                    appState.listWorktrees()
                    if let url {
                        terminalLauncher.openURL(url)
                    }
                } catch {
                    await MainActor.run {
                        actionError = "Failed to mark PR ready: \(error.localizedDescription)"
                        showingActionError = true
                    }
                }
            }
            return
        }

        // If open/merged/closed PR exists, just open it
        if let prURL = worktree.prURL {
            terminalLauncher.openURL(prURL)
            return
        }

        // No PR exists - create one (fallback, shouldn't happen with auto-draft)
        Task {
            if let existingURL = await worktreeService.getExistingPRURL(in: worktreeURL) {
                terminalLauncher.openURL(existingURL)
                return
            }
            do {
                _ = try await worktreeService.createPR(in: worktreeURL)
                appState.listWorktrees()
                if let createdURL = await worktreeService.getExistingPRURL(in: worktreeURL) {
                    terminalLauncher.openURL(createdURL)
                }
            } catch {
                await MainActor.run {
                    actionError = "Failed to create PR: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }

    private func landPR() {
        Task {
            do {
                try await appState.landPR(for: worktree)
            } catch {
                await MainActor.run {
                    actionError = "Failed to land PR: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }

    private func abandonWorktree() {
        Task {
            do {
                try await appState.deleteWorktree(worktree)
            } catch {
                await MainActor.run {
                    actionError = "Failed to abandon worktree: \(error.localizedDescription)"
                    showingActionError = true
                }
            }
        }
    }

    private func openFileInIDE(_ path: String) {
        let fullPath = URL(fileURLWithPath: worktree.path).appendingPathComponent(path)
        do {
            try terminalLauncher.openInIDE(ideApp, at: fullPath)
        } catch {
            actionError = "Failed to open file in \(ideApp.displayName): \(error.localizedDescription)"
            showingActionError = true
        }
    }

    private func openFileInTerminal(_ path: String) {
        let dir = URL(fileURLWithPath: worktree.path).appendingPathComponent(path).deletingLastPathComponent()
        do {
            try terminalLauncher.launchTerminal(terminalApp, at: dir)
        } catch {
            actionError = "Failed to open \(terminalApp.displayName): \(error.localizedDescription)"
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
                commits = []
            }
            isLoadingCommits = false
        }
    }

    private func loadFileStats() {
        isLoadingFiles = true
        Task {
            do {
                let worktreeURL = URL(fileURLWithPath: worktree.path)
                let base = worktree.baseBranch ?? "main"
                fileStats = try await worktreeService.getDiffStats("\(base)...HEAD", in: worktreeURL)
            } catch {
                fileStats = []
            }
            isLoadingFiles = false
        }
    }

    private func loadFileDiff(_ path: String) {
        Task {
            do {
                let worktreeURL = URL(fileURLWithPath: worktree.path)
                let base = worktree.baseBranch ?? "main"
                selectedFileDiff = try await worktreeService.getDiff("\(base)...HEAD -- \(path)", in: worktreeURL)
                showingDiffSheet = true
            } catch {
                selectedFileDiff = ""
            }
        }
    }
}

// MARK: - Collapsed Launcher

struct CollapsedLauncher: View {
    @Bindable var appState: AppState
    let worktree: Worktree

    @Environment(\.colorScheme) private var colorScheme

    @State private var taskSearchText: String = ""
    @State private var argsText: String = ""
    @State private var selectedTask: PromptCard?
    @State private var isShowingDropdown = false
    @State private var launchError: String?
    @State private var showingLaunchError = false
    @FocusState private var isTaskFieldFocused: Bool

    private let terminalLauncher = TerminalLauncher()

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

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
                    ContextChip(label: "Clipboard", isOn: $appState.includePaste, color: .accentColor)
                }
            }

            HStack(spacing: 12) {
                Spacer()

                Button {
                    run(interactive: false)
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
                    run(interactive: true)
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
        .background(palette.surface)
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
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(palette.border, lineWidth: 1)
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

    private func run(interactive: Bool) {
        let command = buildCommand(interactive: interactive)
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

// MARK: - Dark Button Style

struct DarkButtonStyle: ButtonStyle {
    @Environment(\.colorScheme) private var colorScheme

    func makeBody(configuration: Configuration) -> some View {
        let palette = LoopflowPalette.make(for: colorScheme)

        configuration.label
            .font(.subheadline)
            .foregroundStyle(Color.loopflowCream)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(configuration.isPressed ? palette.accentHover : palette.accent)
            )
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
        .frame(width: 600, height: 600)
}
