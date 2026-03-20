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

    private struct MultiplexerContext {
        let waveId: String
<<<<<<< HEAD
<<<<<<< HEAD
        let focusedPane: PaneState
=======
        let worktreePath: String
=======
>>>>>>> 14032ed8 (Remove checked-in build artifacts and trim multiplexer scaffolding)
        let focusedPane: PaneState
<<<<<<< HEAD
        let routesToTerminal: Bool
>>>>>>> 19106fdd (concerto: simplify multiplexer helpers)
=======
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
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

        if multiplexerContext() != nil {
            actions.append(PaletteAction(
                "Split vertical",
                icon: "rectangle.split.1x2",
                shortcut: keyboardRouter.keyDisplay(for: .splitVertical)
            ) {
                self.handleMultiplexerSplit(axis: .horizontal)
            })
            actions.append(PaletteAction(
                "Split horizontal",
                icon: "rectangle.split.2x1",
                shortcut: keyboardRouter.keyDisplay(for: .splitHorizontal)
            ) {
                self.handleMultiplexerSplit(axis: .vertical)
            })
            actions.append(PaletteAction(
                "Close pane",
                icon: "xmark.square",
                shortcut: keyboardRouter.keyDisplay(for: .closePane)
            ) {
                self.handleClosePane()
            })
            actions.append(PaletteAction(
                "New shell",
                icon: "terminal",
                shortcut: keyboardRouter.keyDisplay(for: .newShellPane)
            ) {
                self.handleNewShell()
            })
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
                try await repoState.deleteWaveAndCleanupTmux(wave)
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
        case .splitVertical:
            handleMultiplexerSplit(axis: .horizontal)
        case .splitHorizontal:
            handleMultiplexerSplit(axis: .vertical)
        case .closePane:
            handleClosePane()
        case .newShellPane:
            handleNewShell()
        case .focusNextPane:
            handleFocusPane(.next)
        case .focusPreviousPane:
            handleFocusPane(.previous)
<<<<<<< HEAD
<<<<<<< HEAD
=======
        case .snapHalf, .snapThird, .snapQuarter:
            break // Phase 3
>>>>>>> 55cd605c (lf commit: implement)
=======
>>>>>>> 14032ed8 (Remove checked-in build artifacts and trim multiplexer scaffolding)
        case .switchToCurrentTab:
            post(.switchToCurrentTab)
        case .switchToRunsTab:
            post(.switchToRunsTab)
        case .focusSessionComposer:
            if let selectedWave = repoState.selectedWave,
               let terminalSession = repoState.terminalWorkspaceStore.activeSession(for: selectedWave.id) {
                repoState.selectTerminalSession(terminalSession.id)
            } else if let selectedWave = repoState.selectedWave,
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

    private func handleMultiplexerSplit(axis: SplitAxis) {
        guard let context = multiplexerContext() else { return }

<<<<<<< HEAD
<<<<<<< HEAD
        _ = repoState.multiplexerStore.splitPane(
            context.focusedPane.id,
            axis: axis,
            newPaneType: splitPaneType(for: context.focusedPane),
=======
        if context.routesToTerminal {
            performTmuxAction(context) { try await $0.split(axis) }
            return
        }

        _ = repoState.multiplexerStore.splitPane(
            context.focusedPane.id,
            axis: axis,
            newPaneType: nextOuterPaneType(after: context.focusedPane.type),
>>>>>>> 55cd605c (lf commit: implement)
=======
        _ = repoState.multiplexerStore.splitPane(
            context.focusedPane.id,
            axis: axis,
            newPaneType: splitPaneType(for: context.focusedPane),
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
            for: context.waveId
        )
    }

    private func handleClosePane() {
        guard let context = multiplexerContext() else { return }

<<<<<<< HEAD
<<<<<<< HEAD
        if let closedPane = repoState.multiplexerStore.closePane(context.focusedPane.id, for: context.waveId),
           let sessionName = closedPane.config.terminalSessionName {
            TmuxSessionRegistry.shared.killSession(named: sessionName)
        }
=======
        if context.routesToTerminal {
            performTmuxAction(context) { try await $0.closeFocusedThing() }
            return
        }

        _ = repoState.multiplexerStore.closePane(context.focusedPane.id, for: context.waveId)
>>>>>>> 55cd605c (lf commit: implement)
=======
        if let closedPane = repoState.multiplexerStore.closePane(context.focusedPane.id, for: context.waveId),
           let sessionName = closedPane.config.terminalSessionName {
            TmuxSessionRegistry.shared.killSession(named: sessionName)
        }
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
    }

    private func handleNewShell() {
        guard let context = multiplexerContext() else { return }
<<<<<<< HEAD
<<<<<<< HEAD
<<<<<<< HEAD
=======
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
        _ = repoState.multiplexerStore.splitPane(
            context.focusedPane.id,
            axis: .horizontal,
            newPaneType: .terminal,
            for: context.waveId
        )
<<<<<<< HEAD
=======

        Task {
            do {
                try await TmuxSession(waveId: context.waveId, worktreePath: context.worktreePath).createWindow()
            } catch {
                repoState.errorMessage = error.localizedDescription
            }
        }
>>>>>>> 55cd605c (lf commit: implement)
=======
        performTmuxAction(context) { try await $0.createWindow() }
>>>>>>> 19106fdd (concerto: simplify multiplexer helpers)
=======
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
    }

    private func handleFocusPane(_ direction: FocusDirection) {
        guard let context = multiplexerContext() else { return }
<<<<<<< HEAD
<<<<<<< HEAD
        repoState.multiplexerStore.moveFocus(direction, for: context.waveId)
    }

    private func multiplexerContext() -> MultiplexerContext? {
        guard let wave = repoState.selectedWave,
              (wave.worktreePath ?? wave.api.localWorktree) != nil,
=======

        if context.routesToTerminal {
            performTmuxAction(context) { tmux in
                switch direction {
                case .next:
                    try await tmux.selectNextWindow()
                case .previous:
                    try await tmux.selectPreviousWindow()
                }
            }
            return
        }

=======
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
        repoState.multiplexerStore.moveFocus(direction, for: context.waveId)
    }

    private func multiplexerContext() -> MultiplexerContext? {
        guard let wave = repoState.selectedWave,
<<<<<<< HEAD
              let worktreePath = wave.worktreePath ?? wave.api.localWorktree,
>>>>>>> 55cd605c (lf commit: implement)
=======
              (wave.worktreePath ?? wave.api.localWorktree) != nil,
>>>>>>> 14032ed8 (Remove checked-in build artifacts and trim multiplexer scaffolding)
              let focusedPane = repoState.multiplexerStore.focusedPane(for: wave.id) else {
            return nil
        }

<<<<<<< HEAD
<<<<<<< HEAD
        return MultiplexerContext(
            waveId: wave.id,
            focusedPane: focusedPane
        )
    }

    private func splitPaneType(for pane: PaneState) -> PaneType {
        pane.type == .terminal ? .launchpad : nextCompanionPaneType(after: pane.type)
    }

    private func nextCompanionPaneType(after paneType: PaneType) -> PaneType {
        switch paneType {
        case .terminal, .launchpad:
=======
        let routesToTerminal = focusedPane.type == .terminal && isTerminalResponder(NSApp.keyWindow?.firstResponder)
        return (wave.id, worktreePath, focusedPane, routesToTerminal)
=======
        return MultiplexerContext(
            waveId: wave.id,
            worktreePath: worktreePath,
            focusedPane: focusedPane
        )
>>>>>>> 19106fdd (concerto: simplify multiplexer helpers)
    }

    private func splitPaneType(for pane: PaneState) -> PaneType {
        pane.type == .terminal ? .launchpad : nextCompanionPaneType(after: pane.type)
    }

    private func nextCompanionPaneType(after paneType: PaneType) -> PaneType {
        switch paneType {
<<<<<<< HEAD
        case .terminal:
>>>>>>> 55cd605c (lf commit: implement)
=======
        case .terminal, .launchpad:
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
            .markdown
        case .markdown:
            .diff
        case .diff:
            .launchpad
<<<<<<< HEAD
<<<<<<< HEAD
        }
    }

=======
        case .launchpad:
            .markdown
        }
    }

    private func isTerminalResponder(_ responder: Any?) -> Bool {
        guard let responder else { return false }
        return String(describing: type(of: responder)).contains("GhosttyMetalView")
    }

>>>>>>> 55cd605c (lf commit: implement)
=======
        }
    }

>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
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
        if repoState.showingAnalytics {
            AnalyticsDashboardView()
        } else if let wave = repoState.selectedWave {
            WaveWorkspaceView(wave: wave)
                .id(wave.id)
        } else {
            AttentionQueueView()
        }
    }
}

#Preview {
    ContentView()
        .environment(RepoState())
        .environment(OutputBuffer())
        .environment(KeyboardRouter())
}
