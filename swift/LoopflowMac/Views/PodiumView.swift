import AppKit
import Foundation
import Loopflow
import SwiftUI

struct PodiumView: View {
    let portfolioService: PortfolioService
    let initialRepoPath: String?

    @Environment(\.palette) private var palette
    @State private var showsAutomation = false
    @State private var model: PodiumModel
    /// Per-window terminal workspaces: this window's panes and surfaces are
    /// never shared with another window showing the same repository.
    @State private var sessionWorkspaces = SessionsWorkspaceRegistry()
    private let taskLinks: WorkspaceLinkRouter?
    private let query: RegistryQuery

    init(
        portfolioService: PortfolioService,
        initialRepoPath: String? = nil,
        query: RegistryQuery = RegistryQueryLocal.shared,
        taskLinks: WorkspaceLinkRouter? = nil
    ) {
        self.portfolioService = portfolioService
        self.initialRepoPath = initialRepoPath
        self.query = query
        self.taskLinks = taskLinks
        let restoredRepoPath = initialRepoPath == nil && !AppTestMode.shouldBypassRegistry
            ? loadLoopflowState()?.selectedRepoPath
                .flatMap(PortfolioDiscovery.resolveLaunchRepo)
            : nil
        let startingRepoPath = initialRepoPath
            .flatMap(PortfolioDiscovery.resolveLaunchRepo)
            ?? restoredRepoPath
        let model = PodiumModel(query: query, repoPath: startingRepoPath)
        PodiumFixture.applyIfRequested(to: model)
        _model = State(initialValue: model)
    }

    var body: some View {
        @Bindable var model = model
        VStack(spacing: 0) {
            Group {
                if let repoPath = model.repoPath {
                    SessionsView(
                        model: model, repoPath: repoPath,
                        workspaces: sessionWorkspaces, query: query
                    )
                    .id(repoPath.normalizedFilePath)
                } else {
                    WorkspaceNavigator(model: model, onOpenSession: { _ in })
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .foregroundStyle(palette.text)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(palette.background)
        .accessibilityElement(children: .contain)
        .toolbar {
            if let repo = model.repoPath {
                Button("Background progress", systemImage: "clock.arrow.circlepath") { showsAutomation = true }
                    .popover(isPresented: $showsAutomation) { TaskAutomationView(query: query, repo: repo) }
            }
        }
        .accessibilityLabel("The Podium")
        .accessibilityIdentifier("podium")
        .background {
            if let taskLinks {
                WorkspaceLinkReceiver(router: taskLinks) { url in
                    Task { await model.openTaskLink(url) }
                }.frame(width: 0, height: 0)
            }
        }
        .sheet(isPresented: Binding(
            get: { model.showsTaskLink },
            set: { if !$0 { model.dismissTaskLink() } }
        )) {
            TaskLinkView(model: model)
        }
        .task { await model.activeSessionsLifetime() }
        .onReceive(NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.didWakeNotification)) { _ in
            Task { await model.rescanActiveSessions() }
        }
        .task {
            await model.refreshPortfolio(
                initialRepoPath: initialRepoPath,
                persistedRepos: portfolioService.repos
            )
        }
        .task(id: model.repoPath) {
            await model.refresh()
            while !Task.isCancelled {
                do {
                    try await Task.sleep(for: .seconds(15))
                } catch {
                    return
                }
                await model.refresh()
            }
        }
        .task(id: model.repoPath) {
            // Fixture and UI-test runs render without background helpers.
            guard let repo = model.repoPath, AppTestMode.current() == nil else { return }
            CIWatchers.shared.open(repo)
            defer { CIWatchers.shared.close(repo) }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(3600))
            }
        }
        .onChange(of: portfolioService.repos.map(\.path)) { _, _ in
            Task {
                await model.refreshPortfolio(
                    initialRepoPath: initialRepoPath,
                    persistedRepos: portfolioService.repos
                )
            }
        }
        .task(id: model.repoPath) {
            while !Task.isCancelled {
                await model.refreshSessions()
                do { try await Task.sleep(for: .seconds(2)) }
                catch { return }
            }
        }
    }
}
