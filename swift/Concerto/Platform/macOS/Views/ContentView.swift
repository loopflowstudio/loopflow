// Main content view with wave sidebar and detail panel.

import SwiftUI
import LoopflowCore

struct ContentView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(KeyboardRouter.self) private var keyboardRouter

    @State private var showingError = false
    @State private var showCommandPalette = false
    @State private var windowNumber: Int?
    @State private var noWaveToast: String?
    @State private var lastNoWaveToastAt = Date.distantPast
    @State private var toastDismissTask: Task<Void, Never>?

    @Environment(\.palette) private var palette

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
        .background(WindowAccessor { window in
            let number = window?.windowNumber
            guard number != windowNumber else { return }

            if let old = windowNumber {
                keyboardRouter.unregister(windowNumber: old)
            }

            windowNumber = number

            if let number {
                keyboardRouter.register(windowNumber: number) { action in
                    handleShortcut(action)
                }
                keyboardRouter.setCommandPaletteVisible(showCommandPalette, for: number)
            }
        })
        .overlay {
            if showCommandPalette {
                commandPaletteOverlay
            }
        }
        .overlay {
            if keyboardRouter.isHelpOverlayVisible && isKeyWindowActive {
                ShortcutHelpOverlay(
                    isPresented: isHelpOverlayPresented,
                    shortcuts: keyboardRouter.shortcuts,
                    chords: keyboardRouter.chords
                )
            }
        }
        .overlay(alignment: .bottomTrailing) {
            if let chordIndicator = keyboardRouter.chordIndicator, isKeyWindowActive {
                Text(chordIndicator)
                    .font(Typography.code(12))
                    .foregroundStyle(.white)
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xs)
                    .background(.black.opacity(0.65))
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                    .padding(.trailing, Spacing.lg)
                    .padding(.bottom, Spacing.lg)
                    .transition(.opacity)
                    .zIndex(ZIndex.toast)
            }
        }
        .overlay(alignment: .bottom) {
            if let noWaveToast {
                Text(noWaveToast)
                    .font(Typography.caption())
                    .foregroundStyle(.white)
                    .padding(.horizontal, Spacing.md)
                    .padding(.vertical, Spacing.sm)
                    .background(.black.opacity(0.75))
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                    .padding(.bottom, Spacing.lg)
                    .transition(.opacity)
                    .zIndex(ZIndex.toast)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleCommandPalette)) { _ in
            keyboardRouter.isHelpOverlayVisible = false
            showCommandPalette = true
        }
        .onChange(of: showCommandPalette) { _, isVisible in
            if let windowNumber {
                keyboardRouter.setCommandPaletteVisible(isVisible, for: windowNumber)
            }
        }
        .onDisappear {
            toastDismissTask?.cancel()
            if let number = windowNumber {
                keyboardRouter.unregister(windowNumber: number)
            }
        }
    }

    private var commandPaletteOverlay: some View {
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
        .zIndex(ZIndex.modal)
    }

    private func buildPaletteActions() -> [PaletteAction] {
        var actions: [PaletteAction] = []

        actions.append(PaletteAction("New Wave", icon: "plus.square", shortcut: "C") {
            NotificationCenter.default.post(name: .newWaveRequested, object: nil)
        })
        actions.append(PaletteAction("Refresh Waves", icon: "arrow.clockwise") {
            Task { await repoState.refreshWaves() }
        })

        if let wave = repoState.selectedWave,
           let worktreePath = wave.worktreePath {
            let terminalLauncher = TerminalLauncher()
            let terminal = TerminalApp.warp
            let ide = IDEApp.cursor
            let remoteHost = repoState.repoTarget?.remoteHost

            actions.append(PaletteAction("Open Terminal", icon: "terminal", shortcut: "T") {
                do {
                    try terminalLauncher.openTerminal(terminal, at: worktreePath, remoteHost: remoteHost)
                } catch {
                    repoState.errorMessage = "Failed to open terminal: \(error.localizedDescription)"
                }
            })

            actions.append(PaletteAction("Open \(ide.displayName)", icon: "curlybraces", shortcut: "I") {
                do {
                    try terminalLauncher.openInIDE(ide, at: URL(fileURLWithPath: worktreePath), remoteHost: remoteHost)
                } catch {
                    repoState.errorMessage = "Failed to open \(ide.displayName): \(error.localizedDescription)"
                }
            })

            if let host = remoteHost {
                actions.append(PaletteAction("Copy SSH Command", icon: "doc.on.doc") {
                    terminalLauncher.copySSHCommand(host: host, path: worktreePath)
                })
            } else {
                actions.append(PaletteAction("Reveal in Finder", icon: "folder", shortcut: "F") {
                    terminalLauncher.openInFinder(at: URL(fileURLWithPath: worktreePath))
                })
            }

            if wave.prURL != nil {
                actions.append(PaletteAction("View PR", icon: "arrow.up.right.square", shortcut: "P") {
                    NotificationCenter.default.post(name: .viewWavePR, object: nil)
                })
            }
        }

        return actions
    }

    private func handleShortcut(_ action: ShortcutAction) {
        if keyboardRouter.requiresWaveActions.contains(action), repoState.selectedWave == nil {
            showNoWaveShortcutToast(for: action)
            return
        }

        switch action {
        case .moveDown:
            post(.moveFocusDown)
        case .moveUp:
            post(.moveFocusUp)
        case .selectFocused:
            post(.selectFocusedWave)
        case .goToFirst:
            post(.goToFirstWave)
        case .goToLast:
            post(.goToLastWave)
        case .createWave:
            post(.newWaveRequested)
        case .editName:
            post(.editWaveName)
        case .deleteWave:
            guard let wave = repoState.selectedWave else { return }
            performWaveAction("delete wave") {
                try await repoState.deleteWave(wave)
            }
        case .retryWave:
            guard let wave = repoState.selectedWave else { return }
            outputBuffer.clearOutput(for: wave.id)
            performWaveAction("retry wave") {
                try await repoState.runWave(wave: wave)
            }
        case .stopWave:
            guard let wave = repoState.selectedWave else { return }
            performWaveAction("stop wave") {
                try await repoState.stopWave(wave)
            }
        case .landWave:
            guard let wave = repoState.selectedWave else { return }
            performWaveAction("land wave") {
                try await repoState.landWave(wave)
            }
        case .nextWave:
            guard let wave = repoState.selectedWave else { return }
            performWaveAction("start next iteration") {
                try await repoState.nextWave(wave)
            }
        case .openTerminal:
            openTerminalForSelectedWave()
        case .openIDE:
            openIDEForSelectedWave()
        case .openFinder:
            openFinderForSelectedWave()
        case .viewPR:
            openPRForSelectedWave()
        case .switchToCurrentTab:
            post(.switchToCurrentTab)
        case .switchToRunsTab:
            post(.switchToRunsTab)
        case .focusSessionComposer:
            if let selectedWave = repoState.selectedWave,
               repoState.shouldShowInteractiveSession(for: selectedWave) {
                NotificationCenter.default.post(
                    name: .focusSessionComposer,
                    object: nil,
                    userInfo: ["waveId": selectedWave.id]
                )
            } else {
                keyboardRouter.isHelpOverlayVisible = false
                showCommandPalette = true
            }
        case .openCommandPalette:
            keyboardRouter.isHelpOverlayVisible = false
            showCommandPalette = true
        case .showHelp:
            showCommandPalette = false
            keyboardRouter.isHelpOverlayVisible.toggle()
        }
    }

    private func showNoWaveShortcutToast(for action: ShortcutAction) {
        let now = Date()
        guard now.timeIntervalSince(lastNoWaveToastAt) > 1.5 else { return }

        lastNoWaveToastAt = now
        let key = keyboardRouter.keyDisplay(for: action) ?? "shortcut"
        noWaveToast = "Select a wave to use \(key)"

        toastDismissTask?.cancel()
        toastDismissTask = Task {
            try? await Task.sleep(for: .seconds(1.5))
            noWaveToast = nil
        }
    }

    private func performWaveAction(_ label: String, action: @escaping () async throws -> Void) {
        Task {
            do {
                try await action()
            } catch {
                repoState.errorMessage = "Failed to \(label): \(error.localizedDescription)"
            }
        }
    }

    private func openTerminalForSelectedWave() {
        performLauncherAction(failureLabel: "open terminal") { launcher, worktreeURL in
            try launcher.openTerminal(.warp, at: worktreeURL.path(), remoteHost: repoState.repoTarget?.remoteHost)
        }
    }

    private func openIDEForSelectedWave() {
        performLauncherAction(failureLabel: "open Cursor") { launcher, worktreeURL in
            try launcher.openInIDE(.cursor, at: worktreeURL, remoteHost: repoState.repoTarget?.remoteHost)
        }
    }

    private func openFinderForSelectedWave() {
        guard !repoState.isRemoteTarget else { return }
        guard let worktreeURL = selectedWorktreeURL else { return }
        let launcher = TerminalLauncher()
        launcher.openInFinder(at: worktreeURL)
    }

    private func openPRForSelectedWave() {
        guard let url = repoState.selectedWave?.prURL else { return }
        let launcher = TerminalLauncher()
        launcher.openURL(url)
    }

    private var isHelpOverlayPresented: Binding<Bool> {
        Binding(
            get: { keyboardRouter.isHelpOverlayVisible },
            set: { keyboardRouter.isHelpOverlayVisible = $0 }
        )
    }

    private var selectedWorktreeURL: URL? {
        guard let path = repoState.selectedWave?.worktreePath else { return nil }
        return URL(fileURLWithPath: path)
    }

    private var isKeyWindowActive: Bool {
        guard let windowNumber else { return false }
        return NSApp.keyWindow?.windowNumber == windowNumber
    }

    private func performLauncherAction(
        failureLabel: String,
        _ action: (TerminalLauncher, URL) throws -> Void
    ) {
        guard let worktreeURL = selectedWorktreeURL else { return }
        let launcher = TerminalLauncher()
        do {
            try action(launcher, worktreeURL)
        } catch {
            repoState.errorMessage = "Failed to \(failureLabel): \(error.localizedDescription)"
        }
    }

    private func post(_ name: Notification.Name) {
        NotificationCenter.default.post(name: name, object: nil)
    }

    @ViewBuilder
    private var detailContent: some View {
        if let wave = repoState.selectedWave {
            WaveDetailPanel(wave: wave)
                .id(wave.id)
        } else {
            CatchWaveView()
        }
    }
}

#Preview {
    ContentView()
        .environment(RepoState())
        .environment(OutputBuffer())
        .environment(KeyboardRouter())
}
