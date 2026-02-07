// Main content view with wave sidebar and detail panel.

import SwiftUI
import LoopflowCore

// MARK: - Notification Names for Keyboard Actions

extension Notification.Name {
    static let toggleCommandPalette = Notification.Name("toggleCommandPalette")
}

struct ContentView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(SessionState.self) private var sessionState

    @State private var showingError = false
    @State private var showCommandPalette = false
    @Environment(\.colorScheme) private var colorScheme

    private var palette: LoopflowPalette {
        LoopflowPalette.make(for: colorScheme)
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
        actions.append(PaletteAction("Refresh Waves", icon: "arrow.clockwise", shortcut: "R") {
            Task { await repoState.refreshWaves() }
        })

        if let wave = repoState.selectedWave, let worktreePath = wave.worktreePath {
            let terminalLauncher = TerminalLauncher()
            let terminal = TerminalApp.warp
            let ide = IDEApp.cursor

            actions.append(PaletteAction("Open Terminal", icon: "terminal", shortcut: "T") {
                try? terminalLauncher.launchTerminal(terminal, at: URL(fileURLWithPath: worktreePath))
            })

            actions.append(PaletteAction("Open \(ide.displayName)", icon: "curlybraces", shortcut: "I") {
                try? terminalLauncher.openInIDE(ide, at: URL(fileURLWithPath: worktreePath), workspace: nil)
            })

            actions.append(PaletteAction("Reveal in Finder", icon: "folder", shortcut: "⌘⇧F") {
                terminalLauncher.openInFinder(at: URL(fileURLWithPath: worktreePath))
            })
        }

        return actions
    }

    @ViewBuilder
    private var detailContent: some View {
        if let wave = repoState.selectedWave {
            WaveDetailPanel(wave: wave)
        } else {
            QuickExperimentDetailView { step in
                launchQuickExperiment(step: step)
            }
        }
    }

    private func launchQuickExperiment(step: String) {
        guard let repo = repoState.currentRepo else { return }
        let terminal = TerminalApp.warp
        do {
            try TerminalLauncher().launchStep(step, terminal: terminal, at: repo)
        } catch {
            repoState.errorMessage = "Failed to launch: \(error.localizedDescription)"
        }
    }
}

#Preview {
    ContentView()
        .environment(RepoState())
        .environment(SessionState())
}
