// Main content view with wave sidebar and detail panel.

import SwiftUI
import LoopflowCore

// MARK: - Notification Names for Keyboard Actions

extension Notification.Name {
    static let showNewWorktreeSheet = Notification.Name("showNewWorktreeSheet")
    static let focusPromptInput = Notification.Name("focusPromptInput")
    static let viewWorktreeDiff = Notification.Name("viewWorktreeDiff")
    static let runCurrentStep = Notification.Name("runCurrentStep")
    static let toggleCommandPalette = Notification.Name("toggleCommandPalette")
}

struct ContentView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(SessionState.self) private var sessionState
    @Environment(LauncherState.self) private var launcherState

    @State private var showingError = false
    @State private var showCommandPalette = false
    @Environment(\.colorScheme) private var colorScheme

    private var palette: LoopflowPalette {
        LoopflowPalette.make(for: colorScheme)
    }

    private func flowEditorBinding(_ flow: Flow) -> some View {
        @Bindable var repoState = repoState
        return FlowEditor(
            flow: Binding(
                get: { repoState.selectedFlow ?? flow },
                set: { repoState.selectedFlow = $0 }
            ),
            availablePrompts: repoState.prompts,
            availableFlows: repoState.flows.filter { $0.name != flow.name },
            onSave: {
                if let selected = repoState.selectedFlow {
                    repoState.saveFlow(selected)
                }
            },
            onRun: {
                // TODO: Launch flow in terminal
            },
            onDelete: {
                if let selected = repoState.selectedFlow {
                    repoState.deleteFlow(selected)
                }
            }
        )
    }

    var body: some View {
        @Bindable var repoState = repoState
        NavigationSplitView {
            WaveSidebar()
                .navigationSplitViewColumnWidth(min: 240, ideal: 280, max: 360)
        } detail: {
            if repoState.isLoading {
                ProgressView("Loading...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if repoState.currentRepo != nil {
                detailContent
            } else {
                ProgressView("Opening repository...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .onChange(of: repoState.errorMessage) { _, newValue in
            showingError = newValue != nil
        }
        .alert("Error", isPresented: $showingError) {
            Button("OK") {
                repoState.errorMessage = nil
            }
        } message: {
            Text(repoState.errorMessage ?? "An unknown error occurred")
        }
        .navigationTitle(repoState.currentRepo?.lastPathComponent ?? "Loopflow Concerto")
        .toolbar {
            ToolbarItem(placement: .automatic) {
                if repoState.isRefreshingWorktrees {
                    ProgressView()
                        .controlSize(.small)
                } else if let message = repoState.refreshMessage {
                    Text(message)
                        .foregroundStyle(.secondary)
                        .font(.caption)
                }
            }
            ToolbarItem(placement: .primaryAction) {
                Button {
                    Task { await repoState.refreshWorktrees(showFeedback: true) }
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .help("Refresh waves and worktrees")
            }
        }
        .background(palette.background)
        .overlay {
            if showCommandPalette {
                ZStack {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                        .onTapGesture {
                            showCommandPalette = false
                        }

                    VStack {
                        CommandPalette(
                            isPresented: $showCommandPalette,
                            actions: buildPaletteActions()
                        )
                        .padding(.top, 80)

                        Spacer()
                    }
                }
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleCommandPalette)) { _ in
            showCommandPalette.toggle()
        }
    }

    private func buildPaletteActions() -> [PaletteAction] {
        var actions: [PaletteAction] = []

        actions.append(PaletteAction("New Wave", icon: "plus.square", shortcut: "⌘N") {
            NotificationCenter.default.post(name: .newWaveRequested, object: nil)
        })

        actions.append(PaletteAction("Focus Prompt", icon: "text.cursor", shortcut: "⌘L") {
            NotificationCenter.default.post(name: .focusPromptInput, object: nil)
        })

        actions.append(PaletteAction("Refresh", icon: "arrow.clockwise") {
            Task { await repoState.refreshWorktrees(showFeedback: true) }
        })

        if let wave = repoState.selectedWave, let worktreePath = wave.worktreePath {
            let terminalLauncher = TerminalLauncher()
            let terminal = repoState.config?.terminalApp ?? .warp
            let ide = repoState.config?.ideApp ?? .cursor

            actions.append(PaletteAction("Open Terminal", icon: "terminal", shortcut: "T") {
                try? terminalLauncher.launchTerminal(terminal, at: URL(fileURLWithPath: worktreePath))
            })

            actions.append(PaletteAction("Open \(ide.displayName)", icon: "curlybraces", shortcut: "I") {
                try? terminalLauncher.openInIDE(ide, at: URL(fileURLWithPath: worktreePath), workspace: repoState.config?.workspace)
            })

            actions.append(PaletteAction("View Diff", icon: "doc.text.magnifyingglass", shortcut: "D") {
                NotificationCenter.default.post(name: .viewWorktreeDiff, object: wave)
            })

            if wave.prURL != nil {
                actions.append(PaletteAction("View PR", icon: "arrow.up.right.square", shortcut: "P") {
                    if let url = wave.prURL {
                        terminalLauncher.openURL(url)
                    }
                })

                if wave.prState == .open {
                    actions.append(PaletteAction("Land PR", icon: "airplane.arrival", shortcut: "L") {
                        Task {
                            try? await repoState.landBranch(for: wave)
                        }
                    })
                }
            } else if wave.hasDiff {
                actions.append(PaletteAction("Create PR", icon: "arrow.triangle.pull", shortcut: "P") {
                    Task {
                        try? await repoState.landBranch(for: wave)
                    }
                })
            }

            actions.append(PaletteAction("Reveal in Finder", icon: "folder", shortcut: "⌘⇧F") {
                terminalLauncher.openInFinder(at: URL(fileURLWithPath: worktreePath))
            })
        }

        if !repoState.prompts.isEmpty {
            actions.append(PaletteAction("Run Step", icon: "play.fill", shortcut: "⌘↵") {
                NotificationCenter.default.post(name: .runCurrentStep, object: nil)
            })
        }

        return actions
    }

    @ViewBuilder
    private var detailContent: some View {
        if Flags.beta, let flow = repoState.selectedFlow {
            flowEditorBinding(flow)
        } else if let wave = repoState.selectedWave {
            WaveDetailPanel(wave: wave)
        } else {
            QuickExperimentDetailView { step in
                launchQuickExperiment(step: step)
            }
        }
    }

    private func launchQuickExperiment(step: String) {
        guard let repo = repoState.currentRepo else { return }

        let terminalLauncher = TerminalLauncher()
        let terminal = repoState.config?.terminalApp ?? .warp

        // Launch terminal in main repo with the step command
        let command = "lf \(step)"
        do {
            try terminalLauncher.launchTerminal(terminal, at: repo, command: command)
        } catch {
            // Show error in UI
            repoState.errorMessage = "Failed to launch: \(error.localizedDescription)"
        }
    }
}

#Preview {
    ContentView()
        .environment(RepoState())
        .environment(SessionState())
        .environment(LauncherState())
}
