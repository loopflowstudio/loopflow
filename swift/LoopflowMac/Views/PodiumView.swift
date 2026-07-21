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
                        .frame(width: 224)
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
            model.repoIdentity($0.path) == model.repoIdentity(repoPath)
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
        let state = from(nodes: snapshot.nodes)
        if state == .off, !snapshot.providerProcesses.isEmpty { return .waiting }
        return state
    }

    static func from(nodes: [ActivityNode]) -> PodiumSignalState {
        let providers = nodes.filter { $0.kind == .providerLaunch }
        if providers.contains(where: { $0.state == .stalled }) { return .blocked }
        if providers.contains(where: { $0.direct.outputTokensPerSecondFast > 0 })
            || providers.contains(where: { $0.state == .working }) {
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

            TokenOutputMeter(
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

private struct TokenOutputMeter: View {
    let fastRate: Double
    let slowRate: Double
    let state: PodiumSignalState
    var width: CGFloat = 22
    var height: CGFloat = 48

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
        .frame(width: width, height: height)
    }
}

private struct WaveScore: View {
    @Bindable var model: PodiumModel
    @State private var expandedWaves = Set<String>()
    @State private var expandedProjects = Set<String>()
    @State private var expandedTasks = Set<String>()

    private var outlinedWaves: [(wave: WaveViewModel, indent: Int)] {
        waveOutline(model.visibleWaves)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("SCORE")
                    .font(Typography.caption(9).weight(.bold))
                    .tracking(1.5)
                    .foregroundStyle(.white.opacity(0.68))
                Spacer()
                Text("\(model.visibleWaves.count.formatted()) WAVES")
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
                    LazyVStack(alignment: .leading, spacing: 1) {
                        ForEach(outlinedWaves, id: \.wave.id) { entry in
                            waveBranch(entry.wave, baseLevel: entry.indent)
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
        .accessibilityLabel("Work score")
        .accessibilityIdentifier("podium-wave-score")
    }

    @ViewBuilder
    private func waveBranch(_ wave: WaveViewModel, baseLevel: Int) -> some View {
        let roadmap = roadmap(for: wave)
        let projects = roadmap?.projects.items ?? []
        let nodes = outputNodes(wave: wave)
        ScoreRow(
            identifier: "wave-\(wave.id)",
            kind: .wave,
            title: wave.displayName,
            level: baseLevel,
            outputNodes: nodes,
            state: outputState(nodes),
            hasChildren: !projects.isEmpty,
            isExpanded: expandedWaves.contains(wave.id),
            isSelected: model.selection == .wave(id: wave.id),
            onToggle: { toggle(wave.id, in: &expandedWaves) },
            onSelect: { model.select(.wave(id: wave.id)) }
        )

        if expandedWaves.contains(wave.id) {
            ForEach(projects) { project in
                projectBranch(project, wave: wave, level: baseLevel + 1)
            }
        }
    }

    @ViewBuilder
    private func projectBranch(
        _ project: RoadmapProject,
        wave: WaveViewModel,
        level: Int
    ) -> some View {
        let key = "\(wave.id):\(project.id)"
        let nodes = outputNodes(wave: wave, project: project)
        ScoreRow(
            identifier: "project-\(project.id)",
            kind: .project,
            title: project.project.name,
            level: level,
            outputNodes: nodes,
            state: outputState(nodes),
            hasChildren: !project.tasks.isEmpty,
            isExpanded: expandedProjects.contains(key),
            isSelected: model.selection == .project(id: project.id),
            onToggle: { toggle(key, in: &expandedProjects) },
            onSelect: {
                model.select(.project(id: project.id))
            }
        )

        if expandedProjects.contains(key) {
            ForEach(project.tasks) { task in
                taskBranch(task, project: project, wave: wave, level: level + 1)
            }
        }
    }

    @ViewBuilder
    private func taskBranch(
        _ task: RoadmapTask,
        project: RoadmapProject,
        wave: WaveViewModel,
        level: Int
    ) -> some View {
        let key = "\(wave.id):\(task.id)"
        let nodes = outputNodes(wave: wave, project: project, task: task)
        let execs = execNodes(for: nodes)
        ScoreRow(
            identifier: "task-\(task.id)",
            kind: .task,
            title: task.task.name,
            level: level,
            outputNodes: nodes,
            state: outputState(nodes),
            hasChildren: !execs.isEmpty,
            isExpanded: expandedTasks.contains(key),
            isSelected: model.selection == .task(id: task.id),
            onToggle: { toggle(key, in: &expandedTasks) },
            onSelect: { model.select(.task(id: task.id)) }
        )

        if expandedTasks.contains(key) {
            ForEach(execs) { exec in
                let providers = nodes.filter { $0.parentId == exec.id }
                ScoreRow(
                    identifier: "exec-\(exec.id)",
                    kind: .exec,
                    title: exec.label,
                    level: level + 1,
                    outputNodes: providers,
                    state: outputState(providers),
                    hasChildren: false,
                    isExpanded: false,
                    isSelected: false,
                    onToggle: {},
                    onSelect: { model.select(.task(id: task.id)) }
                )
            }
        }
    }

    private func roadmap(for wave: WaveViewModel) -> WaveRoadmap? {
        model.visibleRoadmaps.first { roadmap in
            roadmap.wave.id == wave.id
                || (
                    roadmap.wave.name == wave.api.name
                        && normalized(roadmap.wave.repo) == normalized(wave.api.repo)
                )
        }
    }

    private func outputNodes(
        wave: WaveViewModel,
        project: RoadmapProject? = nil,
        task: RoadmapTask? = nil
    ) -> [ActivityNode] {
        guard let snapshot = model.processActivity.value else { return [] }
        let projectNames = project.map { [$0.id, $0.project.slug] } ?? []
        let taskNames = task.map { task in
            [task.id, task.task.identifier, task.reference.workspace?.slug].compactMap { $0 }
        } ?? []
        return snapshot.nodes.filter { node in
            guard node.kind == .providerLaunch,
                  node.wave == wave.api.name,
                  node.repo.map(normalized) == normalized(wave.api.repo) else {
                return false
            }
            if !projectNames.isEmpty, !projectNames.contains(node.project ?? "") { return false }
            if !taskNames.isEmpty, !taskNames.contains(node.task ?? "") { return false }
            return true
        }
    }

    private func execNodes(for providers: [ActivityNode]) -> [ActivityNode] {
        guard let snapshot = model.processActivity.value else { return [] }
        let ids = Set(providers.compactMap(\.parentId))
        return snapshot.nodes
            .filter { $0.kind == .exec && ids.contains($0.id) }
            .sorted { $0.startedAt < $1.startedAt }
    }

    private func outputState(_ nodes: [ActivityNode]) -> PodiumSignalState {
        model.processActivity.value == nil ? .unknown : .from(nodes: nodes)
    }

    private func normalized(_ path: String) -> String {
        model.repoIdentity(path)
    }

    private func toggle(_ id: String, in expanded: inout Set<String>) {
        if expanded.contains(id) {
            expanded.remove(id)
        } else {
            expanded.insert(id)
        }
    }
}

private enum ScoreRowKind: Equatable {
    case wave
    case project
    case task
    case exec

    var label: String {
        switch self {
        case .wave: "Wave"
        case .project: "Project"
        case .task: "Task"
        case .exec: "Exec"
        }
    }

    var font: Font {
        switch self {
        case .wave: Typography.sectionTitle(14).weight(.semibold)
        case .project: Typography.body(11).weight(.semibold)
        case .task: Typography.body(10)
        case .exec: Typography.code(9)
        }
    }

    var rowHeight: CGFloat {
        switch self {
        case .wave: 38
        case .project: 34
        case .task, .exec: 30
        }
    }
}

private struct ScoreRow: View {
    let identifier: String
    let kind: ScoreRowKind
    let title: String
    let level: Int
    let outputNodes: [ActivityNode]
    let state: PodiumSignalState
    let hasChildren: Bool
    let isExpanded: Bool
    let isSelected: Bool
    let onToggle: () -> Void
    let onSelect: () -> Void

    @State private var isHovering = false

    private var fastRate: Double {
        outputNodes.reduce(0) { $0 + $1.direct.outputTokensPerSecondFast }
    }

    private var slowRate: Double {
        outputNodes.reduce(0) { $0 + $1.direct.outputTokensPerSecondSlow }
    }

    var body: some View {
        HStack(spacing: 2) {
            Group {
                if hasChildren {
                    Button(action: onToggle) {
                        Image(systemName: "chevron.right")
                            .font(.system(size: 8, weight: .bold))
                            .rotationEffect(.degrees(isExpanded ? 90 : 0))
                            .foregroundStyle(.white.opacity(0.62))
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                    .buttonStyle(.plain)
                    .help(isExpanded ? "Collapse \(kind.label)" : "Expand \(kind.label)")
                    .accessibilityLabel(isExpanded ? "Collapse \(title)" : "Expand \(title)")
                    .accessibilityIdentifier("podium-score-\(identifier)-disclosure")
                } else {
                    Color.clear
                }
            }
            .frame(width: 18, height: kind.rowHeight)

            Button(action: onSelect) {
                HStack(spacing: Spacing.xs) {
                    Text(title)
                        .font(kind.font)
                        .foregroundStyle(.white.opacity(kind == .exec ? 0.68 : 0.92))
                        .lineLimit(1)
                        .truncationMode(.tail)
                    Spacer(minLength: 2)
                    TokenOutputMeter(
                        fastRate: fastRate,
                        slowRate: slowRate,
                        state: state,
                        width: 18,
                        height: kind.rowHeight - 4
                    )
                    .help(
                        "\(fastRate.formatted(.number.precision(.fractionLength(1)))) TOK/s · \(state.label)"
                    )
                    .accessibilityHidden(true)
                }
                .padding(.leading, 2)
                .padding(.trailing, Spacing.xs)
                .frame(maxWidth: .infinity, minHeight: kind.rowHeight, alignment: .leading)
                .background(
                    RoundedRectangle(cornerRadius: CornerRadius.sm)
                        .fill(
                            isSelected
                                ? Color.white.opacity(0.18)
                                : (isHovering ? Color.white.opacity(0.07) : Color.clear)
                        )
                )
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .onHover { isHovering = $0 }
            .help(title)
            .accessibilityLabel("\(kind.label): \(title)")
            .accessibilityIdentifier("podium-score-\(identifier)")
            .accessibilityValue(
                "\(fastRate.formatted(.number.precision(.fractionLength(1)))) tokens per second, \(state.label)"
            )
            .accessibilityAddTraits(isSelected ? [.isSelected] : [])
        }
        .padding(.leading, CGFloat(level) * 12)
    }
}
