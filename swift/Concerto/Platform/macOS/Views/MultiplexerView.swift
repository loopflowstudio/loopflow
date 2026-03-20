// Wave multiplexer — recursive split layout with pane-scoped tmux-backed terminals.
// The workspace persists per wave and surfaces roadmap/readme/runs/launcher panes
// alongside terminal, diff, markdown, and launchpad panes.

import AppKit
import SwiftUI
import LoopflowCore

struct MultiplexerView: View {
    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState

    private var currentWave: WaveViewModel {
        repoState.waveStore.wave(for: wave.id) ?? wave
    }

    private var store: MultiplexerStore { repoState.multiplexerStore }
    private var layout: LayoutNode { store.layout(for: currentWave) }
    private var focusedPaneId: String? { store.focusedPaneId(for: currentWave.id) }

    var body: some View {
        LayoutNodeView(
            node: layout,
            wave: currentWave,
            worktreePath: currentWave.worktreePath ?? currentWave.api.localWorktree,
            focusedPaneId: focusedPaneId
        )
        .background(LoopflowPalette.dark.background)
    }
}

// MARK: - Recursive layout rendering

private struct LayoutNodeView: View {
    let node: LayoutNode
    let wave: WaveViewModel
    let worktreePath: String?
    let focusedPaneId: String?

    var body: some View {
        switch node {
        case .leaf(let pane):
            PaneContainerView(
                pane: pane,
                wave: wave,
                worktreePath: worktreePath,
                isFocused: pane.id == focusedPaneId
            )
        case .split(let axis, let first, let second, let ratio):
            SplitLayoutView(
                axis: axis,
                first: first,
                second: second,
                ratio: ratio,
                wave: wave,
                worktreePath: worktreePath,
                focusedPaneId: focusedPaneId
            )
        }
    }
}

private struct SplitLayoutView: View {
    let axis: SplitAxis
    let first: LayoutNode
    let second: LayoutNode
    let ratio: Double
    let wave: WaveViewModel
    let worktreePath: String?
    let focusedPaneId: String?

    var body: some View {
        GeometryReader { geometry in
            let totalSize = axis == .vertical ? geometry.size.height : geometry.size.width
            let firstSize = totalSize * ratio
            let secondSize = max(0, totalSize - firstSize - 1)

            if axis == .vertical {
                VStack(spacing: 0) {
                    LayoutNodeView(node: first, wave: wave, worktreePath: worktreePath, focusedPaneId: focusedPaneId)
                        .frame(height: firstSize)
                    Divider()
                    LayoutNodeView(node: second, wave: wave, worktreePath: worktreePath, focusedPaneId: focusedPaneId)
                        .frame(height: secondSize)
                }
            } else {
                HStack(spacing: 0) {
                    LayoutNodeView(node: first, wave: wave, worktreePath: worktreePath, focusedPaneId: focusedPaneId)
                        .frame(width: firstSize)
                    Divider()
                    LayoutNodeView(node: second, wave: wave, worktreePath: worktreePath, focusedPaneId: focusedPaneId)
                        .frame(width: secondSize)
                }
            }
        }
    }
}

// MARK: - Pane container

private struct PaneContainerView: View {
    let pane: PaneState
    let wave: WaveViewModel
    let worktreePath: String?
    let isFocused: Bool

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(spacing: 0) {
            PaneHeader(title: pane.type.displayName)
            paneContent
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(backgroundColor)
        .overlay {
            Rectangle()
                .stroke(isFocused ? Color.loopflowBurgundy.opacity(0.7) : palette.border.opacity(0.5), lineWidth: isFocused ? 2 : 0.5)
                .allowsHitTesting(false)
        }
        .contentShape(Rectangle())
        .onTapGesture {
            repoState.multiplexerStore.setFocusedPane(pane.id, for: wave.id)
        }
    }

    @ViewBuilder
    private var paneContent: some View {
        switch pane.type {
        case .terminal:
            TerminalPaneView(pane: pane, waveId: wave.id, worktreePath: worktreePath)
        case .markdown:
            MarkdownPaneView(pane: pane, basePath: worktreePath ?? repoState.currentRepo?.path())
        case .diff:
            DiffPaneView(wave: wave, worktreePath: worktreePath)
        case .launchpad:
            LaunchpadPaneView(pane: pane, waveId: wave.id, worktreePath: worktreePath)
        case .roadmap:
            RoadmapPaneView(waveId: wave.id)
        case .readme:
            ReadmePaneView(waveId: wave.id)
        case .runs:
            RunsPaneView(waveId: wave.id)
        case .launcher:
            LauncherPaneView(waveId: wave.id)
        }
    }

    private var backgroundColor: Color {
        pane.type == .terminal ? LoopflowPalette.dark.background : palette.surface
    }
}

private struct PaneHeader: View {
    let title: String

    @Environment(\.palette) private var palette

    var body: some View {
        HStack {
            Text(title)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
                .textCase(.uppercase)
                .tracking(0.8)
            Spacer()
        }
        .padding(.horizontal, Spacing.md)
        .padding(.vertical, Spacing.sm)
        .background(palette.surfaceMuted)
    }
}

// MARK: - Terminal pane

private struct TerminalPaneView: View {
    let pane: PaneState
    let waveId: String
    let worktreePath: String?

    @Environment(RepoState.self) private var repoState
    @ObservedObject private var ghosttyManager = GhosttyManager.shared
    @State private var tmuxReady = false
    @State private var errorMessage: String?

    var body: some View {
        Group {
            if let errorMessage {
                ContentUnavailableView(
                    "Terminal unavailable",
                    systemImage: "exclamationmark.triangle",
                    description: Text(errorMessage)
                )
            } else if let worktreePath {
                if tmuxReady {
                    GhosttyTerminalView(
                        workingDirectory: worktreePath,
                        argv: tmuxSession(for: worktreePath).attachCommand(),
                        sessionId: pane.id,
                        manager: ghosttyManager
                    )
                } else {
                    ProgressView("Starting tmux…")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .background(LoopflowPalette.dark.background)
                }
            } else {
                paneUnavailable(
                    title: "No worktree",
                    systemImage: "folder.badge.questionmark",
                    description: "Create a worktree for this wave to open a terminal pane."
                )
            }
        }
        .task(id: pane.id) {
            guard let worktreePath else { return }
            do {
                try await tmuxSession(for: worktreePath).ensureBaseSession(launchCommand: pane.config.launchCommand)
                clearLaunchCommandIfNeeded()
                tmuxReady = true
                errorMessage = nil
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    private func tmuxSession(for worktreePath: String) -> TmuxSession {
        TmuxSession(
            sessionName: pane.config.terminalSessionName ?? "lf-\(waveId)-\(pane.id)",
            worktreePath: worktreePath
        )
    }

    private func clearLaunchCommandIfNeeded() {
        guard pane.config.launchCommand != nil else { return }
        var config = pane.config
        config.launchCommand = nil
        repoState.multiplexerStore.updatePaneConfig(pane.id, config: config, for: waveId)
    }
}

// MARK: - Native panes

private struct MarkdownPaneView: View {
    let pane: PaneState
    let basePath: String?

    @Environment(\.palette) private var palette
    @State private var contents = ""
    @State private var displayPath = ""

    var body: some View {
        Group {
            if contents.isEmpty {
                ContentUnavailableView(
                    "No markdown document",
                    systemImage: "doc.text",
                    description: Text("Put a design doc in scratch/ or wave/ to open it here.")
                )
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: Spacing.md) {
                        if !displayPath.isEmpty {
                            Text(displayPath)
                                .font(Typography.caption())
                                .foregroundStyle(palette.textSecondary)
                        }
                        Text(contents)
                            .font(Typography.code())
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .padding(Spacing.lg)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .background(palette.surface)
            }
        }
        .task(id: pane.id) {
            let fileURL = resolveMarkdownURL(from: pane.config.filePath, basePath: basePath)
            displayPath = fileURL?.lastPathComponent ?? ""
            contents = fileURL.flatMap { try? String(contentsOf: $0, encoding: .utf8) } ?? ""
        }
    }
}

private struct DiffPaneView: View {
    let wave: WaveViewModel
    let worktreePath: String?

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette
    @State private var diffOutput = "Loading diff…"

    var body: some View {
        ScrollView {
            Text(diffOutput)
                .font(Typography.code())
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(Spacing.lg)
        }
        .background(palette.surface)
        .task(id: diffTaskKey) {
            do {
                if let worktreePath {
                    let output = try await runShellCommand(
                        ["git", "-C", worktreePath, "diff", "--no-color", "--stat", "main...HEAD"]
                    )
                    let trimmed = output.trimmingCharacters(in: .whitespacesAndNewlines)
                    diffOutput = trimmed.isEmpty ? "No diff against main." : output
                } else if let repoRoot = repoState.currentRepo?.path(), let branch = wave.branch, !branch.isEmpty {
                    let output = try await runShellCommand(
                        ["git", "-C", repoRoot, "diff", "--no-color", "--stat", "main...\(branch)"]
                    )
                    let trimmed = output.trimmingCharacters(in: .whitespacesAndNewlines)
                    diffOutput = trimmed.isEmpty ? "No diff against main." : output
                } else {
                    diffOutput = "No worktree or branch available for diff."
                }
            } catch {
                diffOutput = "Failed to load diff.\n\n\(error.localizedDescription)"
            }
        }
    }

    private var diffTaskKey: String {
        [wave.id, worktreePath ?? "", wave.branch ?? ""].joined(separator: "|")
    }
}

private struct RoadmapPaneView: View {
    let waveId: String

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    @State private var expandedItemIds: Set<String> = []
    @State private var actionErrors: [String: String] = [:]
    @State private var ingestingItemIds: Set<String> = []
    @State private var reprioritizingItemIds: Set<String> = []

    private var wave: WaveViewModel? {
        repoState.waveStore.wave(for: waveId)
    }

    private var roadmapItems: [RoadmapItem] {
        wave?.content?.roadmapItems ?? []
    }

    private var waveIsRunning: Bool {
        guard let wave else { return false }
        return wave.status == .running || wave.status == .waiting
    }

    var body: some View {
        Group {
            if wave?.content == nil {
                ProgressView("Loading roadmap…")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if roadmapItems.isEmpty {
                paneUnavailable(
                    title: "No roadmap items",
                    systemImage: "list.bullet.rectangle",
                    description: "Add numbered roadmap docs under wave/ to surface them here."
                )
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: Spacing.md) {
                        ForEach(roadmapItems) { item in
                            roadmapCard(item)
                        }
                    }
                    .padding(Spacing.lg)
                    .frame(maxWidth: .infinity, alignment: .topLeading)
                }
                .background(palette.surface)
            }
        }
    }

    private func roadmapCard(_ item: RoadmapItem) -> some View {
        let isExpanded = expandedItemIds.contains(item.id)
        let preview = previewText(item.content, maxLines: 3)
        let isIngesting = ingestingItemIds.contains(item.id)
        let isReprioritizing = reprioritizingItemIds.contains(item.id)
        let errorMessage = actionErrors[item.id]

        return VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(alignment: .top, spacing: Spacing.sm) {
                Image(systemName: item.isShipped ? "checkmark.circle.fill" : "circle")
                    .foregroundStyle(item.isShipped ? Color.statusSuccess : palette.textSecondary)
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    Text(item.title)
                        .font(Typography.body())
                        .fontWeight(.medium)
                        .foregroundStyle(Color.loopflowBurgundy)
                    HStack(spacing: Spacing.sm) {
                        Text(item.isShipped ? "Shipped" : "Planned")
                            .font(Typography.caption(10))
                            .foregroundStyle(item.isShipped ? Color.statusSuccess : palette.textSecondary)
                        Text("·")
                            .font(Typography.caption(10))
                            .foregroundStyle(palette.textSecondary)
                        priorityMenu(for: item, isLoading: isReprioritizing)
                    }
                }
                Spacer()
                if !item.isShipped {
                    Button {
                        ingestAndBuild(item)
                    } label: {
                        Group {
                            if isIngesting {
                                ProgressView()
                                    .controlSize(.small)
                            } else {
                                Image(systemName: "play.fill")
                            }
                        }
                        .frame(width: HitTarget.minimum, height: HitTarget.minimum)
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(waveIsRunning ? palette.textSecondary : Color.statusSuccess)
                    .disabled(waveIsRunning || isIngesting)
                    .accessibilityLabel("Ingest \(item.title) and run the wave")
                }
            }

            if let text = item.content, !text.isEmpty {
                Text(renderMarkdown(isExpanded ? text : preview))
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)

                if preview != text {
                    Button(isExpanded ? "Show less" : "Show more") {
                        toggleExpansion(for: item.id)
                    }
                    .buttonStyle(.plain)
                    .font(Typography.caption())
                    .foregroundStyle(Color.loopflowBurgundy)
                }
            }

            if let errorMessage, !errorMessage.isEmpty {
                Text(errorMessage)
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusError)
                    .fixedSize(horizontal: false, vertical: true)
            }

        }
        .padding(Spacing.md)
        .background(palette.surfaceMuted)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }

    private func priorityMenu(for item: RoadmapItem, isLoading: Bool) -> some View {
        Menu {
            ForEach(RoadmapPriority.allCases, id: \.self) { priority in
                Button {
                    updatePriority(priority, for: item)
                } label: {
                    if item.priority == priority {
                        Label(priority.displayName, systemImage: "checkmark")
                    } else {
                        Text(priority.displayName)
                    }
                }
            }
        } label: {
            HStack(spacing: Spacing.xs) {
                if isLoading {
                    ProgressView()
                        .controlSize(.small)
                }
                Text(item.priority.displayName)
                    .font(Typography.caption(10))
                Image(systemName: "chevron.up.chevron.down")
                    .font(.system(size: 9, weight: .semibold))
            }
            .foregroundStyle(palette.textSecondary)
        }
        .menuStyle(.borderlessButton)
        .disabled(isLoading)
    }

    private func toggleExpansion(for itemId: String) {
        if expandedItemIds.contains(itemId) {
            expandedItemIds.remove(itemId)
        } else {
            expandedItemIds.insert(itemId)
        }
    }

    private func ingestAndBuild(_ item: RoadmapItem) {
        guard let wave else { return }
        actionErrors[item.id] = nil
        ingestingItemIds.insert(item.id)
        Task { @MainActor in
            defer { ingestingItemIds.remove(item.id) }
            do {
                try await repoState.ingestAndBuild(wave: wave, item: item)
            } catch {
                actionErrors[item.id] = error.localizedDescription
            }
        }
    }

    private func updatePriority(_ priority: RoadmapPriority, for item: RoadmapItem) {
        guard let wave else { return }
        actionErrors[item.id] = nil
        reprioritizingItemIds.insert(item.id)
        Task { @MainActor in
            defer { reprioritizingItemIds.remove(item.id) }
            do {
                try repoState.updateRoadmapPriority(wave: wave, item: item, priority: priority)
            } catch {
                actionErrors[item.id] = error.localizedDescription
            }
        }
    }

}

private struct ReadmePaneView: View {
    let waveId: String

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    private var wave: WaveViewModel? {
        repoState.waveStore.wave(for: waveId)
    }

    private var sections: [(String, String)] {
        guard let content = wave?.content else { return [] }
        return [
            ("Vision", content.vision),
            ("Strategy", content.strategy),
            ("Goals", content.goals),
            ("Risks", content.risks),
            ("Metrics", content.metrics),
        ].compactMap { title, text in
            guard let text, !text.isEmpty else { return nil }
            return (title, text)
        }
    }

    var body: some View {
        Group {
            if wave?.content == nil {
                ProgressView("Loading README…")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if sections.isEmpty {
                paneUnavailable(
                    title: "No README content",
                    systemImage: "text.document",
                    description: "Add Vision, Strategy, Goals, Risks, or Metrics sections to the wave README."
                )
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: Spacing.lg) {
                        ForEach(Array(sections.enumerated()), id: \.offset) { _, section in
                            VStack(alignment: .leading, spacing: Spacing.sm) {
                                Text(section.0)
                                    .font(Typography.caption())
                                    .foregroundStyle(palette.textSecondary)
                                    .textCase(.uppercase)
                                Text(renderMarkdown(section.1))
                                    .font(Typography.body())
                                    .foregroundStyle(palette.text)
                                    .fixedSize(horizontal: false, vertical: true)
                                    .textSelection(.enabled)
                            }
                            .padding(Spacing.md)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(palette.surfaceMuted)
                            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                        }
                    }
                    .padding(Spacing.lg)
                    .frame(maxWidth: .infinity, alignment: .topLeading)
                }
                .background(palette.surface)
            }
        }
    }
}

private struct RunsPaneView: View {
    let waveId: String

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    private var wave: WaveViewModel? {
        repoState.waveStore.wave(for: waveId)
    }

    private var runs: [WaveRun] {
        repoState.runStore.runs(for: waveId)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.lg) {
                if let wave {
                    VStack(alignment: .leading, spacing: Spacing.xs) {
                        Text(wave.displayName)
                            .font(Typography.sectionTitle())
                        HStack(spacing: Spacing.sm) {
                            Text(wave.statusText)
                            if let branch = wave.branch, !branch.isEmpty {
                                Text("· \(branch)")
                            }
                        }
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                    }
                }

                WaveRunsTab(runs: runs) {
                    try await repoState.combinePRs(waveId)
                }
            }
            .padding(Spacing.lg)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .background(palette.surface)
    }
}

private struct LauncherPaneView: View {
    let waveId: String

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    @State private var selectedFlow = ""
    @State private var isSendingRun = false
    @State private var errorMessage: String?

    private var wave: WaveViewModel? {
        repoState.waveStore.wave(for: waveId)
    }

    private var isRunnableWorkspace: Bool {
        guard let wave else { return false }
        return wave.status == .idle || wave.status == .paused || wave.status == .failed
    }

    var body: some View {
        Group {
            if let wave {
                ScrollView {
                    VStack(alignment: .leading, spacing: Spacing.xl) {
                        VStack(alignment: .leading, spacing: Spacing.sm) {
                            Text("Run this wave")
                                .font(Typography.sectionTitle())
                                .foregroundStyle(Color.loopflowBurgundy)
                            Text("Pick a flow or step, then launch it directly from the workspace.")
                                .font(Typography.body())
                                .foregroundStyle(palette.textSecondary)
                        }

                        FlowTypeahead(
                            wave: wave,
                            initialSelection: selectedFlow.isEmpty ? wave.configuredFlow : selectedFlow,
                            style: .compact,
                            onSubmitSelection: { selection in
                                run(selection)
                            }
                        ) { selection in
                            selectedFlow = selection
                            persistFlow(selection, for: wave)
                        }

                        Button {
                            run(selectedFlow.isEmpty ? wave.configuredFlow : selectedFlow)
                        } label: {
                            HStack(spacing: Spacing.sm) {
                                if isSendingRun {
                                    ProgressView()
                                        .tint(.white)
                                } else {
                                    Image(systemName: "play.fill")
                                }
                                Text("Run")
                                    .fontWeight(.semibold)
                            }
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, Spacing.lg)
                            .background(runDisabled ? Color.statusNeutral : Color.statusSuccess)
                            .foregroundStyle(.white)
                            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                        }
                        .buttonStyle(.plain)
                        .disabled(runDisabled)
                        .opacity(runDisabled ? 0.6 : 1)

                        VStack(alignment: .leading, spacing: Spacing.sm) {
                            detailRow("Status", wave.statusText)
                            detailRow("Flow", wave.displayFlow.isEmpty ? "—" : wave.displayFlow)
                            if let branch = wave.branch, !branch.isEmpty {
                                detailRow("Branch", branch)
                            }
                            if let worktreePath = wave.worktreePath ?? wave.api.localWorktree {
                                detailRow("Worktree", worktreePath)
                            }
                        }

                        if let errorMessage {
                            Text(errorMessage)
                                .font(Typography.caption())
                                .foregroundStyle(Color.statusError)
                        }
                    }
                    .padding(Spacing.lg)
                    .frame(maxWidth: .infinity, alignment: .topLeading)
                }
                .background(palette.surface)
                .onAppear {
                    selectedFlow = wave.configuredFlow
                }
                .onChange(of: wave.flow) { _, newFlow in
                    if selectedFlow.isEmpty || selectedFlow == wave.configuredFlow {
                        selectedFlow = newFlow
                    }
                }
            } else {
                paneUnavailable(
                    title: "Wave unavailable",
                    systemImage: "play.square",
                    description: "Select a wave to launch it from this pane."
                )
            }
        }
    }

    private var runDisabled: Bool {
        let target = selectedFlow.trimmingCharacters(in: .whitespacesAndNewlines)
        return isSendingRun || !isRunnableWorkspace || (target.isEmpty && (wave?.configuredFlow.isEmpty ?? true))
    }

    private func run(_ selection: String) {
        guard let wave else { return }
        let target = selection.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? wave.configuredFlow : selection.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !target.isEmpty, !isSendingRun, isRunnableWorkspace else { return }

        isSendingRun = true
        errorMessage = nil
        Task {
            do {
                try await repoState.runWave(wave: wave, flow: target)
            } catch {
                await MainActor.run {
                    errorMessage = error.localizedDescription
                }
            }
            await MainActor.run {
                isSendingRun = false
            }
        }
    }

    private func persistFlow(_ selection: String, for wave: WaveViewModel) {
        let trimmed = selection.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed != wave.configuredFlow else { return }
        Task {
            try? await repoState.updateWave(wave, flow: trimmed)
        }
    }

    private func detailRow(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
            Text(label)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
                .frame(width: 60, alignment: .leading)
            Text(value)
                .font(Typography.caption())
                .textSelection(.enabled)
        }
    }
}

private struct LaunchpadPaneView: View {
    let pane: PaneState
    let waveId: String
    let worktreePath: String?

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette
    @State private var selectedStep = "design"
    @State private var prompt = ""

    private var wave: WaveViewModel? {
        repoState.waveStore.wave(for: waveId)
    }

    private var terminalLauncher: TerminalLauncher {
        TerminalLauncher()
    }

    private let loopflowSteps = [
        "design",
        "implement",
        "debug",
        "refine",
        "review-design",
    ]

    var body: some View {
        if let worktreePath {
            ScrollView {
                VStack(alignment: .leading, spacing: Spacing.xl) {
                    VStack(alignment: .leading, spacing: Spacing.md) {
                        Text("Start in this split")
                            .font(Typography.sectionTitle())
                            .foregroundStyle(Color.loopflowBurgundy)
                        Text("Launch a fresh shell or drop straight into an lf session.")
                            .font(Typography.body())
                            .foregroundStyle(palette.textSecondary)
                    }

                    VStack(alignment: .leading, spacing: Spacing.md) {
                        Text("Loopflow session")
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                            .textCase(.uppercase)

                        Picker("Step", selection: $selectedStep) {
                            ForEach(loopflowSteps, id: \.self) { step in
                                Text(step).tag(step)
                            }
                        }
                        .pickerStyle(.menu)

                        TextField("Prompt (optional)", text: $prompt, axis: .vertical)
                            .textFieldStyle(.roundedBorder)

                        HStack(spacing: Spacing.sm) {
                            launchButton("Launch lf session", icon: "sparkles") {
                                launchInteractiveSession(worktreePath: worktreePath)
                            }
                            launchButton("Fresh shell", icon: "terminal") {
                                launchTerminal(command: nil)
                            }
                        }
                    }

                    Divider()

                    VStack(alignment: .leading, spacing: Spacing.sm) {
                        Text("Open in…")
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                            .textCase(.uppercase)

                        launchButton("Cursor", icon: "curlybraces") {
                            try? terminalLauncher.openInIDE(.cursor, at: URL(fileURLWithPath: worktreePath), remoteHost: repoState.repoTarget?.remoteHost)
                        }
                        launchButton("Finder", icon: "folder") {
                            terminalLauncher.openInFinder(at: URL(fileURLWithPath: worktreePath))
                        }
                        if let prURL = wave?.prURL {
                            launchButton("Pull Request", icon: "arrow.triangle.pull") {
                                NSWorkspace.shared.open(prURL)
                            }
                        }
                    }

                    Spacer(minLength: 0)
                }
                .padding(Spacing.xl)
                .frame(maxWidth: .infinity, alignment: .topLeading)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(palette.surface)
        } else {
            paneUnavailable(
                title: "No worktree",
                systemImage: "folder.badge.questionmark",
                description: "Create a worktree for this wave before launching a shell from this pane."
            )
        }
    }

    private func launchButton(_ label: String, icon: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Label(label, systemImage: icon)
                .font(Typography.body())
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, Spacing.md)
                .padding(.vertical, Spacing.sm)
        }
        .buttonStyle(.plain)
        .background(palette.surfaceMuted.opacity(0.5))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }

    private func launchInteractiveSession(worktreePath: String) {
        let trimmedPrompt = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        let session = InteractiveSession(
            waveId: waveId,
            step: selectedStep,
            worktreePath: worktreePath,
            prompt: trimmedPrompt.isEmpty ? nil : trimmedPrompt
        )
        launchTerminal(command: session.command)
    }

    private func launchTerminal(command: String?) {
        _ = repoState.multiplexerStore.replacePane(
            pane.id,
            with: .terminal,
            config: PaneConfig(
                terminalSessionName: pane.config.terminalSessionName,
                launchCommand: command
            ),
            for: waveId
        )
    }
}

// MARK: - Helpers

extension PaneType {
    var displayName: String {
        switch self {
        case .terminal: "Terminal"
        case .markdown: "Markdown"
        case .diff: "Diff"
        case .launchpad: "Launchpad"
        case .roadmap: "Roadmap"
        case .readme: "README"
        case .runs: "Runs"
        case .launcher: "Launcher"
        }
    }

    var systemImage: String {
        switch self {
        case .terminal: "terminal"
        case .markdown: "doc.text"
        case .diff: "arrow.left.arrow.right.square"
        case .launchpad: "sparkles.rectangle.stack"
        case .roadmap: "list.bullet.rectangle"
        case .readme: "text.document"
        case .runs: "clock.arrow.circlepath"
        case .launcher: "play.square"
        }
    }
}

@ViewBuilder
private func paneUnavailable(title: String, systemImage: String, description: String) -> some View {
    ContentUnavailableView(title, systemImage: systemImage, description: Text(description))
}

private func resolveMarkdownURL(from configuredPath: String?, basePath: String?) -> URL? {
    guard let basePath else { return nil }
    let baseURL = URL(fileURLWithPath: basePath)

    if let configuredPath {
        let candidate = URL(fileURLWithPath: configuredPath, relativeTo: baseURL)
        if FileManager.default.fileExists(atPath: candidate.path()) {
            return candidate
        }
    }

    let fallbackPaths = [
        "scratch/pane-routing-spec.md",
        "wave/agent-embedding/README.md",
        "README.md",
    ]

    for path in fallbackPaths {
        let candidate = baseURL.appendingPathComponent(path)
        if FileManager.default.fileExists(atPath: candidate.path()) {
            return candidate
        }
    }

    return nil
}

private func runShellCommand(_ argv: [String]) async throws -> String {
    let process = Process()
    let stdout = Pipe()
    let stderr = Pipe()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
    process.arguments = argv
    process.standardOutput = stdout
    process.standardError = stderr
    process.environment = ProcessInfo.processInfo.environment

    try process.run()
    process.waitUntilExit()

    let stdoutData = stdout.fileHandleForReading.readDataToEndOfFile()
    let stderrData = stderr.fileHandleForReading.readDataToEndOfFile()
    let stdoutText = String(data: stdoutData, encoding: .utf8) ?? ""
    let stderrText = String(data: stderrData, encoding: .utf8) ?? ""

    guard process.terminationStatus == 0 else {
        let detail = stderrText.trimmingCharacters(in: .whitespacesAndNewlines)
        throw NSError(
            domain: "MultiplexerView",
            code: Int(process.terminationStatus),
            userInfo: [NSLocalizedDescriptionKey: detail.isEmpty ? "Command failed." : detail]
        )
    }

    return stdoutText
}

private func renderMarkdown(_ text: String) -> AttributedString {
    (try? AttributedString(
        markdown: text,
        options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
    )) ?? AttributedString(text)
}

private func previewText(_ text: String?, maxLines: Int = 4) -> String {
    guard let text else { return "" }
    let lines = text.components(separatedBy: .newlines)
    if lines.count <= maxLines {
        return text.trimmingCharacters(in: .whitespacesAndNewlines)
    }
    return lines.prefix(maxLines).joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
}
