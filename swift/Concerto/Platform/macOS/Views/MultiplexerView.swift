// Wave multiplexer — recursive split layout with one outer terminal pane and native companion panes.
// Concerto owns the outer layout; tmux owns shell subdivision inside the terminal pane.

import AppKit
import SwiftUI
import LoopflowCore

struct MultiplexerView: View {
    let waveId: String
    let worktreePath: String?

    @Environment(RepoState.self) private var repoState

    private var store: MultiplexerStore { repoState.multiplexerStore }
    private var layout: LayoutNode { store.layout(for: waveId) }
    private var focusedPaneId: String? { store.focusedPaneId(for: waveId) }

    var body: some View {
        if let worktreePath {
            LayoutNodeView(
                node: layout,
                waveId: waveId,
                worktreePath: worktreePath,
                focusedPaneId: focusedPaneId
            )
            .background(Color.black)
        } else {
            ContentUnavailableView(
                "No worktree",
                systemImage: "folder.badge.questionmark",
                description: Text("This wave needs a worktree before you can work in it.\nRun: lf ops wt create \(waveId)")
            )
        }
    }
}

// MARK: - Recursive layout rendering

private struct LayoutNodeView: View {
    let node: LayoutNode
    let waveId: String
    let worktreePath: String
    let focusedPaneId: String?

    var body: some View {
        switch node {
        case .leaf(let pane):
            PaneContainerView(
                pane: pane,
                waveId: waveId,
                worktreePath: worktreePath,
                isFocused: pane.id == focusedPaneId
            )
        case .split(let axis, let first, let second, let ratio):
            SplitLayoutView(
                axis: axis,
                first: first,
                second: second,
                ratio: ratio,
                waveId: waveId,
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
    let waveId: String
    let worktreePath: String
    let focusedPaneId: String?

    var body: some View {
        GeometryReader { geometry in
            let totalSize = axis == .vertical ? geometry.size.height : geometry.size.width
            let firstSize = totalSize * ratio
            let secondSize = max(0, totalSize - firstSize - 1)

            if axis == .vertical {
                VStack(spacing: 0) {
                    LayoutNodeView(node: first, waveId: waveId, worktreePath: worktreePath, focusedPaneId: focusedPaneId)
                        .frame(height: firstSize)
                    Divider()
                    LayoutNodeView(node: second, waveId: waveId, worktreePath: worktreePath, focusedPaneId: focusedPaneId)
                        .frame(height: secondSize)
                }
            } else {
                HStack(spacing: 0) {
                    LayoutNodeView(node: first, waveId: waveId, worktreePath: worktreePath, focusedPaneId: focusedPaneId)
                        .frame(width: firstSize)
                    Divider()
                    LayoutNodeView(node: second, waveId: waveId, worktreePath: worktreePath, focusedPaneId: focusedPaneId)
                        .frame(width: secondSize)
                }
            }
        }
    }
}

// MARK: - Pane container

private struct PaneContainerView: View {
    let pane: PaneState
    let waveId: String
    let worktreePath: String
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
            RoundedRectangle(cornerRadius: 0)
                .stroke(isFocused ? Color.loopflowBurgundy.opacity(0.7) : palette.border, lineWidth: isFocused ? 2 : 1)
                .allowsHitTesting(false)
        }
        .contentShape(Rectangle())
        .onTapGesture {
            repoState.multiplexerStore.setFocusedPane(pane.id, for: waveId)
        }
    }

    @ViewBuilder
    private var paneContent: some View {
        switch pane.type {
        case .terminal:
            TerminalPaneView(pane: pane, waveId: waveId, worktreePath: worktreePath)
        case .markdown:
            MarkdownPaneView(pane: pane, worktreePath: worktreePath)
        case .diff:
            DiffPaneView(worktreePath: worktreePath)
        case .launchpad:
            LaunchpadPaneView(waveId: waveId, worktreePath: worktreePath)
        }
    }

    private var backgroundColor: Color {
        pane.type == .terminal ? .black : palette.surface
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
    let worktreePath: String

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
            } else if tmuxReady {
                GhosttyTerminalView(
                    workingDirectory: worktreePath,
                    argv: TmuxSession(waveId: waveId, worktreePath: worktreePath).attachCommand(),
                    sessionId: pane.id,
                    manager: ghosttyManager
                )
            } else {
                ProgressView("Starting tmux…")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .background(Color.black)
            }
        }
        .task(id: pane.id) {
            let tmux = TmuxSession(waveId: waveId, worktreePath: worktreePath)
            do {
                try await tmux.ensureBaseSession()
                tmuxReady = true
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}

// MARK: - Native panes

private struct MarkdownPaneView: View {
    let pane: PaneState
    let worktreePath: String

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
            let fileURL = resolveMarkdownURL(from: pane.config.filePath, worktreePath: worktreePath)
            displayPath = fileURL?.lastPathComponent ?? ""
            contents = fileURL.flatMap { try? String(contentsOf: $0, encoding: .utf8) } ?? ""
        }
    }
}

private struct DiffPaneView: View {
    let worktreePath: String

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
        .task(id: worktreePath) {
            do {
                let output = try await runShellCommand(
                    ["git", "-C", worktreePath, "diff", "--no-color", "--stat", "main...HEAD"]
                )
                let trimmed = output.trimmingCharacters(in: .whitespacesAndNewlines)
                diffOutput = trimmed.isEmpty ? "No diff against main." : output
            } catch {
                diffOutput = "Failed to load diff.\n\n\(error.localizedDescription)"
            }
        }
    }
}

private struct LaunchpadPaneView: View {
    let waveId: String
    let worktreePath: String

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    private let terminalLauncher = TerminalLauncher()
    private let worktreeURL: URL

    init(waveId: String, worktreePath: String) {
        self.waveId = waveId
        self.worktreePath = worktreePath
        self.worktreeURL = URL(fileURLWithPath: worktreePath)
    }

    private var wave: WaveViewModel? {
        repoState.waveStore.wave(for: waveId)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xl) {
            Text("Open in…")
                .font(Typography.sectionTitle())
                .foregroundStyle(Color.loopflowBurgundy)

            VStack(alignment: .leading, spacing: Spacing.sm) {
                launchButton("Cursor", icon: "curlybraces") {
                    try? terminalLauncher.openInIDE(.cursor, at: worktreeURL, remoteHost: repoState.repoTarget?.remoteHost)
                }
                launchButton("Finder", icon: "folder") {
                    terminalLauncher.openInFinder(at: worktreeURL)
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
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(palette.surface)
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
}

// MARK: - Helpers

private extension PaneType {
    var displayName: String {
        switch self {
        case .terminal: "Terminal"
        case .markdown: "Markdown"
        case .diff: "Diff"
        case .launchpad: "Launchpad"
        }
    }
}

private func resolveMarkdownURL(from configuredPath: String?, worktreePath: String) -> URL? {
    let worktreeURL = URL(fileURLWithPath: worktreePath)

    if let configuredPath {
        let candidate = URL(fileURLWithPath: configuredPath, relativeTo: worktreeURL)
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
        let candidate = worktreeURL.appendingPathComponent(path)
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
