// Portfolio dashboard window showing repositories and waves.

import SwiftUI
import LoopflowCore

struct PortfolioWindow: View {
    let portfolioService: PortfolioService

    @Environment(\.openWindow) private var openWindow

    @State private var connectionStore = ConnectionStore()
    @State private var repoStates: [String: PortfolioRepoState] = [:]
    @State private var eventService: EventService?
    @State private var isShowingTypeahead = false

    private let columns = [GridItem(.adaptive(minimum: 280), spacing: Spacing.lg)]

    private var repoPaths: [String] {
        portfolioService.repos.map(\.path)
    }

    var body: some View {
        ZStack {
            ScrollView {
                LazyVGrid(columns: columns, alignment: .leading, spacing: Spacing.lg) {
                    ForEach(portfolioService.repos) { repo in
                        if let state = repoStates[repo.path] {
                            PortfolioRepoCard(
                                repoState: state,
                                onSelectWave: selectWave,
                                onOpenRepo: openRepo,
                                onRemoveRepo: removeRepo
                            )
                        }
                    }

                    AddRepoCard {
                        isShowingTypeahead = true
                    }
                }
                .padding(Spacing.xxl)
            }

            if isShowingTypeahead {
                Color.black.opacity(0.45)
                    .ignoresSafeArea()
                    .onTapGesture {
                        isShowingTypeahead = false
                    }

                RepoTypeahead(
                    excludedPaths: Set(repoPaths),
                    onSelect: { repoURL in
                        addRepo(repoURL)
                    },
                    onCancel: {
                        isShowingTypeahead = false
                    }
                )
                .frame(maxWidth: 560, maxHeight: 520)
                .padding(Spacing.xxl)
            }
        }
        .background(Color.loopflowBurgundy.ignoresSafeArea())
        .task {
            await bootstrap()
        }
        .onChange(of: repoPaths) { _, _ in
            Task {
                await syncRepoStates()
            }
        }
        .onDisappear {
            guard let service = eventService else { return }
            Task {
                await service.disconnect()
            }
            eventService = nil
        }
    }

    private func bootstrap() async {
        await syncRepoStates()
        await startEventSubscription()
    }

    private func syncRepoStates() async {
        let desiredRepos = portfolioService.repos
        let desiredPaths = Set(desiredRepos.map(\.path))

        let stalePaths = repoStates.keys.filter { !desiredPaths.contains($0) }
        for stalePath in stalePaths {
            repoStates.removeValue(forKey: stalePath)
        }

        let connection = connectionStore.activeConnection
        let token = connectionStore.token(for: connection)

        for repo in desiredRepos where repoStates[repo.path] == nil {
            let state = PortfolioRepoState(repo: repo, connection: connection, token: token)
            repoStates[repo.path] = state
            await state.connect()
        }
    }

    private func startEventSubscription() async {
        guard eventService == nil else { return }

        let connection = connectionStore.activeConnection
        let token = connectionStore.token(for: connection)
        let service = EventService(connection: connection, tokenProvider: { token })
        eventService = service

        await service.subscribe(
            onEvent: { event in
                Task { @MainActor in
                    handleEvent(event)
                }
            },
            onConnectionStateChange: { state in
                Task { @MainActor in
                    for repoState in repoStates.values {
                        repoState.updateConnectionState(state)
                    }
                }
            }
        )
    }

    private func handleEvent(_ event: LFDEvent) {
        switch event {
        case .connected(let connected):
            let grouped = Dictionary(grouping: connected.waves) {
                URL(fileURLWithPath: $0.repo).standardizedFileURL.path(percentEncoded: false)
            }

            for repo in portfolioService.repos {
                repoStates[repo.path]?.applyConnectedWaves(grouped[repo.path] ?? [])
            }

        case .wave(let waveEvent):
            if waveEvent.wave == nil {
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

        case .worktree, .agentStarted, .agentEnded, .output:
            break
        }
    }

    private func addRepo(_ repoURL: URL) {
        portfolioService.addRepo(repoURL)
        isShowingTypeahead = false
    }

    private func removeRepo(_ repoURL: URL) {
        portfolioService.removeRepo(repoURL)
    }

    private func openRepo(_ repoURL: URL) {
        portfolioService.addRepo(repoURL)
        openWindow(id: "repo", value: repoURL)
    }

    private func selectWave(_ waveId: String, _ repoURL: URL) {
        openRepo(repoURL)
        postWaveSelection(waveId: waveId, repoURL: repoURL)
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(400))
            postWaveSelection(waveId: waveId, repoURL: repoURL)
        }
    }

    private func postWaveSelection(waveId: String, repoURL: URL) {
        NotificationCenter.default.post(
            name: .selectPortfolioWave,
            object: nil,
            userInfo: [
                "waveId": waveId,
                "repoPath": repoURL.standardizedFileURL.path(percentEncoded: false),
            ]
        )
    }
}

struct PortfolioRepoCard: View {
    let repoState: PortfolioRepoState
    let onSelectWave: (String, URL) -> Void
    let onOpenRepo: (URL) -> Void
    var onRemoveRepo: ((URL) -> Void)? = nil

    private var visibleWaves: [WaveViewModel] {
        Array(repoState.waves.prefix(5))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Button {
                onOpenRepo(repoState.repo.url)
            } label: {
                HStack(spacing: Spacing.sm) {
                    Text(repoState.repo.displayName)
                        .font(Typography.sectionTitle(18))
                        .foregroundStyle(.white)
                        .lineLimit(1)

                    Spacer(minLength: 0)

                    Circle()
                        .fill(repoState.isConnected ? Color.statusSuccess : Color.white.opacity(0.35))
                        .frame(width: 8, height: 8)
                }
            }
            .buttonStyle(.plain)

            Text(summaryText)
                .font(Typography.caption())
                .foregroundStyle(.white.opacity(0.72))

            if repoState.isLoading {
                ProgressView()
                    .tint(.white)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.vertical, Spacing.md)
            } else if visibleWaves.isEmpty {
                Text("No waves")
                    .font(Typography.caption())
                    .foregroundStyle(.white.opacity(0.55))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.vertical, Spacing.sm)
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: Spacing.xs) {
                        ForEach(visibleWaves) { wave in
                            waveRow(wave)
                        }

                        if repoState.waves.count > visibleWaves.count {
                            Text("\(repoState.waves.count - visibleWaves.count) more waves")
                                .font(Typography.caption(11))
                                .foregroundStyle(.white.opacity(0.45))
                                .padding(.top, Spacing.xs)
                        }
                    }
                }
                .frame(maxHeight: 180)
            }
        }
        .padding(Spacing.lg)
        .frame(maxWidth: .infinity, minHeight: 220, alignment: .topLeading)
        .background(
            RoundedRectangle(cornerRadius: CornerRadius.lg)
                .fill(Color.black.opacity(repoState.needsAttention ? 0.3 : 0.2))
                .overlay(
                    RoundedRectangle(cornerRadius: CornerRadius.lg)
                        .stroke(Color.white.opacity(0.12), lineWidth: 1)
                )
        )
        .contextMenu {
            if let onRemoveRepo {
                Button(role: .destructive) {
                    onRemoveRepo(repoState.repo.url)
                } label: {
                    Label("Remove from portfolio", systemImage: "trash")
                }
            }
        }
    }

    private var summaryText: String {
        "\(repoState.waves.count) waves · \(repoState.blockedCount) blocked · +\(repoState.totalInsertions) -\(repoState.totalDeletions)"
    }

    @ViewBuilder
    private func waveRow(_ wave: WaveViewModel) -> some View {
        Button {
            onSelectWave(wave.id, repoState.repo.url)
        } label: {
            HStack(spacing: Spacing.sm) {
                Circle()
                    .fill(wave.statusIndicator.color)
                    .frame(width: 8, height: 8)

                Text(wave.displayName)
                    .font(Typography.body(13))
                    .foregroundStyle(.white)
                    .lineLimit(1)

                Spacer(minLength: Spacing.xs)

                if let diff = repoState.diffSummary(for: wave) {
                    Text(diff)
                        .font(Typography.code(11))
                        .foregroundStyle(.white.opacity(0.75))
                }

                if wave.pendingPR != nil {
                    Text("PR")
                        .font(Typography.caption(10))
                        .foregroundStyle(Color.statusSuccess)
                        .padding(.horizontal, 5)
                        .padding(.vertical, 1)
                        .background(Color.statusSuccess.opacity(0.2))
                        .clipShape(Capsule())
                }
            }
            .padding(.horizontal, Spacing.sm)
            .padding(.vertical, Spacing.xs)
            .contentShape(Rectangle())
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.sm)
                    .fill(Color.white.opacity(0.06))
            )
        }
        .buttonStyle(.plain)
    }
}

private struct AddRepoCard: View {
    let onAdd: () -> Void

    var body: some View {
        Button(action: onAdd) {
            VStack(spacing: Spacing.sm) {
                Image(systemName: "plus")
                    .font(.system(size: 28, weight: .semibold))
                    .foregroundStyle(.white.opacity(0.8))
                Text("Add repo")
                    .font(Typography.body())
                    .foregroundStyle(.white.opacity(0.7))
            }
            .frame(maxWidth: .infinity, minHeight: 220)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.lg)
                    .stroke(
                        Color.white.opacity(0.35),
                        style: StrokeStyle(lineWidth: 1.5, dash: [6, 4])
                    )
                    .background(
                        RoundedRectangle(cornerRadius: CornerRadius.lg)
                            .fill(Color.black.opacity(0.12))
                    )
            )
        }
        .buttonStyle(.plain)
    }
}

struct RepoTypeahead: View {
    @State private var query = ""
    @State private var candidates: [URL] = []
    @FocusState private var isQueryFocused: Bool

    let excludedPaths: Set<String>
    let onSelect: (URL) -> Void
    let onCancel: () -> Void

    private let scanner = RepoScanner()

    private var filteredCandidates: [URL] {
        let available = candidates.filter { url in
            !excludedPaths.contains(url.standardizedFileURL.path(percentEncoded: false))
        }

        let trimmedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedQuery.isEmpty else {
            return available
        }

        let terms = trimmedQuery
            .lowercased()
            .split(separator: " ")
            .map(String.init)

        return available.filter { url in
            let name = url.lastPathComponent.lowercased()
            let path = url.path(percentEncoded: false).lowercased()
            return terms.allSatisfy { term in
                name.contains(term) || path.contains(term)
            }
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            HStack {
                Text("Add repository")
                    .font(Typography.sectionTitle(24))
                    .foregroundStyle(.white)

                Spacer()

                Button("Done") {
                    onCancel()
                }
                .buttonStyle(.plain)
                .foregroundStyle(.white.opacity(0.75))
            }

            TextField("Search ~/src", text: $query)
                .textFieldStyle(.roundedBorder)
                .focused($isQueryFocused)

            ScrollView {
                LazyVStack(alignment: .leading, spacing: Spacing.xs) {
                    if filteredCandidates.isEmpty {
                        Text(emptyStateMessage)
                            .font(Typography.body())
                            .foregroundStyle(.white.opacity(0.72))
                            .padding(.vertical, Spacing.lg)
                    } else {
                        ForEach(filteredCandidates, id: \.self) { url in
                            Button {
                                onSelect(url)
                            } label: {
                                VStack(alignment: .leading, spacing: Spacing.xs) {
                                    Text(url.lastPathComponent)
                                        .font(Typography.body())
                                        .foregroundStyle(.white)
                                    Text(url.path(percentEncoded: false))
                                        .font(Typography.caption())
                                        .foregroundStyle(.white.opacity(0.6))
                                        .lineLimit(1)
                                        .truncationMode(.middle)
                                }
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(.horizontal, Spacing.md)
                                .padding(.vertical, Spacing.sm)
                                .background(
                                    RoundedRectangle(cornerRadius: CornerRadius.md)
                                        .fill(Color.white.opacity(0.08))
                                )
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
        }
        .padding(Spacing.xl)
        .background(
            RoundedRectangle(cornerRadius: CornerRadius.xl)
                .fill(Color(hex: 0x3A1C21))
                .overlay(
                    RoundedRectangle(cornerRadius: CornerRadius.xl)
                        .stroke(Color.white.opacity(0.2), lineWidth: 1)
                )
        )
        .onAppear {
            candidates = scanner.scanDefaultRoot()
            isQueryFocused = true
        }
        .onExitCommand {
            onCancel()
        }
    }

    private var emptyStateMessage: String {
        let trimmedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmedQuery.isEmpty {
            return "No repositories found in ~/src"
        }
        return "No matching repositories"
    }
}
