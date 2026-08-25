import Foundation
import Loopflow
import SwiftUI

private enum PodiumSurface {
    case sessions
    case work
}

struct PodiumView: View {
    let portfolioService: PortfolioService
    let initialRepoPath: String?

    @Environment(\.palette) private var palette
    @State private var model: PodiumModel
    @State private var surface: PodiumSurface

    init(
        portfolioService: PortfolioService,
        initialRepoPath: String? = nil,
        query: RegistryQuery = RegistryQueryLocal.shared
    ) {
        self.portfolioService = portfolioService
        self.initialRepoPath = initialRepoPath
        let restoredRepoPath = initialRepoPath == nil && !AppTestMode.shouldBypassRegistry
            ? loadLoopflowState()?.selectedRepoPath
            : nil
        let startingRepoPath = initialRepoPath
            .map { PortfolioDiscovery.resolveLaunchRepo($0).path }
            ?? restoredRepoPath
        let model = PodiumModel(query: query, repoPath: startingRepoPath)
        PodiumFixture.applyIfRequested(to: model)
        _model = State(initialValue: model)
        _surface = State(initialValue: model.repoPath == nil ? .work : .sessions)
    }

    var body: some View {
        @Bindable var model = model
        VStack(spacing: 0) {
            PodiumBar(
                model: model,
                onOpenSessions: {
                    if model.repoPath != nil { surface = .sessions }
                }
            )
            Divider()
            PodiumConsole(model: model) {
                if surface == .sessions, let repoPath = model.repoPath {
                    SessionsView(
                        scope: .repo(repoPath),
                        initialRecords: model.userAskAttention.value,
                        onQueueChanged: { await model.refreshUserAskAttention() },
                        onShowWork: { surface = .work }
                    )
                    .id(repoPath.normalizedFilePath)
                } else {
                    HSplitView {
                        WorkSurfaceView(model: model)
                        .frame(
                            minWidth: 350,
                            idealWidth: 660,
                            maxWidth: .infinity,
                            maxHeight: .infinity,
                            alignment: .top
                        )
                        .accessibilityIdentifier("podium-work")

                        WorkActivityView(model: model)
                            .frame(
                                minWidth: 265,
                                idealWidth: 390,
                                maxWidth: 520,
                                maxHeight: .infinity,
                                alignment: .top
                            )
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(palette.background)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("The Podium")
        .accessibilityIdentifier("podium")
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
        .onChange(of: model.repoPath) { _, repoPath in
            surface = repoPath == nil ? .work : .sessions
        }
        .onReceive(NotificationCenter.default.publisher(for: .openSessions)) { _ in
            if model.repoPath != nil { surface = .sessions }
        }
    }
}

private struct PodiumBar: View {
    @Bindable var model: PodiumModel
    let onOpenSessions: () -> Void

    private var repoTitle: String {
        guard let repoPath = model.repoPath else { return "All repositories" }
        return model.allRepos.first {
            model.repoIdentity($0.path) == model.repoIdentity(repoPath)
        }?.displayName ?? URL(fileURLWithPath: repoPath).lastPathComponent
    }

    var body: some View {
        HStack(spacing: Spacing.lg) {
            repoSelector

            Rectangle()
                .fill(Color.white.opacity(0.18))
                .frame(width: 1, height: 30)

            if let summary = model.waveSummary {
                HStack(spacing: Spacing.md) {
                    compactMetric(summary.waves == 1 ? "Wave" : "Waves", summary.waves)
                }
                .accessibilityIdentifier("podium-wave-summary")
            }

            Spacer(minLength: Spacing.sm)

            UserAskAttentionButton(model: model, onOpen: onOpenSessions)

            ProcessActivityInstrument(reading: model.processActivity)
                .accessibilityIdentifier("podium-process-activity")
        }
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.sm)
        .frame(maxWidth: .infinity, minHeight: 64, alignment: .leading)
        .background(Color.loopflowBurgundy)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("podium-bar")
    }

    /// The bar leads with where you're conducting, not a product name: the
    /// repository selector wears the old title block's clothes. Selection
    /// persists the same way the retired sidebar's selector did.
    private var repoSelector: some View {
        Menu {
            Button {
                selectRepo(nil)
            } label: {
                Label("All repositories", systemImage: "square.stack.3d.up")
            }
            if !model.allRepos.isEmpty { Divider() }
            ForEach(model.allRepos) { repo in
                Button {
                    selectRepo(repo.path)
                } label: {
                    Label(repo.displayName, systemImage: "folder")
                }
            }
        } label: {
            VStack(alignment: .leading, spacing: 0) {
                Text("CONDUCTING WAVES")
                    .font(Typography.caption(8).weight(.bold))
                    .tracking(1.7)
                    .foregroundStyle(.white.opacity(0.68))
                HStack(spacing: Spacing.xs) {
                    Text(repoTitle)
                        .font(Typography.sectionTitle(19))
                        .foregroundStyle(.white)
                        .lineLimit(1)
                        .truncationMode(.tail)
                    Image(systemName: "chevron.down")
                        .font(.system(size: 9, weight: .semibold))
                        .foregroundStyle(.white.opacity(0.6))
                }
            }
            .contentShape(Rectangle())
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .accessibilityIdentifier("podium-repo-scope")
        .accessibilityLabel("Repository: \(repoTitle)")
    }

    private func selectRepo(_ path: String?) {
        model.setRepoPath(path)
        Task.detached {
            try? saveLoopflowState(LoopflowState(selectedRepoPath: path?.normalizedFilePath))
        }
    }

    private func compactMetric(_ label: String, _ value: Int) -> some View {
        HStack(spacing: Spacing.xs) {
            Text(value.formatted())
                .font(Typography.code(10).weight(.bold))
                .foregroundStyle(.white)
            Text(label)
                .font(Typography.caption(9))
                .foregroundStyle(.white.opacity(0.58))
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

private struct ProcessActivityInstrument: View {
    let reading: PodiumReading<ActivitySnapshot>

    private var snapshot: ActivitySnapshot? { reading.value }
    private var state: PodiumSignalState {
        snapshot.map(PodiumSignalState.from) ?? .unknown
    }

    var body: some View {
        HStack(spacing: Spacing.sm) {
            VStack(alignment: .trailing, spacing: Spacing.xxs) {
                Text(state.label)
                    .font(Typography.code(12).weight(.bold))
                    .foregroundStyle(.white)
                    .monospacedDigit()
                Text(detailLabel)
                    .font(Typography.caption(8))
                    .foregroundStyle(.white.opacity(0.68))
                    .lineLimit(1)
            }

            FaderSwitch(
                phase: ConsoleSignal.phase(humanStop: false, agentRunning: false, signal: state),
                width: 20,
                height: 48,
                verb: nil,
                accessibilityId: "podium-master-fader",
                accessibilityLabel: "Provider process activity"
            )
        }
        .help("Exact live provider processes across every Wave. \(state.label).")
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Provider process activity")
        .accessibilityValue("\(state.label). \(detailLabel)")
    }

    private var detailLabel: String {
        guard let snapshot else { return reading.errorMessage ?? "Signal unavailable" }
        let count = snapshot.nodes.filter { $0.kind == .providerProcess }.count
        return "\(count) exact live provider \(count == 1 ? "process" : "processes")"
    }
}
