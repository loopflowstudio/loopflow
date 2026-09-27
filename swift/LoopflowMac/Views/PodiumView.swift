import AppKit
import Foundation
import Loopflow
import SwiftUI

struct PodiumView: View {
    let portfolioService: PortfolioService
    let initialRepoPath: String?

    @Environment(\.palette) private var palette
    @State private var model: PodiumModel
    /// Per-window terminal workspaces: this window's panes and surfaces are
    /// never shared with another window showing the same repository.
    @State private var sessionWorkspaces = SessionsWorkspaceRegistry()
    private let query: RegistryQuery

    init(
        portfolioService: PortfolioService,
        initialRepoPath: String? = nil,
        query: RegistryQuery = RegistryQueryLocal.shared
    ) {
        self.portfolioService = portfolioService
        self.initialRepoPath = initialRepoPath
        self.query = query
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
        .accessibilityLabel("The Podium")
        .accessibilityIdentifier("podium")
        .task { await model.activeRunsLifetime() }
        .onReceive(NSWorkspace.shared.notificationCenter.publisher(for: NSWorkspace.didWakeNotification)) { _ in
            Task { await model.rescanActiveRuns() }
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
