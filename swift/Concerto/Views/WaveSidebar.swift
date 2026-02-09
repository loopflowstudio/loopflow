// Unified sidebar showing all waves.

import SwiftUI
import LoopflowCore

struct WaveSidebar: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer

    @State private var showingDiagnostics = false
    @State private var actionError: String?
    @State private var showingActionError = false
    @State private var isCreatingWave = false

    // Keyboard navigation state
    @State private var keyboardFocusedId: String?
    @State private var isEditingWaveName = false
    @FocusState private var isSidebarFocused: Bool

    private var waveGroups: WaveGroups {
        repoState.waveGroups
    }

    private func sectionHeader(_ title: String, icon: String, color: Color, count: Int) -> some View {
        HStack(spacing: 6) {
            Image(systemName: icon)
                .font(.system(size: 9))
                .foregroundStyle(color)
            Text(title)
                .font(.caption2)
                .fontWeight(.medium)
                .foregroundStyle(.white.opacity(0.6))
            if count > 0 {
                Text("(\(count))")
                    .font(.caption2)
                    .foregroundStyle(.white.opacity(0.4))
            }
        }
        .padding(.leading, 8)
        .padding(.top, 12)
        .padding(.bottom, 4)
    }

    private func waveRows(_ waves: [WaveViewModel]) -> some View {
        ForEach(waves) { wave in
            WaveRow(
                wave: wave,
                isSelected: repoState.selectedWaveId == wave.id,
                isKeyboardFocused: keyboardFocusedId == wave.id,
                onSelect: {
                    selectWave(wave.id)
                },
                onDelete: {
                    Task {
                        try? await repoState.deleteWave(wave)
                    }
                },
                onRename: { newName in
                    Task {
                        try? await repoState.renameWave(wave, to: newName)
                    }
                },
                isEditingAnyName: $isEditingWaveName
            )
        }
    }

    var body: some View {
        @Bindable var repoState = repoState
        VStack(alignment: .leading, spacing: 0) {
            header

            if repoState.waves.isEmpty && !repoState.lfdConnected {
                disconnectedState
            } else if repoState.waves.isEmpty {
                emptyState
            } else {
                waveList
            }
        }
        .background(Color.loopflowBurgundy)
        .sheet(isPresented: $showingDiagnostics) {
            DiagnosticsView()
        }
        .alert("Error", isPresented: $showingActionError) {
            Button("OK") { actionError = nil }
        } message: {
            Text(actionError ?? "An error occurred")
        }
        .onReceive(NotificationCenter.default.publisher(for: .newWaveRequested)) { _ in
            createWaveDirectly()
        }
        .onReceive(NotificationCenter.default.publisher(for: .selectWave)) { notification in
            if let waveId = notification.userInfo?["waveId"] as? String,
               repoState.waveStore.wave(for: waveId) != nil {
                selectWave(waveId)
            }
        }
    }

    private var header: some View {
        HStack {
            Text("Waves")
                .font(.caption)
                .fontWeight(.medium)
                .foregroundStyle(.white.opacity(0.7))
                .help("Waves are autonomous AI workers that run flows on your codebase")

            if waveGroups.attentionCount > 0 {
                HStack(spacing: 4) {
                    Circle()
                        .fill(Color.statusWarning)
                        .frame(width: 6, height: 6)
                    Text("\(waveGroups.attentionCount)")
                        .font(.caption2)
                        .fontWeight(.medium)
                        .foregroundStyle(Color.statusWarning)
                }
                .help("\(waveGroups.attentionCount) wave\(waveGroups.attentionCount == 1 ? "" : "s") need\(waveGroups.attentionCount == 1 ? "s" : "") attention")
            }

            Spacer()

            Circle()
                .fill(repoState.lfdConnected ? Color.statusSuccess : Color.white.opacity(0.3))
                .frame(width: 6, height: 6)
                .help(repoState.lfdConnected ? "Connected to lfd daemon" : "lfd daemon not connected")

            Button {
                createWaveDirectly()
            } label: {
                Image(systemName: "plus")
                    .font(.caption)
                    .foregroundStyle(.white.opacity(isCreatingWave || !repoState.lfdConnected ? 0.3 : 0.7))
            }
            .buttonStyle(.plain)
            .disabled(isCreatingWave || !repoState.lfdConnected)
            .help(repoState.lfdConnected ? "Create a new wave" : "Connect to lfd first")
            .accessibleButton("Create new wave")
            .minHitTarget()

            Button {
                showingDiagnostics = true
            } label: {
                Image(systemName: "doc.text.magnifyingglass")
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.7))
            }
            .buttonStyle(.plain)
            .help("Open diagnostics")
            .accessibleButton("Open diagnostics")
            .minHitTarget()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    private var disconnectedState: some View {
        VStack(spacing: 0) {
            Spacer()
                .frame(maxHeight: .infinity)

            VStack(spacing: 12) {
                Image(systemName: "link.circle")
                    .font(.system(size: 28))
                    .foregroundStyle(.white.opacity(0.3))

                VStack(spacing: 4) {
                    Text("Connect to lfd")
                        .fontWeight(.medium)
                        .foregroundStyle(.white.opacity(0.7))
                    Text("Start the daemon to manage waves.")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.5))
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 16)
                }

                Button {
                    Task {
                        do {
                            try await repoState.connectLfd(outputBuffer: outputBuffer)
                        } catch {
                            actionError = "Failed to connect: \(error.localizedDescription)"
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

            Spacer()
                .frame(maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity)
        .padding()
    }

    private var emptyState: some View {
        VStack(spacing: 0) {
            Spacer()
                .frame(maxHeight: .infinity)

            VStack(spacing: Spacing.xl) {
                // Quick experiment section (primary)
                QuickExperimentSidebarView { step in
                    launchQuickExperiment(step: step)
                }
                .accessibilityIdentifier("quick-experiment-section")

                // Divider
                Rectangle()
                    .fill(Color.white.opacity(0.1))
                    .frame(height: 1)
                    .padding(.horizontal, Spacing.lg)

                // Create wave section (secondary)
                VStack(spacing: Spacing.sm) {
                    Button {
                        createWaveDirectly()
                    } label: {
                        Label("Create Wave", systemImage: "plus")
                            .font(.caption)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .disabled(isCreatingWave)
                    .accessibilityIdentifier("wave-empty-create")

                    // Sidebar preview showing wave structure
                    SidebarPreviewView()
                }
            }

            Spacer()
                .frame(maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity)
        .padding()
    }

    private func launchQuickExperiment(step: String) {
        guard let repo = repoState.currentRepo else { return }
        let terminal = TerminalApp.warp
        do {
            try TerminalLauncher().launchStep(step, terminal: terminal, at: repo)
        } catch {
            actionError = "Failed to launch: \(error.localizedDescription)"
            showingActionError = true
        }
    }

    private var waveList: some View {
        @Bindable var repoState = repoState
        return ScrollView {
            LazyVStack(alignment: .leading, spacing: 4) {
                if !waveGroups.blocked.isEmpty {
                    sectionHeader("Needs Attention", icon: "exclamationmark.triangle.fill", color: .orange, count: waveGroups.blocked.count)
                    waveRows(waveGroups.blocked)
                }

                if !waveGroups.pr.isEmpty {
                    sectionHeader("Open PRs", icon: "arrow.triangle.pull", color: .statusSuccess, count: waveGroups.openPRCount)
                    waveRows(waveGroups.pr)
                }

                if !waveGroups.recentActivity.isEmpty {
                    sectionHeader("Recent Activity", icon: "clock.arrow.circlepath", color: .cyan, count: waveGroups.recentActivity.count)
                    waveRows(waveGroups.recentActivity)
                }

                if !waveGroups.active.isEmpty {
                    sectionHeader("Active", icon: "circle.fill", color: .blue, count: waveGroups.active.count)
                    waveRows(waveGroups.active)
                }

                if !waveGroups.idle.isEmpty {
                    sectionHeader("Idle", icon: "circle", color: .white.opacity(0.5), count: waveGroups.idle.count)
                    waveRows(waveGroups.idle)
                }
            }
            .padding(.horizontal, 8)
        }
        .focusable()
        .focused($isSidebarFocused)
        .focusEffectDisabled()
        .onKeyPress(.upArrow) {
            moveFocus(-1)
            return .handled
        }
        .onKeyPress(.downArrow) {
            moveFocus(1)
            return .handled
        }
        .onKeyPress(.return) {
            guard !isEditingWaveName else { return .ignored }
            if let id = keyboardFocusedId,
               repoState.waveStore.wave(for: id) != nil {
                selectWave(id)
            }
            return .handled
        }
    }

    private func moveFocus(_ delta: Int) {
        let waves = waveGroups.allInOrder
        guard !waves.isEmpty else { return }

        if let currentId = keyboardFocusedId,
           let currentIndex = waves.firstIndex(where: { $0.id == currentId }) {
            let newIndex = max(0, min(waves.count - 1, currentIndex + delta))
            keyboardFocusedId = waves[newIndex].id
        } else {
            keyboardFocusedId = delta > 0 ? waves.first?.id : waves.last?.id
        }
    }

    private func selectWave(_ waveId: String) {
        repoState.selectedWaveId = waveId
        keyboardFocusedId = waveId
    }

    private func createWaveDirectly() {
        LoggingService.ui("createWave: button clicked, lfdConnected=\(repoState.lfdConnected)")
        guard !isCreatingWave else {
            LoggingService.ui("createWave: already in progress, ignoring")
            return
        }
        isCreatingWave = true

        Task {
            do {
                // Ensure lfd is connected before creating wave
                if !repoState.lfdConnected {
                    LoggingService.ui("createWave: lfd not connected, attempting connect")
                    try await repoState.connectLfd(outputBuffer: outputBuffer)
                    // Brief delay to let the daemon start
                    try await Task.sleep(for: .milliseconds(500))
                    LoggingService.ui("createWave: connect completed, lfdConnected=\(repoState.lfdConnected)")
                }

                // Create with auto-generated name, then select it
                LoggingService.ui("createWave: calling repoState.createWave")
                try await repoState.createWave(name: "")
                LoggingService.ui("createWave: success, triggering name edit")
                // The wave is selected in createWave, trigger name edit
                NotificationCenter.default.post(name: .editWaveName, object: nil)
            } catch {
                LoggingService.ui("createWave: error=\(error.localizedDescription)")
                // Provide clearer error message for daemon issues
                if !repoState.lfdConnected {
                    actionError = "lfd daemon not running. Run 'lfd install' in terminal."
                } else {
                    actionError = error.localizedDescription
                }
                showingActionError = true
            }
            isCreatingWave = false
        }
    }
}

// MARK: - Notification Names

extension Notification.Name {
    static let newWaveRequested = Notification.Name("newWaveRequested")
    static let editWaveName = Notification.Name("editWaveName")
}

#Preview {
    let state = RepoState()
    state.configureMockWaves()
    return WaveSidebar()
        .environment(state)
        .environment(OutputBuffer())
        .frame(width: 280, height: 400)
}
