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
    @State private var isCreatingWave = false
    @State private var showingInstantiateAllConfirm = false
    @State private var pendingSchemas: [WaveSchema] = []

    // Keyboard navigation state
    @State private var keyboardFocusedId: String?
    @State private var isEditingWaveName = false

    private var waveGroups: WaveGroups {
        repoState.waveGroups
    }

    private var orphanWorktrees: [WorktreeInfo] {
        repoState.worktreeStore.orphans
    }

    private var uninstantiatedSchemas: [WaveSchema] {
        repoState.waveSchemas.filter { !$0.isInstantiated }.sorted { $0.name < $1.name }
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
        .confirmationDialog(
            "Instantiate schemas",
            isPresented: $showingInstantiateAllConfirm,
            titleVisibility: .visible
        ) {
            Button("Instantiate and start \(pendingSchemas.count) waves") {
                instantiateSchemas(pendingSchemas, startImmediately: true)
            }
            Button("Cancel", role: .cancel) {
                pendingSchemas = []
            }
        } message: {
            Text(batchConfirmationMessage)
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

                Text(repoState.connectionStore.activeConnection.displayName)
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

            Menu {
                Button {
                    createWaveDirectly()
                } label: {
                    Label("New wave", systemImage: "plus")
                }

                if !uninstantiatedSchemas.isEmpty {
                    Divider()

                    ForEach(uninstantiatedSchemas.prefix(8)) { schema in
                        Button {
                            instantiateSchemas([schema], startImmediately: true)
                        } label: {
                            Label(schema.name, systemImage: schemaIcon(schema))
                        }
                    }

                    Divider()

                    Button {
                        pendingSchemas = uninstantiatedSchemas
                        showingInstantiateAllConfirm = true
                    } label: {
                        Label("Instantiate all", systemImage: "sparkles")
                    }
                }
            } label: {
                Image(systemName: "plus")
                    .font(Typography.caption())
                    .foregroundStyle(.white.opacity(isCreatingWave || !repoState.isConnected || repoState.repoTarget == nil ? 0.3 : 0.7))
            }
            .menuStyle(.borderlessButton)
            .disabled(isCreatingWave || !repoState.isConnected || repoState.repoTarget == nil)
            .help(repoState.isConnected ? "Create or instantiate waves (C)" : repoState.connectionSummary)
            .accessibleButton("Create or instantiate wave")
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

                if uninstantiatedSchemas.isEmpty {
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
                } else {
                    VStack(spacing: Spacing.xs) {
                        ForEach(uninstantiatedSchemas.prefix(3)) { schema in
                            Button {
                                instantiateSchemas([schema], startImmediately: true)
                            } label: {
                                Label("Start \(schema.name)", systemImage: schemaIcon(schema))
                                    .font(Typography.caption())
                            }
                            .buttonStyle(.borderedProminent)
                            .controlSize(.small)
                            .disabled(isCreatingWave)
                        }

                        if uninstantiatedSchemas.count > 1 {
                            Button {
                                pendingSchemas = uninstantiatedSchemas
                                showingInstantiateAllConfirm = true
                            } label: {
                                Label("Instantiate all", systemImage: "sparkles")
                                    .font(Typography.caption())
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.small)
                            .disabled(isCreatingWave)
                        }
                    }
                }
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

                    sectionHeader("Worktrees", icon: "folder", count: orphanWorktrees.count)

                    ForEach(orphanWorktrees) { worktree in
                        WorktreeRow(worktree: worktree) {
                            upgradeWorktree(worktree)
                        }
                    }
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
                try await ensureLfdConnected()

                guard repoState.repoTarget != nil else {
                    actionError = "Select a repository in Connection Settings first."
                    showingActionError = true
                    return
                }


                try await repoState.createWave(name: "")
                NotificationCenter.default.post(name: .editWaveName, object: nil)
            } catch {
                actionError = repoState.isConnected
                    ? error.localizedDescription
                    : repoState.connectionSummary
                showingActionError = true
            }
        }
    }

    private var batchConfirmationMessage: String {
        guard !pendingSchemas.isEmpty else {
            return "No schemas selected."
        }

        let names = pendingSchemas.map(\.name).joined(separator: ", ")
        let stimulusKinds = Set(
            pendingSchemas.compactMap { schema in
                schema.stimulus?.kind
            }
        )
        let stimulusText = stimulusKinds.isEmpty ? "none" : stimulusKinds.sorted().joined(separator: ", ")
        return "Schemas: \(names)\nStimuli: \(stimulusText)\nWaves will start immediately."
    }

    private func schemaIcon(_ schema: WaveSchema) -> String {
        switch schema.stimulus?.kind {
        case "cron": return "clock"
        case "watch": return "eye"
        case "loop": return "repeat"
        default: return "waveform.path.ecg"
        }
    }

    private func instantiateSchemas(_ schemas: [WaveSchema], startImmediately: Bool) {
        guard !schemas.isEmpty else { return }
        guard !isCreatingWave else { return }
        isCreatingWave = true

        Task {
            defer {
                isCreatingWave = false
                pendingSchemas = []
            }

            do {
                try await ensureLfdConnected()

                for schema in schemas {
                    try await repoState.instantiateSchema(schema, startImmediately: startImmediately)
                }
            } catch {
                actionError = error.localizedDescription
                showingActionError = true
            }
        }
    }

    private func ensureLfdConnected() async throws {
        guard !repoState.lfdConnected else { return }
        try await repoState.connectLfd(outputBuffer: outputBuffer)
        try await Task.sleep(for: .milliseconds(500))
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
