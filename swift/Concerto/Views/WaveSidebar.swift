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
                .font(Typography.caption())
                .fontWeight(.medium)
                .foregroundStyle(.white.opacity(0.7))
                .help("Waves are autonomous AI workers that run flows on your codebase")

            Spacer()

            Circle()
                .fill(repoState.lfdConnected ? Color.statusSuccess : Color.white.opacity(0.3))
                .frame(width: 6, height: 6)
                .help(repoState.lfdConnected ? "Connected to lfd daemon" : "lfd daemon not connected")

            Button {
                createWaveDirectly()
            } label: {
                Image(systemName: "plus")
                    .font(Typography.caption())
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

    private var disconnectedState: some View {
        VStack(spacing: 0) {
            Spacer()
                .frame(maxHeight: .infinity)

            VStack(spacing: Spacing.md) {
                Image(systemName: "link.circle")
                    .font(Typography.heroTitle(28))
                    .foregroundStyle(.white.opacity(0.3))

                VStack(spacing: Spacing.xs) {
                    Text("Connect to lfd")
                        .fontWeight(.medium)
                        .foregroundStyle(.white.opacity(0.7))
                    Text("Start the daemon to manage waves.")
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
                    createWaveDirectly()
                } label: {
                    Label("Create Wave", systemImage: "plus")
                        .font(Typography.caption())
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
                .disabled(isCreatingWave)
                .accessibilityIdentifier("wave-empty-create")
            }

            Spacer()
                .frame(maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity)
        .padding()
    }

    private var waveList: some View {
        return ScrollView {
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

                    sectionHeader("On Disk", icon: "folder", count: orphanWorktrees.count)

                    ForEach(orphanWorktrees) { worktree in
                        WorktreeRow(worktree: worktree) {
                            upgradeWorktree(worktree)
                        }
                    }
                }
            }
            .padding(.horizontal, Spacing.sm)
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

    private func createWaveDirectly() {
        guard !isCreatingWave else { return }
        isCreatingWave = true

        Task {
            defer { isCreatingWave = false }

            do {
                if !repoState.lfdConnected {
                    try await repoState.connectLfd(outputBuffer: outputBuffer)
                    try await Task.sleep(for: .milliseconds(500))
                }

                try await repoState.createWave(name: "")
                NotificationCenter.default.post(name: .editWaveName, object: nil)
            } catch {
                actionError = repoState.lfdConnected
                    ? error.localizedDescription
                    : "lfd daemon not running. Run 'lfd install' in terminal."
                showingActionError = true
            }
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
