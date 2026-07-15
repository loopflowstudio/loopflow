#if os(macOS)
import CryptoKit
import Loopflow
import SwiftUI

private enum TraceArtifactTab: String, CaseIterable, Identifiable {
    case system = "System prompt"
    case task = "Task prompt"
    case conversation = "Conversation"

    var id: String { rawValue }
}

struct TraceEvidenceView: View {
    let address: TraceAddress

    @Environment(\.palette) private var palette
    @State private var content: TraceContentSnapshot?
    @State private var tab = TraceArtifactTab.task
    @State private var errorMessage: String?

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Trace evidence")
                        .font(Typography.heroTitle(24))
                        .foregroundStyle(palette.text)
                    Text("\(shortTrace(address.runId)) / \(shortTrace(address.launchId)) / \(shortTrace(address.turnId))")
                        .font(Typography.code(10))
                        .foregroundStyle(palette.textSecondary)
                        .textSelection(.enabled)
                }
                Spacer()
                Picker("Artifact", selection: $tab) {
                    ForEach(TraceArtifactTab.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
                .frame(width: 390)
            }
            .padding(Spacing.xl)
            Divider()
            if let errorMessage {
                ContentUnavailableView(
                    "Trace unavailable",
                    systemImage: "exclamationmark.triangle",
                    description: Text(errorMessage)
                )
            } else if let content {
                artifactView(selectedArtifact(content))
            } else {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(palette.background)
        .task { await load() }
    }

    private func selectedArtifact(_ content: TraceContentSnapshot) -> TraceArtifactSnapshot {
        switch tab {
        case .system: content.systemPrompt
        case .task: content.taskPrompt
        case .conversation: content.conversation
        }
    }

    @ViewBuilder
    private func artifactView(_ artifact: TraceArtifactSnapshot) -> some View {
        if let body = artifact.content {
            VStack(alignment: .leading, spacing: 0) {
                if let path = artifact.path {
                    Text(path)
                        .font(Typography.code(9))
                        .foregroundStyle(palette.textSecondary)
                        .textSelection(.enabled)
                        .padding(.horizontal, Spacing.xl)
                        .padding(.vertical, Spacing.sm)
                    Divider()
                }
                ScrollView([.horizontal, .vertical]) {
                    Text(body)
                        .font(Typography.code(10))
                        .foregroundStyle(palette.text)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .topLeading)
                        .padding(Spacing.xl)
                }
                .background(palette.background)
            }
            .background(palette.background)
        } else {
            ContentUnavailableView(
                "Artifact missing",
                systemImage: "doc.questionmark",
                description: Text(artifact.unavailableReason ?? "This trace did not capture the selected artifact.")
            )
        }
    }

    private func load() async {
        do {
            content = try await RegistryQueryLocal.shared.traceContent(address)
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

struct RefinementTaskChoice: Identifiable, Hashable {
    let waveId: String
    let waveName: String
    let repoPath: String
    let task: RoadmapTask

    var id: String { task.task.identifier }
}

struct RefinementTaskSheet: View {
    let query: SessionSetQuery
    let evidence: SourceEvidence
    let backlink: ContextLabRoute
    let onLaunch: (TaskWorkspaceRoute) -> Void

    @Environment(\.dismiss) private var dismiss
    @Environment(\.palette) private var palette
    @State private var tasks: [RefinementTaskChoice] = []
    @State private var selectedTaskId: String?
    @State private var isLoading = true
    @State private var isLaunching = false
    @State private var errorMessage: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("Refine \(evidence.label)")
                    .font(Typography.heroTitle(24))
                    .foregroundStyle(palette.text)
                Text("Start a fresh, trace-linked session in an existing Intelligence Task worktree.")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            .padding(Spacing.xl)
            Divider()

            if isLoading {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                VStack(alignment: .leading, spacing: Spacing.lg) {
                    VStack(alignment: .leading, spacing: Spacing.sm) {
                        Text("SOURCE REVISION")
                            .font(Typography.caption(9).weight(.bold))
                            .foregroundStyle(palette.textSecondary)
                        Text(evidence.sourcePath ?? evidence.label)
                            .font(Typography.code(10))
                            .textSelection(.enabled)
                        Text(evidence.contentSha256)
                            .font(Typography.code(9))
                            .foregroundStyle(palette.textSecondary)
                            .textSelection(.enabled)
                    }

                    if tasks.isEmpty {
                        ContentUnavailableView(
                            "No ready Intelligence Task",
                            systemImage: "hammer",
                            description: Text("Start a Task for this repo, then reopen Refine source. Context Lab never creates external work without confirmation.")
                        )
                    } else {
                        Picker("Task", selection: $selectedTaskId) {
                            Text("Choose a Task").tag(String?.none)
                            ForEach(tasks) { choice in
                                Text("\(choice.task.task.identifier) · \(choice.task.task.name)")
                                    .tag(Optional(choice.id))
                            }
                        }
                        .pickerStyle(.radioGroup)
                    }

                    if let errorMessage {
                        Label(errorMessage, systemImage: "exclamationmark.triangle")
                            .font(Typography.caption(10))
                            .foregroundStyle(Color.statusError)
                            .textSelection(.enabled)
                    }
                    Spacer()
                    HStack {
                        Button("Cancel") { dismiss() }
                            .buttonStyle(GhostButtonStyle())
                        Spacer()
                        Button(isLaunching ? "Launching…" : "Launch refinement") {
                            Task { await launch() }
                        }
                        .buttonStyle(DarkButtonStyle())
                        .disabled(selectedTaskId == nil || isLaunching)
                    }
                }
                .padding(Spacing.xl)
            }
        }
        .background(palette.background)
        .task { await loadTasks() }
    }

    private func loadTasks() async {
        defer { isLoading = false }
        do {
            let roadmap = try await RegistryQueryLocal.shared.roadmap(wave: "intelligence")
            tasks = refinementTaskChoices(in: roadmap, query: query)
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    @MainActor
    private func launch() async {
        guard let selectedTaskId,
              let selectedChoice = tasks.first(where: { $0.id == selectedTaskId })
        else { return }
        isLaunching = true
        defer { isLaunching = false }
        do {
            let refreshed = try await RegistryQueryLocal.shared.contextLab(query)
            guard let currentEvidence = refreshed.evidence.first(where: { $0.nodeId == evidence.nodeId }),
                  currentEvidence.isEditable,
                  currentEvidence.contentSha256 == evidence.contentSha256,
                  let currentSourcePath = currentEvidence.sourcePath,
                  let currentSourceSha256 = currentEvidence.currentSourceSha256
            else {
                throw ContextRefinementError(
                    "The selected source revision changed. Refresh Context Lab and select the current revision."
                )
            }
            let roadmap = try await RegistryQueryLocal.shared.roadmap(wave: "intelligence")
            guard let choice = refinementTaskChoices(in: roadmap, query: query)
                .first(where: { $0.id == selectedTaskId })
            else {
                throw ContextRefinementError(
                    "The selected Task is no longer idle with a durable worktree. Let its current work finish, then reopen Refine source."
                )
            }
            guard refinementWorkspaceIsCurrent(selectedChoice, current: choice),
                  let runtime = choice.task.runtime,
                  let workspace = choice.task.reference.workspace
            else {
                throw ContextRefinementError(
                    "The selected Task workspace changed. Close this sheet, reselect the Task, and retry."
                )
            }
            let taskSource = try taskSourcePath(
                sourcePath: currentSourcePath,
                repoPath: choice.repoPath,
                worktree: workspace.worktree
            )
            guard FileManager.default.fileExists(atPath: workspace.worktree) else {
                throw ContextRefinementError(
                    "The Task worktree is missing. Resume or recreate \(choice.task.task.identifier) first."
                )
            }

            let seed = RefinementSeed(
                query: query,
                selectedNodeId: evidence.nodeId,
                sourcePath: taskSource,
                startingContentSha256: evidence.contentSha256,
                measurements: evidence.measurements,
                evidence: evidence.representatives.map(\.address)
            )
            let seedData = try JSONEncoder().encode(seed)
            let body = """
            <lf:context-refinement-seed>
            \(String(decoding: seedData, as: UTF8.self))
            </lf:context-refinement-seed>

            Improve the canonical source while preserving its intent. Inspect the linked immutable traces only as needed. Refuse to edit if the effective source hash is no longer \(evidence.contentSha256).
            """
            let lfPath = try LocalWaveAgentLauncher.resolveWaveCapableLf(originRepo: choice.repoPath)
            let encoded = Data(body.utf8).base64EncodedString()
            let command = "env \(shellQuote("LF_TASK_SESSION_ID=\(runtime.sessionId)")) "
                + "\(shellQuote("LF_WAVE_ID=\(choice.waveId)")) \(shellQuote(lfPath)) --tui refine "
                + "\"$(printf '%s' '\(encoded)' | /usr/bin/base64 -D)\""
            let terminal = try await TaskTerminalStore.shared.addTerminal(
                taskSessionId: runtime.sessionId,
                issueIdentifier: choice.task.task.identifier,
                worktree: workspace.worktree
            )
            do {
                guard sourceFileHash(path: currentSourcePath) == currentSourceSha256 else {
                    throw ContextRefinementError(
                        "The canonical source changed during launch. Refresh Context Lab and select the current revision."
                    )
                }
                guard sourceFileHash(path: taskSource) == currentSourceSha256 else {
                    throw ContextRefinementError(
                        "The Task worktree does not contain the selected source revision. Rebase it, then retry."
                    )
                }
                try await TmuxSession(
                    sessionName: terminal.tmuxName,
                    worktreePath: workspace.worktree
                ).sendCommand(command)
            } catch {
                TaskTerminalStore.shared.close(terminal, taskSessionId: runtime.sessionId)
                throw error
            }
            errorMessage = nil
            onLaunch(TaskWorkspaceRoute(
                wave: choice.waveName,
                issue: choice.task.task.identifier,
                repoPath: choice.repoPath,
                initialSection: .terminal,
                context: backlink
            ))
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

struct TaskWorkspaceWindow: View {
    let route: TaskWorkspaceRoute

    @Environment(\.openWindow) private var openWindow
    @Environment(\.palette) private var palette
    @State private var task: WaveTaskWork?
    @State private var errorMessage: String?

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Button {
                    openWindow(id: "context-lab", value: route.context)
                } label: {
                    Label("Back to Context Lab", systemImage: "arrow.uturn.backward")
                }
                .buttonStyle(.borderless)
                Spacer()
                Text(route.issue)
                    .font(Typography.code(10))
                    .foregroundStyle(palette.textSecondary)
            }
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.sm)
            Divider()
            if let task {
                TaskWorkspaceView(
                    task: task.task,
                    reference: task.reference,
                    runtime: task.runtime,
                    repoPath: route.repoPath,
                    terminalStore: TaskTerminalStore.shared,
                    initialSection: route.initialSection
                )
            } else if let errorMessage {
                ContentUnavailableView(
                    "Task workspace unavailable",
                    systemImage: "exclamationmark.triangle",
                    description: Text(errorMessage)
                )
            } else {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(palette.background)
        .task { await load() }
    }

    private func load() async {
        do {
            let status = try await RegistryQueryLocal.shared.status(wave: route.wave, cwd: route.repoPath)
            task = status.workMap.projects
                .flatMap(\.tasks)
                .first { $0.task.identifier == route.issue }
            if task == nil { errorMessage = "\(route.issue) is no longer in the \(route.wave) work map." }
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

func refinementTaskChoices(
    in roadmap: RoadmapSnapshot,
    query: SessionSetQuery
) -> [RefinementTaskChoice] {
    let selectedRepos = Set(query.repoPaths.map(\.normalizedFilePath))
    return roadmap.waves
        .filter { selectedRepos.isEmpty || selectedRepos.contains($0.wave.repo.normalizedFilePath) }
        .flatMap { wave in
            wave.projects.items.flatMap { project in
                project.tasks.compactMap { task in
                    guard !task.task.completed,
                          let runtime = task.runtime,
                          !runtime.processAlive,
                          task.reference.workspace != nil
                    else { return nil }
                    return RefinementTaskChoice(
                        waveId: wave.wave.id,
                        waveName: wave.wave.name,
                        repoPath: wave.wave.repo,
                        task: task
                    )
                }
            }
        }
}

func refinementWorkspaceIsCurrent(
    _ selected: RefinementTaskChoice,
    current: RefinementTaskChoice
) -> Bool {
    guard let selectedRuntime = selected.task.runtime,
          let currentRuntime = current.task.runtime,
          let selectedWorkspace = selected.task.reference.workspace,
          let currentWorkspace = current.task.reference.workspace
    else { return false }
    return selected.waveId == current.waveId
        && selected.repoPath.normalizedFilePath == current.repoPath.normalizedFilePath
        && selected.task.task.id == current.task.task.id
        && selected.task.task.identifier == current.task.task.identifier
        && selectedRuntime.sessionId == currentRuntime.sessionId
        && selectedRuntime.projectSessionId == currentRuntime.projectSessionId
        && selectedWorkspace.slug == currentWorkspace.slug
        && selectedWorkspace.branch == currentWorkspace.branch
        && selectedWorkspace.worktree.normalizedFilePath == currentWorkspace.worktree.normalizedFilePath
}

struct ContextRefinementError: LocalizedError {
    let message: String

    init(_ message: String) {
        self.message = message
    }

    var errorDescription: String? {
        message
    }
}

func taskSourcePath(sourcePath: String, repoPath: String, worktree: String) throws -> String {
    let source = URL(fileURLWithPath: sourcePath).standardizedFileURL.path
    let repo = URL(fileURLWithPath: repoPath).standardizedFileURL.path
    guard source == repo || source.hasPrefix(repo + "/") else {
        throw ContextRefinementError(
            "The selected source is outside this Task's repo and cannot be refined from its worktree."
        )
    }
    let relative = source == repo ? "" : String(source.dropFirst(repo.count + 1))
    return URL(fileURLWithPath: worktree).appendingPathComponent(relative).standardizedFileURL.path
}

func sourceFileHash(path: String) -> String? {
    guard let content = FileManager.default.contents(atPath: path) else { return nil }
    return SHA256.hash(data: content).map { String(format: "%02x", $0) }.joined()
}

private func shellQuote(_ value: String) -> String {
    "'\(value.replacingOccurrences(of: "'", with: "'\\''"))'"
}

private func shortTrace(_ value: String) -> String {
    String(value.prefix(10))
}
#endif
