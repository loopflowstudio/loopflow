// Unified sidebar showing all waves.

import SwiftUI
import LoopflowCore

struct WaveSidebar: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer

    @State private var showingDiagnostics = false
    @State private var showingConnectionSettings = false
    @State private var actionError: String?
    @State private var showingActionError = false
    @AppStorage("wave-sidebar.show-orphan-worktrees") private var showingOrphanWorktrees = false

    // Keyboard navigation state
    @State private var keyboardFocusedId: String?
    @State private var isEditingWaveName = false

    private var waveGroups: WaveGroups {
        repoState.waveGroups
    }

    private var orphanWorktrees: [WorktreeInfo] {
        repoState.worktreeStore.orphans
    }

    private func sectionHeader(_ title: String, icon: String, count: Int) -> some View {
        HStack(spacing: Spacing.xs) {
            Image(systemName: icon)
                .foregroundStyle(.white.opacity(0.15))
            Text(title.uppercased())
                .foregroundStyle(.white.opacity(0.25))
                .tracking(0.5)
            if count > 0 {
                Text("\(count)")
                    .foregroundStyle(.white.opacity(0.2))
            }
        }
        .font(Typography.caption(9))
        .padding(.leading, Spacing.sm)
        .padding(.top, Spacing.sm)
        .padding(.bottom, Spacing.xs)
    }

    private func waveRows(_ waves: [WaveViewModel]) -> some View {
        ForEach(waves) { wave in
            WaveRow(
                wave: wave,
                isSelected: repoState.selectedWaveId == wave.id,
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
            analyticsRow

            if repoState.waves.isEmpty && !repoState.lfdConnected {
                disconnectedState
            } else if repoState.waves.isEmpty && orphanWorktrees.isEmpty {
                emptyState
            } else {
                waveList
            }
        }
        .background(Color.loopflowBurgundy)
        .sheet(isPresented: $showingDiagnostics) {
            DiagnosticsView()
        }
        .sheet(isPresented: $showingConnectionSettings) {
            ConnectionSettingsView()
                .environment(repoState)
                .environment(outputBuffer)
        }
        .alert("Error", isPresented: $showingActionError) {
            Button("OK") { actionError = nil }
        } message: {
            Text(actionError ?? "An error occurred")
        }
        .onReceive(NotificationCenter.default.publisher(for: .newWaveRequested)) { _ in
            openDesignEntry()
        }
        .onReceive(NotificationCenter.default.publisher(for: .selectWave)) { notification in
            if let waveId = notification.userInfo?["waveId"] as? String {
                selectWave(waveId)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .moveFocusDown)) { _ in
            moveFocus(1)
        }
        .onReceive(NotificationCenter.default.publisher(for: .moveFocusUp)) { _ in
            moveFocus(-1)
        }
        .onReceive(NotificationCenter.default.publisher(for: .selectFocusedWave)) { _ in
            selectFocusedWave()
        }
        .onReceive(NotificationCenter.default.publisher(for: .goToFirstWave)) { _ in
            moveFocusToBoundary(isFirst: true)
        }
        .onReceive(NotificationCenter.default.publisher(for: .goToLastWave)) { _ in
            moveFocusToBoundary(isFirst: false)
        }
        .onChange(of: repoState.selectedWaveId) { _, newValue in
            if let newValue {
                keyboardFocusedId = newValue
            }
        }
    }

    private var header: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("Waves")
                    .font(Typography.caption())
                    .fontWeight(.medium)
                    .foregroundStyle(.white.opacity(0.7))

                Text(connectionSubtitle)
                    .font(Typography.caption(10))
                    .foregroundStyle(.white.opacity(0.45))
            }
            .help("Waves are autonomous AI workers that run flows on your codebase")

            Spacer()

            Circle()
                .fill(repoState.lfdConnected ? Color.statusSuccess : Color.white.opacity(0.3))
                .frame(width: 6, height: 6)
                .help(repoState.connectionSummary)

            Button {
                showingConnectionSettings = true
            } label: {
                Image(systemName: "network")
                    .font(Typography.caption())
                    .foregroundStyle(.white.opacity(0.7))
            }
            .buttonStyle(.plain)
            .help("Connection settings")
            .accessibleButton("Open connection settings")
            .minHitTarget()

            createButton

            Button {
                showingDiagnostics = true
            } label: {
                Image(systemName: "doc.text.magnifyingglass")
                    .font(Typography.caption())
                    .foregroundStyle(.white.opacity(0.7))
            }
            .buttonStyle(.plain)
            .help("Open diagnostics")
            .accessibleButton("Open diagnostics")
            .minHitTarget()
        }
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.md)
    }

    private var connectionSubtitle: String {
        switch repoState.connectionStore.mode {
        case .bundled:
            return "Bundled daemon"
        case .remote:
            return repoState.connectionStore.activeConnection.displayName
        }
    }

    private var createButtonDisabled: Bool {
        !repoState.isConnected || repoState.repoTarget == nil
    }

    private var analyticsRow: some View {
        Button {
            repoState.selectedWaveId = nil
            repoState.showingAnalytics = true
        } label: {
            HStack(spacing: Spacing.sm) {
                Image(systemName: "chart.line.uptrend.xyaxis")
                    .font(Typography.caption())
                Text("Analytics")
                    .font(Typography.caption())
                Spacer()
            }
            .foregroundStyle(repoState.showingAnalytics ? .white : .white.opacity(0.8))
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.sm)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .fill(repoState.showingAnalytics ? .white.opacity(0.14) : .clear)
            )
            .padding(.horizontal, Spacing.sm)
            .padding(.bottom, Spacing.xs)
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("sidebar-analytics")
    }

    private var createButtonLabel: some View {
        Image(systemName: "plus")
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(createButtonDisabled ? .white.opacity(0.35) : .white)
    }

    @ViewBuilder
    private var createButton: some View {
        Button {
            openDesignEntry()
        } label: {
            createButtonLabel
        }
        .buttonStyle(.plain)
        .disabled(createButtonDisabled)
        .help(repoState.isConnected ? "Start designing (C)" : repoState.connectionSummary)
        .accessibleButton("Start designing")
    }

    private var disconnectedState: some View {
        VStack(spacing: 0) {
            Spacer()
                .frame(maxHeight: .infinity)

            VStack(spacing: Spacing.md) {
                Image(systemName: "link.circle")
                    .font(Typography.heroTitle(28))
                    .foregroundStyle(.white.opacity(0.3))

                VStack(spacing: Spacing.xs) {
                    Text("Connect to server")
                        .fontWeight(.medium)
                        .foregroundStyle(.white.opacity(0.7))
                    Text(repoState.connectionSummary)
                        .font(Typography.caption())
                        .foregroundStyle(.white.opacity(0.5))
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, Spacing.lg)
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
                        .font(Typography.caption())
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)

                Button("Settings") {
                    showingConnectionSettings = true
                }
                .font(Typography.caption())
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

            VStack(spacing: Spacing.sm) {
                Text("No waves yet")
                    .font(Typography.caption())
                    .foregroundStyle(.white.opacity(0.5))

                Button {
                    openDesignEntry()
                } label: {
                    Label("Start designing", systemImage: "sparkles")
                        .font(Typography.caption())
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
                .accessibilityIdentifier("wave-empty-create")
            }

            Spacer()
                .frame(maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity)
        .padding()
    }

    private var waveList: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: Spacing.xs) {
                if !waveGroups.active.isEmpty {
                    sectionHeader("Active", icon: "circle.fill", count: waveGroups.active.count)
                    waveRows(waveGroups.active)
                }

                if !waveGroups.idle.isEmpty {
                    sectionHeader("Idle", icon: "circle", count: waveGroups.idle.count)
                    waveRows(waveGroups.idle)
                }

                // Orphan worktrees — on disk but not tracked by any wave
                if !orphanWorktrees.isEmpty {
                    Divider()
                        .background(.white.opacity(0.15))
                        .padding(.vertical, Spacing.xs)

                    DisclosureGroup(isExpanded: $showingOrphanWorktrees) {
                        VStack(spacing: Spacing.xs) {
                            ForEach(orphanWorktrees) { worktree in
                                WorktreeRow(worktree: worktree) {
                                    upgradeWorktree(worktree)
                                }
                            }
                        }
                        .padding(.top, Spacing.xs)
                    } label: {
                        HStack(spacing: Spacing.xs) {
                            Image(systemName: "folder")
                                .foregroundStyle(.white.opacity(0.25))
                            Text("WORKTREES")
                                .foregroundStyle(.white.opacity(0.25))
                                .tracking(0.5)
                            Text("\(orphanWorktrees.count)")
                                .foregroundStyle(.white.opacity(0.2))
                        }
                        .font(Typography.caption(9))
                        .padding(.leading, Spacing.sm)
                        .padding(.top, Spacing.sm)
                        .padding(.bottom, Spacing.xs)
                    }
                    .tint(.white.opacity(0.7))
                }
            }
            .padding(.horizontal, Spacing.sm)
        }
    }

    private func moveFocus(_ delta: Int) {
        guard !isEditingWaveName else { return }
        let waves = waveGroups.allInOrder
        guard !waves.isEmpty else { return }

        if let currentId = keyboardFocusedId,
           let currentIndex = waves.firstIndex(where: { $0.id == currentId }) {
            let newIndex = max(0, min(waves.count - 1, currentIndex + delta))
            keyboardFocusedId = waves[newIndex].id
        } else {
            keyboardFocusedId = delta > 0 ? waves.first?.id : waves.last?.id
        }

        if let keyboardFocusedId {
            repoState.selectedWaveId = keyboardFocusedId
        }
    }

    private func moveFocusToBoundary(isFirst: Bool) {
        let waves = waveGroups.allInOrder
        guard !waves.isEmpty else { return }
        let target = isFirst ? waves.first?.id : waves.last?.id
        guard let target else { return }
        keyboardFocusedId = target
        repoState.selectedWaveId = target
    }

    private func selectFocusedWave() {
        guard !isEditingWaveName else { return }
        if let id = keyboardFocusedId,
           repoState.waveStore.wave(for: id) != nil {
            selectWave(id)
        } else if let selectedWaveId = repoState.selectedWaveId,
                  repoState.waveStore.wave(for: selectedWaveId) != nil {
            selectWave(selectedWaveId)
        } else if let firstId = waveGroups.allInOrder.first?.id {
            selectWave(firstId)
        }
    }

    private func selectWave(_ waveId: String) {
        repoState.selectedWaveId = waveId
        keyboardFocusedId = waveId
        repoState.showingAnalytics = false
    }

    private func upgradeWorktree(_ worktree: WorktreeInfo) {
        guard let name = worktree.shortName else { return }
        Task {
            do {
                try await repoState.createWave(name: name)
            } catch {
                actionError = error.localizedDescription
                showingActionError = true
            }
        }
    }

    private func openDesignEntry() {
        guard repoState.isConnected else {
            actionError = repoState.connectionSummary
            showingActionError = true
            return
        }

        guard repoState.repoTarget != nil else {
            actionError = "Select a repository in Connection Settings first."
            showingActionError = true
            return
        }

        repoState.showingAnalytics = false
        repoState.selectedWaveId = nil
    }
}

#Preview {
    let state = RepoState()
    state.configureMockWaves()
    return WaveSidebar()
        .environment(state)
        .environment(OutputBuffer())
        .frame(width: 280, height: 400)
}
