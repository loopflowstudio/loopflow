import Foundation
import Loopflow
import SwiftUI

struct PodiumView: View {
    let portfolioService: PortfolioService
    let initialRepoPath: String?

    @Environment(\.palette) private var palette
    @State private var model: PodiumModel
    @AppStorage("podiumShowsWaveScore") private var showsWaveScore = true

    init(
        portfolioService: PortfolioService,
        initialRepoPath: String? = nil,
        query: RegistryQuery = RegistryQueryLocal.shared
    ) {
        self.portfolioService = portfolioService
        self.initialRepoPath = initialRepoPath
        let model = PodiumModel(query: query)
        PodiumFixture.applyIfRequested(to: model)
        _model = State(initialValue: model)
    }

    var body: some View {
        @Bindable var model = model
        VStack(spacing: 0) {
            PodiumBar(model: model, showsWaveScore: $showsWaveScore)
            Divider()
            HStack(spacing: 0) {
                if showsWaveScore {
                    WaveScore(model: model)
                        .frame(width: 190)
                    Divider()
                }
                HSplitView {
                    RoadmapView(
                        model: model,
                        selection: Binding(
                            get: { model.selection },
                            set: { model.select($0) }
                        )
                    )
                    .frame(minWidth: 350, idealWidth: 660, maxWidth: .infinity)
                    .accessibilityIdentifier("podium-work")

                    WorkActivityView(model: model)
                        .frame(minWidth: 265, idealWidth: 390, maxWidth: 520)
                }
            }
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

private struct PodiumBar: View {
    @Bindable var model: PodiumModel
    @Binding var showsWaveScore: Bool

    private var scopeTitle: String {
        guard let repoPath = model.repoPath else { return "All repositories" }
        return model.allRepos.first {
            WaveOrigin.resolve($0.path).normalizedFilePath
                == WaveOrigin.resolve(repoPath).normalizedFilePath
        }?.displayName ?? URL(fileURLWithPath: repoPath).lastPathComponent
    }

    var body: some View {
        HStack(spacing: Spacing.lg) {
            Button {
                showsWaveScore.toggle()
            } label: {
                Image(systemName: "sidebar.left")
                    .font(.system(size: 13, weight: .semibold))
            }
            .buttonStyle(.plain)
            .foregroundStyle(.white.opacity(0.84))
            .frame(width: HitTarget.comfortable, height: HitTarget.comfortable)
            .background(Color.white.opacity(showsWaveScore ? 0.12 : 0.06), in: Circle())
            .help(showsWaveScore ? "Close Wave score" : "Open Wave score")
            .accessibilityLabel(showsWaveScore ? "Close Wave score" : "Open Wave score")
            .accessibilityIdentifier("podium-wave-score-toggle")

            VStack(alignment: .leading, spacing: 0) {
                Text("CONDUCTING WAVES")
                    .font(Typography.caption(8).weight(.bold))
                    .tracking(1.7)
                    .foregroundStyle(.white.opacity(0.68))
                Text("The Podium")
                    .font(Typography.sectionTitle(19))
                    .foregroundStyle(.white)
            }

            Rectangle()
                .fill(Color.white.opacity(0.18))
                .frame(width: 1, height: 30)

            Menu {
                Button("All repositories") { model.setRepoPath(nil) }
                if !model.allRepos.isEmpty { Divider() }
                ForEach(model.allRepos) { repo in
                    Button(repo.displayName) { model.setRepoPath(repo.path) }
                }
            } label: {
                HStack(spacing: Spacing.xs) {
                    Text(scopeTitle)
                        .font(Typography.body(11).weight(.semibold))
                        .lineLimit(1)
                    Image(systemName: "chevron.down")
                        .font(.system(size: 8, weight: .bold))
                }
                .foregroundStyle(.white.opacity(0.86))
                .padding(.horizontal, Spacing.sm)
                .padding(.vertical, Spacing.xs)
                .background(Color.black.opacity(0.13), in: Capsule())
            }
            .menuStyle(.button)
            .buttonStyle(.plain)
            .fixedSize()
            .accessibilityIdentifier("podium-repo-scope")

            if let summary = model.waveSummary {
                HStack(spacing: Spacing.md) {
                    compactMetric(summary.registeredWaves == 1 ? "Wave" : "Waves", summary.registeredWaves)
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
        let providers = snapshot.nodes.filter { $0.kind == .providerLaunch }
        if providers.contains(where: { $0.state == .stalled }) { return .blocked }
        if snapshot.aggregate.outputTokensPerSecondFast > 0
            || providers.contains(where: { $0.state == .working }) {
            return .producing
        }
        if !providers.isEmpty || !snapshot.providerProcesses.isEmpty { return .waiting }
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

            TokenOutputRail(
                fastRate: snapshot?.aggregate.outputTokensPerSecondFast ?? 0,
                slowRate: snapshot?.aggregate.outputTokensPerSecondSlow ?? 0,
                state: state
            )
        }
        .help("Five-minute TOK/s; rail tick is the 30-minute baseline. \(state.label).")
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Output signal")
        .accessibilityValue("\(rateLabel), \(state.label). \(detailLabel)")
    }

    private var rateLabel: String {
        guard let snapshot else { return "— TOK/s" }
        return "\(snapshot.aggregate.outputTokensPerSecondFast.formatted(.number.precision(.fractionLength(1)))) TOK/s"
    }

    private var detailLabel: String {
        guard let snapshot else { return reading.errorMessage ?? "Signal unavailable" }
        let providers = snapshot.nodes.filter { $0.kind == .providerLaunch }
        let stalled = providers.count { $0.state == .stalled }
        if stalled > 0 {
            return stalled == 1 ? "1 blocked" : "\(stalled) blocked"
        }
        let working = providers.count { $0.state == .working }
        if working > 0 {
            return working == 1 ? "1 working" : "\(working) working"
        }
        let waiting = providers.count { $0.state == .waiting }
        if waiting > 0 {
            return waiting == 1 ? "1 waiting" : "\(waiting) waiting"
        }
        let unclaimed = snapshot.providerProcesses.count { $0.claim == .unclaimed }
        if unclaimed > 0 {
            return unclaimed == 1 ? "1 unclaimed" : "\(unclaimed) unclaimed"
        }
        let orphaned = snapshot.providerProcesses.count { $0.claim == .orphaned }
        if orphaned > 0 {
            return orphaned == 1 ? "1 orphaned" : "\(orphaned) orphaned"
        }
        return "No output"
    }
}

private struct TokenOutputRail: View {
    let fastRate: Double
    let slowRate: Double
    let state: PodiumSignalState

    var body: some View {
        Canvas { context, size in
            let centerX = size.width / 2
            let lamp = CGRect(x: centerX - 4, y: 1, width: 8, height: 8)
            context.fill(Path(ellipseIn: lamp), with: .color(Color.black.opacity(0.52)))
            let lens = lamp.insetBy(dx: 1.25, dy: 1.25)
            context.fill(Path(ellipseIn: lens), with: .color(state.lens.glow))

            let rail = CGRect(x: centerX - 5, y: 14, width: 10, height: size.height - 16)
            context.fill(
                Path(roundedRect: rail, cornerRadius: 5),
                with: .color(Color.black.opacity(0.24))
            )
            context.stroke(
                Path(roundedRect: rail, cornerRadius: 5),
                with: .color(Color.white.opacity(0.28)),
                lineWidth: 1
            )

            let inset = rail.insetBy(dx: 2, dy: 2)
            let fastLevel = TokenRateScale.level(fastRate)
            if fastLevel > 0 {
                let height = max(inset.height * fastLevel, 2)
                let fill = CGRect(
                    x: inset.minX,
                    y: inset.maxY - height,
                    width: inset.width,
                    height: height
                )
                context.fill(
                    Path(roundedRect: fill, cornerRadius: inset.width / 2),
                    with: .linearGradient(
                        Gradient(colors: [
                            WaveLensColor.green.glow,
                            WaveLensColor.green.glow.opacity(0.72),
                        ]),
                        startPoint: CGPoint(x: fill.midX, y: fill.maxY),
                        endPoint: CGPoint(x: fill.midX, y: fill.minY)
                    )
                )
            }

            let baseline = TokenRateScale.level(slowRate)
            if baseline > 0 {
                let y = inset.maxY - inset.height * baseline
                let tick = CGRect(x: rail.minX - 2, y: y - 0.75, width: rail.width + 4, height: 1.5)
                context.fill(
                    Path(roundedRect: tick, cornerRadius: 0.75),
                    with: .color(Color.white.opacity(0.86))
                )
            }
        }
        .frame(width: 22, height: 48)
    }
}

private struct WaveScore: View {
    @Bindable var model: PodiumModel

    private var outlinedWaves: [(wave: WaveViewModel, indent: Int)] {
        waveOutline(model.visibleWaves)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("WAVES")
                    .font(Typography.caption(9).weight(.bold))
                    .tracking(1.5)
                    .foregroundStyle(.white.opacity(0.68))
                Spacer()
                Text(model.visibleWaves.count.formatted())
                    .font(Typography.code(9))
                    .foregroundStyle(.white.opacity(0.68))
            }
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.md)

            if let error = model.waves.errorMessage {
                Label(error, systemImage: "exclamationmark.triangle.fill")
                    .font(Typography.caption(9))
                    .foregroundStyle(.white.opacity(0.78))
                    .lineLimit(3)
                    .padding(Spacing.md)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color.white.opacity(0.08))
                    .accessibilityIdentifier("podium-wave-score-error")
            }

            if model.waves.isLoading {
                ProgressView("Reading Waves…")
                    .controlSize(.small)
                    .foregroundStyle(.white.opacity(0.65))
                    .padding(Spacing.lg)
                    .accessibilityIdentifier("podium-wave-score-loading")
            } else if outlinedWaves.isEmpty {
                Text(model.repoPath == nil ? "No Waves found." : "No Waves in this repository.")
                    .font(Typography.caption())
                    .foregroundStyle(.white.opacity(0.58))
                    .padding(Spacing.lg)
                    .accessibilityIdentifier("podium-wave-score-empty")
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: Spacing.xs) {
                        ForEach(outlinedWaves, id: \.wave.id) { entry in
                            WaveRow(
                                wave: entry.wave,
                                isSelected: model.selection?.waveId == entry.wave.id,
                                onSelect: { model.select(.wave(waveId: entry.wave.id)) },
                                indentLevel: entry.indent
                            )
                        }
                    }
                    .padding(.horizontal, Spacing.sm)
                    .padding(.bottom, Spacing.sm)
                }
                .accessibilityIdentifier("podium-wave-list")
            }
            Spacer(minLength: 0)
        }
        .frame(maxHeight: .infinity, alignment: .top)
        .background(Color.loopflowBurgundy)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Wave score")
        .accessibilityIdentifier("podium-wave-score")
    }

}
