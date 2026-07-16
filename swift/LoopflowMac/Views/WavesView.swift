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
    /// have not been served yet. One stable alphabetical list — the row's lens
    /// carries state, so rows never reorder as processes start and stop.
    private func mergedWaves(for repo: PortfolioRepo) -> [WaveViewModel] {
        let live = repoStates[repo.path]?.waves ?? []
        let liveNames = Set(live.map(\.name))
        let placeholders = (authoredWavesByRepo[repo.path] ?? [])
            .filter { !liveNames.contains($0.name) }
            .map { authoredPlaceholder(repoPath: repo.path, snapshot: $0) }
        return (live + placeholders).sorted {
            $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending
        }
    }

    /// The filtered Waves arranged as an outline: roots alphabetical, each parent
    /// immediately followed by its children (indented). Today's Waves are flat,
    /// but a future child Wave — a Project promoted to residency — indents under
    /// its parent with the same lens and selection behavior.
    private var outlinedWaves: [(wave: WaveViewModel, indent: Int)] {
        let waves = filteredWaves
        let byId = Dictionary(waves.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
        func isRoot(_ w: WaveViewModel) -> Bool {
            guard let parent = w.parentWaveId else { return true }
            return byId[parent] == nil
        }
        let childrenByParent = Dictionary(
            grouping: waves.filter { !isRoot($0) },
            by: { $0.parentWaveId ?? "" }
        )
        var result: [(WaveViewModel, Int)] = []
        func emit(_ wave: WaveViewModel, _ indent: Int) {
            result.append((wave, indent))
            let children = (childrenByParent[wave.id] ?? [])
                .sorted { $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending }
            for child in children { emit(child, indent + 1) }
        }
        for root in waves.filter(isRoot) { emit(root, 0) }
        return result
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
            waveListColumn
                .frame(width: 300)

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

    // MARK: - Wave list (burgundy)

    private var waveListColumn: some View {
        VStack(alignment: .leading, spacing: 0) {
            waveListHeader

            if filteredWaves.isEmpty {
                emptyWaveState
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: Spacing.xs) {
                        ForEach(outlinedWaves, id: \.wave.id) { entry in
                            WaveRow(
                                wave: entry.wave,
                                isSelected: selectedWaveId == waveSelectionId(entry.wave),
                                onSelect: { selectedWaveId = waveSelectionId(entry.wave) },
                                indentLevel: entry.indent
                            )
                        }
                    }
                    .padding(.horizontal, Spacing.sm)
                    .padding(.top, Spacing.xs)
                }
                .accessibilityIdentifier("repo-wave-list")
            }

            Spacer(minLength: 0)
        }
        .frame(maxHeight: .infinity, alignment: .top)
        .background(Color.loopflowBurgundy)
    }

    /// Repository scope as a dropdown above the Wave list, plus New. The rail is
    /// gone: repo selection is a scope control, not a full-height column.
    private var waveListHeader: some View {
        HStack(spacing: Spacing.sm) {
            repoDropdown

            Spacer(minLength: 0)

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

    private var repoDropdown: some View {
        Menu {
            Button { selection = .all } label: {
                Label("All Repos", systemImage: "square.stack.3d.up")
            }
            if !repos.isEmpty { Divider() }
            ForEach(repos) { repo in
                Button { selection = .repo(repo.path) } label: {
                    Label(repo.displayName, systemImage: "folder")
                }
            }
        } label: {
            HStack(spacing: Spacing.xs) {
                Text(headerTitle)
                    .font(Typography.sectionTitle(16))
                    .foregroundStyle(.white)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Image(systemName: "chevron.down")
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundStyle(.white.opacity(0.6))
            }
            .contentShape(Rectangle())
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
        .accessibilityIdentifier("repo-dropdown")
        .accessibilityLabel("Repository: \(headerTitle)")
    }

    private var emptyWaveState: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(repos.isEmpty ? "No repositories found." : "No waves yet.")
                .font(Typography.caption())
                .foregroundStyle(.white.opacity(0.55))
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
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.md)
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

    /// The dropdown label: "All Repos", or the selected repo's name (its
    /// GitHub/dir name, e.g. "loopflow").
    private var headerTitle: String {
        switch selection {
        case .all:
            return "All Repos"
        case .repo(let path):
            return repos.first { $0.path.normalizedFilePath == path.normalizedFilePath }?.displayName
                ?? URL(fileURLWithPath: path).lastPathComponent
        }
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
        if AppTestMode.shouldBypassRegistry { return nil }
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
        if AppTestMode.shouldBypassRegistry {
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
        if AppTestMode.shouldBypassRegistry { return }
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
        if AppTestMode.current() == .mockWaves {
            seedMockWaves()
            return
        }
        if AppTestMode.shouldBypassRegistry { return }
        ensureRepoStates()

        do {
            let waves = try await RegistryQueryLocal.shared.allWaves()
            let plans = await buildWavePlanCache(registryWaves: waves)
            plansByWaveKey = plans
            for state in repoStates.values {
                state.applyConnectedWaves(waves, plans: plans)
            }
            selectRequestedWaveIfNeeded()
        } catch {
            for state in repoStates.values {
                state.markRefreshFailed()
            }
        }
    }

    /// Seed the populated Wave surface from `MockWaveFixture`, bypassing the
    /// registry, so the `mock-waves` UI-test mode renders the stable list lenses
    /// and auto-selects a Wave into its full detail hierarchy. Idempotent.
    private func seedMockWaves() {
        let repo = PortfolioRepo(path: MockWaveFixture.repoPath, lastOpened: Date())
        if !portfolioService.repos.contains(where: { $0.path == repo.path }) {
            portfolioService.addRepo(repo.url)
        }
        repos = [repo]
        let state = repoStates[repo.path]
            ?? PortfolioRepoState(repo: repo, registryQuery: RegistryQueryLocal.shared)
        state.applyConnectedWaves(MockWaveFixture.waves, plans: MockWaveFixture.plans)
        repoStates[repo.path] = state
        if selectedWaveId == nil,
           let selected = state.waves.first(where: { $0.name == MockWaveFixture.selectedWaveName }) {
            selectedWaveId = waveSelectionId(selected)
        }
        // Drive the sheet-presentation leg of the AttributeGraph zero-cycle
        // matrix headlessly: once the populated detail hierarchy has settled,
        // raise the create sheet over it — a genuine mid-session presentation,
        // not a cold-launch-with-sheet. Gated so the plain mock render stays
        // sheet-free. See scripts/check_attributegraph_cycles.sh --sheet.
        if ProcessInfo.processInfo.environment["LOOPFLOW_UI_TEST_PRESENT_SHEET"] != nil,
           !isShowingCreate {
            Task { @MainActor in
                try? await Task.sleep(nanoseconds: 1_500_000_000)
                isShowingCreate = true
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
        if AppTestMode.shouldBypassRegistry { return }
        while !Task.isCancelled {
            try? await Task.sleep(for: .seconds(5))
            if Task.isCancelled { return }
            await syncRepoStates()
            await refreshAuthoredWaves()
        }
    }

    /// A live screenshot can request one real Wave after the registry resolves.
    /// Fixture selection stays in `seedMockWaves`; production has no requested
    /// branch, so this is inert outside the explicit `live` harness.
    private func selectRequestedWaveIfNeeded() {
        guard selectedWaveId == nil,
              let requested = AppTestMode.selectBranch,
              let selected = allWaves.first(where: { $0.name == requested })
        else { return }
        selectedWaveId = waveSelectionId(selected)
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
