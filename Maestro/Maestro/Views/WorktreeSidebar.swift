// Sidebar view showing worktrees with status and actions.

import SwiftUI

struct WorktreeSidebar: View {
    @Bindable var appState: AppState
    @State private var showingNewWorktreeSheet = false
    @State private var showingNewPipelineSheet = false
    @State private var showingDeleteConfirmation = false
    @State private var worktreeToDelete: Worktree?
    @State private var actionError: String?
    @State private var showingActionError = false
    @State private var showingDiffSheet = false
    @State private var diffWorktree: Worktree?
    @State private var diffContent: String?
    @State private var diffLoading = false
    @State private var showingCompareSheet = false
    @State private var compareWorktrees: (Worktree, Worktree)?
    @State private var compareContent: String?
    @State private var compareLoading = false

    private let terminalLauncher = TerminalLauncher()

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header

            if appState.worktrees.isEmpty {
                emptyState
            } else {
                worktreeList
            }

            if Flags.beta {
                pipelinesSection
                agentsSection
            }
        }
        .sheet(isPresented: $showingNewWorktreeSheet) {
            NewWorktreeSheet(appState: appState)
        }
        .confirmationDialog(
            "Delete Worktree",
            isPresented: $showingDeleteConfirmation,
            presenting: worktreeToDelete
        ) { worktree in
            Button("Delete", role: .destructive) {
                Task {
                    do {
                        try await appState.deleteWorktree(worktree)
                    } catch {
                        actionError = "Failed to delete worktree: \(error.localizedDescription)"
                        showingActionError = true
                    }
                }
            }
        } message: { worktree in
            Text("Are you sure you want to delete '\(worktree.branch)'? This will remove the worktree and its local branch.")
        }
        .alert("Error", isPresented: $showingActionError) {
            Button("OK") {
                actionError = nil
            }
        } message: {
            Text(actionError ?? "An error occurred")
        }
        .sheet(isPresented: $showingDiffSheet) {
            if let worktree = diffWorktree {
                DiffSheet(
                    worktree: worktree,
                    diffContent: diffContent,
                    isLoading: diffLoading,
                    onOpenWeb: {
                        if let repoURL = appState.currentRepo {
                            let service = WorktreeService()
                            Task {
                                if let url = try? await service.getGitHubCompareURL(branch: worktree.branch, in: repoURL) {
                                    terminalLauncher.openURL(url)
                                }
                            }
                        }
                    }
                )
            }
        }
        .sheet(isPresented: $showingCompareSheet) {
            if let (worktreeA, worktreeB) = compareWorktrees {
                CompareSheet(
                    worktreeA: worktreeA,
                    worktreeB: worktreeB,
                    diffContent: compareContent,
                    isLoading: compareLoading
                )
            }
        }
    }

    private func viewDiff(_ worktree: Worktree) {
        diffWorktree = worktree
        diffContent = nil
        diffLoading = true
        showingDiffSheet = true

        Task {
            guard let repoURL = appState.currentRepo else {
                diffLoading = false
                return
            }
            let service = WorktreeService()
            do {
                let diff = try await service.getDiff("main...\(worktree.branch)", in: repoURL)
                diffContent = diff
            } catch {
                diffContent = "Error loading diff: \(error.localizedDescription)"
            }
            diffLoading = false
        }
    }

    private func compareWith(_ worktreeA: Worktree, _ worktreeB: Worktree) {
        compareWorktrees = (worktreeA, worktreeB)
        compareContent = nil
        compareLoading = true
        showingCompareSheet = true

        Task {
            guard let repoURL = appState.currentRepo else {
                compareLoading = false
                return
            }
            let service = WorktreeService()
            do {
                let diff = try await service.getDiff(
                    "\(worktreeA.branch)...\(worktreeB.branch)",
                    in: repoURL
                )
                compareContent = diff
            } catch {
                compareContent = "Error loading diff: \(error.localizedDescription)"
            }
            compareLoading = false
        }
    }

    private var header: some View {
        HStack {
            Text("WORKTREES")
                .font(.caption)
                .fontWeight(.semibold)
                .foregroundStyle(.secondary)

            Spacer()

            Button {
                showingNewWorktreeSheet = true
            } label: {
                Image(systemName: "plus")
                    .font(.caption)
            }
            .buttonStyle(.plain)
            .help("New Worktree")
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    private var emptyState: some View {
        VStack(spacing: 8) {
            Text("No worktrees")
                .foregroundStyle(.secondary)
            Text("Click + to create one")
                .font(.caption)
                .foregroundStyle(.tertiary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var worktreeList: some View {
        ScrollView {
            LazyVStack(spacing: 4) {
                ForEach(appState.worktrees) { worktree in
                    WorktreeRow(
                        worktree: worktree,
                        isSelected: appState.selectedWorktree?.id == worktree.id,
                        terminalName: terminalDisplayName,
                        ideName: ideDisplayName,
                        otherWorktrees: appState.worktrees.filter { $0.id != worktree.id },
                        onSelect: {
                            appState.selectedWorktree = worktree
                        },
                        onDoubleClick: {
                            openInTerminal(worktree)
                        },
                        onOpenTerminal: {
                            openInTerminal(worktree)
                        },
                        onOpenIDE: {
                            openInIDE(worktree)
                        },
                        onOpenFinder: {
                            terminalLauncher.openInFinder(at: URL(fileURLWithPath: worktree.path))
                        },
                        onViewDiff: {
                            viewDiff(worktree)
                        },
                        onCompareWith: { other in
                            compareWith(worktree, other)
                        },
                        onCreatePR: {
                            Task {
                                do {
                                    try await appState.createPR(for: worktree)
                                } catch {
                                    actionError = error.localizedDescription
                                    showingActionError = true
                                }
                            }
                        },
                        onViewPR: {
                            if let url = worktree.prURL {
                                terminalLauncher.openURL(url)
                            }
                        },
                        onLandPR: {
                            Task {
                                do {
                                    try await appState.landPR(for: worktree)
                                } catch {
                                    actionError = error.localizedDescription
                                    showingActionError = true
                                }
                            }
                        },
                        onDelete: {
                            worktreeToDelete = worktree
                            showingDeleteConfirmation = true
                        }
                    )
                }
            }
            .padding(.horizontal, 8)
        }
    }

    private func openInTerminal(_ worktree: Worktree) {
        let terminal = appState.config?.terminalApp ?? .warp
        do {
            try terminalLauncher.launchTerminal(terminal, at: URL(fileURLWithPath: worktree.path))
        } catch {
            actionError = "Failed to open terminal: \(error.localizedDescription)"
            showingActionError = true
        }
    }

    private func openInIDE(_ worktree: Worktree) {
        let ide = appState.config?.ideApp ?? .cursor
        let workspace = appState.config?.workspace
        do {
            try terminalLauncher.openInIDE(ide, at: URL(fileURLWithPath: worktree.path), workspace: workspace)
        } catch {
            actionError = "Failed to open \(ide.displayName): \(error.localizedDescription)"
            showingActionError = true
        }
    }

    private var terminalDisplayName: String {
        appState.config?.terminalApp.displayName ?? "Warp"
    }

    private var ideDisplayName: String {
        appState.config?.ideApp.displayName ?? "Cursor"
    }

    private var pipelinesSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("PIPELINES")
                    .font(.caption)
                    .fontWeight(.semibold)
                    .foregroundStyle(.secondary)

                Spacer()

                Button {
                    showingNewPipelineSheet = true
                } label: {
                    Image(systemName: "plus")
                        .font(.caption)
                }
                .buttonStyle(.plain)
                .help("New Pipeline")
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .padding(.top, 8)

            if appState.pipelines.isEmpty {
                VStack(spacing: 4) {
                    Text("No pipelines")
                        .foregroundStyle(.secondary)
                        .font(.caption)
                }
                .frame(maxWidth: .infinity)
                .padding(.bottom, 12)
            } else {
                LazyVStack(spacing: 4) {
                    ForEach(appState.pipelines) { pipeline in
                        PipelineRow(
                            pipeline: pipeline,
                            isSelected: appState.selectedPipeline?.name == pipeline.name,
                            onSelect: {
                                appState.selectedPipeline = pipeline
                            }
                        )
                    }
                }
                .padding(.horizontal, 8)
            }
        }
        .sheet(isPresented: $showingNewPipelineSheet) {
            NewPipelineSheet(isPresented: $showingNewPipelineSheet) { name in
                appState.createPipeline(name: name)
            }
        }
    }

    private var agentsSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("AGENTS")
                    .font(.caption)
                    .fontWeight(.semibold)
                    .foregroundStyle(.secondary)

                Spacer()
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .padding(.top, 8)

            if appState.agents.isEmpty {
                VStack(spacing: 4) {
                    Text("No agents")
                        .foregroundStyle(.secondary)
                        .font(.caption)
                }
                .frame(maxWidth: .infinity)
                .padding(.bottom, 12)
            } else {
                ScrollView {
                    LazyVStack(spacing: 4) {
                        ForEach(appState.agents) { agent in
                            AgentRow(
                                agent: agent,
                                onStart: {
                                    Task {
                                        do {
                                            try await appState.startAgent(agent)
                                        } catch {
                                            actionError = error.localizedDescription
                                            showingActionError = true
                                        }
                                    }
                                },
                                onStop: {
                                    Task {
                                        do {
                                            try await appState.stopAgent(agent)
                                        } catch {
                                            actionError = error.localizedDescription
                                            showingActionError = true
                                        }
                                    }
                                }
                            )
                        }
                    }
                    .padding(.horizontal, 8)
                }
            }
        }
    }
}

struct AgentRow: View {
    let agent: Agent
    let onStart: () -> Void
    let onStop: () -> Void

    @State private var isHovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Circle()
                    .fill(agent.status.color)
                    .frame(width: 8, height: 8)

                Text(agent.name)
                    .fontWeight(.medium)

                Spacer()

                Text(agent.statusText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            HStack(spacing: 4) {
                Image(systemName: "arrow.triangle.2.circlepath")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)

                if agent.iteration > 0 {
                    Text("\(agent.iteration) iterations")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    if let lastRun = agent.lastRunText {
                        Text("•")
                            .foregroundStyle(.tertiary)
                        Text(lastRun)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                } else {
                    Text("trigger: \(agent.triggerText)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(isHovering ? Color.primary.opacity(0.05) : Color.clear)
        )
        .contentShape(Rectangle())
        .onHover { hovering in
            isHovering = hovering
        }
        .contextMenu {
            if agent.status == .running {
                Button("Stop") {
                    onStop()
                }
            } else {
                Button("Start") {
                    onStart()
                }
            }
        }
    }
}

struct WorktreeRow: View {
    let worktree: Worktree
    let isSelected: Bool
    let terminalName: String
    let ideName: String
    let otherWorktrees: [Worktree]
    let onSelect: () -> Void
    let onDoubleClick: () -> Void
    let onOpenTerminal: () -> Void
    let onOpenIDE: () -> Void
    let onOpenFinder: () -> Void
    let onViewDiff: () -> Void
    let onCompareWith: (Worktree) -> Void
    let onCreatePR: () -> Void
    let onViewPR: () -> Void
    let onLandPR: () -> Void
    let onDelete: () -> Void

    @State private var isHovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(worktree.branch)
                    .fontWeight(.medium)

                Spacer()

                if isHovering {
                    hoverActions
                } else {
                    statusBadge
                }
            }

            HStack(spacing: 4) {
                Image(systemName: "arrow.turn.down.right")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)

                Text(worktree.commitsText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(isSelected ? Color.accentColor.opacity(0.15) : (isHovering ? Color.primary.opacity(0.05) : Color.clear))
        )
        .contentShape(Rectangle())
        .onHover { hovering in
            isHovering = hovering
        }
        .onTapGesture(count: 2) {
            onDoubleClick()
        }
        .onTapGesture {
            onSelect()
        }
        .contextMenu {
            Button("Open in \(terminalName)") {
                onOpenTerminal()
            }
            Button("Open in \(ideName)") {
                onOpenIDE()
            }
            Button("Reveal in Finder") {
                onOpenFinder()
            }

            Divider()

            Button("View Diff") {
                onViewDiff()
            }

            if !otherWorktrees.isEmpty {
                Menu("Compare with...") {
                    ForEach(otherWorktrees) { other in
                        Button(other.branch) {
                            onCompareWith(other)
                        }
                    }
                }
            }

            Divider()

            if worktree.prURL != nil {
                Button("View PR") {
                    onViewPR()
                }
                if worktree.prState == .open {
                    Button("Land PR") {
                        onLandPR()
                    }
                }
            } else {
                Button("Create PR") {
                    onCreatePR()
                }
            }

            Divider()

            Button("Delete", role: .destructive) {
                onDelete()
            }
        }
    }

    private var hoverActions: some View {
        HStack(spacing: 8) {
            Button {
                onViewDiff()
            } label: {
                Image(systemName: "doc.text.magnifyingglass")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .help("View diff against main")

            // PR button: create or view based on current state
            if worktree.prURL != nil {
                Button {
                    onViewPR()
                } label: {
                    Image(systemName: "arrow.up.right.square")
                        .font(.caption)
                        .foregroundStyle(.green)
                }
                .buttonStyle(.plain)
                .help("View PR #\(worktree.prNumber ?? 0)")
            } else {
                Button {
                    onCreatePR()
                } label: {
                    Image(systemName: "plus.rectangle")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .help("Create PR")
            }

            Button {
                onOpenTerminal()
            } label: {
                Image(systemName: "terminal")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .help("Open in \(terminalName)")

            Button {
                onOpenIDE()
            } label: {
                Image(systemName: "curlybraces")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .help("Open in \(ideName)")
        }
    }

    private var statusBadge: some View {
        HStack(spacing: 6) {
            if let task = worktree.lastCompletedTask ?? worktree.lastTask {
                stageBadge(task)
            }

            if worktree.isDirty {
                Circle()
                    .fill(.orange)
                    .frame(width: 6, height: 6)
            } else {
                Image(systemName: "checkmark")
                    .font(.caption2)
                    .foregroundStyle(.green)
            }
        }
    }

    private func stageBadge(_ task: String) -> some View {
        Text(task)
            .font(.caption2)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(stageColor(task).opacity(0.2))
            .foregroundStyle(stageColor(task))
            .clipShape(Capsule())
    }

    private func stageColor(_ task: String) -> Color {
        switch task {
        case "design": return .blue
        case "implement": return .purple
        case "review": return .orange
        case "polish": return .green
        default: return .gray
        }
    }
}

struct NewWorktreeSheet: View {
    @Bindable var appState: AppState
    @Environment(\.dismiss) private var dismiss

    @State private var branchName = ""
    @State private var baseBranch = "main"
    @State private var isCreating = false
    @State private var errorMessage: String?

    var body: some View {
        VStack(spacing: 20) {
            Text("New Worktree")
                .font(.headline)

            VStack(alignment: .leading, spacing: 8) {
                Text("Branch name")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                TextField("feature-name", text: $branchName)
                    .textFieldStyle(.roundedBorder)
            }

            VStack(alignment: .leading, spacing: 8) {
                Text("Base branch")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Picker("", selection: $baseBranch) {
                    Text("main").tag("main")
                    ForEach(appState.worktrees.filter { $0.branch != "main" }) { wt in
                        Text(wt.branch).tag(wt.branch)
                    }
                }
                .pickerStyle(.menu)
                .labelsHidden()
            }

            if let error = errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
            }

            HStack {
                Button("Cancel") {
                    dismiss()
                }
                .keyboardShortcut(.escape)

                Spacer()

                Button("Create") {
                    createWorktree()
                }
                .keyboardShortcut(.defaultAction)
                .disabled(branchName.isEmpty || isCreating)
            }
        }
        .padding(24)
        .frame(width: 320)
    }

    private func createWorktree() {
        isCreating = true
        errorMessage = nil

        Task {
            do {
                try await appState.createWorktree(name: branchName, baseBranch: baseBranch)
                dismiss()
            } catch {
                errorMessage = error.localizedDescription
            }
            isCreating = false
        }
    }
}

struct DiffSheet: View {
    let worktree: Worktree
    let diffContent: String?
    let isLoading: Bool
    let onOpenWeb: () -> Void

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(worktree.branch)
                        .font(.headline)
                    Text("Diff against main")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()

                Button {
                    onOpenWeb()
                } label: {
                    Image(systemName: "safari")
                }
                .buttonStyle(.plain)
                .help("Open in GitHub")

                Button {
                    dismiss()
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }
            .padding()

            Divider()

            // Content
            if isLoading {
                VStack {
                    ProgressView()
                        .scaleEffect(0.8)
                    Text("Loading diff...")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let content = diffContent {
                ScrollView {
                    DiffContentView(content: content)
                        .padding()
                }
            } else {
                Text("No diff available")
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(minWidth: 600, minHeight: 400)
        .frame(idealWidth: 800, idealHeight: 600)
    }
}

struct DiffContentView: View {
    let content: String

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(content.split(separator: "\n", omittingEmptySubsequences: false).enumerated()), id: \.offset) { _, line in
                let lineStr = String(line)
                Text(lineStr)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundStyle(colorForLine(lineStr))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(backgroundForLine(lineStr))
            }
        }
    }

    private func colorForLine(_ line: String) -> Color {
        if line.hasPrefix("+++") || line.hasPrefix("---") {
            return .secondary
        } else if line.hasPrefix("+") {
            return .green
        } else if line.hasPrefix("-") {
            return .red
        } else if line.hasPrefix("@@") {
            return .cyan
        } else if line.hasPrefix("diff ") {
            return .blue
        }
        return .primary
    }

    private func backgroundForLine(_ line: String) -> Color {
        if line.hasPrefix("+") && !line.hasPrefix("+++") {
            return .green.opacity(0.1)
        } else if line.hasPrefix("-") && !line.hasPrefix("---") {
            return .red.opacity(0.1)
        }
        return .clear
    }
}

struct CompareSheet: View {
    let worktreeA: Worktree
    let worktreeB: Worktree
    let diffContent: String?
    let isLoading: Bool

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("\(worktreeA.branch) vs \(worktreeB.branch)")
                        .font(.headline)
                    Text("Comparison")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()

                Button {
                    dismiss()
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }
            .padding()

            Divider()

            // Content
            if isLoading {
                VStack {
                    ProgressView()
                        .scaleEffect(0.8)
                    Text("Loading comparison...")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let content = diffContent {
                ScrollView {
                    DiffContentView(content: content)
                        .padding()
                }
            } else {
                Text("No differences")
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(minWidth: 600, minHeight: 400)
        .frame(idealWidth: 800, idealHeight: 600)
    }
}

#Preview {
    let state = AppState()
    return WorktreeSidebar(appState: state)
        .frame(width: 280, height: 400)
}
