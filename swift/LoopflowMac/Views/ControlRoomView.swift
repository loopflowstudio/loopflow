import Loopflow
import SwiftUI

struct ControlRoomView: View {
    let portfolioService: PortfolioService
    let initialRepoPath: String?

    @Environment(\.palette) private var palette
    @State private var model: ControlRoomModel

    init(
        portfolioService: PortfolioService,
        initialRepoPath: String? = nil,
        query: RegistryQuery = RegistryQueryLocal.shared
    ) {
        self.portfolioService = portfolioService
        self.initialRepoPath = initialRepoPath
        let model = ControlRoomModel(query: query)
        ControlRoomFixture.applyIfRequested(to: model)
        _model = State(initialValue: model)
    }

    var body: some View {
        @Bindable var model = model
        VStack(spacing: 0) {
            ControlRoomStatusStrip(model: model)
            Divider()
            HSplitView {
                ControlRoomSidebar(model: model)
                    .frame(minWidth: 205, idealWidth: 245, maxWidth: 290)

                RoadmapView(
                    model: model,
                    selection: Binding(
                        get: { model.selection },
                        set: { model.select($0) }
                    )
                )
                    .frame(minWidth: 390, idealWidth: 650, maxWidth: .infinity)
                    .accessibilityIdentifier("control-room-work")

                ControlRoomInspector(model: model)
                    .frame(minWidth: 245, idealWidth: 310, maxWidth: 390)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(palette.background)
        .accessibilityIdentifier("control-room")
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

private struct ControlRoomSidebar: View {
    @Bindable var model: ControlRoomModel

    private var outlinedWaves: [(wave: WaveViewModel, indent: Int)] {
        let waves = model.visibleWaves
        let byId = Dictionary(waves.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
        let roots = waves.filter { wave in
            guard let parent = wave.parentWaveId else { return true }
            return byId[parent] == nil
        }
        let children = Dictionary(
            grouping: waves.filter { !roots.contains($0) },
            by: { $0.parentWaveId ?? "" }
        )
        var result: [(WaveViewModel, Int)] = []
        func append(_ wave: WaveViewModel, indent: Int) {
            result.append((wave, indent))
            for child in (children[wave.id] ?? []).sorted(by: Self.sortWaves) {
                append(child, indent: indent + 1)
            }
        }
        for root in roots.sorted(by: Self.sortWaves) {
            append(root, indent: 0)
        }
        return result
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            if let error = model.waves.errorMessage {
                rosterError(error)
            }
            if model.waves.isLoading {
                ProgressView("Reading Waves…")
                    .controlSize(.small)
                    .foregroundStyle(.white.opacity(0.65))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(Spacing.lg)
                    .accessibilityIdentifier("control-room-roster-loading")
            } else if outlinedWaves.isEmpty {
                Text(model.repoPath == nil ? "No Waves found." : "No Waves in this repository.")
                    .font(Typography.caption())
                    .foregroundStyle(.white.opacity(0.58))
                    .padding(Spacing.lg)
                    .accessibilityIdentifier("control-room-roster-empty")
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: Spacing.xs) {
                        ForEach(outlinedWaves, id: \.wave.id) { entry in
                            WaveRow(
                                wave: entry.wave,
                                isSelected: model.selection?.waveId == entry.wave.id,
                                onSelect: {
                                    model.select(.wave(waveId: entry.wave.id))
                                },
                                indentLevel: entry.indent
                            )
                        }
                    }
                    .padding(.horizontal, Spacing.sm)
                    .padding(.top, Spacing.xs)
                }
                .accessibilityIdentifier("control-room-wave-list")
            }
            Spacer(minLength: 0)
        }
        .background(Color.loopflowBurgundy)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Portfolio scope")
        .accessibilityIdentifier("control-room-sidebar")
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("LOOPFLOW")
                .font(Typography.caption(9).weight(.bold))
                .tracking(1.8)
                .foregroundStyle(.white.opacity(0.48))
            Menu {
                Button("All repositories") { model.setRepoPath(nil) }
                if !model.allRepos.isEmpty { Divider() }
                ForEach(model.allRepos) { repo in
                    Button(repo.displayName) { model.setRepoPath(repo.path) }
                }
            } label: {
                HStack(spacing: Spacing.xs) {
                    Text(scopeTitle)
                        .font(Typography.sectionTitle(18))
                        .foregroundStyle(.white)
                        .lineLimit(1)
                    Image(systemName: "chevron.down")
                        .font(.system(size: 9, weight: .semibold))
                        .foregroundStyle(.white.opacity(0.55))
                    Spacer(minLength: 0)
                }
                .contentShape(Rectangle())
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .accessibilityIdentifier("control-room-repo-scope")
        }
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.md)
    }

    private var scopeTitle: String {
        guard let repoPath = model.repoPath else { return "All repositories" }
        return model.allRepos.first {
            WaveOrigin.resolve($0.path).normalizedFilePath
                == WaveOrigin.resolve(repoPath).normalizedFilePath
        }?.displayName ?? URL(fileURLWithPath: repoPath).lastPathComponent
    }

    private func rosterError(_ message: String) -> some View {
        Label(message, systemImage: "exclamationmark.triangle.fill")
            .font(Typography.caption(10))
            .foregroundStyle(.white.opacity(0.82))
            .lineLimit(3)
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.sm)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.white.opacity(0.08))
            .accessibilityIdentifier("control-room-roster-error")
    }

    private static func sortWaves(_ lhs: WaveViewModel, _ rhs: WaveViewModel) -> Bool {
        lhs.displayName.localizedCaseInsensitiveCompare(rhs.displayName) == .orderedAscending
    }
}

private struct ControlRoomInspector: View {
    @Bindable var model: ControlRoomModel
    @Environment(\.palette) private var palette
    @State private var isSettingTurnIntent = false
    @State private var turnIntentError: String?
    @State private var openingTraceId: String?
    @State private var traceError: String?
    @State private var traceRequest: TraceAddress?

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    Text("Selected Work")
                        .font(Typography.sectionTitle(18))
                        .foregroundStyle(palette.text)
                    Text("Shared state and next move")
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                }
                Spacer()
                if model.selection != nil {
                    Button {
                        model.select(nil)
                    } label: {
                        Image(systemName: "xmark")
                    }
                    .buttonStyle(.borderless)
                    .help("Clear selection")
                    .accessibilityLabel("Clear selected Work")
                }
            }
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.md)
            Divider()
            content
        }
        .background(palette.surface)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("control-room-inspector")
        .task(id: model.selection) {
            traceError = nil
            guard model.selection != nil else { return }
            while !Task.isCancelled {
                await model.refreshSelectedRuns()
                do {
                    try await Task.sleep(for: .seconds(15))
                } catch {
                    return
                }
            }
        }
        .sheet(item: $traceRequest) { address in
            TraceEvidenceView(address: address)
                .frame(minWidth: 900, minHeight: 620)
        }
    }

    @ViewBuilder
    private var content: some View {
        if let selection = model.selection {
            ScrollView {
                VStack(alignment: .leading, spacing: Spacing.lg) {
                    switch selection {
                    case .wave(let waveId):
                        waveDetail(waveId)
                    case .project(let waveId, let projectId):
                        projectDetail(waveId: waveId, projectId: projectId)
                    case .task(let waveId, let taskId):
                        taskDetail(waveId: waveId, taskId: taskId)
                    }
                    runHistory(selection)
                }
                .padding(Spacing.lg)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        } else {
            ContentUnavailableView(
                "Select Work",
                systemImage: "scope",
                description: Text("Choose a Wave, Project, or Task without leaving the live control room.")
            )
            .accessibilityIdentifier("control-room-no-selection")
        }
    }

    @ViewBuilder
    private func waveDetail(_ waveId: String) -> some View {
        if let roadmap = model.wave(id: waveId) {
            eyebrow("Wave")
            title(roadmap.wave.name)
            if !roadmap.wave.goal.isEmpty {
                Text(roadmap.wave.goal)
                    .font(Typography.body(12))
                    .foregroundStyle(palette.text)
                    .textSelection(.enabled)
            }
            facts([
                ("State", roadmap.wave.status.label),
                ("Turns", roadmap.wave.paused ? "Paused" : "Enabled"),
                ("Home", roadmap.wave.home.route == "local" ? roadmap.wave.home.id : roadmap.wave.home.route),
                ("Projects", String(roadmap.wave.activeProjects)),
                ("Tasks", String(roadmap.wave.activeTasks)),
            ])
            Button(roadmap.wave.paused ? "Resume turns" : "Pause turns") {
                Task { await setPaused(!roadmap.wave.paused, waveId: roadmap.wave.id) }
            }
            .buttonStyle(.plain)
            .font(Typography.body(11).weight(.semibold))
            .foregroundStyle(Color.loopflowBurgundy)
            .padding(.horizontal, Spacing.md)
            .padding(.vertical, Spacing.xs)
            .background(Color.loopflowBurgundy.opacity(0.10))
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
            .overlay {
                RoundedRectangle(cornerRadius: CornerRadius.sm)
                    .stroke(Color.loopflowBurgundy.opacity(0.35), lineWidth: 1)
            }
            .disabled(isSettingTurnIntent)
            .accessibilityIdentifier("control-room-wave-turn-control")
            if isSettingTurnIntent {
                ProgressView()
                    .controlSize(.small)
            }
            if let turnIntentError {
                unavailable(turnIntentError)
            }
            if let reason = roadmap.projects.unavailableReason {
                unavailable(reason)
            }
        } else if let wave = model.rosterWave(id: waveId) {
            eyebrow("Authored Wave")
            title(wave.displayName)
            Text(wave.lens.reason)
                .font(Typography.body(12))
                .foregroundStyle(palette.textSecondary)
        } else {
            unavailable("This Wave is no longer in the selected scope.")
        }
    }

    @ViewBuilder
    private func projectDetail(waveId: String, projectId: String) -> some View {
        if let project = model.project(waveId: waveId, projectId: projectId) {
            eyebrow("Project · \(waveName(waveId))")
            title(project.project.name)
            if !project.project.definition.isEmpty {
                Text(project.project.definition)
                    .font(Typography.body(12))
                    .foregroundStyle(palette.text)
                    .textSelection(.enabled)
            }
            nextMove(owner: project.nextMove.owner.rawValue, reason: project.nextMove.reason)
            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text("Key results")
                    .font(Typography.caption(10).weight(.semibold))
                    .foregroundStyle(palette.textSecondary)
                    .textCase(.uppercase)
                ForEach(project.project.krs) { kr in
                    Label(kr.text, systemImage: kr.holds ? "checkmark.circle.fill" : "circle")
                        .font(Typography.body(11))
                        .foregroundStyle(kr.holds ? Color.statusSuccess : palette.text)
                }
            }
        } else {
            unavailable("This Project is absent from the latest roadmap evidence.")
        }
    }

    @ViewBuilder
    private func taskDetail(waveId: String, taskId: String) -> some View {
        if let selected = model.task(waveId: waveId, taskId: taskId) {
            let task = selected.task
            eyebrow("Task · \(waveName(waveId)) · \(selected.project.project.name)")
            Text(task.task.identifier)
                .font(Typography.caption(10).weight(.bold))
                .foregroundStyle(Color.loopflowBurgundy)
            title(task.task.name)
            if !task.task.description.isEmpty {
                Text(task.task.description)
                    .font(Typography.body(12))
                    .foregroundStyle(palette.text)
                    .textSelection(.enabled)
            }
            nextMove(owner: task.attention.nextOwner.rawValue, reason: task.attention.reason)
            facts([
                ("Plan", task.task.completed ? "complete" : "open"),
                ("Work", task.runtime?.status.label ?? "not started"),
                ("Process", processLabel(task)),
                ("Section", sectionLabel(task.section)),
            ])
            if let action = roadmapTaskAction(task) {
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    Text("Legal action")
                        .font(Typography.caption(10).weight(.semibold))
                        .foregroundStyle(palette.textSecondary)
                        .textCase(.uppercase)
                    Text(action.label)
                        .font(Typography.sectionTitle(15))
                        .foregroundStyle(Color.loopflowBurgundy)
                    Text(task.attention.actions.reason)
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                }
            }
            if let workspace = task.reference.workspace {
                fact(label: "Worktree", value: workspace.worktree)
            }
            if let github = task.activePr?.publication?.github {
                Link("Open PR #\(github.number)", destination: github.url)
                    .font(Typography.body(11).weight(.semibold))
            }
        } else {
            unavailable("This Task is absent from the latest roadmap evidence.")
        }
    }

    @ViewBuilder
    private func runHistory(_ selection: ControlRoomSelection) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack {
                Text("Recent runs")
                    .font(Typography.caption(10).weight(.semibold))
                    .foregroundStyle(palette.textSecondary)
                    .textCase(.uppercase)
                Spacer()
                if case .available(let runs) = model.selectedRuns, runs.count > 6 {
                    Text("Latest 6")
                        .font(Typography.caption(9))
                        .foregroundStyle(palette.textSecondary)
                }
            }

            if model.selectedRuns.isLoading {
                ProgressView("Reading Runs…")
                    .controlSize(.small)
                    .accessibilityIdentifier("control-room-runs-loading")
            } else {
                let runs = model.runs(for: selection)
                if runs.isEmpty, model.selectedRuns.errorMessage == nil {
                    Text("No recent attributed Runs.")
                        .font(Typography.body(11))
                        .foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("control-room-runs-empty")
                } else {
                    ForEach(Array(runs.prefix(6))) { run in
                        runCard(run)
                    }
                }
                if let reason = model.selectedRuns.errorMessage {
                    unavailable(reason)
                        .accessibilityIdentifier("control-room-runs-unavailable")
                }
            }

            if let traceError {
                unavailable(traceError)
                    .accessibilityIdentifier("control-room-trace-error")
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("control-room-run-history")
    }

    private func runCard(_ run: SkillRunEntry) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                Circle()
                    .fill(runColor(run))
                    .frame(width: 7, height: 7)
                Text(runLabel(run))
                    .font(Typography.body(11).weight(.semibold))
                    .foregroundStyle(palette.text)
                    .lineLimit(1)
                Spacer(minLength: 0)
                Text(run.status)
                    .font(Typography.caption(9).weight(.semibold))
                    .foregroundStyle(runColor(run))
            }

            Text("\(run.provider)\(run.model.map { ":\($0)" } ?? "") · \(runStarted(run))")
                .font(Typography.caption(9))
                .foregroundStyle(palette.textSecondary)
                .lineLimit(1)

            HStack(spacing: Spacing.sm) {
                Text(runEvidence(run))
                    .font(Typography.caption(9))
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(1)
                Spacer(minLength: 0)
                Button(openingTraceId == run.id ? "Opening…" : "Open trace") {
                    Task { await openTrace(run) }
                }
                .buttonStyle(.plain)
                .font(Typography.caption(9).weight(.semibold))
                .foregroundStyle(Color.loopflowBurgundy)
                .disabled(openingTraceId != nil)
                .accessibilityIdentifier("control-room-open-trace-\(run.id)")
            }
        }
        .padding(Spacing.sm)
        .background(palette.surfaceMuted)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(runLabel(run)), \(run.status), \(runEvidence(run))")
    }

    private func eyebrow(_ value: String) -> some View {
        Text(value.uppercased())
            .font(Typography.caption(9).weight(.bold))
            .tracking(1.2)
            .foregroundStyle(palette.textSecondary)
    }

    private func title(_ value: String) -> some View {
        Text(value)
            .font(Typography.sectionTitle(23))
            .foregroundStyle(palette.text)
            .textSelection(.enabled)
            .accessibilityIdentifier("control-room-selection-title")
    }

    private func nextMove(owner: String, reason: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xxs) {
            Text("Next · \(owner)")
                .font(Typography.caption(10).weight(.semibold))
                .foregroundStyle(Color.loopflowBurgundy)
                .textCase(.uppercase)
            Text(reason)
                .font(Typography.body(12))
                .foregroundStyle(palette.text)
                .textSelection(.enabled)
        }
        .padding(Spacing.md)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.loopflowBurgundy.opacity(0.08))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }

    private func facts(_ values: [(String, String)]) -> some View {
        Grid(alignment: .leading, horizontalSpacing: Spacing.md, verticalSpacing: Spacing.sm) {
            ForEach(Array(values.enumerated()), id: \.offset) { _, value in
                GridRow {
                    Text(value.0)
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                    Text(value.1)
                        .font(Typography.caption(10).weight(.medium))
                        .foregroundStyle(palette.text)
                        .textSelection(.enabled)
                }
            }
        }
    }

    private func fact(label: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xxs) {
            Text(label.uppercased())
                .font(Typography.caption(9).weight(.semibold))
                .foregroundStyle(palette.textSecondary)
            Text(value)
                .font(Typography.caption(10))
                .foregroundStyle(palette.text)
                .textSelection(.enabled)
        }
    }

    private func unavailable(_ reason: String) -> some View {
        Label(reason, systemImage: "exclamationmark.triangle")
            .font(Typography.body(11))
            .foregroundStyle(Color.statusWarning)
            .textSelection(.enabled)
    }

    private func runLabel(_ run: SkillRunEntry) -> String {
        guard let flow = run.flow, flow != run.skill else { return run.skill }
        return "\(flow) / \(run.skill)"
    }

    private func runStarted(_ run: SkillRunEntry) -> String {
        Date(timeIntervalSince1970: TimeInterval(run.started))
            .formatted(date: .abbreviated, time: .shortened)
    }

    private func runEvidence(_ run: SkillRunEntry) -> String {
        var evidence: [String] = []
        if run.inputTokens != nil || run.outputTokens != nil {
            let tokens = (run.inputTokens ?? 0) + (run.outputTokens ?? 0)
            evidence.append("\(tokens.formatted()) tokens")
        }
        evidence.append(run.turns == 1 ? "1 turn" : "\(run.turns) turns")
        if let cost = run.costUsd {
            evidence.append(cost.formatted(.currency(code: "USD")))
        }
        return evidence.joined(separator: " · ")
    }

    private func runColor(_ run: SkillRunEntry) -> Color {
        if run.ended == nil { return .statusInfo }
        switch run.status.lowercased() {
        case "ok", "completed", "succeeded": return .statusSuccess
        case "failed", "error": return .statusError
        case "interrupted": return .statusWarning
        default: return .statusNeutral
        }
    }

    private func waveName(_ waveId: String) -> String {
        model.wave(id: waveId)?.wave.name ?? model.rosterWave(id: waveId)?.displayName ?? "Wave"
    }

    @MainActor
    private func setPaused(_ paused: Bool, waveId: String) async {
        isSettingTurnIntent = true
        turnIntentError = nil
        defer { isSettingTurnIntent = false }
        do {
            try await model.setWavePaused(waveId: waveId, paused: paused)
        } catch {
            turnIntentError = error.localizedDescription
        }
    }

    @MainActor
    private func openTrace(_ run: SkillRunEntry) async {
        openingTraceId = run.id
        traceError = nil
        defer { openingTraceId = nil }
        do {
            traceRequest = try await model.traceAddress(for: run)
        } catch {
            traceError = error.localizedDescription
        }
    }

    private func processLabel(_ task: RoadmapTask) -> String {
        switch task.attention.process.state {
        case .observed: task.attention.process.alive == true ? "alive" : "dead"
        case .notExpected: "not expected"
        case .notApplicable: "none"
        case .unavailable: "unknown"
        }
    }

    private func sectionLabel(_ section: RoadmapSection) -> String {
        switch section {
        case .now: "Now"
        case .needsAttention: "Needs attention"
        case .available: "Available"
        case .later: "Later"
        }
    }
}
