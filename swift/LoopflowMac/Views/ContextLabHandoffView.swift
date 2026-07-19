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
                    Text("\(shortTrace(address.runId)) / \(shortTrace(address.invocationId)) / \(shortTrace(address.turnId))")
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
                    attention: task.attention,
                    repoPath: route.repoPath,
                    terminalStore: TaskTerminalStore.shared,
                    initialSection: task.attention.level == .blue ? .feedback : .changes
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

struct ContextRefinementError: LocalizedError {
    let message: String

    init(_ message: String) {
        self.message = message
    }

    var errorDescription: String? {
        message
    }
}

func contextRefinementProject(
    _ projects: [WaveProject],
    projectId: String?
) throws -> WaveProject {
    if let projectId {
        guard let project = projects.first(where: { $0.id == projectId }) else {
            throw ContextRefinementError(
                "The saved refinement Project no longer belongs to this Wave. Choose another Project."
            )
        }
        return project
    }
    guard projects.count == 1, let project = projects.first else {
        let repair = projects.isEmpty
            ? "Sync or create a Project in this Wave first."
            : "Choose the Project that should own refinement Tasks for this Wave."
        throw ContextRefinementError(repair)
    }
    return project
}

func contextRefinementSourcePath(sourcePath: String, repoPath: String) throws -> String {
    guard let relativePath = contextRelativeSourcePath(sourcePath, repoPath: repoPath) else {
        throw ContextRefinementError(
            "The selected source is outside this Wave's repo and cannot seed its Task worker."
        )
    }
    return relativePath
}

func contextRefinementTaskTitle(label: String, contentSha256: String) -> String {
    "Refine \(label) \(contentSha256.prefix(8))"
}

func contextRefinementDirective(
    label: String,
    sourcePath: String,
    sourceSha256: String,
    seed: RefinementSeed
) throws -> String {
    let seedData = try JSONEncoder().encode(seed)
    return """
    Refine text for \(label).

    Work on `\(sourcePath)` in this Task worktree. Preserve the source's intent, use the refine skill, and inspect linked immutable traces only as needed. Before editing, verify the file's raw SHA-256 is `\(sourceSha256)`; stop and report drift if it is not.

    <lf:context-refinement-seed>
    \(String(decoding: seedData, as: UTF8.self))
    </lf:context-refinement-seed>
    """
}

func sourceFileHash(path: String) -> String? {
    guard let content = FileManager.default.contents(atPath: path) else { return nil }
    return SHA256.hash(data: content).map { String(format: "%02x", $0) }.joined()
}

private func shortTrace(_ value: String) -> String {
    String(value.prefix(10))
}
#endif
