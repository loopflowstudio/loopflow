import AppKit
import Foundation
import Loopflow
import SwiftUI

struct RepoView: View {
    let portfolioService: PortfolioService
    let initialRepoPath: String?

    @Environment(\.palette) private var palette
    @State private var model: WorkModel
    /// The application retains these panes when this window closes.
    private let sessionWorkspaces: SessionsWorkspaceRegistry
    private let taskLinks: WorkLinkRouter?
    private let repository: String?
    private let openingRequests: Set<UUID>
    private let query: RegistryQuery

    init(
        portfolioService: PortfolioService,
        sessionWorkspaces: SessionsWorkspaceRegistry,
        initialRepoPath: String? = nil,
        query: RegistryQuery = RegistryQueryLocal.shared,
        taskLinks: WorkLinkRouter? = nil,
        repository: String? = nil,
        openingRequests: Set<UUID> = [],
        openRepository: ((String, LinkedWork?) -> Void)? = nil
    ) {
        self.portfolioService = portfolioService
        self.sessionWorkspaces = sessionWorkspaces
        self.initialRepoPath = initialRepoPath
        self.query = query
        self.taskLinks = taskLinks
        self.repository = repository
        self.openingRequests = openingRequests
        let restored = initialRepoPath == nil && !AppTestMode.shouldBypassRegistry
            ? loadLoopflowState()?.selectedRepoPath : nil
        let model = WorkModel.window(query: query, launchCandidates: [initialRepoPath, restored].compactMap { $0 })
        model.openRepository = openRepository
        _model = State(initialValue: model)
    }

    @ToolbarContentBuilder
    private var workToolbar: some ToolbarContent {
        ToolbarItem(placement: .navigation) { WorkBreadcrumbs(model: model) }
            .sharedBackgroundVisibility(.hidden)
    }

    var body: some View {
        @Bindable var model = model
        VStack(spacing: 0) {
            Group {
                if let repoPath = model.repoPath {
                    SessionsView(
                        model: model, repoPath: repoPath,
                        workspaces: sessionWorkspaces, repository: repository, query: query
                    )
                    .id(repository ?? repoPath.normalizedFilePath)
                } else {
                    WorkNavigator(model: model, onOpenSession: { _ in })
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .foregroundStyle(palette.text)
            if let error = portfolioService.error {
                Text(error).font(.caption).foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled).padding(Spacing.sm)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .overlay(alignment: .bottom) { ShipNoticeOverlay(model: model) }
        .background(palette.background)
        .accessibilityElement(children: .contain)
        .navigationTitle(model.repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Work")
        .toolbar { workToolbar }
        .toolbarBackground(palette.titlebarBackground, for: .windowToolbar)
        .toolbarBackgroundVisibility(.visible, for: .windowToolbar)
        .toolbarColorScheme(palette.titlebarColorScheme, for: .windowToolbar)
        .accessibilityLabel("Loopflow Desktop")
        .accessibilityIdentifier("loopflow")
        .background {
            if let taskLinks, let repository {
                WorkLinkReceiver(router: taskLinks, repository: repository, openingRequests: openingRequests, inspect: { repository, incarnation in
                    model.inspectDesktop(repository: repository, window: incarnation, workspaces: sessionWorkspaces)
                }, perform: { try DesktopAction.perform($0, model: model, workspaces: sessionWorkspaces) },
                   readText: { try sessionWorkspaces.readText($0) }) { link in
                    if sessionWorkspaces.localMachineId == nil { await sessionWorkspaces.refreshMachine(query: query) }
                    if let machine = sessionWorkspaces.localMachineId {
                        model.confirmMachine(machine)
                        model.bindWorkspaces(sessionWorkspaces, repository: repository)
                    }
                    await model.receiveWorkLink(link)
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
            Task {
                await portfolioService.refresh()
                if portfolioService.error == nil {
                    await model.applyPortfolio(portfolioService.repos, initialRepoPath: initialRepoPath)
                }
                await model.rereadDefinitions()
            }
        }
        .task { await portfolioService.refresh() }
        .task(id: portfolioService.selection) {
            guard let selection = portfolioService.selection, selection.error == nil else { return }
            await model.applyPortfolio(portfolioService.repos, initialRepoPath: initialRepoPath)
        }
        .task { await model.keepWorkCurrent() }
        .task { await model.keepPlanningCurrent() }
        .onChange(of: taskLinks?.repositoryPath(repository ?? ""), initial: true) { _, path in
            if let path { model.relocateRepository(to: path) }
        }
        .onChange(of: model.workScope) { _, _ in model.syncWorkScope() }
    }
}
