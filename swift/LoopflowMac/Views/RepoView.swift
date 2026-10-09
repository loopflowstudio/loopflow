import AppKit
import Foundation
import Loopflow
import SwiftUI

struct RepoView: View {
    let portfolioService: PortfolioService
    let initialRepoPath: String?

    @Environment(\.palette) private var palette
    @State private var showsAutomation = false
    @State private var model: WorkModel
    /// The repository window retains its panes and native surfaces through
    /// navigation. Other repository windows never mount these surfaces.
    @State private var sessionWorkspaces: SessionsWorkspaceRegistry
    private let taskLinks: WorkLinkRouter?
    private let repository: String?
    private let query: RegistryQuery

    init(
        portfolioService: PortfolioService,
        initialRepoPath: String? = nil,
        query: RegistryQuery = RegistryQueryLocal.shared,
        taskLinks: WorkLinkRouter? = nil,
        repository: String? = nil,
        openRepository: ((String, URL?) -> Void)? = nil
    ) {
        self.portfolioService = portfolioService
        self.initialRepoPath = initialRepoPath
        self.query = query
        self.taskLinks = taskLinks
        self.repository = repository
        let restored = initialRepoPath == nil && !AppTestMode.shouldBypassRegistry
            ? loadLoopflowState()?.selectedRepoPath : nil
        let model = WorkModel.window(query: query, launchCandidates: [initialRepoPath, restored].compactMap { $0 })
        model.openRepository = openRepository
        _model = State(initialValue: model)
        _sessionWorkspaces = State(initialValue: SessionsWorkspaceRegistry(localMachineId: model.savedMachineId))
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
                    WorkNavigator(model: model, onOpenSession: { _ in })
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
        .accessibilityLabel("Loopflow Desktop")
        .accessibilityIdentifier("loopflow")
        .background {
            if let taskLinks, let repository {
                WorkLinkReceiver(router: taskLinks, repository: repository, inspect: { incarnation in
                    model.inspectDesktop(repository: repository, window: incarnation, workspaces: sessionWorkspaces)
                }, controlPane: { try sessionWorkspaces.controlPane($0, model: model) }) { url in
                    await model.openTaskLink(url)
                }.frame(width: 0, height: 0)
            }
        }
        .sheet(isPresented: Binding(
            get: { model.showsTaskLink },
            set: { if !$0 { model.dismissTaskLink() } }
        )) {
            TaskLinkView(model: model)
        }
        .onAppear { LaunchJournal.home.markAfterCommit(.firstFrame) }
        .onReceive(NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.didWakeNotification)) { _ in
            model.rescanWork()
        }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            Task { await model.rereadDefinitions() }
        }
        .task {
            await model.refreshPortfolio(
                initialRepoPath: initialRepoPath,
                persistedRepos: portfolioService.repos
            )
        }
        .task { await model.keepWorkCurrent() }
        .onChange(of: model.workScope) { _, _ in model.syncWorkScope() }
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
    }
}
