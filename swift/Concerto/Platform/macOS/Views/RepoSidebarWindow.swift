// Repo sidebar + wave list — the blank-slate exposed surface.
//
// Left: the repo registry (collapsed to main worktrees) plus an "All" entry.
// Center: every active wave, filtered to the selected repo, with a "+" that
// creates a wave against the selected repo's lfd.

import SwiftUI
import LoopflowCore

enum RepoFilter: Hashable {
    case all
    case repo(String)
}

struct RepoSidebarWindow: View {
    let portfolioService: PortfolioService

    @Environment(\.palette) private var palette

    @State private var connectionStore = ConnectionStore()
    @State private var repoStates: [String: PortfolioRepoState] = [:]
    @State private var selection: RepoFilter = .all

    /// Repos shown in the sidebar: the persisted registry collapsed to main
    /// worktrees and deduped. Derived off the main thread in `refreshRepos`.
    @State private var repos: [PortfolioRepo] = []
    @State private var didSeedFromScan = false
    @State private var isShowingCreate = false

    /// All waves across every repo, in the registry's repo order.
    private var allWaves: [WaveViewModel] {
        repos.flatMap { repoStates[$0.path]?.waves ?? [] }
    }

    private var filteredWaves: [WaveViewModel] {
        wavesMatching(selection, in: allWaves)
    }

    private var isAnyConnected: Bool {
        repoStates.values.contains { $0.isConnected }
    }

    var body: some View {
        NavigationSplitView {
            sidebar
        } detail: {
            detail
        }
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
        }
        .onChange(of: portfolioService.repos.map(\.path)) { _, _ in
            Task {
                await prepareConnectionIfNeeded()
                await refreshRepos()
                ensureRepoStates()
                await syncRepoStates()
            }
        }
    }

    // MARK: - Sidebar

    private var sidebar: some View {
        List(selection: $selection) {
            Label {
                Text("All Repos").font(Typography.body())
            } icon: {
                Image(systemName: "square.stack.3d.up")
            }
            .tag(RepoFilter.all)
            .listRowBackground(Color.clear)

            Section {
                ForEach(repos) { repo in
                    Label {
                        Text(repo.displayName).font(Typography.body())
                    } icon: {
                        Image(systemName: "folder")
                    }
                    .tag(RepoFilter.repo(repo.path))
                    .listRowBackground(Color.clear)
                }
            } header: {
                Text("Repos")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
        }
        .listStyle(.sidebar)
        .scrollContentBackground(.hidden)
        .background(palette.background)
        .tint(palette.accent)
        .navigationTitle("Loopflow")
        .navigationSplitViewColumnWidth(min: 200, ideal: 240)
        .accessibilityIdentifier("repo-sidebar")
    }

    // MARK: - Detail

    private var detail: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            Divider()

            if filteredWaves.isEmpty {
                emptyState
            } else {
                waveList
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(palette.background)
    }

    private var header: some View {
        HStack(alignment: .top, spacing: Spacing.md) {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("Waves")
                    .font(Typography.sectionTitle(24))
                    .foregroundStyle(palette.text)

                Text(selectionSubtitle)
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }

            Spacer()

            Circle()
                .fill(isAnyConnected ? Color.statusSuccess : palette.textSecondary.opacity(0.4))
                .frame(width: 7, height: 7)
                .help(isAnyConnected ? "Connected" : "Connecting…")

            Button {
                isShowingCreate = true
            } label: {
                Image(systemName: "plus")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(repos.isEmpty ? palette.textSecondary : palette.accent)
            }
            .buttonStyle(.plain)
            .disabled(repos.isEmpty)
            .help("New wave")
            .accessibilityIdentifier("new-wave-button")
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.lg)
    }

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
            return "\(repoChipText(for: path)) · \(connectionSubtitle)"
        }
    }

    private var defaultCreatePath: String? {
        if case .repo(let path) = selection {
            return path
        }
        return repos.first?.path
    }

    private var emptyState: some View {
        Text("No waves")
            .font(Typography.body())
            .foregroundStyle(palette.textSecondary)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var waveList: some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                ForEach(filteredWaves) { wave in
                    RepoSidebarWaveRow(wave: wave)
                    Divider()
                }
            }
            .padding(.horizontal, Spacing.xl)
            .padding(.top, Spacing.md)
        }
        .accessibilityIdentifier("repo-wave-list")
    }

    // MARK: - Data

    /// Repo filter predicate. Single-repo today (`Wave.repo`); when the model
    /// splits into `repos: [RepoWork]`, swap the `.repo` case body to
    /// `wave.repos.contains { $0.path.normalizedFilePath == path.normalizedFilePath }`.
    private func wavesMatching(_ filter: RepoFilter, in waves: [WaveViewModel]) -> [WaveViewModel] {
        switch filter {
        case .all:
            return waves
        case .repo(let path):
            let target = path.normalizedFilePath
            return waves.filter { $0.repo.normalizedFilePath == target }
        }
    }

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
}

private struct RepoSidebarWaveRow: View {
    let wave: WaveViewModel

    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.md) {
            Image(systemName: wave.status.icon)
                .foregroundStyle(wave.status.color)
                .frame(width: 16)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text(wave.displayName)
                    .font(Typography.body())
                    .fontWeight(.medium)
                    .foregroundStyle(palette.text)
                    .lineLimit(1)
                    .accessibilityIdentifier("wave-name")

                Text(wave.statusText)
                    .font(Typography.caption())
                    .foregroundStyle(wave.status.color)
                    .accessibilityIdentifier("wave-status")
            }

            Spacer()

            Text(repoChipText(for: wave.repo))
                .font(Typography.caption(11))
                .foregroundStyle(palette.textSecondary)
                .padding(.horizontal, Spacing.sm)
                .padding(.vertical, Spacing.xxs)
                .background(
                    RoundedRectangle(cornerRadius: CornerRadius.sm)
                        .fill(palette.text.opacity(0.06))
                )
                .accessibilityIdentifier("wave-repo-chip")
        }
        .padding(.vertical, Spacing.md)
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(wave.displayName), \(repoChipText(for: wave.repo)), \(wave.statusText)")
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
                .disabled(isCreating)
            }

            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("Wave name")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)

                TextField("Wave name", text: $waveName)
                    .textFieldStyle(.roundedBorder)
                    .font(Typography.body())
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
    RepoSidebarWindow(portfolioService: PortfolioService())
}
