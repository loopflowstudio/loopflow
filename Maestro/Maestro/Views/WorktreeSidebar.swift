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
    @State private var showingDiagnostics = false
    @State private var showingLandConfirmation = false
    @State private var loopToLand: Loop?

    private let terminalLauncher = TerminalLauncher()

    private var featureWorktrees: [Worktree] {
        appState.worktrees.filter { $0.branch != "main" }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header

            if featureWorktrees.isEmpty {
                emptyState
            } else {
                worktreeList
            }

            if Flags.beta {
                pipelinesSection
            }

            loopsSection
        }
        .background(Color.loopflowBurgundy)
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
        .sheet(isPresented: $showingDiagnostics) {
            DiagnosticsView()
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
            Text("Workspaces")
                .font(.caption)
                .fontWeight(.medium)
                .foregroundStyle(.white.opacity(0.7))
                .help("Each workspace is an isolated folder where AI works without affecting your main code")

            Spacer()

            // lfd connection indicator
            Circle()
                .fill(appState.lfdConnected ? Color.green : Color.white.opacity(0.3))
                .frame(width: 6, height: 6)
                .help(appState.lfdConnected ? "Connected to lfd daemon" : "lfd daemon not connected (using file watcher)")

            Button {
                showingNewWorktreeSheet = true
            } label: {
                Image(systemName: "plus")
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.7))
            }
            .buttonStyle(.plain)
            .help("Create a new workspace")

            Button {
                showingDiagnostics = true
            } label: {
                Image(systemName: "doc.text.magnifyingglass")
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.7))
            }
            .buttonStyle(.plain)
            .help("Open diagnostics")
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    private var emptyState: some View {
        VStack(spacing: 0) {
            // Optical centering: 40% space above, 60% below
            Spacer()
                .frame(maxHeight: .infinity)

            VStack(spacing: 12) {
                Image(systemName: "square.stack.3d.up")
                    .font(.system(size: 28))
                    .foregroundStyle(.white.opacity(0.3))

                VStack(spacing: 4) {
                    Text("No workspaces yet")
                        .fontWeight(.medium)
                        .foregroundStyle(.white.opacity(0.7))
                        .accessibilityIdentifier("worktree-empty-title")
                    Text("Create a workspace to let AI work on a feature without affecting your main code.")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.5))
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 16)
                        .accessibilityIdentifier("worktree-empty-description")
                }

                Button {
                    showingNewWorktreeSheet = true
                } label: {
                    Label("Create Workspace", systemImage: "plus")
                        .font(.caption)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
                .accessibilityIdentifier("worktree-empty-create")
            }

            // More space below for optical centering
            Spacer()
                .frame(maxHeight: .infinity)
            Spacer()
                .frame(maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity)
        .padding()
    }

    private var worktreeList: some View {
        ScrollView {
            LazyVStack(spacing: 4) {
                ForEach(featureWorktrees) { worktree in
                    WorktreeRow(
                        worktree: worktree,
                        isSelected: appState.selectedWorktree?.id == worktree.id,
                        terminalName: terminalDisplayName,
                        ideName: ideDisplayName,
                        otherWorktrees: featureWorktrees.filter { $0.id != worktree.id },
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
                    .foregroundStyle(.white.opacity(0.7))

                Spacer()

                Button {
                    showingNewPipelineSheet = true
                } label: {
                    Image(systemName: "plus")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.7))
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
                        .foregroundStyle(.white.opacity(0.5))
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

    private var loopsSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("Loops")
                    .font(.caption)
                    .fontWeight(.medium)
                    .foregroundStyle(.white.opacity(0.7))

                Spacer()

                // Connection indicator
                if appState.lfdConnected {
                    Circle()
                        .fill(Color.green)
                        .frame(width: 6, height: 6)
                        .help("Connected to lfd")
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .padding(.top, 8)

            if !appState.lfdConnected {
                // Disconnected state - show affordance, not status
                VStack(spacing: 8) {
                    Button {
                        Task {
                            do {
                                try await appState.connectLfd()
                            } catch {
                                actionError = "Failed to connect lfd: \(error.localizedDescription)"
                                showingActionError = true
                            }
                        }
                    } label: {
                        Label("Connect lfd", systemImage: "link")
                            .font(.caption)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                }
                .frame(maxWidth: .infinity)
                .padding(.bottom, 12)
            } else if appState.loops.isEmpty {
                VStack(spacing: 4) {
                    Text("No loops")
                        .foregroundStyle(.white.opacity(0.5))
                        .font(.caption)
                }
                .frame(maxWidth: .infinity)
                .padding(.bottom, 12)
            } else {
                LazyVStack(spacing: 4) {
                    ForEach(appState.loops) { loop in
                        LoopRow(
                            loop: loop,
                            isSelected: appState.selectedLoop?.id == loop.id,
                            liveOutput: appState.liveOutput(for: loop),
                            hasLandableWork: loop.commitsAhead > 0,
                            onSelect: {
                                appState.selectedLoop = loop
                            },
                            onLand: {
                                loopToLand = loop
                                showingLandConfirmation = true
                            }
                        )
                    }
                }
                .padding(.horizontal, 8)
            }
        }
        .confirmationDialog(
            "Land Loop",
            isPresented: $showingLandConfirmation,
            presenting: loopToLand
        ) { loop in
            Button("Land") {
                Task {
                    do {
                        try await appState.squashLandLoop(loop)
                    } catch {
                        actionError = "Failed to land: \(error.localizedDescription)"
                        showingActionError = true
                    }
                }
            }
        } message: { loop in
            Text("Squash and land '\(loop.goalName)' loop commits to main?")
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
                Text(worktree.displayName)
                    .fontWeight(.medium)
                    .foregroundStyle(worktree.staleness.isStale ? .white.opacity(0.5) : .white)
                    .accessibilityIdentifier("worktree-branch")
                    .help(worktree.branch != worktree.displayName ? "Branch: \(worktree.branch)" : "")

                if worktree.staleness.isStale {
                    stalenessBadge
                }

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
                    .foregroundStyle(.white.opacity(0.3))

                Text(worktree.commitsText)
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.6))
                    .accessibilityIdentifier("worktree-commits")
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(isSelected ? Color.white.opacity(0.2) : (isHovering ? Color.white.opacity(0.1) : Color.clear))
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
                    .foregroundStyle(.white.opacity(0.7))
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
                        .foregroundStyle(.white.opacity(0.7))
                }
                .buttonStyle(.plain)
                .help("Create PR")
            }

            Button {
                onOpenTerminal()
            } label: {
                Image(systemName: "terminal")
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.7))
            }
            .buttonStyle(.plain)
            .help("Open in \(terminalName)")

            Button {
                onOpenIDE()
            } label: {
                Image(systemName: "curlybraces")
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.7))
            }
            .buttonStyle(.plain)
            .help("Open in \(ideName)")

            // Land button - only visible when PR is open
            if worktree.prState == .open {
                Button {
                    onLandPR()
                } label: {
                    Image(systemName: "airplane.arrival")
                        .font(.caption)
                        .foregroundStyle(.green)
                }
                .buttonStyle(.plain)
                .help("Land PR")
            }

            // Abandon button
            Button {
                onDelete()
            } label: {
                Image(systemName: "trash")
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.5))
            }
            .buttonStyle(.plain)
            .help("Abandon worktree")
        }
    }

    private var statusBadge: some View {
        HStack(spacing: 6) {
            // CI status badge
            if let ci = worktree.ciStatus {
                Image(systemName: ci.icon)
                    .font(.system(size: 10))
                    .foregroundStyle(ci.color)
                    .help("CI: \(ci.rawValue)")
                    .accessibilityIdentifier("worktree-ci-badge")
            }

            if let task = worktree.lastCompletedTask ?? worktree.lastTask {
                stageBadge(task)
            }

            // Commit count badge
            if worktree.aheadMain > 0 {
                Text("\(worktree.aheadMain)")
                    .font(.system(size: 10, weight: .medium))
                    .foregroundStyle(.white)
                    .padding(.horizontal, 5)
                    .padding(.vertical, 1)
                    .background(Capsule().fill(.blue))
                    .accessibilityLabel("\(worktree.aheadMain) commits ahead of main")
                    .accessibilityIdentifier("worktree-ahead-badge")
            }
        }
    }

    private static let stageStyles: [String: (icon: String, color: Color)] = [
        "design": ("lightbulb", .blue),
        "implement": ("hammer", .loopflowBurgundy),
        "review": ("magnifyingglass", .orange),
        "polish": ("sparkles", .green),
    ]

    private func stageBadge(_ task: String) -> some View {
        let style = Self.stageStyles[task] ?? ("circle", .gray)
        return HStack(spacing: 3) {
            Image(systemName: style.icon)
                .font(.system(size: 8))
            Text(task)
                .font(.caption2)
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 2)
        .background(style.color.opacity(0.2))
        .foregroundStyle(style.color)
        .clipShape(Capsule())
    }

    @ViewBuilder
    private var stalenessBadge: some View {
        let (icon, color, text): (String, Color, String) = {
            switch worktree.staleness {
            case .active:
                return ("", .clear, "")
            case .merged:
                return ("checkmark.circle.fill", .green, "Merged")
            case .remoteDeleted:
                return ("xmark.circle.fill", .orange, "Remote deleted")
            case .inactive(let days):
                return ("moon.zzz.fill", .gray, "\(days)d inactive")
            }
        }()

        if worktree.staleness.isStale {
            HStack(spacing: 2) {
                Image(systemName: icon)
                    .font(.system(size: 8))
                Text(text)
                    .font(.caption2)
            }
            .foregroundStyle(color)
            .help("This worktree may be ready for cleanup")
            .accessibilityIdentifier("worktree-staleness")
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

// MARK: - Diff Views

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

struct DiffSheetView: View {
    let title: String
    let subtitle: String
    let diffContent: String?
    let isLoading: Bool
    var action: AnyView? = nil

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.headline)
                    Text(subtitle)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()

                if let action = action {
                    action
                }

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

            if isLoading {
                VStack {
                    ProgressView()
                        .scaleEffect(0.8)
                    Text("Loading...")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let content = diffContent, !content.isEmpty {
                ScrollView {
                    DiffContentView(content: content)
                        .padding()
                }
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "doc.text")
                        .font(.title2)
                        .foregroundStyle(.tertiary)
                    Text("No diff available")
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(minWidth: 600, minHeight: 400)
        .frame(idealWidth: 800, idealHeight: 600)
    }
}

struct DiffSheet: View {
    let worktree: Worktree
    let diffContent: String?
    let isLoading: Bool
    let onOpenWeb: () -> Void

    var body: some View {
        DiffSheetView(
            title: worktree.branch,
            subtitle: "Diff against main",
            diffContent: diffContent,
            isLoading: isLoading,
            action: AnyView(
                Button {
                    onOpenWeb()
                } label: {
                    Image(systemName: "safari")
                }
                .buttonStyle(.plain)
                .help("Open in GitHub")
            )
        )
    }
}

struct CompareSheet: View {
    let worktreeA: Worktree
    let worktreeB: Worktree
    let diffContent: String?
    let isLoading: Bool

    var body: some View {
        DiffSheetView(
            title: "\(worktreeA.branch) vs \(worktreeB.branch)",
            subtitle: "Comparison",
            diffContent: diffContent,
            isLoading: isLoading
        )
    }
}

#Preview {
    let state = AppState()
    return WorktreeSidebar(appState: state)
        .frame(width: 280, height: 400)
}
