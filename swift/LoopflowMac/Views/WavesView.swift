// Repository rail, Wave list, and the selected Wave's work map + conversation.
// Discovery is a periodic registry query (`lf ls`); live conversation and
// resident motion stream directly from that Wave's listener.

import SwiftUI
import Loopflow

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
    /// dir holds quick-launch templates that live on main by design. The
    /// `LOOPFLOW_DEV_WAVE_REPO` dev override reads the launched checkout AS-IS
    /// instead (see `resolveLaunchRepo`).
    var initialRepoPath: String? = nil

    @Environment(\.palette) private var palette

    @State private var repoStates: [String: PortfolioRepoState] = [:]

    /// Repos shown in the rail: a live `~/src` scan of main (non-worktree) repos.
    /// Derived off the main thread in `refreshRepos`.
    @State private var repos: [PortfolioRepo] = []

    /// Authored waves discovered on disk per repo path: `<repo>/wave/<name>/GOAL.md`.
    /// Merged with registered waves so not-yet-launched waves are still listed.
    @State private var authoredWavesByRepo: [String: [AuthoredWaveSnapshot]] = [:]
    @State private var plansByWaveKey: [String: WavePlan] = [:]

    @State private var selection: RepoFilter = .all
    @State private var selectedWaveId: String?
    @State private var isShowingCreate = false
    @State private var didApplyInitialRepo = false
    @State private var didRestoreStickyRepo = false

    /// Synthetic id prefix for an authored Wave without a registry row yet.
    private static let authoredIdPrefix = "authored:"

    /// All waves across every repo: registry rows merged with Waves authored on
    /// disk that have not been served yet.
    private var allWaves: [WaveViewModel] {
        repos.flatMap { mergedWaves(for: $0) }
    }

    /// Registered waves plus idle placeholders for authored GOAL.md files that
    /// have not been served yet. Running Waves sort first, then paused and idle.
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
        ), plan: plansByWaveKey[Self.wavePlanKey(repoPath: repoPath, waveName: snapshot.name)],
        isRegistered: false)
    }

    private static func statusPriority(_ status: WaveStatus) -> Int {
        switch status {
        case .running: 0
        case .paused: 1
        case .idle: 2
        }
    }

    private var filteredWaves: [WaveViewModel] {
        switch selection {
        case .all:
            return allWaves
        case .repo(let path):
            let target = WaveOrigin.resolve(path).normalizedFilePath
            return allWaves.filter {
                WaveOrigin.resolve($0.repo).normalizedFilePath == target
            }
        }
    }

    private var selectedWave: WaveViewModel? {
        guard let selectedWaveId else { return nil }
        return allWaves.first { waveSelectionId($0) == selectedWaveId }
    }

    private func waveSelectionId(_ wave: WaveViewModel) -> String {
        Self.wavePlanKey(
            repoPath: WaveOrigin.resolve(wave.repo),
            waveName: wave.name
        )
    }

    private var isAnyConnected: Bool {
        repoStates.values.contains { $0.isConnected }
    }

    var body: some View {
        HStack(spacing: 0) {
            repoRail
                .frame(width: 100)

            Divider()

            waveListColumn
                .frame(width: 400)

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
            let initialMain = await registerInitialRepoIfNeeded()
            await refreshRepos()
            await refreshAuthoredWaves()
            if let initialMain, !didApplyInitialRepo {
                didApplyInitialRepo = true
                selection = .repo(initialMain)
            } else {
                restoreStickyRepoSelectionIfNeeded()
            }
            ensureRepoStates()
            await syncRepoStates()
            await pollRegistry()
        }
        .onChange(of: portfolioService.repos.map(\.path)) { _, _ in
            Task {
                await refreshRepos()
                await refreshAuthoredWaves()
                ensureRepoStates()
                await syncRepoStates()
            }
        }
        .onChange(of: selection) { _, _ in
            // Drop a wave selection that no longer matches the active repo filter.
            if let id = selectedWaveId,
               !filteredWaves.contains(where: { waveSelectionId($0) == id }) {
                selectedWaveId = nil
            }
            persistRepoSelection()
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
                .padding(.horizontal, Spacing.sm)
                .padding(.top, Spacing.lg)
                .padding(.bottom, Spacing.sm)

            ScrollView {
                LazyVStack(alignment: .leading, spacing: Spacing.xxs) {
                    repoRow(
                        title: "All",
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
                    .frame(width: 14)
                Text(title)
                    .font(Typography.caption(11))
                    .foregroundStyle(.white.opacity(isSelected ? 1 : 0.7))
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, Spacing.xs)
            .padding(.vertical, Spacing.xs)
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
                                isSelected: selectedWaveId == waveSelectionId(wave),
                                onSelect: { selectedWaveId = waveSelectionId(wave) }
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
            .id(waveSelectionId(wave))
        } else {
            RoadmapView(
                repoPath: roadmapRepoPath,
                onOpenWave: openRoadmapWave
            )
        }
    }

    private var roadmapRepoPath: String? {
        if case .repo(let path) = selection { return path }
        return nil
    }

    private func openRoadmapWave(_ snapshot: WaveSnapshot) {
        guard let wave = allWaves.first(where: {
            $0.id == snapshot.id
                || ($0.name == snapshot.name
                    && WaveOrigin.resolve($0.repo).normalizedFilePath
                        == WaveOrigin.resolve(snapshot.repo).normalizedFilePath)
        }) else { return }
        selectedWaveId = waveSelectionId(wave)
    }

    /// On-disk repo root for the wave's state, where its
    /// `wave/<name>/.wave-endpoint` discovery pointer lives. Wave state is
    /// published at the ORIGIN repo, so a worktree rail path resolves here
    /// (memoized) — chat discovery, the launcher, and the tmux-attach hint all
    /// inherit the same origin path.
    private func waveRepoPath(for wave: WaveViewModel) -> String {
        WaveOrigin.resolve(repoState(for: wave)?.repo.path ?? wave.repo)
    }

    private func repoState(for wave: WaveViewModel) -> PortfolioRepoState? {
        repoStates[wave.repo.normalizedFilePath]
            ?? repoStates.values.first { state in
                state.waves.contains { $0.id == wave.id }
            }
    }

    // MARK: - Header helpers

    private var connectionSubtitle: String {
        "Local registry"
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

    private nonisolated static func wavePlanKey(repoPath: String, waveName: String) -> String {
        PortfolioRepoState.wavePlanKey(repoPath: repoPath, waveName: waveName)
    }

    private func createWave(repoPath: String, name: String) async throws {
        ensureRepoStates()
        guard let state = repoStates[repoPath] else {
            throw PortfolioRepoError.unknownRepo(repoPath)
        }
        try await state.createWave(name: name)
    }

    /// Register the launch-provided repo so it shows in the rail, and return its
    /// read path for pre-selection. Production collapses to the main worktree; the
    /// `LOOPFLOW_DEV_WAVE_REPO` dev override reads the launched checkout AS-IS.
    private func registerInitialRepoIfNeeded() async -> String? {
        guard let initialRepoPath, !didApplyInitialRepo else { return nil }
        if AppTestMode.current() != nil { return nil }
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
    /// quick-launch templates that live on main by design. The
    /// `LOOPFLOW_DEV_WAVE_REPO` dev override (set by loopflow-dev on `run` /
    /// `run-debug`) instead reads the launched checkout AS-IS, so a dev launch
    /// enumerates its own worktree's waves. The rail label is always the collapsed
    /// main-repo name, so the rail stays clean and worktree-free.
    private nonisolated static func resolveLaunchRepo(_ initialPath: String) -> (path: String, mainPath: String, displayName: String) {
        let scanner = RepoScanner()
        let dev = ProcessInfo.processInfo.environment["LOOPFLOW_DEV_WAVE_REPO"]
        let devOverride = (dev?.isEmpty == false) ? dev : nil
        let readURL = URL(fileURLWithPath: devOverride ?? initialPath)
        let mainURL = scanner.resolveMainWorktree(readURL)
        let readPath = devOverride != nil ? readURL.normalizedFilePath : mainURL.normalizedFilePath
        return (readPath, mainURL.normalizedFilePath, mainURL.lastPathComponent)
    }

    /// Source the rail directly from a `~/src` scan of main (non-worktree) repos,
    /// every time. A launch-provided `initialRepoPath` is merged in via
    /// `resolveLaunchRepo`: production reads its collapsed main worktree; the
    /// `LOOPFLOW_DEV_WAVE_REPO` dev override reads the launched checkout AS-IS
    /// (worktree included) as a single row labeled with the main-repo name.
    /// Runs the git/FS work off the main thread.
    private func refreshRepos() async {
        if AppTestMode.current() != nil {
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
                    // row (reads its own wave/ dir + registry), labeled with the main name.
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
    /// rows before they have a registry entry.
    private func refreshAuthoredWaves() async {
        if AppTestMode.current() != nil { return }
        let paths = repos.map(\.path)
        authoredWavesByRepo = await Task.detached {
            let liveSessions = LocalWaveAgentLauncher.tmuxSessionNames()
            var result: [String: [AuthoredWaveSnapshot]] = [:]
            for path in paths {
                result[path] = Self.authoredWaves(inRepo: path, liveTmuxSessionNames: liveSessions)
            }
            return result
        }.value
    }

    /// Wave names authored on disk at `<repo>/wave/<name>/GOAL.md`, sorted.
    private nonisolated static func authoredWaves(
        inRepo repoPath: String,
        liveTmuxSessionNames: Set<String>
    ) -> [AuthoredWaveSnapshot] {
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
                let sessionName = PortfolioRepoState.waveAgentSessionName(repoPath: repoPath, waveName: name)
                let status: WaveStatus = liveTmuxSessionNames.contains(sessionName)
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

        for repo in repos where repoStates[repo.path] == nil {
            repoStates[repo.path] = PortfolioRepoState(
                repo: repo,
                registryQuery: RegistryQueryLocal.shared
            )
        }
    }

    private func syncRepoStates() async {
        if AppTestMode.current() != nil { return }
        ensureRepoStates()

        do {
            let waves = try await RegistryQueryLocal.shared.allWaves()
            let plans = await buildWavePlanCache(registryWaves: waves)
            plansByWaveKey = plans
            for state in repoStates.values {
                state.applyConnectedWaves(waves, plans: plans)
            }
        } catch {
            for state in repoStates.values {
                state.markRefreshFailed()
            }
        }
    }

    private func buildWavePlanCache(registryWaves: [Wave]) async -> [String: WavePlan] {
        let authored = authoredWavesByRepo
        var targets: [String: (repoPath: String, waveName: String)] = [:]
        for wave in registryWaves {
            let key = Self.wavePlanKey(repoPath: wave.repo, waveName: wave.name)
            targets[key] = (wave.repo, wave.name)
        }
        for (repoPath, snapshots) in authored {
            for snapshot in snapshots {
                let key = Self.wavePlanKey(repoPath: repoPath, waveName: snapshot.name)
                targets[key] = (repoPath, snapshot.name)
            }
        }

        return await withTaskGroup(of: (String, WavePlan?).self) { group in
            for (key, target) in targets {
                group.addTask {
                    let objective = WavePlanParser.objective(
                        repoRoot: URL(fileURLWithPath: target.repoPath),
                        waveName: target.waveName
                    ) ?? ""
                    let plan = try? await RegistryQueryLocal.shared.plan(
                        wave: target.waveName,
                        objective: objective,
                        cwd: target.repoPath
                    )
                    return (key, plan ?? (objective.isEmpty ? nil : WavePlan(objective: objective)))
                }
            }
            var plans: [String: WavePlan] = [:]
            for await (key, plan) in group {
                plans[key] = plan
            }
            return plans
        }
    }

    private func restoreStickyRepoSelectionIfNeeded() {
        guard !didRestoreStickyRepo else { return }
        didRestoreStickyRepo = true
        guard let path = loadLoopflowState()?.selectedRepoPath?.normalizedFilePath else { return }
        guard repos.contains(where: { $0.path.normalizedFilePath == path }) else { return }
        selection = .repo(path)
    }

    private func persistRepoSelection() {
        let selectedRepoPath: String?
        switch selection {
        case .all:
            selectedRepoPath = nil
        case .repo(let path):
            selectedRepoPath = path.normalizedFilePath
        }

        Task.detached {
            try? saveLoopflowState(LoopflowState(selectedRepoPath: selectedRepoPath))
        }
    }

    /// Discovery has no stream: re-query the registry on a slow cadence so a
    /// wave that started, stopped, or advanced since the last read shows up.
    /// Each wave's live conversation + run motion is its own per-wave SSE,
    /// opened by the detail pane — not funnelled through a center.
    private func pollRegistry() async {
        if AppTestMode.current() != nil { return }
        while !Task.isCancelled {
            try? await Task.sleep(for: .seconds(5))
            if Task.isCancelled { return }
            await syncRepoStates()
            await refreshAuthoredWaves()
        }
    }
}


/// Minimal create-wave flow: pick a target repo, name the wave, submit. Creates
/// the Wave files through `PortfolioRepoState.createWave`.
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
