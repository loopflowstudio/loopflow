// Unified sidebar showing all waves.

import SwiftUI
import LoopflowCore

struct WaveSidebar: View {
    @Environment(RepoState.self) private var repoState
    @Environment(SessionState.self) private var sessionState

    @State private var showingDiagnostics = false
    @State private var actionError: String?
    @State private var showingActionError = false
    @State private var isCreatingWave = false

    // Keyboard navigation state
    @State private var keyboardFocusedId: String?
    @FocusState private var isSidebarFocused: Bool

    // MARK: - Wave Sections

    private var blockedWaves: [Wave] {
        repoState.waves.filter { $0.status == .error }
    }

    private var prWaves: [Wave] {
        repoState.waves.filter { wave in
            wave.status != .error && pendingPR(for: wave) != nil
        }
    }

    private var activeWaves: [Wave] {
        repoState.waves.filter { wave in
            (wave.status == .running || wave.status == .waiting) &&
            wave.status != .error &&
            pendingPR(for: wave) == nil
        }
    }

    private var idleWaves: [Wave] {
        repoState.waves.filter { wave in
            wave.status == .idle && pendingPR(for: wave) == nil
        }
    }

    private var attentionCount: Int {
        blockedWaves.count + prWaves.count
    }

    private var allWavesInOrder: [Wave] {
        blockedWaves + prWaves + activeWaves + idleWaves
    }

    private func pendingPR(for wave: Wave) -> (number: Int, url: URL?)? {
        guard let prNumber = wave.prNumber,
              wave.prState == .open else {
            return nil
        }
        return (number: prNumber, url: wave.prURL)
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

    private func waveRows(_ waves: [Wave]) -> some View {
        ForEach(waves) { wave in
            WaveRow(
                wave: wave,
                isSelected: repoState.selectedWave?.id == wave.id,
                isKeyboardFocused: keyboardFocusedId == wave.id,
                liveOutput: sessionState.output(for: wave.id),
                pendingPR: pendingPR(for: wave),
                onSelect: {
                    repoState.selectedWave = wave
                    keyboardFocusedId = wave.id
                },
                onDelete: {
                    Task {
                        try? await repoState.deleteWave(wave)
                    }
                }
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
               let wave = repoState.waves.first(where: { $0.id == waveId }) {
                repoState.selectedWave = wave
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

            if attentionCount > 0 {
                HStack(spacing: 4) {
                    Circle()
                        .fill(Color.statusWarning)
                        .frame(width: 6, height: 6)
                    Text("\(attentionCount)")
                        .font(.caption2)
                        .fontWeight(.medium)
                        .foregroundStyle(Color.statusWarning)
                }
                .help("\(attentionCount) wave\(attentionCount == 1 ? "" : "s") need\(attentionCount == 1 ? "s" : "") attention")
            }

            Spacer()

            Circle()
                .fill(repoState.lfdConnected ? Color.green : Color.white.opacity(0.3))
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
                            try await repoState.connectLfd()
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

            VStack(spacing: 12) {
                Image(systemName: "cpu")
                    .font(.system(size: 28))
                    .foregroundStyle(.white.opacity(0.3))

                VStack(spacing: 4) {
                    Text("No waves yet")
                        .fontWeight(.medium)
                        .foregroundStyle(.white.opacity(0.7))
                        .accessibilityIdentifier("wave-empty-title")
                    Text("Create a wave to start AI-powered work on your codebase.")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.5))
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 16)
                        .accessibilityIdentifier("wave-empty-description")
                }

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
            }

            Spacer()
                .frame(maxHeight: .infinity)
            Spacer()
                .frame(maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity)
        .padding()
    }

    private var waveList: some View {
        @Bindable var repoState = repoState
        return ScrollView {
            LazyVStack(alignment: .leading, spacing: 4) {
                if !blockedWaves.isEmpty {
                    sectionHeader("Needs Attention", icon: "exclamationmark.triangle.fill", color: .orange, count: blockedWaves.count)
                    waveRows(blockedWaves)
                }

                if !prWaves.isEmpty {
                    sectionHeader("Open PRs", icon: "arrow.triangle.pull", color: .green, count: prWaves.count)
                    waveRows(prWaves)
                }

                if !activeWaves.isEmpty {
                    sectionHeader("Active", icon: "circle.fill", color: .blue, count: activeWaves.count)
                    waveRows(activeWaves)
                }

                if !idleWaves.isEmpty {
                    sectionHeader("Idle", icon: "circle", color: .white.opacity(0.5), count: idleWaves.count)
                    waveRows(idleWaves)
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
            if let id = keyboardFocusedId,
               let wave = repoState.waves.first(where: { $0.id == id }) {
                repoState.selectedWave = wave
            }
            return .handled
        }
    }

    private func moveFocus(_ delta: Int) {
        let waves = allWavesInOrder
        guard !waves.isEmpty else { return }

        if let currentId = keyboardFocusedId,
           let currentIndex = waves.firstIndex(where: { $0.id == currentId }) {
            let newIndex = max(0, min(waves.count - 1, currentIndex + delta))
            keyboardFocusedId = waves[newIndex].id
        } else {
            keyboardFocusedId = delta > 0 ? waves.first?.id : waves.last?.id
        }
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
                    try await repoState.connectLfd()
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
        .environment(SessionState())
        .frame(width: 280, height: 400)
}
