import Foundation
import Loopflow
import SwiftUI

struct PodiumView: View {
    let portfolioService: PortfolioService
    let initialRepoPath: String?

    @Environment(\.palette) private var palette
    @State private var model: PodiumModel
    @State private var openAsk: AskAttentionRecord?

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
        let model = PodiumModel(query: query, repoPath: restoredRepoPath)
        PodiumFixture.applyIfRequested(to: model)
        _model = State(initialValue: model)
    }

    var body: some View {
        @Bindable var model = model
        VStack(spacing: 0) {
            PodiumBar(
                model: model,
                onOpenAsk: { openAsk = $0 }
            )
            Divider()
            PodiumConsole(model: model) {
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
        .onChange(of: portfolioService.repos.map(\.path)) { _, _ in
            Task {
                await model.refreshPortfolio(
                    initialRepoPath: initialRepoPath,
                    persistedRepos: portfolioService.repos
                )
            }
        }
        .sheet(item: $openAsk) { ask in
            AskSessionView(initialAsk: ask) {
                await model.refreshUserAskAttention()
            }
        }
    }
}

private struct PodiumBar: View {
    @Bindable var model: PodiumModel
    let onOpenAsk: (AskAttentionRecord) -> Void

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
                    compactMetric(summary.activeRuns == 1 ? "Run" : "Runs", summary.activeRuns)
                    if summary.unservedRuns > 0 {
                        Label(
                            "\(summary.unservedRuns) without listener",
                            systemImage: "waveform.slash"
                        )
                        .font(Typography.caption(9).weight(.semibold))
                        .foregroundStyle(Color(hex: 0xF2C36B))
                        .lineLimit(1)
                    }
                }
                .accessibilityIdentifier("podium-wave-summary")
            }

            Spacer(minLength: Spacing.sm)

            UserAskAttentionButton(model: model, onOpen: onOpenAsk)

            TokenOutputInstrument(reading: model.processActivity)
                .accessibilityIdentifier("podium-token-meter")
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
        (snapshot.usage.global?.interval(seconds: 5)?.outputTokensPerSecond ?? 0) > 0
            ? .producing : .off
    }

    static func from(nodes: [ActivityNode], usage: UsageReading?) -> PodiumSignalState {
        let providers = nodes.filter { $0.kind == .providerLaunch }
        if providers.contains(where: { $0.state == .stalled }) { return .blocked }
        if (usage?.interval(seconds: 5)?.outputTokensPerSecond ?? 0) > 0 {
            return .producing
        }
        if !providers.isEmpty { return .waiting }
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

enum TokenRateScale {
    /// A VU-style logarithmic drawing scale. Ten TOK/s reaches the top; the
    /// adjacent number remains the unscaled measurement.
    static func level(_ tokensPerSecond: Double) -> Double {
        guard tokensPerSecond > 0 else { return 0 }
        return min(log1p(tokensPerSecond) / log(11), 1)
    }
}

private struct TokenOutputInstrument: View {
    let reading: PodiumReading<ActivitySnapshot>

    private var snapshot: ActivitySnapshot? { reading.value }
    private var state: PodiumSignalState {
        snapshot.map(PodiumSignalState.from) ?? .unknown
    }

    var body: some View {
        HStack(spacing: Spacing.sm) {
            VStack(alignment: .trailing, spacing: Spacing.xxs) {
                Text(rateLabel)
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
                rate: snapshot?.usage.global?.interval(seconds: 5)?.outputTokensPerSecond ?? 0,
                width: 20,
                height: 48,
                verb: nil,
                accessibilityId: "podium-master-fader",
                accessibilityLabel: "Master output"
            )
        }
        .help("Five-second TOK/s across every Wave. \(state.label).")
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Output signal")
        .accessibilityValue("\(rateLabel), \(state.label). \(detailLabel)")
    }

    private var rateLabel: String {
        guard let snapshot else { return "— TOK/s" }
        let rate = snapshot.usage.global?.interval(seconds: 5)?.outputTokensPerSecond ?? 0
        return "\(rate.formatted(.number.precision(.fractionLength(1)))) TOK/s"
    }

    private var detailLabel: String {
        guard let snapshot else { return reading.errorMessage ?? "Signal unavailable" }
        let slowRate = snapshot.usage.global?.interval(seconds: 300)?.outputTokensPerSecond ?? 0
        if slowRate > 0 {
            return "\(slowRate.formatted(.number.precision(.fractionLength(1)))) TOK/s · 5m avg"
        }
        let day = snapshot.usage.global?.interval(seconds: 86_400)?.outputTokens ?? 0
        return day > 0 ? "\(day.formatted()) output · 24h" : "No measured output · 24h"
    }
}
