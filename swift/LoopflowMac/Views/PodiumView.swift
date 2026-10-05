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
    @State private var sessionWorkspaces: SessionsWorkspaceRegistry
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
        let restored = initialRepoPath == nil && !AppTestMode.shouldBypassRegistry
            ? loadLoopflowState()?.selectedRepoPath : nil
        let model = PodiumModel.window(query: query, launchCandidates: [initialRepoPath, restored].compactMap { $0 })
        _model = State(initialValue: model)
        _sessionWorkspaces = State(initialValue: SessionsWorkspaceRegistry(localHomeId: model.savedHomeId))
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
        .onAppear { LaunchJournal.home.markAfterCommit(.firstFrame) }
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
        .task {
            while !Task.isCancelled {
                await model.refreshProcessActivity()
                do {
                    try await Task.sleep(for: .seconds(2))
                } catch {
                    return
                }
            }
        }
        .task(id: model.repoPath) { await model.keepWorkspaceCurrent() }
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

enum PodiumSignalState: Equatable {
    case off
    case producing
    case blocked
    case waiting
    case unknown

    static func from(_ snapshot: ActivitySnapshot) -> PodiumSignalState {
        from(nodes: snapshot.nodes)
    }

    static func from(nodes: [ActivityNode]) -> PodiumSignalState {
        let providers = nodes.filter { $0.kind == .providerProcess }
        if providers.contains(where: { $0.state == .stalled }) { return .blocked }
        if providers.contains(where: { $0.state == .working }) { return .producing }
        if providers.contains(where: { $0.state == .waiting }) { return .waiting }
        return .off
    }

    var lens: WaveLensColor {
        switch self {
        case .off: .black
        case .producing: .green
        case .blocked: .blue
        case .waiting, .unknown: .unknown
        }
    }

    var label: String {
        switch self {
        case .off: "Off"
        case .producing: "Producing"
        case .blocked: "Blocked"
        case .waiting: "Waiting"
        case .unknown: "Unknown"
        }
    }
}
