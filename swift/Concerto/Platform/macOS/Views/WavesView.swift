// The exposed main window: a burgundy repo rail (left) filtering a burgundy wave
// list (center), with a "+" new-wave launcher, and a WaveChat detail (right) that
// streams the selected wave's live conversation from its `lf wave` chat server.
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

private struct AuthoredWaveSnapshot: Hashable {
    let name: String
    let status: WaveStatus
}

struct WavesView: View {
    let portfolioService: PortfolioService

    /// A repo to pre-select on appear (from `--repo`, a deep link, or the repo
    /// window). Collapsed to its main worktree for reads — the on-disk `wave/`
    /// dir holds quick-launch templates that live on main by design, and lfd owns
    /// the real waves. The `CONCERTO_DEV_WAVE_REPO` dev override reads the launched
    /// checkout AS-IS instead (see `resolveLaunchRepo`).
    var initialRepoPath: String? = nil

    @Environment(\.palette) private var palette

    @State private var connectionStore = ConnectionStore()
    @State private var authProviderStore = AuthProviderStore()
    @State private var repoStates: [String: PortfolioRepoState] = [:]
    @State private var eventService: EventService?

    /// Repos shown in the rail: a live `~/src` scan of main (non-worktree) repos.
    /// Derived off the main thread in `refreshRepos`.
    @State private var repos: [PortfolioRepo] = []

    /// Authored waves discovered on disk per repo path: `<repo>/wave/<name>/GOAL.md`.
    /// Merged with lfd's running waves so not-yet-launched waves are still listed.
    @State private var authoredWavesByRepo: [String: [AuthoredWaveSnapshot]] = [:]

    @State private var selection: RepoFilter = .all
    @State private var selectedWaveId: String?
    @State private var isShowingCreate = false
    @State private var didApplyInitialRepo = false

    /// Synthetic id prefix for an authored-on-disk wave that lfd hasn't created yet.
    private static let authoredIdPrefix = "authored:"

    /// All waves across every repo: lfd's running waves merged with waves authored
    /// on disk that lfd hasn't created yet.
    private var allWaves: [WaveViewModel] {
        repos.flatMap { mergedWaves(for: $0) }
    }

    /// lfd's live waves for a repo, plus idle placeholders for any authored wave
    /// (a `<repo>/wave/<name>/GOAL.md` on disk) that isn't already live. Sorted
    /// running-first, then by name.
    private func mergedWaves(for repo: PortfolioRepo) -> [WaveViewModel] {
        let live = repoStates[repo.path]?.waves ?? []
        let liveNames = Set(live.map(\.name))
        let placeholders = (authoredWavesByRepo[repo.path] ?? [])
            .filter { !liveNames.contains($0.name) }
            .map { authoredPlaceholder(repoPath: repo.path, snapshot: $0) }
        return (live + placeholders).sorted { lhs, rhs in
            let lp = Self.statusPriority(lhs.status)
            let rp = Self.statusPriority(rhs.status)
            if lp != rp { return lp < rp }
            return lhs.displayName.localizedCaseInsensitiveCompare(rhs.displayName) == .orderedAscending
        }
    }

    /// An idle, not-yet-launched row for a wave authored on disk. Its id carries the
    /// `authoredIdPrefix` so the terminal pane knows to create-and-run on launch.
    private func authoredPlaceholder(repoPath: String, snapshot: AuthoredWaveSnapshot) -> WaveViewModel {
        WaveViewModel(api: Wave(
            id: "\(Self.authoredIdPrefix)\(repoPath)#\(snapshot.name)",
            name: snapshot.name,
            repo: repoPath,
            status: snapshot.status
        ))
    }

    private static func statusPriority(_ status: WaveStatus) -> Int {
        switch status {
        case .running: 0
        case .waiting: 1
        case .failed: 2
        case .paused: 3
        case .idle: 4
        }
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

            waveDetail
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
            let initialMain = await registerInitialRepoIfNeeded()
            await refreshRepos()
            await refreshAuthoredWaves()
            if let initialMain, !didApplyInitialRepo {
                didApplyInitialRepo = true
                selection = .repo(initialMain)
            }
            ensureRepoStates()
            await syncRepoStates()
            await startEventSubscription()
        }
        .onChange(of: portfolioService.repos.map(\.path)) { _, _ in
            Task {
                await prepareConnectionIfNeeded()
                await refreshRepos()
                await refreshAuthoredWaves()
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
                Text(headerTitle)
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

    // MARK: - Wave detail (WaveChat)

    @ViewBuilder
    private var waveDetail: some View {
        if let wave = selectedWave {
            WaveDetailPane(
                wave: wave,
                repoPath: waveRepoPath(for: wave),
                onClose: { selectedWaveId = nil }
            )
            .id(wave.id)
        } else {
            VStack(spacing: Spacing.md) {
                Image(systemName: "bubble.left.and.bubble.right")
                    .font(Typography.heroTitle(28))
                    .foregroundStyle(palette.textSecondary.opacity(0.5))
                Text("Select a wave")
                    .font(Typography.sectionTitle())
                    .foregroundStyle(palette.text)
                Text("Its live conversation opens here.")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    /// On-disk repo root for the wave, where its `wave/<name>/.wave-endpoint`
    /// discovery pointer lives.
    private func waveRepoPath(for wave: WaveViewModel) -> String {
        repoState(for: wave)?.repo.path ?? wave.repo
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

    /// The surface is named after the repo you're in (its GitHub/dir name),
    /// e.g. "loopflow" — not a generic "Waves".
    private var headerTitle: String {
        switch selection {
        case .all:
            return "All Repos"
        case .repo(let path):
            return repos.first { $0.path.normalizedFilePath == path.normalizedFilePath }?.displayName
                ?? URL(fileURLWithPath: path).lastPathComponent
        }
    }

    private var selectionSubtitle: String {
        connectionSubtitle
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

    /// Register the launch-provided repo so it shows in the rail, and return its
    /// read path for pre-selection. Production collapses to the main worktree; the
    /// `CONCERTO_DEV_WAVE_REPO` dev override reads the launched checkout AS-IS.
    private func registerInitialRepoIfNeeded() async -> String? {
        guard let initialRepoPath, !didApplyInitialRepo else { return nil }
        if RepoState.uiTestMode() != nil { return nil }
        let readPath = await Task.detached {
            Self.resolveLaunchRepo(initialRepoPath).path
        }.value
        if !portfolioService.repos.contains(where: { $0.path.normalizedFilePath == readPath }) {
            portfolioService.addRepo(URL(fileURLWithPath: readPath))
        }
        return readPath
    }

    /// Resolve the launch-provided repo into its (read path, main path, rail label).
    /// Production reads the collapsed main worktree — the on-disk `wave/` dir is
    /// quick-launch templates that live on main by design, and lfd is the authority
    /// for real waves. The `CONCERTO_DEV_WAVE_REPO` dev override (set by concerto-dev
    /// on `run` / `run-debug`) instead reads the launched checkout AS-IS, so a dev
    /// launch enumerates its own worktree's waves. The rail label is always the
    /// collapsed main-repo name, so the rail stays clean and worktree-free.
    private nonisolated static func resolveLaunchRepo(_ initialPath: String) -> (path: String, mainPath: String, displayName: String) {
        let scanner = RepoScanner()
        let dev = ProcessInfo.processInfo.environment["CONCERTO_DEV_WAVE_REPO"]
        let devOverride = (dev?.isEmpty == false) ? dev : nil
        let readURL = URL(fileURLWithPath: devOverride ?? initialPath)
        let mainURL = scanner.resolveMainWorktree(readURL)
        let readPath = devOverride != nil ? readURL.normalizedFilePath : mainURL.normalizedFilePath
        return (readPath, mainURL.normalizedFilePath, mainURL.lastPathComponent)
    }

    /// Source the rail directly from a `~/src` scan of main (non-worktree) repos,
    /// every time. A launch-provided `initialRepoPath` is merged in via
    /// `resolveLaunchRepo`: production reads its collapsed main worktree; the
    /// `CONCERTO_DEV_WAVE_REPO` dev override reads the launched checkout AS-IS
    /// (worktree included) as a single row labeled with the main-repo name.
    /// Runs the git/FS work off the main thread.
    private func refreshRepos() async {
        if RepoState.uiTestMode() != nil {
            repos = portfolioService.repos
            return
        }

        let initialPath = initialRepoPath
        repos = await Task.detached {
            let scanner = RepoScanner()
            var seen = Set<String>()
            var result: [PortfolioRepo] = []
            for url in scanner.scanDefaultRoot() {
                let path = url.normalizedFilePath
                guard seen.insert(path).inserted else { continue }
                result.append(PortfolioRepo(path: path, lastOpened: Date()))
            }
            if let initialPath {
                let launch = Self.resolveLaunchRepo(initialPath)
                if launch.path == launch.mainPath {
                    // Production (or a launched main repo): read main. Keep the
                    // scanned row in place; add it only if the scan missed it.
                    if seen.insert(launch.path).inserted {
                        result.append(PortfolioRepo(
                            path: launch.path,
                            lastOpened: Date(),
                            displayNameOverride: launch.displayName
                        ))
                    }
                } else {
                    // Dev override: the launched worktree stands in for its main
                    // repo. Drop the scanned main row so the worktree is the sole
                    // row (reads its own wave/ dir + lfd), labeled with the main name.
                    result.removeAll { $0.path == launch.mainPath || $0.path == launch.path }
                    result.append(PortfolioRepo(
                        path: launch.path,
                        lastOpened: Date(),
                        displayNameOverride: launch.displayName
                    ))
                }
            }
            return result
        }.value
    }

    /// Enumerate each rail repo's authored waves — `<repo>/wave/<name>/GOAL.md`
    /// directories — off the main thread, so the list can offer them as launchable
    /// rows before lfd has created them.
    private func refreshAuthoredWaves() async {
        if RepoState.uiTestMode() != nil { return }
        let paths = repos.map(\.path)
        authoredWavesByRepo = await Task.detached {
            var result: [String: [AuthoredWaveSnapshot]] = [:]
            for path in paths {
                result[path] = Self.authoredWaves(inRepo: path)
            }
            return result
        }.value
    }

    /// Wave names authored on disk at `<repo>/wave/<name>/GOAL.md`, sorted.
    private nonisolated static func authoredWaves(inRepo repoPath: String) -> [AuthoredWaveSnapshot] {
        let waveDir = URL(fileURLWithPath: repoPath).appendingPathComponent("wave", isDirectory: true)
        let fm = FileManager.default
        guard let children = try? fm.contentsOfDirectory(
            at: waveDir,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        ) else {
            return []
        }
        return children
            .filter { url in
                var isDir: ObjCBool = false
                let goal = url.appendingPathComponent("GOAL.md")
                return fm.fileExists(atPath: goal.path, isDirectory: &isDir) && !isDir.boolValue
            }
            .map { url in
                let name = url.lastPathComponent
                let status: WaveStatus = PortfolioRepoState.waveAgentSessionExists(repoPath: repoPath, waveName: name)
                    ? .running
                    : .idle
                return AuthoredWaveSnapshot(name: name, status: status)
            }
            .sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
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
            for repo in repos {
                repoStates[repo.path]?.applyConnectedWaves(connected.waves)
            }

        case .wave(let waveEvent):
            guard waveEvent.wave != nil else {
                Task {
                    for repoState in repoStates.values {
                        await repoState.refresh()
                    }
                }
                return
            }
            for repoState in repoStates.values {
                repoState.applyWaveEvent(waveEvent)
            }

        case .auth(let authEvent):
            authProviderStore.handleEvent(authEvent)

        case .worktree, .agentStarted, .agentEnded, .output, .attention, .terminalSession, .secrets:
            break
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
