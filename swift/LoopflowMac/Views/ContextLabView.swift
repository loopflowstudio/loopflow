#if os(macOS)
import Loopflow
import SwiftUI

enum ContextLabMode: String, Codable, CaseIterable, Identifiable, Hashable {
    case aggregate, lanes, sources

    var id: String { rawValue }

    var title: String {
        switch self {
        case .aggregate: "Initial prompts"
        case .lanes: "Agent sessions"
        case .sources: "Sources"
        }
    }
}

private enum ContextLaneSort: String, CaseIterable, Identifiable {
    case context = "Initial prompt"
    case lifetimeInput = "Lifetime input"
    case windowPressure = "Peak window"
    case selectedShare = "Selected-source share"
    case outcome = "Outcome"
    case steering = "Steering"
    case time = "Recent"

    var id: String { rawValue }
}

private enum ContextNodeSort: String, CaseIterable, Identifiable {
    case impressions = "Impressions"
    case recent = "Recently seen"
    case label = "Name"

    var id: String { rawValue }
}

private struct ContextLabSavedView: Codable, Hashable {
    let query: SessionSetQuery
    let mode: ContextLabMode

    var name: String {
        let days = max(1, (query.startedBefore - query.startedAfter) / (24 * 60 * 60))
        let end = Date(timeIntervalSince1970: TimeInterval(query.startedBefore))
        return "\(days)d · \(end.formatted(date: .abbreviated, time: .omitted))"
    }
}

struct TaskWorkspaceRoute: Codable, Hashable {
    let issue: String
    let context: ContextLabRoute

    var wave: String { context.query.waves[0] }
    var repoPath: String { context.query.repoPaths[0] }
}

struct ContextLabRoute: Codable, Hashable {
    let query: SessionSetQuery
    let selectedNodeId: String
    let focusNodeId: String
    let mode: ContextLabMode

    var isWaveScoped: Bool {
        query.repoPaths.count == 1
            && !(query.repoPaths.first ?? "").isEmpty
            && query.waves.count == 1
            && !(query.waves.first ?? "").isEmpty
    }

    static func wave(
        repoPath: String,
        wave: String,
        now: Int64 = Int64(Date().timeIntervalSince1970)
    ) -> Self {
        return Self(
            query: SessionSetQuery(
                repoPaths: [WaveOrigin.resolve(repoPath)],
                startedAfter: now - 30 * 24 * 60 * 60,
                startedBefore: now,
                waves: [wave],
                projects: [],
                tasks: [],
                flows: [],
                skills: [],
                providers: [],
                models: [],
                surfaces: [],
                outcomes: [],
                captureStates: [],
                steeredOnly: false,
                currentRevisionOnly: false
            ),
            selectedNodeId: "session-set",
            focusNodeId: "session-set",
            mode: .aggregate
        )
    }
}

struct ContextLabView: View {
    private let defaultQuery: SessionSetQuery
    private let savedViewsKey: String

    @Environment(\.palette) private var palette
    @Environment(\.openWindow) private var openWindow

    @State private var query: SessionSetQuery
    @State private var snapshot: ContextLabSnapshot?
    @State private var selectedNodeId = "session-set"
    @State private var focusNodeId = "session-set"
    @State private var mode = ContextLabMode.aggregate
    @State private var laneSort = ContextLaneSort.context
    @State private var nodeSort = ContextNodeSort.impressions
    @State private var sourceSearch = ""
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var traceRequest: TraceAddress?
    @State private var isLaunchingRefinement = false
    @State private var refinementErrorMessage: String?
    @State private var refinementProjects: [WaveProject] = []
    @State private var refinementProjectId: String?
    @State private var isLoadingRefinementProjects = false
    @State private var refinementProjectLoadError: String?
    @State private var savedViews: [ContextLabSavedView]

    init(route: ContextLabRoute) {
        let repoPath = route.query.repoPaths.first ?? ""
        let wave = route.query.waves.first ?? ""
        let defaultQuery = ContextLabRoute.wave(
            repoPath: repoPath,
            wave: wave,
            now: route.query.startedBefore
        ).query
        let savedViewsKey = Self.savedViewsKey(repoPath: repoPath, wave: wave)
        self.defaultQuery = defaultQuery
        self.savedViewsKey = savedViewsKey
        var initialQuery = route.query
        initialQuery.repoPaths = initialQuery.repoPaths.map(WaveOrigin.resolve)
        initialQuery.repoPaths = defaultQuery.repoPaths
        initialQuery.waves = defaultQuery.waves
        initialQuery.projects = []
        initialQuery.tasks = []
        _query = State(initialValue: initialQuery)
        _selectedNodeId = State(initialValue: route.selectedNodeId)
        _focusNodeId = State(initialValue: route.focusNodeId)
        _mode = State(initialValue: route.mode)
        _savedViews = State(initialValue: Self.loadSavedViews(key: savedViewsKey))
    }

    private var windowDays: Int {
        max(1, Int((query.startedBefore - query.startedAfter) / (24 * 60 * 60)))
    }

    private var windowDaysBinding: Binding<Int> {
        Binding(
            get: { windowDays },
            set: { query.startedAfter = query.startedBefore - Int64($0 * 24 * 60 * 60) }
        )
    }

    private var refinementProjectScope: String {
        "\(query.repoPaths.first ?? "")\u{0}\(query.waves.first ?? "")"
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            HSplitView {
                filterRail
                    .frame(minWidth: 190, idealWidth: 220, maxWidth: 260)
                    .frame(maxHeight: .infinity)
                center
                    .frame(minWidth: 600, maxWidth: .infinity)
                    .frame(maxHeight: .infinity)
                evidenceRail
                    .frame(minWidth: 270, idealWidth: 310, maxWidth: 360)
                    .frame(maxHeight: .infinity)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(palette.background)
        .task(id: refinementProjectScope) { await loadRefinementProjects() }
        .task(id: query) { await refresh() }
        .sheet(item: $traceRequest) { address in
            TraceEvidenceView(address: address)
                .frame(minWidth: 760, minHeight: 620)
        }
    }

    private var header: some View {
        VStack(spacing: Spacing.md) {
            HStack(spacing: Spacing.lg) {
                VStack(alignment: .leading, spacing: 1) {
                    Text("Context Lab")
                        .font(Typography.heroTitle(26))
                        .foregroundStyle(palette.text)
                    Text("\(query.waves[0]) · what text shaped these sessions")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                }
                Spacer()
                if isLoading { ProgressView().controlSize(.small) }
                if let snapshot {
                    Text(windowLabel(snapshot.query))
                        .font(Typography.code(11))
                        .foregroundStyle(palette.textSecondary)
                }
                Button {
                    let duration = query.startedBefore - query.startedAfter
                    query.startedBefore = Int64(Date().timeIntervalSince1970)
                    query.startedAfter = query.startedBefore - duration
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .buttonStyle(.plain)
                .disabled(isLoading)
                .help("Refresh the selected session set")
            }
            if let snapshot {
                stats(snapshot)
            }
        }
        .padding(.horizontal, Spacing.xxl)
        .padding(.vertical, Spacing.lg)
    }

    private func stats(_ snapshot: ContextLabSnapshot) -> some View {
        let totals = snapshot.totals
        return ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: Spacing.lg) {
                ContextStat(label: "Runs", value: totals.runs.formatted())
                ContextStat(label: "Agent sessions", value: totals.agentSessions.formatted())
                ContextStat(
                    label: "Turns",
                    value: totals.turns.formatted(),
                    denominator: "\(totals.steeringTurns) steering"
                )
                ContextStat(
                    label: "Initial prompts",
                    value: optionalTokens(totals.initialPromptTokens),
                    denominator: "\(totals.initialPromptAgentSessions) / \(totals.agentSessions) captured"
                )
                ContextStat(
                    label: "Initial p50 / p95",
                    value: "\(optionalTokens(totals.medianInitialPromptTokens)) / \(optionalTokens(totals.p95InitialPromptTokens))"
                )
                ContextStat(
                    label: "Instruction share",
                    value: share(totals.instructionTokens, of: totals.initialPromptTokens),
                    denominator: "of initial prompts"
                )
                ContextStat(
                    label: "Lifetime input",
                    value: optionalTokens(totals.lifetimeInputTokens),
                    denominator: "\(totals.lifetimeInputAgentSessions) / \(totals.agentSessions) measured"
                )
                ContextStat(
                    label: "Lifetime p50 / p95",
                    value: "\(optionalTokens(totals.medianLifetimeInputTokens)) / \(optionalTokens(totals.p95LifetimeInputTokens))"
                )
                ContextStat(
                    label: "Peak window p50 / p95",
                    value: "\(optionalPercent(totals.medianPeakContextPercent)) / \(optionalPercent(totals.p95PeakContextPercent))",
                    denominator: "\(totals.peakContextAgentSessions) / \(totals.agentSessions) measured"
                )
                ContextStat(
                    label: "Outcomes",
                    value: "\(totals.completedLaunches) done · \(totals.failedLaunches) failed",
                    denominator: "\(totals.runningLaunches) running · \(totals.interruptedLaunches) interrupted"
                )
                ContextStat(
                    label: "Steering",
                    value: "\(totals.steeringTurns) turns",
                    denominator: "across \(totals.steeredLaunches) agent sessions"
                )
                ContextStat(
                    label: "Capture",
                    value: "\(snapshot.coverage.completeLaunches) / \(totals.agentSessions) complete",
                    denominator: "prompts \(snapshot.coverage.promptArtifactsAvailable) / \(totals.turns)"
                )
            }
        }
    }

    private var filterRail: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.lg) {
                railTitle("Wave")
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    Text(query.waves[0])
                        .font(Typography.sectionTitle(15))
                        .foregroundStyle(palette.text)
                    Text(URL(fileURLWithPath: query.repoPaths[0]).lastPathComponent)
                        .font(Typography.code(10))
                        .foregroundStyle(palette.textSecondary)
                }
                railTitle("Session set")
                Picker("Window", selection: windowDaysBinding) {
                    Text("7 days").tag(7)
                    Text("30 days").tag(30)
                    Text("90 days").tag(90)
                }
                .pickerStyle(.menu)

                facetPicker("Flow", query: \.flows, values: facets(\.flow))
                facetPicker("Skill", query: \.skills, values: facets(\.skill))
                facetPicker("Provider", query: \.providers, values: facets { $0.provider })
                facetPicker("Model", query: \.models, values: facets(\.model))
                facetPicker("Surface", query: \.surfaces, values: facets { $0.surface })

                Divider()
                railTitle("Outcome")
                ForEach(SessionOutcome.allCases, id: \.self) { value in
                    filterToggle(value.rawValue.capitalized, selected: query.outcomes.contains(value)) {
                        toggle(value, in: &query.outcomes)
                        query.outcomes.sort { $0.rawValue < $1.rawValue }
                    }
                }
                railTitle("Capture")
                ForEach(CaptureState.allCases, id: \.self) { value in
                    filterToggle(displayCapture(value), selected: query.captureStates.contains(value)) {
                        toggle(value, in: &query.captureStates)
                        query.captureStates.sort { $0.rawValue < $1.rawValue }
                    }
                }
                railTitle("Research state")
                filterToggle("Observed steering only", selected: query.steeredOnly) {
                    query.steeredOnly.toggle()
                }
                filterToggle("Contains current file instruction", selected: query.currentRevisionOnly) {
                    query.currentRevisionOnly.toggle()
                }

                Button("Clear filters") { clearFilters() }
                    .buttonStyle(GhostButtonStyle())
                    .disabled(filtersAreEmpty)

                Divider()
                railTitle("Saved views")
                ForEach(savedViews, id: \.self) { saved in
                    Button {
                        apply(saved)
                    } label: {
                        HStack {
                            Image(systemName: "bookmark")
                            Text(saved.name).lineLimit(1)
                            Spacer()
                        }
                    }
                    .buttonStyle(.plain)
                    .font(Typography.body(10))
                    .foregroundStyle(palette.text)
                    .contextMenu {
                        Button("Delete") { delete(saved) }
                    }
                }
                Button("Save current view") { saveCurrentView() }
                    .buttonStyle(.borderless)
                    .font(Typography.caption(10))

                if let snapshot {
                    Divider()
                    coverageSummary(snapshot)
                }
            }
            .padding(Spacing.lg)
        }
        .background(palette.surface)
    }

    private var center: some View {
        VStack(spacing: 0) {
            centerToolbar
            Divider()
            Group {
                if let errorMessage {
                    ContentUnavailableView(
                        "Context unavailable",
                        systemImage: "exclamationmark.triangle",
                        description: Text(errorMessage)
                    )
                } else if let snapshot {
                    switch mode {
                    case .aggregate:
                        aggregate(snapshot)
                    case .lanes:
                        lanes(snapshot)
                    case .sources:
                        sources(snapshot)
                    }
                } else {
                    ProgressView()
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(palette.background)
    }

    private var centerToolbar: some View {
        HStack(spacing: Spacing.md) {
            if mode == .sources {
                Text("Instruction sources")
                    .font(Typography.body(12).weight(.semibold))
                    .foregroundStyle(palette.text)
            } else if let snapshot {
                breadcrumb(snapshot.aggregateRoot)
            }
            Spacer()
            if mode == .lanes {
                Picker("Sort", selection: $laneSort) {
                    ForEach(ContextLaneSort.allCases) { Text($0.rawValue).tag($0) }
                }
                .labelsHidden()
                .frame(width: 190)
            } else if mode == .sources {
                Picker("Sort", selection: $nodeSort) {
                    ForEach(ContextNodeSort.allCases) { Text($0.rawValue).tag($0) }
                }
                .labelsHidden()
                .frame(width: 120)
            }
            Picker("Mode", selection: $mode) {
                ForEach(ContextLabMode.allCases) { Text($0.title).tag($0) }
            }
            .labelsHidden()
            .pickerStyle(.segmented)
            .frame(width: 360)
        }
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.md)
    }

    private func aggregate(_ snapshot: ContextLabSnapshot) -> some View {
        let focus = findNode(focusNodeId, in: snapshot.aggregateRoot) ?? snapshot.aggregateRoot
        let coverage = snapshot.totals.turns == 0
            ? 0.25
            : 0.35 + 0.65 * Double(snapshot.coverage.attributedTurns) / Double(snapshot.totals.turns)
        return ScrollView {
            VStack(alignment: .leading, spacing: Spacing.xl) {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(focus.label)
                            .font(Typography.sectionTitle(20))
                            .foregroundStyle(palette.text)
                        Text("Width is initial-prompt tokens · opacity is capture coverage")
                            .font(Typography.caption(11))
                            .foregroundStyle(palette.textSecondary)
                    }
                    Spacer()
                    if selectedNodeId != focusNodeId,
                       let selected = findNode(selectedNodeId, in: snapshot.aggregateRoot),
                       !selected.children.isEmpty {
                        Button("Zoom to selection") { focusNodeId = selected.id }
                            .buttonStyle(.borderless)
                    }
                }
                ContextIcicle(
                    root: focus,
                    selectedNodeId: selectedNodeId,
                    opacity: coverage,
                    onSelect: { selectedNodeId = $0.id }
                )
                .frame(minHeight: 120)
                selectedSummary(snapshot)
            }
            .padding(Spacing.xl)
        }
    }

    private func lanes(_ snapshot: ContextLabSnapshot) -> some View {
        let selectedIds = revisionIds(for: selectedNodeId, root: snapshot.aggregateRoot)
        return ScrollView {
            LazyVStack(alignment: .leading, spacing: Spacing.md) {
                HStack {
                    Text("Shared token scale")
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                    Spacer()
                    Text("\(maximumTurnTokens(snapshot.sessions).formatted()) tokens")
                        .font(Typography.code(10))
                        .foregroundStyle(palette.textSecondary)
                }
                ForEach(sortedSessions(snapshot.sessions, selectedIds: selectedIds)) { session in
                    SessionLaneView(
                        session: session,
                        maximumTokens: maximumTurnTokens(snapshot.sessions),
                        selectedNodeIds: selectedIds,
                        onSelect: { selectedNodeId = $0.nodeId }
                    )
                }
            }
            .padding(Spacing.xl)
        }
    }

    private func sources(_ snapshot: ContextLabSnapshot) -> some View {
        HSplitView {
            sourceRanking(snapshot)
                .frame(minWidth: 300, idealWidth: 340, maxWidth: 400)
            if let source = selectedInstructionSource(in: snapshot) {
                ContextSourceDocumentView(source: source)
                    .id(source.id)
                    .frame(minWidth: 320, maxWidth: .infinity)
            } else {
                ContentUnavailableView(
                    "Select a source",
                    systemImage: "doc.text.magnifyingglass",
                    description: Text("Open main's current file beside its impression evidence.")
                )
                .frame(minWidth: 320, maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }

    private func sourceRanking(_ snapshot: ContextLabSnapshot) -> some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    Text("One impression is one agent session whose initial prompt contains the source.")
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                    TextField("Find a source", text: $sourceSearch)
                        .textFieldStyle(.roundedBorder)
                }
                .padding(Spacing.md)
                HStack {
                    Text("Source").frame(maxWidth: .infinity, alignment: .leading)
                    Text("Impressions").frame(width: 72, alignment: .trailing)
                    Text("Reach").frame(width: 46, alignment: .trailing)
                }
                .font(Typography.caption(9))
                .foregroundStyle(palette.textSecondary)
                .padding(.horizontal, Spacing.md)
                .padding(.vertical, Spacing.sm)
                ForEach(sortedSources(snapshot)) { source in
                    let current = currentEvidence(for: source, in: snapshot)
                    let accessibilityLabel = source.impressions.map {
                        "\(source.label), \($0) impressions"
                    } ?? "\(source.label), impressions not captured"
                    Button {
                        selectedNodeId = source.currentRevisionNodeId ?? source.id
                    } label: {
                        HStack(spacing: Spacing.sm) {
                            Circle().fill(contextColor(source.kind)).frame(width: 7, height: 7)
                            VStack(alignment: .leading, spacing: 1) {
                                Text(source.label).lineLimit(1)
                                Text(source.sourcePath)
                                    .font(Typography.code(8))
                                    .foregroundStyle(palette.textSecondary)
                                    .lineLimit(1)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            Text(source.impressions?.formatted() ?? "—")
                                .frame(width: 72, alignment: .trailing)
                            Text(source.impressions.map {
                                percent($0, of: snapshot.coverage.sourceObservableAgentSessions)
                            } ?? "—")
                                .frame(width: 46, alignment: .trailing)
                        }
                        .font(Typography.body(10))
                        .foregroundStyle(palette.text)
                        .padding(.horizontal, Spacing.md)
                        .padding(.vertical, Spacing.sm)
                        .background(
                            source.id == selectedNodeId || current?.nodeId == selectedNodeId
                                ? palette.surfaceMuted : Color.clear
                        )
                    }
                    .buttonStyle(.plain)
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(Text(accessibilityLabel))
                    .accessibilityHint(Text("Open main's current file and its revision evidence"))
                    Divider()
                }
            }
        }
        .background(palette.surface)
    }

    private var evidenceRail: some View {
        ScrollView {
            if let snapshot {
                let matches = selectedEvidence(in: snapshot)
                VStack(alignment: .leading, spacing: Spacing.lg) {
                    railTitle("Evidence")
                    if matches.isEmpty {
                        ContentUnavailableView(
                            "Select a revision",
                            systemImage: "scope",
                            description: Text("Choose a flame or table segment to inspect exact local evidence.")
                        )
                    } else if matches.count == 1, let evidence = matches.first {
                        evidenceDetail(evidence, snapshot: snapshot)
                    } else {
                        Text("\(matches.count) revisions in this selection")
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                        revisionComparison(matches)
                        ForEach(matches, id: \.nodeId) { evidence in
                            Button {
                                selectedNodeId = evidence.nodeId
                            } label: {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(evidence.label)
                                    Text("\(shortHash(evidence.contentSha256)) · \(evidence.measurements.attributedTokens.formatted()) tokens")
                                        .font(Typography.code(9))
                                        .foregroundStyle(palette.textSecondary)
                                }
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(Spacing.sm)
                                .background(palette.surfaceMuted, in: RoundedRectangle(cornerRadius: CornerRadius.md))
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
                .padding(Spacing.lg)
            }
        }
        .background(palette.surface)
    }

    private func evidenceDetail(_ evidence: SourceEvidence, snapshot: ContextLabSnapshot) -> some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text(evidence.label)
                    .font(Typography.sectionTitle(19))
                    .foregroundStyle(palette.text)
                Text(displayKind(evidence.kind))
                    .font(Typography.caption(10))
                    .foregroundStyle(contextColor(evidence.kind))
                Text(evidence.sourcePath ?? "No canonical file source")
                    .font(Typography.code(9))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                Text(evidence.contentSha256)
                    .font(Typography.code(9))
                    .foregroundStyle(palette.text)
                    .textSelection(.enabled)
            }

            VStack(alignment: .leading, spacing: Spacing.sm) {
                evidenceMetric("Runs", "\(evidence.measurements.exposedSessions) / \(snapshot.totals.runs)")
                evidenceMetric("Impressions", "\(evidence.measurements.exposedLaunches) agent sessions")
                evidenceMetric("Turns", evidence.measurements.exposedTurns.formatted())
                evidenceMetric("Attributed", "\(evidence.measurements.attributedTokens.formatted()) tokens")
                evidenceMetric("Median / p95", "\(optionalTokens(evidence.measurements.medianTokensPerExposedTurn)) / \(optionalTokens(evidence.measurements.p95TokensPerExposedTurn))")
                evidenceMetric(
                    "Observed",
                    observationRange(
                        first: evidence.measurements.firstSeen,
                        last: evidence.measurements.lastSeen
                    )
                )
                evidenceMetric(
                    "Provider / model",
                    providerModelSummary(evidence.measurements.providerModels)
                )
                evidenceMetric("Precedence", evidence.precedenceLayers.joined(separator: " · "))
            }

            if !evidence.isEditable {
                Label(editabilityReason(evidence), systemImage: "lock.trianglebadge.exclamationmark")
                    .font(Typography.caption(10))
                    .foregroundStyle(Color.statusWarning)
            }

            refinementProjectDestination

            VStack(alignment: .leading, spacing: Spacing.sm) {
                railTitle("Representative sessions")
                if evidence.representatives.isEmpty {
                    Text("No agent session has observed this revision in the selected population.")
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                } else {
                    ForEach(Array(evidence.representatives.enumerated()), id: \.offset) { _, trace in
                        VStack(alignment: .leading, spacing: Spacing.xs) {
                            HStack {
                                Text(displayRole(trace.role))
                                    .font(Typography.caption(10).weight(.semibold))
                                Spacer()
                                Text(shortHash(trace.address.runId))
                                    .font(Typography.code(9))
                            }
                            Text("\(trace.outcome.rawValue) · \(optionalTokens(trace.suppliedContextTokens)) context · \(trace.selectedSourceTokens) selected")
                                .font(Typography.caption(9))
                                .foregroundStyle(palette.textSecondary)
                            HStack {
                                captureBadge("Prompt", available: trace.promptArtifactAvailable)
                                captureBadge("Conversation", available: trace.conversationAvailable)
                                Spacer()
                                Button("Open trace") { traceRequest = trace.address }
                                    .buttonStyle(.borderless)
                            }
                        }
                        .padding(Spacing.sm)
                        .background(palette.surfaceMuted, in: RoundedRectangle(cornerRadius: CornerRadius.md))
                    }
                }
            }

            if let refinementErrorMessage {
                Label(refinementErrorMessage, systemImage: "exclamationmark.triangle")
                    .font(Typography.caption(10))
                    .foregroundStyle(Color.statusError)
                    .textSelection(.enabled)
            }

            Button(isLaunchingRefinement ? "Starting task-worker…" : "Refine in task-worker") {
                Task { await launchRefinement(evidence, snapshot: snapshot) }
            }
                .buttonStyle(DarkButtonStyle())
                .disabled(!canRefine(evidence, in: snapshot.query) || isLaunchingRefinement)
                .opacity(canRefine(evidence, in: snapshot.query) && !isLaunchingRefinement ? 1 : 0.4)
                .help(refinementHelp(evidence, in: snapshot.query))
        }
    }

    private func revisionComparison(_ revisions: [SourceEvidence]) -> some View {
        let ordered = revisions.sorted {
            ($0.measurements.firstSeen ?? .min) < ($1.measurements.firstSeen ?? .min)
        }
        let earlier = ordered[ordered.count - 2]
        let later = ordered[ordered.count - 1]

        return VStack(alignment: .leading, spacing: Spacing.sm) {
            railTitle("Revision comparison")
            Text("\(shortHash(earlier.contentSha256)) → \(shortHash(later.contentSha256))")
                .font(Typography.code(9))
                .foregroundStyle(palette.textSecondary)

            if let blocker = comparisonBlocker(earlier, later) {
                Label(blocker, systemImage: "chart.xyaxis.line")
                    .font(Typography.caption(9))
                    .foregroundStyle(Color.statusWarning)
            } else {
                comparisonMetric(
                    "Median source tokens / turn",
                    optionalTokens(earlier.measurements.medianTokensPerExposedTurn),
                    optionalTokens(later.measurements.medianTokensPerExposedTurn)
                )
                comparisonMetric(
                    "Completed launches",
                    percent(earlier.measurements.completedLaunches, of: earlier.measurements.exposedLaunches),
                    percent(later.measurements.completedLaunches, of: later.measurements.exposedLaunches)
                )
                comparisonMetric(
                    "Failed launches",
                    percent(earlier.measurements.failedLaunches, of: earlier.measurements.exposedLaunches),
                    percent(later.measurements.failedLaunches, of: later.measurements.exposedLaunches)
                )
                comparisonMetric(
                    "Steering / launch",
                    rate(earlier.measurements.steeringTurns, over: earlier.measurements.exposedLaunches),
                    rate(later.measurements.steeringTurns, over: later.measurements.exposedLaunches)
                )
                comparisonMetric(
                    "Complete capture",
                    percent(earlier.measurements.completeCaptureLaunches, of: earlier.measurements.exposedLaunches),
                    percent(later.measurements.completeCaptureLaunches, of: later.measurements.exposedLaunches)
                )
                comparisonMetric(
                    "Observation span",
                    observationSpan(first: earlier.measurements.firstSeen, last: earlier.measurements.lastSeen),
                    observationSpan(first: later.measurements.firstSeen, last: later.measurements.lastSeen)
                )
                Text("Capture, provider/model mix, and observation spans are comparable. No quality score is inferred.")
                    .font(Typography.caption(8))
                    .foregroundStyle(palette.textSecondary)
            }
        }
        .padding(Spacing.sm)
        .background(palette.surfaceMuted, in: RoundedRectangle(cornerRadius: CornerRadius.md))
    }

    private func comparisonBlocker(_ earlier: SourceEvidence, _ later: SourceEvidence) -> String? {
        contextRevisionComparisonBlocker(
            earlierLaunches: earlier.measurements.exposedLaunches,
            earlierCompleteCaptures: earlier.measurements.completeCaptureLaunches,
            laterLaunches: later.measurements.exposedLaunches,
            laterCompleteCaptures: later.measurements.completeCaptureLaunches,
            earlierProviderModels: earlier.measurements.providerModels,
            laterProviderModels: later.measurements.providerModels,
            earlierFirstSeen: earlier.measurements.firstSeen,
            earlierLastSeen: earlier.measurements.lastSeen,
            laterFirstSeen: later.measurements.firstSeen,
            laterLastSeen: later.measurements.lastSeen
        )
    }

    private func comparisonMetric(_ label: String, _ earlier: String, _ later: String) -> some View {
        HStack(spacing: Spacing.sm) {
            Text(label)
                .font(Typography.caption(8))
                .foregroundStyle(palette.textSecondary)
            Spacer()
            Text("\(earlier) → \(later)")
                .font(Typography.code(9))
                .foregroundStyle(palette.text)
        }
    }

    private func fraction(_ numerator: UInt64, of denominator: UInt64) -> Double {
        guard denominator > 0 else { return 0 }
        return Double(numerator) / Double(denominator)
    }

    private func percent(_ numerator: UInt64, of denominator: UInt64) -> String {
        guard denominator > 0 else { return "Missing" }
        return fraction(numerator, of: denominator).formatted(.percent.precision(.fractionLength(0)))
    }

    private func rate(_ numerator: UInt64?, over denominator: UInt64) -> String {
        guard let numerator, denominator > 0 else { return "Missing" }
        return (Double(numerator) / Double(denominator)).formatted(.number.precision(.fractionLength(2)))
    }

    private func selectedSummary(_ snapshot: ContextLabSnapshot) -> some View {
        Group {
            if let node = findNode(selectedNodeId, in: snapshot.aggregateRoot) {
                HStack(spacing: Spacing.xl) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(node.label).font(Typography.body(13).weight(.semibold))
                        Text(node.sourcePath ?? displayKind(node.kind))
                            .font(Typography.code(9))
                            .foregroundStyle(palette.textSecondary)
                            .lineLimit(1)
                    }
                    Spacer()
                    evidenceMetric("Tokens", node.attributedTokens.formatted())
                    evidenceMetric("Runs", node.runCount.formatted())
                    evidenceMetric("Impressions", node.agentSessionCount.formatted())
                    evidenceMetric("Turns", node.turnCount.formatted())
                }
                .padding(Spacing.md)
                .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.md))
                .overlay {
                    RoundedRectangle(cornerRadius: CornerRadius.md).stroke(palette.border)
                }
            }
        }
    }

    private func coverageSummary(_ snapshot: ContextLabSnapshot) -> some View {
        let coverage = snapshot.coverage
        return VStack(alignment: .leading, spacing: Spacing.sm) {
            railTitle("Coverage")
            evidenceMetric("Attributable", "\(coverage.attributedTurns) / \(snapshot.totals.turns) turns")
            evidenceMetric("Provider total", coverage.providerTotalOnlyTurns.formatted())
            evidenceMetric("Unknown", coverage.unknownTurns.formatted())
            evidenceMetric(
                "Attribution",
                "\(snapshot.aggregateRoot.attributedTokens) / \(optionalTokens(snapshot.totals.initialPromptTokens)) tokens"
            )
            evidenceMetric(
                "Source evidence",
                "\(coverage.sourceObservableAgentSessions) / \(snapshot.totals.agentSessions) agent sessions"
            )
            evidenceMetric("Conversations", "\(coverage.conversationsAvailable) / \(snapshot.totals.agentSessions)")
        }
    }

    private func breadcrumb(_ root: ContextFlameNode) -> some View {
        HStack(spacing: Spacing.xs) {
            ForEach(Array(nodePath(to: focusNodeId, in: root).enumerated()), id: \.offset) { index, node in
                if index > 0 { Text("/").foregroundStyle(palette.textSecondary) }
                Button(node.label) { focusNodeId = node.id }
                    .buttonStyle(.plain)
                    .foregroundStyle(node.id == focusNodeId ? palette.text : palette.textSecondary)
            }
        }
        .font(Typography.code(10))
        .lineLimit(1)
    }

    private func facetPicker(
        _ title: String,
        query keyPath: WritableKeyPath<SessionSetQuery, [String]>,
        values: [String]
    ) -> some View {
        let selection = Binding(
            get: { query[keyPath: keyPath].first ?? "" },
            set: { query[keyPath: keyPath] = $0.isEmpty ? [] : [$0] }
        )
        let options = Array(Set(values + query[keyPath: keyPath])).sorted()
        return Picker(title, selection: selection) {
            Text("Any").tag("")
            ForEach(options, id: \.self) { Text($0).tag($0) }
        }
        .pickerStyle(.menu)
    }

    private func facets(_ keyPath: KeyPath<SessionLane, String?>) -> [String] {
        facets { $0[keyPath: keyPath] }
    }

    private func facets(_ value: (SessionLane) -> String?) -> [String] {
        Array(Set(snapshot?.sessions.compactMap(value) ?? [])).sorted()
    }

    private func filterToggle(_ label: String, selected: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack {
                Image(systemName: selected ? "checkmark.square.fill" : "square")
                    .foregroundStyle(selected ? palette.accent : palette.textSecondary)
                Text(label)
                Spacer()
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .font(Typography.body(11))
        .foregroundStyle(palette.text)
    }

    private func railTitle(_ title: String) -> some View {
        Text(title.uppercased())
            .font(Typography.caption(9).weight(.bold))
            .tracking(0.8)
            .foregroundStyle(palette.textSecondary)
    }

    private func evidenceMetric(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(label.uppercased())
                .font(Typography.caption(8).weight(.bold))
                .tracking(0.5)
                .foregroundStyle(palette.textSecondary)
            Text(value)
                .font(Typography.code(10))
                .foregroundStyle(palette.text)
                .lineLimit(2)
        }
    }

    private func captureBadge(_ label: String, available: Bool) -> some View {
        Label(label, systemImage: available ? "checkmark.circle.fill" : "minus.circle")
            .font(Typography.caption(8))
            .foregroundStyle(available ? Color.statusSuccess : palette.textSecondary)
    }

    private func canRefine(_ evidence: SourceEvidence, in query: SessionSetQuery) -> Bool {
        guard evidence.isEditable,
              let sourcePath = evidence.sourcePath,
              query.waves.count == 1,
              query.repoPaths.count == 1,
              let repoPath = query.repoPaths.first,
              refinementProjectId != nil
        else { return false }
        return contextRelativeSourcePath(sourcePath, repoPath: repoPath) != nil
    }

    private func refinementHelp(_ evidence: SourceEvidence, in query: SessionSetQuery) -> String {
        guard evidence.isEditable, let sourcePath = evidence.sourcePath else {
            return editabilityReason(evidence)
        }
        guard query.waves.count == 1 else {
            return "Choose one Wave before starting refinement work."
        }
        guard refinementProjectId != nil else {
            return refinementProjects.isEmpty
                ? "This Wave needs a Project before it can own a refinement Task."
                : "Choose the Project that should own refinement Tasks for this Wave."
        }
        guard query.repoPaths.count == 1, let repoPath = query.repoPaths.first else {
            return "Narrow the session set to one repo before refining this source."
        }
        guard contextRelativeSourcePath(sourcePath, repoPath: repoPath) != nil else {
            return "This source is outside the selected repo and cannot be changed in its Task worktree."
        }
        return "Create a trace-linked Task in \(query.waves[0]) and open its running agent"
    }

    @MainActor
    private func launchRefinement(
        _ evidence: SourceEvidence,
        snapshot: ContextLabSnapshot
    ) async {
        guard let wave = snapshot.query.waves.first,
              snapshot.query.waves.count == 1,
              let repoPath = snapshot.query.repoPaths.first,
              snapshot.query.repoPaths.count == 1,
              let refinementProjectId
        else { return }
        isLaunchingRefinement = true
        refinementErrorMessage = nil
        defer { isLaunchingRefinement = false }

        do {
            let refreshed = try await RegistryQueryLocal.shared.contextLab(snapshot.query)
            guard let currentEvidence = refreshed.evidence.first(where: {
                $0.nodeId == evidence.nodeId
            }),
            currentEvidence.isEditable,
            currentEvidence.contentSha256 == evidence.contentSha256,
            let sourcePath = currentEvidence.sourcePath,
            let sourceSha256 = currentEvidence.currentSourceSha256,
            sourceFileHash(path: sourcePath) == sourceSha256
            else {
                throw ContextRefinementError(
                    "Main's source changed. Refresh Context Lab and select the current revision."
                )
            }

            let plan = try await RegistryQueryLocal.shared.plan(
                wave: wave,
                objective: "",
                cwd: repoPath,
                sync: true
            )
            let project = try contextRefinementProject(
                plan.projects,
                projectId: refinementProjectId
            )
            let relativePath = try contextRefinementSourcePath(
                sourcePath: sourcePath,
                repoPath: repoPath
            )
            let title = contextRefinementTaskTitle(
                label: currentEvidence.label,
                contentSha256: currentEvidence.contentSha256
            )
            let seed = RefinementSeed(
                query: snapshot.query,
                selectedNodeId: currentEvidence.nodeId,
                sourcePath: relativePath,
                startingContentSha256: currentEvidence.contentSha256,
                measurements: currentEvidence.measurements,
                evidence: currentEvidence.representatives.map(\.address)
            )
            let directive = try contextRefinementDirective(
                label: currentEvidence.label,
                sourcePath: relativePath,
                sourceSha256: sourceSha256,
                seed: seed
            )
            guard sourceFileHash(path: sourcePath) == sourceSha256 else {
                throw ContextRefinementError(
                    "Main's source changed while resolving the Task destination. Refresh Context Lab and retry."
                )
            }

            let receipt = try await Task.detached(priority: .userInitiated) {
                try LocalWaveAgentLauncher.startTask(
                    repoPath: repoPath,
                    title: title,
                    project: project.id,
                    directive: directive
                )
            }.value
            guard receipt.wave == wave, receipt.project == project.id else {
                throw ContextRefinementError(
                    "The created Task receipt did not match \(wave) / \(project.id). Open the Wave work map before retrying."
                )
            }
            openWindow(id: "task-workspace", value: TaskWorkspaceRoute(
                issue: receipt.issueIdentifier,
                context: ContextLabRoute(
                    query: snapshot.query,
                    selectedNodeId: currentEvidence.nodeId,
                    focusNodeId: focusNodeId,
                    mode: .sources
                )
            ))
        } catch {
            refinementErrorMessage = error.localizedDescription
        }
    }

    private func selectedEvidence(in snapshot: ContextLabSnapshot) -> [SourceEvidence] {
        if let direct = snapshot.evidence.first(where: { $0.nodeId == selectedNodeId }) {
            return [direct]
        }
        guard let node = findNode(selectedNodeId, in: snapshot.aggregateRoot),
              node.level == .source || node.level == .revision
        else { return [] }
        let ids = revisionIds(for: node.id, root: snapshot.aggregateRoot)
        return snapshot.evidence.filter { ids.contains($0.nodeId) }
    }

    private func sortedSessions(_ sessions: [SessionLane], selectedIds: Set<String>) -> [SessionLane] {
        sessions.sorted { left, right in
            switch laneSort {
            case .context:
                return sessionTokens(left) > sessionTokens(right)
            case .lifetimeInput:
                return (left.lifetimeInputTokens ?? 0) > (right.lifetimeInputTokens ?? 0)
            case .windowPressure:
                return (left.peakContextPercent ?? 0) > (right.peakContextPercent ?? 0)
            case .selectedShare:
                let leftSelected = selectedTokens(left, ids: selectedIds)
                let rightSelected = selectedTokens(right, ids: selectedIds)
                let leftShare = contextSelectedSourceShare(
                    selectedTokens: leftSelected,
                    contextTokens: sessionTokens(left)
                )
                let rightShare = contextSelectedSourceShare(
                    selectedTokens: rightSelected,
                    contextTokens: sessionTokens(right)
                )
                if leftShare != rightShare { return leftShare > rightShare }
                return leftSelected > rightSelected
            case .outcome:
                return outcomeRank(left.outcome) > outcomeRank(right.outcome)
            case .steering:
                return (left.steeringTurns ?? 0) > (right.steeringTurns ?? 0)
            case .time:
                return left.startedAt > right.startedAt
            }
        }
    }

    private func sortedSources(_ snapshot: ContextLabSnapshot) -> [InstructionSourceSummary] {
        snapshot.sources
            .filter { source in
                sourceSearch.isEmpty
                    || source.label.localizedCaseInsensitiveContains(sourceSearch)
                    || source.sourcePath.localizedCaseInsensitiveContains(sourceSearch)
            }
            .sorted { left, right in
            switch nodeSort {
            case .impressions:
                switch (left.impressions, right.impressions) {
                case let (.some(leftValue), .some(rightValue)) where leftValue != rightValue:
                    return leftValue > rightValue
                case (.some, .none):
                    return true
                case (.none, .some):
                    return false
                default:
                    break
                }
                return left.label.localizedStandardCompare(right.label) == .orderedAscending
            case .recent:
                return (left.lastSeen ?? .min) > (right.lastSeen ?? .min)
            case .label:
                return left.label.localizedStandardCompare(right.label) == .orderedAscending
            }
        }
    }

    private func sourceLastSeen(_ source: InstructionSourceSummary) -> String {
        guard let timestamp = source.lastSeen else { return "Never" }
        return Date(timeIntervalSince1970: TimeInterval(timestamp))
            .formatted(date: .abbreviated, time: .omitted)
    }

    private func currentEvidence(
        for source: InstructionSourceSummary,
        in snapshot: ContextLabSnapshot
    ) -> SourceEvidence? {
        guard let nodeId = source.currentRevisionNodeId else { return nil }
        return snapshot.evidence.first { $0.nodeId == nodeId }
    }

    private func selectedInstructionSource(
        in snapshot: ContextLabSnapshot
    ) -> InstructionSourceSummary? {
        snapshot.sources.first { source in
            source.id == selectedNodeId || source.currentRevisionNodeId == selectedNodeId
        }
    }

    private func refresh() async {
        guard query.waves.count == 1 else {
            snapshot = nil
            errorMessage = nil
            isLoading = false
            return
        }
        isLoading = true
        defer {
            if !Task.isCancelled { isLoading = false }
        }
        do {
            let next = try await RegistryQueryLocal.shared.contextLab(query)
            try Task.checkCancellation()
            snapshot = next
            errorMessage = nil
            if findNode(selectedNodeId, in: next.aggregateRoot) == nil
                && !next.evidence.contains(where: { $0.nodeId == selectedNodeId }) {
                selectedNodeId = "session-set"
            }
            if findNode(focusNodeId, in: next.aggregateRoot) == nil {
                focusNodeId = "session-set"
            }
        } catch is CancellationError {
            return
        } catch {
            guard !Task.isCancelled else { return }
            errorMessage = error.localizedDescription
        }
    }

    @MainActor
    private func loadRefinementProjects() async {
        refinementProjects = []
        refinementProjectId = nil
        refinementProjectLoadError = nil
        guard query.waves.count == 1,
              let wave = query.waves.first,
              query.repoPaths.count == 1,
              let repoPath = query.repoPaths.first
        else { return }

        isLoadingRefinementProjects = true
        defer {
            if !Task.isCancelled { isLoadingRefinementProjects = false }
        }
        do {
            let plan = try await RegistryQueryLocal.shared.plan(
                wave: wave,
                objective: "",
                cwd: repoPath
            )
            try Task.checkCancellation()
            refinementProjects = plan.projects.sorted {
                $0.title.localizedStandardCompare($1.title) == .orderedAscending
            }
            if refinementProjects.count == 1 {
                refinementProjectId = refinementProjects[0].id
            } else if let stored = UserDefaults.standard.string(
                forKey: refinementProjectPreferenceKey(wave: wave, repoPath: repoPath)
            ), refinementProjects.contains(where: { $0.id == stored }) {
                refinementProjectId = stored
            }
        } catch is CancellationError {
            return
        } catch {
            guard !Task.isCancelled else { return }
            refinementProjectLoadError = error.localizedDescription
        }
    }

    @ViewBuilder
    private var refinementProjectDestination: some View {
        if isLoadingRefinementProjects {
            HStack(spacing: Spacing.sm) {
                ProgressView().controlSize(.small)
                Text("Loading Task destination…")
            }
            .font(Typography.caption(10))
            .foregroundStyle(palette.textSecondary)
        } else if let refinementProjectLoadError {
            Label(refinementProjectLoadError, systemImage: "exclamationmark.triangle")
                .font(Typography.caption(10))
                .foregroundStyle(Color.statusError)
        } else if refinementProjects.count > 1 {
            Picker("Refinement Project", selection: refinementProjectBinding) {
                Text("Choose Project").tag(String?.none)
                ForEach(refinementProjects) { project in
                    Text(project.title).tag(Optional(project.id))
                }
            }
            .pickerStyle(.menu)
            .help("Remembered for refinement Tasks in this Wave; does not filter Context Lab evidence")
        } else if let project = refinementProjects.first {
            evidenceMetric("Refinement Project", project.title)
        } else if query.waves.count == 1 {
            Label("This Wave has no Project to own a refinement Task.", systemImage: "tray")
                .font(Typography.caption(10))
                .foregroundStyle(Color.statusWarning)
        }
    }

    private var refinementProjectBinding: Binding<String?> {
        Binding(
            get: { refinementProjectId },
            set: { projectId in
                refinementProjectId = projectId
                guard let projectId,
                      let wave = query.waves.first,
                      let repoPath = query.repoPaths.first
                else { return }
                UserDefaults.standard.set(
                    projectId,
                    forKey: refinementProjectPreferenceKey(wave: wave, repoPath: repoPath)
                )
            }
        )
    }

    private func refinementProjectPreferenceKey(wave: String, repoPath: String) -> String {
        "contextLabRefinementProject:\(repoPath.normalizedFilePath):\(wave)"
    }

    private func clearFilters() {
        query = defaultQuery
        query.startedBefore = Int64(Date().timeIntervalSince1970)
        query.startedAfter = query.startedBefore - 30 * 24 * 60 * 60
    }

    private func saveCurrentView() {
        let saved = ContextLabSavedView(query: query, mode: mode)
        savedViews.removeAll { $0 == saved }
        savedViews.append(saved)
        persistSavedViews()
    }

    private func apply(_ saved: ContextLabSavedView) {
        var savedQuery = saved.query
        savedQuery.repoPaths = defaultQuery.repoPaths
        savedQuery.waves = defaultQuery.waves
        savedQuery.projects = []
        savedQuery.tasks = []
        query = savedQuery
        mode = saved.mode
    }

    private func delete(_ saved: ContextLabSavedView) {
        savedViews.removeAll { $0 == saved }
        persistSavedViews()
    }

    private func persistSavedViews() {
        guard let data = try? JSONEncoder().encode(savedViews) else { return }
        UserDefaults.standard.set(data, forKey: savedViewsKey)
    }

    private static func loadSavedViews(key: String) -> [ContextLabSavedView] {
        guard let data = UserDefaults.standard.data(forKey: key),
              let views = try? JSONDecoder().decode([ContextLabSavedView].self, from: data)
        else { return [] }
        var seen = Set<ContextLabSavedView>()
        return views.filter { seen.insert($0).inserted }
    }

    private static func savedViewsKey(repoPath: String, wave: String) -> String {
        "contextLabSavedViews:\(repoPath.normalizedFilePath):\(wave)"
    }

    private var filtersAreEmpty: Bool {
        var filters = query
        filters.startedAfter = defaultQuery.startedAfter
        filters.startedBefore = defaultQuery.startedBefore
        return windowDays == 30 && filters == defaultQuery
    }
}

func contextRelativeSourcePath(_ sourcePath: String, repoPath: String) -> String? {
    let source = sourcePath.normalizedFilePath
    let normalizedRepo = repoPath.normalizedFilePath
    let repo = normalizedRepo.count > 1 && normalizedRepo.hasSuffix("/")
        ? String(normalizedRepo.dropLast())
        : normalizedRepo
    guard source.hasPrefix(repo + "/") else { return nil }
    return String(source.dropFirst(repo.count + 1))
}

private struct ContextSourceDocumentView: View {
    let source: InstructionSourceSummary

    @Environment(\.palette) private var palette
    @State private var content: String?
    @State private var errorMessage: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text(source.label)
                    .font(Typography.sectionTitle(19))
                    .foregroundStyle(palette.text)
                Text(source.sourcePath)
                    .font(Typography.code(9))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                Text("Main's current file")
                    .font(Typography.caption(9).weight(.semibold))
                    .foregroundStyle(palette.accent)
            }
            .padding(Spacing.lg)
            Divider()
            if let content {
                GeometryReader { proxy in
                    ScrollView([.horizontal, .vertical]) {
                        Text(content)
                            .font(Typography.code(10))
                            .foregroundStyle(palette.text)
                            .textSelection(.enabled)
                            .padding(Spacing.lg)
                            .frame(
                                minWidth: proxy.size.width,
                                minHeight: proxy.size.height,
                                alignment: .topLeading
                            )
                    }
                }
            } else if let errorMessage {
                ContentUnavailableView(
                    "Source unavailable",
                    systemImage: "doc.questionmark",
                    description: Text(errorMessage)
                )
            } else {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(palette.background)
        .task { await load() }
    }

    private func load() async {
        do {
            content = try await Task.detached(priority: .userInitiated) {
                try String(contentsOfFile: source.sourcePath, encoding: .utf8)
            }.value
            errorMessage = nil
        } catch {
            content = nil
            errorMessage = error.localizedDescription
        }
    }
}

private struct ContextStat: View {
    @Environment(\.palette) private var palette
    let label: String
    let value: String
    var denominator: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(label.uppercased())
                .font(Typography.caption(8).weight(.bold))
                .tracking(0.5)
                .foregroundStyle(palette.textSecondary)
            Text(value)
                .font(Typography.code(11))
                .foregroundStyle(palette.text)
            if let denominator {
                Text(denominator)
                    .font(Typography.caption(8))
                    .foregroundStyle(palette.textSecondary)
            }
        }
        .frame(minWidth: 86, alignment: .leading)
    }
}

private struct FlameRectangle: Identifiable {
    let node: ContextFlameNode
    let depth: Int
    let start: UInt64

    var id: String { "\(depth)-\(node.id)" }
}

private struct ContextIcicle: View {
    @Environment(\.palette) private var palette
    let root: ContextFlameNode
    let selectedNodeId: String
    let opacity: Double
    let onSelect: (ContextFlameNode) -> Void

    private var rectangles: [FlameRectangle] {
        var result: [FlameRectangle] = []
        append(root, depth: 0, start: 0, to: &result)
        return result
    }

    private var depth: Int {
        (rectangles.map(\.depth).max() ?? 0) + 1
    }

    var body: some View {
        GeometryReader { geometry in
            let width = geometry.size.width
            let denominator = max(root.attributedTokens, 1)
            ZStack(alignment: .topLeading) {
                ForEach(rectangles) { item in
                    let itemWidth = width * Double(item.node.attributedTokens) / Double(denominator)
                    let offset = width * Double(item.start) / Double(denominator)
                    Button {
                        onSelect(item.node)
                    } label: {
                        Text(item.node.label)
                            .font(Typography.caption(9))
                            .foregroundStyle(Color.white)
                            .lineLimit(1)
                            .padding(.horizontal, 5)
                            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
                            .background(contextColor(item.node.kind).opacity(opacity))
                            .overlay {
                                Rectangle().stroke(
                                    item.node.id == selectedNodeId ? Color.white : palette.background.opacity(0.65),
                                    lineWidth: item.node.id == selectedNodeId ? 2 : 1
                                )
                            }
                    }
                    .buttonStyle(.plain)
                    .frame(width: max(itemWidth, 1), height: 27)
                    .offset(x: offset, y: CGFloat(item.depth * 27))
                    .accessibilityLabel("\(item.node.label), \(item.node.attributedTokens) tokens")
                    .accessibilityHint("Select this context segment")
                }
            }
        }
        .frame(height: CGFloat(depth * 27))
    }

    private func append(
        _ node: ContextFlameNode,
        depth: Int,
        start: UInt64,
        to result: inout [FlameRectangle]
    ) {
        result.append(FlameRectangle(node: node, depth: depth, start: start))
        var cursor = start
        for child in node.children {
            append(child, depth: depth + 1, start: cursor, to: &result)
            cursor += child.attributedTokens
        }
    }
}

private struct SessionLaneView: View {
    @Environment(\.palette) private var palette
    let session: SessionLane
    let maximumTokens: UInt64
    let selectedNodeIds: Set<String>
    let onSelect: (ContextLaneAsset) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack {
                Text(shortHash(session.runId))
                    .font(Typography.code(10))
                Text(session.skill ?? session.flow ?? "inline")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                Spacer()
                Text("\(session.provider)\(session.model.map { ":\($0)" } ?? "")")
                    .font(Typography.code(9))
                    .foregroundStyle(palette.textSecondary)
                Text("life \(optionalTokens(session.lifetimeInputTokens))")
                    .font(Typography.code(9))
                    .foregroundStyle(palette.textSecondary)
                Text("peak \(optionalPercent(session.peakContextPercent))")
                    .font(Typography.code(9))
                    .foregroundStyle(palette.textSecondary)
                Text(session.outcome.rawValue)
                    .font(Typography.caption(9))
                    .foregroundStyle(outcomeColor(session.outcome))
            }
            ForEach(session.turns) { turn in
                HStack(spacing: Spacing.sm) {
                    Text("T\(turn.ordinal)")
                        .font(Typography.code(9))
                        .foregroundStyle(palette.textSecondary)
                        .frame(width: 28, alignment: .trailing)
                    GeometryReader { geometry in
                        ZStack(alignment: .leading) {
                            Rectangle().fill(palette.surfaceMuted)
                            HStack(spacing: 0) {
                                ForEach(Array(turn.assets.enumerated()), id: \.offset) { _, asset in
                                    Button { onSelect(asset) } label: {
                                        Rectangle()
                                            .fill(contextColor(asset.kind).opacity(0.82))
                                            .overlay {
                                                if selectedNodeIds.contains(asset.nodeId) {
                                                    Rectangle().stroke(Color.white, lineWidth: 2)
                                                }
                                            }
                                    }
                                    .buttonStyle(.plain)
                                    .frame(width: max(
                                        1,
                                        geometry.size.width * Double(asset.attributedTokens) / Double(max(maximumTokens, 1))
                                    ))
                                    .help("\(asset.label) · \(asset.attributedTokens) tokens")
                                    .accessibilityLabel("\(asset.label), \(asset.attributedTokens) tokens")
                                }
                                Spacer(minLength: 0)
                            }
                        }
                    }
                    .frame(height: 18)
                    Text(turn.suppliedContextTokens?.formatted() ?? "missing")
                        .font(Typography.code(9))
                        .foregroundStyle(turn.suppliedContextTokens == nil ? Color.statusWarning : palette.textSecondary)
                        .frame(width: 58, alignment: .trailing)
                }
            }
        }
        .padding(Spacing.md)
        .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.md))
        .overlay { RoundedRectangle(cornerRadius: CornerRadius.md).stroke(palette.border) }
    }
}

private func findNode(_ id: String, in node: ContextFlameNode) -> ContextFlameNode? {
    if node.id == id { return node }
    for child in node.children {
        if let match = findNode(id, in: child) { return match }
    }
    return nil
}

private func nodePath(to id: String, in node: ContextFlameNode) -> [ContextFlameNode] {
    if node.id == id { return [node] }
    for child in node.children {
        let path = nodePath(to: id, in: child)
        if !path.isEmpty { return [node] + path }
    }
    return []
}

private func descendants(of node: ContextFlameNode) -> [ContextFlameNode] {
    node.children.flatMap { [$0] + descendants(of: $0) }
}

private func revisionIds(for id: String, root: ContextFlameNode) -> Set<String> {
    guard let node = findNode(id, in: root) else { return [] }
    if node.level == .revision { return [node.id] }
    return Set(descendants(of: node).filter { $0.level == .revision }.map(\.id))
}

private func maximumTurnTokens(_ sessions: [SessionLane]) -> UInt64 {
    sessions
        .flatMap(\.turns)
        .map { $0.suppliedContextTokens ?? $0.assets.reduce(0) { $0 + $1.attributedTokens } }
        .max() ?? 1
}

private func sessionTokens(_ session: SessionLane) -> UInt64 {
    session.turns.reduce(0) { $0 + ($1.suppliedContextTokens ?? 0) }
}

private func selectedTokens(_ session: SessionLane, ids: Set<String>) -> UInt64 {
    session.turns.flatMap(\.assets)
        .filter { ids.contains($0.nodeId) }
        .reduce(0) { $0 + $1.attributedTokens }
}

private func outcomeRank(_ outcome: SessionOutcome) -> Int {
    switch outcome {
    case .failed: 4
    case .interrupted: 3
    case .running: 2
    case .completed: 1
    }
}

private func outcomeColor(_ outcome: SessionOutcome) -> Color {
    switch outcome {
    case .completed: .statusSuccess
    case .failed: .statusError
    case .interrupted: .statusWarning
    case .running: .blue
    }
}

private func contextColor(_ kind: ContextAssetKind?) -> Color {
    guard let kind else { return Color(hex: 0x62666B) }
    return switch kind {
    case .operatingInstructions: Color(hex: 0x7D2948)
    case .surfaceInstructions: Color(hex: 0xA83F5B)
    case .providerInstructions: Color(hex: 0x6C4AA3)
    case .repoInstructions: Color(hex: 0x2F6F8F)
    case .skillInstructions: Color(hex: 0x447C69)
    case .direction: Color(hex: 0x668A3C)
    case .goal: Color(hex: 0xB2762C)
    case .memory: Color(hex: 0x8A6238)
    case .chat: Color(hex: 0x8B4C87)
    case .summary: Color(hex: 0x4F6C8A)
    case .document: Color(hex: 0x5E7185)
    case .scratch: Color(hex: 0x94745C)
    case .diff: Color(hex: 0xA34A3E)
    case .clipboard: Color(hex: 0x5E5A9C)
    case .userMessage: Color(hex: 0x3E7C8C)
    case .assembly: Color(hex: 0x62666B)
    }
}

private func optionalTokens(_ value: UInt64?) -> String {
    value?.formatted() ?? "Missing"
}

private func optionalPercent(_ value: Double?) -> String {
    value.map { "\($0.formatted(.number.precision(.fractionLength(1))))%" } ?? "Missing"
}

private func share(_ numerator: UInt64?, of denominator: UInt64?) -> String {
    guard let numerator, let denominator, denominator > 0 else { return "Missing" }
    return (Double(numerator) / Double(denominator)).formatted(.percent.precision(.fractionLength(1)))
}

func contextSelectedSourceShare(selectedTokens: UInt64, contextTokens: UInt64) -> Double {
    guard contextTokens > 0 else { return 0 }
    return Double(selectedTokens) / Double(contextTokens)
}

func contextRevisionComparisonBlocker(
    earlierLaunches: UInt64,
    earlierCompleteCaptures: UInt64,
    laterLaunches: UInt64,
    laterCompleteCaptures: UInt64,
    earlierProviderModels: [ProviderModelExposure],
    laterProviderModels: [ProviderModelExposure],
    earlierFirstSeen: Int64?,
    earlierLastSeen: Int64?,
    laterFirstSeen: Int64?,
    laterLastSeen: Int64?
) -> String? {
    let minimumLaunches: UInt64 = 3
    guard earlierLaunches >= minimumLaunches, laterLaunches >= minimumLaunches else {
        return "Unavailable until each revision has at least \(minimumLaunches) exposed launches "
            + "(\(earlierLaunches) and \(laterLaunches) captured)."
    }
    let earlierCoverage = Double(earlierCompleteCaptures) / Double(earlierLaunches)
    let laterCoverage = Double(laterCompleteCaptures) / Double(laterLaunches)
    guard abs(earlierCoverage - laterCoverage) <= 0.1 + 1e-12 else {
        let earlierPercent = earlierCoverage.formatted(.percent.precision(.fractionLength(0)))
        let laterPercent = laterCoverage.formatted(.percent.precision(.fractionLength(0)))
        return "Unavailable because complete-capture coverage differs by more than 10 percentage points "
            + "(\(earlierPercent) vs \(laterPercent))."
    }
    guard let earlierMix = providerModelDistribution(
        earlierProviderModels,
        exposedLaunches: earlierLaunches
    ), let laterMix = providerModelDistribution(
        laterProviderModels,
        exposedLaunches: laterLaunches
    ) else {
        return "Unavailable because provider/model exposure is missing for one or both revisions."
    }
    let providerModels = Set(earlierMix.keys).union(laterMix.keys)
    let mixDistance = providerModels.reduce(0.0) { distance, providerModel in
        distance + abs(
            earlierMix[providerModel, default: 0]
                - laterMix[providerModel, default: 0]
        )
    } / 2
    guard mixDistance <= 0.2 + 1e-12 else {
        let distance = mixDistance.formatted(.percent.precision(.fractionLength(0)))
        return "Unavailable because provider/model mix differs by more than 20 percentage points "
            + "(\(distance) distribution distance)."
    }
    guard let earlierFirstSeen,
          let earlierLastSeen,
          let laterFirstSeen,
          let laterLastSeen,
          earlierFirstSeen <= earlierLastSeen,
          laterFirstSeen <= laterLastSeen
    else {
        return "Unavailable because one or both revision observation windows are missing."
    }
    let earlierSpan = earlierLastSeen - earlierFirstSeen
    let laterSpan = laterLastSeen - laterFirstSeen
    guard earlierSpan > 0, laterSpan > 0 else {
        return "Unavailable until each revision has observations at more than one time."
    }
    let shorterSpan = min(earlierSpan, laterSpan)
    let longerSpan = max(earlierSpan, laterSpan)
    guard Double(longerSpan) / Double(shorterSpan) <= 2 + 1e-12 else {
        return "Unavailable because revision observation spans differ by more than 2× "
            + "(\(observationDuration(earlierSpan)) vs \(observationDuration(laterSpan)))."
    }
    return nil
}

private struct ProviderModelKey: Hashable {
    let provider: String
    let model: String?
}

private func providerModelDistribution(
    _ exposures: [ProviderModelExposure],
    exposedLaunches: UInt64
) -> [ProviderModelKey: Double]? {
    guard exposedLaunches > 0 else { return nil }
    let capturedLaunches = exposures.reduce(UInt64(0)) { $0 + $1.exposedLaunches }
    guard capturedLaunches == exposedLaunches else { return nil }
    var distribution: [ProviderModelKey: Double] = [:]
    for exposure in exposures {
        let key = ProviderModelKey(provider: exposure.provider, model: exposure.model)
        distribution[key, default: 0] += Double(exposure.exposedLaunches) / Double(exposedLaunches)
    }
    return distribution
}

private func providerModelSummary(_ exposures: [ProviderModelExposure]) -> String {
    guard !exposures.isEmpty else { return "Missing" }
    return exposures.map { exposure in
        "\(exposure.provider)/\(exposure.model ?? "model missing") \(exposure.exposedLaunches)"
    }.joined(separator: " · ")
}

private func observationRange(first: Int64?, last: Int64?) -> String {
    guard let first, let last else { return "Missing" }
    let firstDate = Date(timeIntervalSince1970: TimeInterval(first))
        .formatted(date: .abbreviated, time: .shortened)
    let lastDate = Date(timeIntervalSince1970: TimeInterval(last))
        .formatted(date: .abbreviated, time: .shortened)
    return "\(firstDate) – \(lastDate)"
}

private func observationSpan(first: Int64?, last: Int64?) -> String {
    guard let first, let last, last >= first else { return "Missing" }
    return observationDuration(last - first)
}

private func observationDuration(_ seconds: Int64) -> String {
    let magnitude = abs(seconds)
    if magnitude >= 24 * 60 * 60 {
        return (Double(seconds) / Double(24 * 60 * 60))
            .formatted(.number.precision(.fractionLength(1))) + "d"
    }
    if magnitude >= 60 * 60 {
        return (Double(seconds) / Double(60 * 60))
            .formatted(.number.precision(.fractionLength(1))) + "h"
    }
    if magnitude >= 60 {
        return (Double(seconds) / 60)
            .formatted(.number.precision(.fractionLength(1))) + "m"
    }
    return "\(seconds)s"
}

private func displayKind(_ kind: ContextAssetKind?) -> String {
    if kind == .assembly { return "Unattributed" }
    return kind?.rawValue.replacingOccurrences(of: "_", with: " ").capitalized ?? "Session set"
}

private func displayCapture(_ capture: CaptureState) -> String {
    capture.rawValue.replacingOccurrences(of: "_", with: " ").capitalized
}

private func displayRole(_ role: EvidenceRole) -> String {
    role.rawValue.replacingOccurrences(of: "_", with: " ").capitalized
}

private func shortHash(_ hash: String) -> String {
    String(hash.prefix(10))
}

private func windowLabel(_ query: SessionSetQuery) -> String {
    let start = Date(timeIntervalSince1970: TimeInterval(query.startedAfter))
    let end = Date(timeIntervalSince1970: TimeInterval(query.startedBefore))
    return "\(start.formatted(date: .abbreviated, time: .omitted)) – \(end.formatted(date: .abbreviated, time: .omitted))"
}

private func editabilityReason(_ evidence: SourceEvidence) -> String {
    if evidence.kind == .goal || evidence.kind == .memory {
        return "This segment is assembled from layered Wave state, not one editable file revision."
    }
    if evidence.sourcePath == nil { return "This segment has no canonical source file." }
    if evidence.currentContentSha256 == nil { return "The canonical source is missing." }
    return "The source changed since this revision was captured. Select the current revision or restore the file."
}

private func toggle<Value: Equatable>(_ value: Value, in values: inout [Value]) {
    if let index = values.firstIndex(of: value) {
        values.remove(at: index)
    } else {
        values.append(value)
    }
}
#endif
