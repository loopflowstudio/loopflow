// The exposed main window: a burgundy repo rail (left) filtering a burgundy wave
// list (center), with a "+" new-wave launcher, and a terminal detail (right) that
// attaches the selected wave's /goal agent in an embedded tmux session.
//
// Reshaped from the battle-tested PortfolioWindow: keeps its connection / daemon /
// EventService plumbing, swaps the multi-repo card grid for the sidebar+list+terminal
// shape sketched by RepoSidebarWindow, and reuses WaveRow + WaveSidebar's burgundy
// treatment so the style matches the app.

import SwiftUI
import LoopflowCore

enum RepoFilter: Hashable {
    case all
    case repo(String)
}

struct WavesView: View {
    let portfolioService: PortfolioService

    @Environment(\.palette) private var palette

    @State private var connectionStore = ConnectionStore()
    @State private var authProviderStore = AuthProviderStore()
    @State private var repoStates: [String: PortfolioRepoState] = [:]
    @State private var eventService: EventService?

    /// Repos shown in the rail: the persisted registry collapsed to main
    /// worktrees and deduped. Derived off the main thread in `refreshRepos`.
    @State private var repos: [PortfolioRepo] = []
    @State private var didSeedFromScan = false

    @State private var selection: RepoFilter = .all
    @State private var selectedWaveId: String?
    @State private var isShowingCreate = false

    /// All waves across every repo, in the registry's repo order.
    private var allWaves: [WaveViewModel] {
        repos.flatMap { repoStates[$0.path]?.waves ?? [] }
    }

    private var filteredWaves: [WaveViewModel] {
        switch selection {
        case .all:
            return allWaves
        case .repo(let path):
            let target = path.normalizedFilePath
            return allWaves.filter { $0.repo.normalizedFilePath == target }
        }
    }

    private var selectedWave: WaveViewModel? {
        guard let selectedWaveId else { return nil }
        return allWaves.first { $0.id == selectedWaveId }
    }

    private var isAnyConnected: Bool {
        repoStates.values.contains { $0.isConnected }
    }

    var body: some View {
        HStack(spacing: 0) {
            repoRail
                .frame(width: 200)

            Divider()

            waveListColumn
                .frame(width: 340)

            Divider()

            terminalDetail
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(palette.background)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(palette.background)
        .sheet(isPresented: $isShowingCreate) {
            CreateWaveSheet(
                repos: repos,
                initialRepoPath: defaultCreatePath,
                onCreate: createWave,
                onCancel: { isShowingCreate = false }
            )
            .environment(\.palette, palette)
        }
        .task {
            await prepareConnectionIfNeeded()
            await refreshRepos()
            ensureRepoStates()
            await syncRepoStates()
            await startEventSubscription()
        }
        .onChange(of: portfolioService.repos.map(\.path)) { _, _ in
            Task {
                await prepareConnectionIfNeeded()
                await refreshRepos()
                ensureRepoStates()
                await syncRepoStates()
            }
        }
        .onChange(of: selection) { _, _ in
            // Drop a wave selection that no longer matches the active repo filter.
            if let id = selectedWaveId, !filteredWaves.contains(where: { $0.id == id }) {
                selectedWaveId = nil
            }
        }
        .onDisappear {
            guard let service = eventService else { return }
            Task { await service.disconnect() }
            eventService = nil
        }
    }

    // MARK: - Repo rail (burgundy)

    private var repoRail: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Repos")
                .font(Typography.caption(9))
                .fontWeight(.medium)
                .foregroundStyle(.white.opacity(0.25))
                .tracking(0.5)
                .padding(.horizontal, Spacing.lg)
                .padding(.top, Spacing.lg)
                .padding(.bottom, Spacing.sm)

            ScrollView {
                LazyVStack(alignment: .leading, spacing: Spacing.xxs) {
                    repoRow(
                        title: "All Repos",
                        icon: "square.stack.3d.up",
                        filter: .all
                    )

                    ForEach(repos) { repo in
                        repoRow(
                            title: repo.displayName,
                            icon: "folder",
                            filter: .repo(repo.path)
                        )
                    }
                }
                .padding(.horizontal, Spacing.sm)
            }

            Spacer(minLength: 0)
        }
        .frame(maxHeight: .infinity, alignment: .top)
        .background(Color.loopflowBurgundy)
        .accessibilityIdentifier("repo-sidebar")
    }

    private func repoRow(title: String, icon: String, filter: RepoFilter) -> some View {
        let isSelected = selection == filter
        return Button {
            selection = filter
        } label: {
            HStack(spacing: Spacing.sm) {
                Image(systemName: icon)
                    .font(Typography.caption())
                    .foregroundStyle(.white.opacity(isSelected ? 0.9 : 0.45))
                    .frame(width: 16)
                Text(title)
                    .font(Typography.body(13))
                    .foregroundStyle(.white.opacity(isSelected ? 1 : 0.7))
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, Spacing.md)
            .padding(.vertical, Spacing.sm)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .fill(isSelected ? Color.white.opacity(0.14) : .clear)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    // MARK: - Wave list (burgundy)

    private var waveListColumn: some View {
        VStack(alignment: .leading, spacing: 0) {
            waveListHeader

            if filteredWaves.isEmpty {
                emptyWaveState
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: Spacing.xs) {
                        ForEach(filteredWaves) { wave in
                            WaveRow(
                                wave: wave,
                                isSelected: selectedWaveId == wave.id,
                                onSelect: { selectedWaveId = wave.id }
                            )
                        }
                    }
                    .padding(.horizontal, Spacing.sm)
                    .padding(.top, Spacing.xs)
                }
                .accessibilityIdentifier("repo-wave-list")
            }
        }
        .frame(maxHeight: .infinity, alignment: .top)
        .background(Color.loopflowBurgundy)
    }

    private var waveListHeader: some View {
        HStack(alignment: .top, spacing: Spacing.md) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Waves")
                    .font(Typography.caption())
                    .fontWeight(.medium)
                    .foregroundStyle(.white.opacity(0.7))

                Text(selectionSubtitle)
                    .font(Typography.caption(10))
                    .foregroundStyle(.white.opacity(0.45))
                    .lineLimit(1)
                    .truncationMode(.middle)
            }

            Spacer()

            Circle()
                .fill(isAnyConnected ? Color.statusSuccess : Color.white.opacity(0.3))
                .frame(width: 6, height: 6)
                .help(isAnyConnected ? "Connected" : "Connecting…")

            Button {
                isShowingCreate = true
            } label: {
                Image(systemName: "plus")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(repos.isEmpty ? .white.opacity(0.35) : .white)
            }
            .buttonStyle(.plain)
            .disabled(repos.isEmpty)
            .help("New wave")
            .accessibilityIdentifier("new-wave-button")
        }
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.md)
    }

    private var emptyWaveState: some View {
        VStack(spacing: Spacing.md) {
            Spacer()
            Text(repos.isEmpty ? "No repos in ~/src" : "No waves")
                .font(Typography.caption())
                .foregroundStyle(.white.opacity(0.5))
            if !repos.isEmpty {
                Button {
                    isShowingCreate = true
                } label: {
                    Label("New wave", systemImage: "plus")
                        .font(Typography.caption())
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
                .accessibilityIdentifier("wave-empty-create")
            }
            Spacer()
        }
        .frame(maxWidth: .infinity)
        .padding()
    }

    // MARK: - Terminal detail

    @ViewBuilder
    private var terminalDetail: some View {
        if let wave = selectedWave, let state = repoState(for: wave) {
            WaveAgentTerminalPane(
                wave: wave,
                attach: { try await state.attachWaveAgent(waveId: $0) },
                onClose: { selectedWaveId = nil }
            )
            .id(wave.id)
        } else {
            VStack(spacing: Spacing.md) {
                Image(systemName: "terminal")
                    .font(Typography.heroTitle(28))
                    .foregroundStyle(palette.textSecondary.opacity(0.5))
                Text("Select a wave")
                    .font(Typography.sectionTitle())
                    .foregroundStyle(palette.text)
                Text("Its /goal agent opens here in an embedded terminal.")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func repoState(for wave: WaveViewModel) -> PortfolioRepoState? {
        repoStates[wave.repo.normalizedFilePath]
            ?? repoStates.values.first { state in
                state.waves.contains { $0.id == wave.id }
            }
    }

    // MARK: - Header helpers

    private var connectionSubtitle: String {
        switch connectionStore.mode {
        case .bundled:
            return "Bundled daemon"
        case .remote:
            return connectionStore.activeConnection.displayName
        }
    }

    private var selectionSubtitle: String {
        switch selection {
        case .all:
            return "All repos · \(connectionSubtitle)"
        case .repo(let path):
            let name = repos.first { $0.path.normalizedFilePath == path.normalizedFilePath }?.displayName
            return "\(name ?? path) · \(connectionSubtitle)"
        }
    }

    private var defaultCreatePath: String? {
        if case .repo(let path) = selection {
            return path
        }
        return repos.first?.path
    }

    // MARK: - Data

    private func createWave(repoPath: String, name: String) async throws {
        ensureRepoStates()
        guard let state = repoStates[repoPath] else {
            throw WaveServiceError.commandFailed("Unknown repo: \(repoPath)")
        }
        try await state.createWave(name: name)
    }

    /// Seed the registry from a `~/src` scan on first empty launch, then collapse
    /// every registry entry to its main worktree and dedupe. Runs the git/FS work
    /// off the main thread.
    private func refreshRepos() async {
        if RepoState.uiTestMode() != nil {
            repos = portfolioService.repos
            return
        }

        if portfolioService.repos.isEmpty && !didSeedFromScan {
            didSeedFromScan = true
            let scanned = await Task.detached { RepoScanner().scanDefaultRoot() }.value
            for url in scanned {
                portfolioService.addRepo(url)
            }
        }

        let entries = portfolioService.repos
        repos = await Task.detached {
            let scanner = RepoScanner()
            var seen = Set<String>()
            var collapsed: [PortfolioRepo] = []
            for entry in entries {
                let mainPath = scanner.resolveMainWorktree(entry.url).normalizedFilePath
                guard !seen.contains(mainPath) else { continue }
                seen.insert(mainPath)
                collapsed.append(PortfolioRepo(path: mainPath, lastOpened: entry.lastOpened))
            }
            return collapsed
        }.value
    }

    private func ensureRepoStates() {
        let desiredPaths = Set(repos.map(\.path))

        for stalePath in repoStates.keys where !desiredPaths.contains(stalePath) {
            repoStates.removeValue(forKey: stalePath)
        }

        let connection = connectionStore.activeConnection
        let token = connectionStore.token(for: connection)

        for repo in repos where repoStates[repo.path] == nil {
            repoStates[repo.path] = PortfolioRepoState(repo: repo, connection: connection, token: token)
        }
    }

    private func prepareConnectionIfNeeded() async {
        // UI tests run against mock data; never start a daemon or reach a remote lfd.
        if RepoState.uiTestMode() != nil { return }
        guard connectionStore.mode == .bundled else { return }
        if let current = SharedDaemon.currentConnection {
            connectionStore.setBundledRuntimeConnection(current)
            return
        }
        if let connection = try? await SharedDaemon.manager.start() {
            connectionStore.setBundledRuntimeConnection(connection)
        }
    }

    private func syncRepoStates() async {
        if RepoState.uiTestMode() != nil { return }
        if connectionStore.mode == .bundled, SharedDaemon.currentConnection == nil {
            return
        }
        ensureRepoStates()

        await withTaskGroup(of: Void.self) { group in
            for state in repoStates.values {
                group.addTask { await state.refresh() }
            }
        }
    }

    private func startEventSubscription() async {
        guard eventService == nil else { return }
        if RepoState.uiTestMode() != nil { return }
        if connectionStore.mode == .bundled, SharedDaemon.currentConnection == nil {
            return
        }

        let connection = connectionStore.activeConnection
        let token = connectionStore.token(for: connection)
        let service = EventService(connection: connection, tokenProvider: { token })
        eventService = service

        let waveService = WaveService(connection: connection, tokenProvider: { token })
        authProviderStore.bindService(waveService)
        Task { await authProviderStore.refresh() }

        await service.subscribe(
            onEvent: { event in
                Task { @MainActor in handleEvent(event) }
            },
            onConnectionStateChange: { state in
                Task { @MainActor in
                    for repoState in repoStates.values {
                        repoState.updateConnectionState(state)
                    }
                    await authProviderStore.handleConnectionState(state)
                }
            }
        )
    }

    private func handleEvent(_ event: LFDEvent) {
        switch event {
        case .connected(let connected):
            let grouped = Dictionary(grouping: connected.waves) {
                $0.repo.normalizedFilePath
            }
            for repo in repos {
                repoStates[repo.path]?.applyConnectedWaves(grouped[repo.path] ?? [])
            }

        case .wave(let waveEvent):
            guard let wave = waveEvent.wave else {
                Task {
                    for repoState in repoStates.values {
                        await repoState.refresh()
                    }
                }
                return
            }
            repoStates[wave.repo.normalizedFilePath]?.applyWaveEvent(waveEvent)

        case .auth(let authEvent):
            authProviderStore.handleEvent(authEvent)

        case .worktree, .agentStarted, .agentEnded, .output, .attention, .terminalSession, .secrets:
            break
        }
    }
}

/// Embedded terminal for a wave's /goal agent: ensures the agent is running,
/// resolves its tmux session, and attaches via GhosttyTerminalView. Mirrors the
/// proven attach flow in TerminalWorkspaceView's SessionTerminalSurface.
private struct WaveAgentTerminalPane: View {
    let wave: WaveViewModel
    let attach: (String) async throws -> SessionConnectionInfo
    let onClose: () -> Void

    @Environment(\.palette) private var palette
    @ObservedObject private var ghosttyManager = GhosttyManager.shared
    @State private var connectionInfo: SessionConnectionInfo?
    @State private var errorMessage: String?

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            terminal
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(LoopflowPalette.dark.background)
        }
        .task(id: wave.id) {
            guard connectionInfo == nil else { return }
            if RepoState.uiTestMode() != nil { return }
            do {
                connectionInfo = try await attach(wave.id)
                errorMessage = nil
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    private var header: some View {
        HStack(spacing: Spacing.sm) {
            Image(systemName: wave.statusIndicator.icon)
                .foregroundStyle(wave.statusIndicator.color)
            Text(wave.displayName)
                .font(Typography.sectionTitle())
                .foregroundStyle(palette.text)
            Text("/goal agent")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
            Spacer()
            Button {
                onClose()
            } label: {
                Image(systemName: "xmark")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            .buttonStyle(.plain)
            .help("Close terminal")
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.md)
    }

    @ViewBuilder
    private var terminal: some View {
        if let command = connectionInfo.map(TerminalAttachCommand.init) {
            GhosttyTerminalView(
                workingDirectory: command.workingDirectory,
                argv: command.argv,
                env: command.env,
                sessionId: "wave-agent-\(wave.id)",
                manager: ghosttyManager
            )
        } else if let errorMessage {
            ContentUnavailableView(
                "Terminal unavailable",
                systemImage: "exclamationmark.triangle",
                description: Text(errorMessage)
            )
        } else {
            ProgressView("Starting /goal agent…")
                .tint(.white)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

/// Minimal create-wave flow: pick a target repo, name the wave, submit. Creates
/// the wave against the repo's lfd via `PortfolioRepoState.createWave`.
private struct CreateWaveSheet: View {
    let repos: [PortfolioRepo]
    let initialRepoPath: String?
    let onCreate: (_ repoPath: String, _ name: String) async throws -> Void
    let onCancel: () -> Void

    @Environment(\.palette) private var palette

    @State private var selectedRepoPath: String = ""
    @State private var waveName = ""
    @State private var isCreating = false
    @State private var errorMessage: String?
    @FocusState private var isNameFocused: Bool

    private var trimmedName: String {
        waveName.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            Text("New wave")
                .font(Typography.sectionTitle(20))
                .foregroundStyle(palette.text)

            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("Repository")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)

                Picker("Repository", selection: $selectedRepoPath) {
                    ForEach(repos) { repo in
                        Text(repo.displayName).tag(repo.path)
                    }
                }
                .labelsHidden()
                .pickerStyle(.menu)
                .font(Typography.body())
                .tint(palette.accent)
                .padding(.horizontal, Spacing.md)
                .padding(.vertical, Spacing.sm)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(palette.surfaceMuted)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                .disabled(isCreating)
            }

            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("Wave name")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)

                TextField("Wave name", text: $waveName)
                    .textFieldStyle(.plain)
                    .font(Typography.body())
                    .foregroundStyle(palette.text)
                    .padding(Spacing.md)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                    .focused($isNameFocused)
                    .onSubmit(submit)
                    .disabled(isCreating)
            }

            if let errorMessage {
                Text(errorMessage)
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusError)
            }

            HStack(spacing: Spacing.md) {
                Spacer()

                Button("Cancel", action: onCancel)
                    .buttonStyle(.plain)
                    .foregroundStyle(palette.textSecondary)
                    .disabled(isCreating)

                Button {
                    submit()
                } label: {
                    if isCreating {
                        ProgressView().controlSize(.small)
                    } else {
                        Text("Create wave")
                    }
                }
                .buttonStyle(DarkButtonStyle())
                .disabled(isCreating || trimmedName.isEmpty || selectedRepoPath.isEmpty)
            }
        }
        .padding(Spacing.xl)
        .frame(width: 420)
        .background(palette.background)
        .onAppear {
            selectedRepoPath = initialRepoPath ?? repos.first?.path ?? ""
            isNameFocused = true
        }
    }

    private func submit() {
        guard !isCreating, !trimmedName.isEmpty, !selectedRepoPath.isEmpty else { return }
        isCreating = true
        errorMessage = nil
        let repoPath = selectedRepoPath
        let name = trimmedName

        Task {
            defer { isCreating = false }
            do {
                try await onCreate(repoPath, name)
                onCancel()
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}

#Preview {
    WavesView(portfolioService: PortfolioService())
}
