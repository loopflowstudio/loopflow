// Main content view with agent sidebar and detail panel.

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
    @Bindable var appState: AppState
    @State private var showingError = false
    @State private var showCommandPalette = false
    @Environment(\.colorScheme) private var colorScheme

    private var palette: LoopflowPalette {
        LoopflowPalette.make(for: colorScheme)
    }

    private func flowEditorBinding(_ flow: Flow) -> some View {
        FlowEditor(
            flow: Binding(
                get: { appState.selectedFlow ?? flow },
                set: { appState.selectedFlow = $0 }
            ),
            availablePrompts: appState.prompts,
            availableFlows: appState.flows.filter { $0.name != flow.name },
            onSave: {
                if let selected = appState.selectedFlow {
                    appState.saveFlow(selected)
                }
            },
            onRun: {
                // TODO: Launch flow in terminal
            },
            onDelete: {
                if let selected = appState.selectedFlow {
                    appState.deleteFlow(selected)
                }
            }
        )
    }

    var body: some View {
        NavigationSplitView {
            AgentSidebar(appState: appState)
                .navigationSplitViewColumnWidth(min: 240, ideal: 280, max: 360)
        } detail: {
            if appState.isLoading {
                ProgressView("Loading...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if appState.currentRepo != nil {
                detailContent
            } else {
                // No repo loaded yet - show loading placeholder
                ProgressView("Opening repository...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .onChange(of: appState.errorMessage) { _, newValue in
            showingError = newValue != nil
        }
        .alert("Error", isPresented: $showingError) {
            Button("OK") {
                appState.errorMessage = nil
            }
        } message: {
            Text(appState.errorMessage ?? "An unknown error occurred")
        }
        .navigationTitle(appState.currentRepo?.lastPathComponent ?? "Loopflow Concerto")
        .toolbar {
            ToolbarItem(placement: .automatic) {
                if appState.isRefreshingWorktrees {
                    ProgressView()
                        .controlSize(.small)
                } else if let message = appState.refreshMessage {
                    Text(message)
                        .foregroundStyle(.secondary)
                        .font(.caption)
                }
            }
            ToolbarItem(placement: .primaryAction) {
                Button {
                    Task { await appState.refreshWorktrees(showFeedback: true) }
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .help("Refresh agents and worktrees")
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

        // Global actions
        actions.append(PaletteAction("New Agent", icon: "plus.square", shortcut: "⌘N") {
            // Post notification to trigger new agent sheet
            NotificationCenter.default.post(name: .showNewAgentSheet, object: nil)
        })

        actions.append(PaletteAction("Focus Prompt", icon: "text.cursor", shortcut: "⌘L") {
            NotificationCenter.default.post(name: .focusPromptInput, object: nil)
        })

        actions.append(PaletteAction("Refresh", icon: "arrow.clockwise") {
            Task { await appState.refreshWorktrees(showFeedback: true) }
        })

        // Worktree-specific actions when a worktree is selected
        if let worktree = appState.selectedWorktree {
            let terminalLauncher = TerminalLauncher()
            let terminal = appState.config?.terminalApp ?? .warp
            let ide = appState.config?.ideApp ?? .cursor

            actions.append(PaletteAction("Open Terminal", icon: "terminal", shortcut: "T") {
                try? terminalLauncher.launchTerminal(terminal, at: URL(fileURLWithPath: worktree.path))
            })

            actions.append(PaletteAction("Open \(ide.displayName)", icon: "curlybraces", shortcut: "I") {
                try? terminalLauncher.openInIDE(ide, at: URL(fileURLWithPath: worktree.path), workspace: appState.config?.workspace)
            })

            actions.append(PaletteAction("View Diff", icon: "doc.text.magnifyingglass", shortcut: "D") {
                NotificationCenter.default.post(name: .viewWorktreeDiff, object: worktree)
            })

            if worktree.prURL != nil {
                actions.append(PaletteAction("View PR", icon: "arrow.up.right.square", shortcut: "P") {
                    if let url = worktree.prURL {
                        terminalLauncher.openURL(url)
                    }
                })

                if worktree.prState == .open {
                    actions.append(PaletteAction("Land PR", icon: "airplane.arrival", shortcut: "L") {
                        Task {
                            try? await appState.landPR(for: worktree)
                        }
                    })
                }
            } else {
                actions.append(PaletteAction("Create PR", icon: "arrow.triangle.pull", shortcut: "P") {
                    Task {
                        try? await appState.createPR(for: worktree)
                    }
                })
            }

            actions.append(PaletteAction("Reveal in Finder", icon: "folder", shortcut: "⌘⇧F") {
                terminalLauncher.openInFinder(at: URL(fileURLWithPath: worktree.path))
            })
        }

        // Run step action
        if !appState.prompts.isEmpty {
            actions.append(PaletteAction("Run Step", icon: "play.fill", shortcut: "⌘↵") {
                NotificationCenter.default.post(name: .runCurrentStep, object: nil)
            })
        }

        return actions
    }

    @ViewBuilder
    private var detailContent: some View {
        if Flags.beta, let flow = appState.selectedFlow {
            flowEditorBinding(flow)
        } else if let agent = appState.selectedAgent {
            // Show agent detail panel when an agent is selected
            AgentDetailPanel(appState: appState, agent: agent)
        } else {
            // No agent selected - show empty state
            // Orphan worktrees (without agents) are intentionally hidden
            VStack(spacing: 16) {
                Image(systemName: "cpu")
                    .font(.system(size: 48))
                    .foregroundStyle(.tertiary)
                Text("Select an agent")
                    .font(.headline)
                    .foregroundStyle(.secondary)
                Text("Or create one to get started")
                    .font(.subheadline)
                    .foregroundStyle(.tertiary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

#Preview {
    ContentView(appState: AppState())
}
