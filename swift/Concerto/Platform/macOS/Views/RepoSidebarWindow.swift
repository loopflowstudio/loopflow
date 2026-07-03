// Repo sidebar + wave list — the blank-slate exposed surface.
//
// Left: the repo registry (PortfolioService.repos) plus an "All" entry.
// Center: every active wave, filtered to the selected repo. Rows are
// display-only in this slice — they prove the sidebar-filters-list shape.

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

    private var repos: [PortfolioRepo] {
        portfolioService.repos
    }

    /// All waves across every repo, in the registry's repo order.
    private var allWaves: [WaveViewModel] {
        repos.flatMap { repoStates[$0.path]?.waves ?? [] }
    }

    private var filteredWaves: [WaveViewModel] {
        wavesMatching(selection, in: allWaves)
    }

    var body: some View {
        NavigationSplitView {
            sidebar
        } detail: {
            detail
        }
        .background(palette.background)
        .task {
            await prepareConnectionIfNeeded()
            ensureRepoStates()
            await syncRepoStates()
        }
        .onChange(of: repos.map(\.path)) { _, _ in
            Task {
                await prepareConnectionIfNeeded()
                ensureRepoStates()
                await syncRepoStates()
            }
        }
    }

    // MARK: - Sidebar

    private var sidebar: some View {
        List(selection: $selection) {
            Label("All Repos", systemImage: "square.stack.3d.up")
                .tag(RepoFilter.all)

            Section("Repos") {
                ForEach(repos) { repo in
                    Label(repo.displayName, systemImage: "folder")
                        .tag(RepoFilter.repo(repo.path))
                }
            }
        }
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
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.lg)
    }

    private var selectionSubtitle: String {
        switch selection {
        case .all:
            return "All repos"
        case .repo(let path):
            return repoChipText(for: path)
        }
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

#Preview {
    RepoSidebarWindow(portfolioService: PortfolioService())
}
