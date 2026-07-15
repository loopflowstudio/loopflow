#if os(macOS)
import CryptoKit
import Loopflow
import SwiftUI

enum ContextLabMode: String, Codable, CaseIterable, Identifiable, Hashable {
    case aggregate = "Aggregate flame"
    case lanes = "Session lanes"
    case table = "Table"

    var id: String { rawValue }
}

private enum ContextLaneSort: String, CaseIterable, Identifiable {
    case context = "Total context"
    case selectedShare = "Selected-source share"
    case outcome = "Outcome"
    case steering = "Steering"
    case time = "Recent"

    var id: String { rawValue }
}

private enum ContextNodeSort: String, CaseIterable, Identifiable {
    case tokens = "Tokens"
    case sessions = "Sessions"
    case turns = "Turns"
    case label = "Name"

    var id: String { rawValue }
}

private struct ContextLabSavedView: Codable, Hashable {
    let query: SessionSetQuery
    let mode: ContextLabMode

    var name: String {
        let repo = query.repoPaths.first
            .flatMap { URL(fileURLWithPath: $0).lastPathComponent }
            ?? "All repos"
        let days = max(1, (query.startedBefore - query.startedAfter) / (24 * 60 * 60))
        let end = Date(timeIntervalSince1970: TimeInterval(query.startedBefore))
        return "\(repo) · \(days)d · \(end.formatted(date: .abbreviated, time: .omitted))"
    }
}

struct TaskWorkspaceRoute: Codable, Hashable {
    let wave: String
    let issue: String
    let repoPath: String
    let initialSection: TaskWorkspaceSection
    let context: ContextLabRoute
}

struct ContextLabRoute: Codable, Hashable {
    let query: SessionSetQuery
    let selectedNodeId: String
    let focusNodeId: String
    let mode: ContextLabMode
}

struct ContextLabView: View {
    private let defaultQuery: SessionSetQuery

    @Environment(\.palette) private var palette
    @Environment(\.openWindow) private var openWindow

    @State private var query: SessionSetQuery
    @State private var snapshot: ContextLabSnapshot?
    @State private var selectedNodeId = "session-set"
    @State private var focusNodeId = "session-set"
    @State private var mode = ContextLabMode.aggregate
    @State private var laneSort = ContextLaneSort.context
    @State private var nodeSort = ContextNodeSort.tokens
    @State private var repoDraft: String
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var traceRequest: TraceAddress?
    @State private var refinementEvidence: SourceEvidence?
    @State private var savedViews: [ContextLabSavedView]

    init(initialRepoPath: String?, route: ContextLabRoute? = nil) {
        let resolvedInitialPath = initialRepoPath.map(WaveOrigin.resolve)
        let now = Int64(Date().timeIntervalSince1970)
        let defaultQuery = SessionSetQuery(
            repoPaths: resolvedInitialPath.map { [$0] } ?? [],
            startedAfter: now - 30 * 24 * 60 * 60,
            startedBefore: now,
            waves: [],
            projects: [],
            tasks: [],
            flows: [],
            skills: [],
            providers: [],
            models: [],
            surfaces: [],
            outcomes: [],
            captureStates: []
        )
        self.defaultQuery = defaultQuery
        var initialQuery = route?.query ?? defaultQuery
        initialQuery.repoPaths = initialQuery.repoPaths.map(WaveOrigin.resolve)
        _query = State(initialValue: initialQuery)
        _repoDraft = State(initialValue: initialQuery.repoPaths.first ?? "")
        if let route {
            _selectedNodeId = State(initialValue: route.selectedNodeId)
            _focusNodeId = State(initialValue: route.focusNodeId)
            _mode = State(initialValue: route.mode)
        }
        _savedViews = State(initialValue: Self.loadSavedViews())
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

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            HSplitView {
                filterRail
                    .frame(minWidth: 190, idealWidth: 220, maxWidth: 260)
                center
                    .frame(minWidth: 600, maxWidth: .infinity)
                evidenceRail
                    .frame(minWidth: 270, idealWidth: 310, maxWidth: 360)
            }
        }
        .background(palette.background)
        .task(id: query) { await refresh() }
        .sheet(item: $traceRequest) { address in
            TraceEvidenceView(address: address)
                .frame(minWidth: 760, minHeight: 620)
        }
        .sheet(item: $refinementEvidence) { evidence in
            RefinementTaskSheet(
                query: snapshot?.query ?? query,
                evidence: evidence,
                backlink: ContextLabRoute(
                    query: snapshot?.query ?? query,
                    selectedNodeId: evidence.nodeId,
                    focusNodeId: focusNodeId,
                    mode: mode
                ),
                onLaunch: { route in
                    refinementEvidence = nil
                    openWindow(id: "task-workspace", value: route)
                }
            )
            .frame(minWidth: 560, minHeight: 440)
        }
    }

    private var header: some View {
        VStack(spacing: Spacing.md) {
            HStack(spacing: Spacing.lg) {
                VStack(alignment: .leading, spacing: 1) {
                    Text("Context Lab")
                        .font(Typography.heroTitle(26))
                        .foregroundStyle(palette.text)
                    Text("What text shaped this session set")
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
                ContextStat(label: "Sessions", value: totals.sessions.formatted())
                ContextStat(label: "Launches", value: totals.launches.formatted())
                ContextStat(
                    label: "Assembled turns",
                    value: "\(snapshot.coverage.assembledTurns.formatted()) / \(totals.turns.formatted())"
                )
                ContextStat(
                    label: "Context tokens",
                    value: optionalTokens(totals.contextTokens),
                    denominator: "\(snapshot.coverage.assembledTurns) measured turns"
                )
                ContextStat(label: "Median / p95", value: "\(optionalTokens(totals.medianContextTokens)) / \(optionalTokens(totals.p95ContextTokens))")
                ContextStat(
                    label: "Instruction share",
                    value: share(totals.instructionTokens, of: totals.contextTokens),
                    denominator: "attributed / supplied"
                )
                ContextStat(
                    label: "Cost",
                    value: totals.costUsd?.formatted(.currency(code: "USD")) ?? "Missing",
                    denominator: "\(totals.costTurns) measured turns"
                )
                ContextStat(
                    label: "Outcomes",
                    value: "\(totals.completedLaunches) done · \(totals.failedLaunches) failed",
                    denominator: "\(totals.launches) launches"
                )
                ContextStat(
                    label: "Steering",
                    value: "\(totals.steeringTurns) turns",
                    denominator: "\(totals.steeredLaunches) launches"
                )
                ContextStat(
                    label: "Capture",
                    value: "\(snapshot.coverage.completeLaunches) / \(totals.launches) complete",
                    denominator: "prompts \(snapshot.coverage.promptArtifactsAvailable) / \(totals.turns)"
                )
            }
        }
    }

    private var filterRail: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.lg) {
                railTitle("Session set")
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    Text("Repo").font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                    TextField("All local repos", text: $repoDraft)
                        .textFieldStyle(.roundedBorder)
                        .font(Typography.code(10))
                        .onSubmit { applyRepoDraft() }
                    if repoDraft != (query.repoPaths.first ?? "") {
                        Button("Apply repo") { applyRepoDraft() }
                        .buttonStyle(.borderless)
                        .font(Typography.caption(10))
                    }
                }
                Picker("Window", selection: windowDaysBinding) {
                    Text("7 days").tag(7)
                    Text("30 days").tag(30)
                    Text("90 days").tag(90)
                }
                .pickerStyle(.menu)

                facetPicker("Wave", query: \.waves, values: facets(\.wave))
                facetPicker("Project", query: \.projects, values: facets(\.project))
                facetPicker("Task", query: \.tasks, values: facets(\.task))
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
                case .table:
                    nodeTable(snapshot)
                }
            } else {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(palette.background)
    }

    private var centerToolbar: some View {
        HStack(spacing: Spacing.md) {
            if let snapshot {
                breadcrumb(snapshot.aggregateRoot)
            }
            Spacer()
            if mode == .lanes {
                Picker("Sort", selection: $laneSort) {
                    ForEach(ContextLaneSort.allCases) { Text($0.rawValue).tag($0) }
                }
                .labelsHidden()
                .frame(width: 190)
            } else if mode == .table {
                Picker("Sort", selection: $nodeSort) {
                    ForEach(ContextNodeSort.allCases) { Text($0.rawValue).tag($0) }
                }
                .labelsHidden()
                .frame(width: 120)
            }
            Picker("Mode", selection: $mode) {
                ForEach(ContextLabMode.allCases) { Text($0.rawValue).tag($0) }
            }
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
            : 0.35 + 0.65 * Double(snapshot.coverage.assembledTurns) / Double(snapshot.totals.turns)
        return ScrollView {
            VStack(alignment: .leading, spacing: Spacing.xl) {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(focus.label)
                            .font(Typography.sectionTitle(20))
                            .foregroundStyle(palette.text)
                        Text("Width is supplied tokens · opacity is session-set capture coverage")
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

    private func nodeTable(_ snapshot: ContextLabSnapshot) -> some View {
        let focus = findNode(focusNodeId, in: snapshot.aggregateRoot) ?? snapshot.aggregateRoot
        return ScrollView {
            LazyVStack(spacing: 0) {
                HStack {
                    Text("Source / revision").frame(maxWidth: .infinity, alignment: .leading)
                    Text("Tokens").frame(width: 90, alignment: .trailing)
                    Text("Sessions").frame(width: 70, alignment: .trailing)
                    Text("Turns").frame(width: 60, alignment: .trailing)
                }
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
                .padding(.horizontal, Spacing.md)
                .padding(.vertical, Spacing.sm)
                ForEach(sortedNodes([focus] + descendants(of: focus))) { node in
                    Button {
                        selectedNodeId = node.id
                    } label: {
                        HStack {
                            HStack(spacing: Spacing.sm) {
                                Circle().fill(contextColor(node.kind)).frame(width: 7, height: 7)
                                VStack(alignment: .leading, spacing: 1) {
                                    Text(node.label).lineLimit(1)
                                    Text(node.sourcePath ?? displayKind(node.kind))
                                        .font(Typography.code(9))
                                        .foregroundStyle(palette.textSecondary)
                                        .lineLimit(1)
                                }
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            Text(node.attributedTokens.formatted()).frame(width: 90, alignment: .trailing)
                            Text(node.sessionCount.formatted()).frame(width: 70, alignment: .trailing)
                            Text(node.turnCount.formatted()).frame(width: 60, alignment: .trailing)
                        }
                        .font(Typography.body(11))
                        .foregroundStyle(palette.text)
                        .padding(.horizontal, Spacing.md)
                        .padding(.vertical, Spacing.sm)
                        .background(node.id == selectedNodeId ? palette.surfaceMuted : Color.clear)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("\(node.label), \(node.attributedTokens) tokens, \(node.sessionCount) sessions")
                    Divider()
                }
            }
            .padding(Spacing.xl)
        }
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
                evidenceMetric("Exposure", "\(evidence.measurements.exposedSessions) / \(snapshot.totals.sessions) sessions")
                evidenceMetric("Turns", evidence.measurements.exposedTurns.formatted())
                evidenceMetric("Attributed", "\(evidence.measurements.attributedTokens.formatted()) tokens")
                evidenceMetric("Median / p95", "\(optionalTokens(evidence.measurements.medianTokensPerExposedTurn)) / \(optionalTokens(evidence.measurements.p95TokensPerExposedTurn))")
                evidenceMetric(
                    "First observed",
                    evidence.measurements.firstSeen.map {
                        Date(timeIntervalSince1970: TimeInterval($0))
                            .formatted(date: .abbreviated, time: .shortened)
                    } ?? "Missing"
                )
                evidenceMetric("Precedence", evidence.precedenceLayers.joined(separator: " · "))
            }

            if !evidence.isEditable {
                Label(editabilityReason(evidence), systemImage: "lock.trianglebadge.exclamationmark")
                    .font(Typography.caption(10))
                    .foregroundStyle(Color.statusWarning)
            }

            VStack(alignment: .leading, spacing: Spacing.sm) {
                railTitle("Representative sessions")
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

            Button("Refine source…") { refinementEvidence = evidence }
                .buttonStyle(DarkButtonStyle())
                .disabled(!canRefine(evidence, in: snapshot.query))
                .opacity(canRefine(evidence, in: snapshot.query) ? 1 : 0.4)
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
                Text("Measured populations share the active session-set query. No quality score is inferred.")
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
            laterCompleteCaptures: later.measurements.completeCaptureLaunches
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
                    evidenceMetric("Sessions", node.sessionCount.formatted())
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
            evidenceMetric("Assembled", "\(coverage.assembledTurns) / \(snapshot.totals.turns) turns")
            evidenceMetric("Provider total", coverage.providerTotalOnlyTurns.formatted())
            evidenceMetric("Unknown", coverage.unknownTurns.formatted())
            evidenceMetric(
                "Attribution",
                "\(snapshot.aggregateRoot.attributedTokens) / \(optionalTokens(snapshot.totals.contextTokens)) tokens"
            )
            evidenceMetric("Conversations", "\(coverage.conversationsAvailable) / \(snapshot.totals.launches)")
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
              let sourcePath = evidence.sourcePath?.normalizedFilePath,
              query.repoPaths.count == 1,
              let repoPath = query.repoPaths.first?.normalizedFilePath
        else { return false }
        return sourcePath == repoPath || sourcePath.hasPrefix(repoPath + "/")
    }

    private func refinementHelp(_ evidence: SourceEvidence, in query: SessionSetQuery) -> String {
        guard evidence.isEditable, let sourcePath = evidence.sourcePath else {
            return editabilityReason(evidence)
        }
        guard query.repoPaths.count == 1, let repoPath = query.repoPaths.first else {
            return "Narrow the session set to one repo before refining this source."
        }
        let source = sourcePath.normalizedFilePath
        let repo = repoPath.normalizedFilePath
        guard source == repo || source.hasPrefix(repo + "/") else {
            return "This source is outside the selected repo and cannot be changed in its Task worktree."
        }
        return "Launch a fresh trace-linked refinement in a Task worktree"
    }

    private func selectedEvidence(in snapshot: ContextLabSnapshot) -> [SourceEvidence] {
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

    private func sortedNodes(_ nodes: [ContextFlameNode]) -> [ContextFlameNode] {
        nodes.sorted { left, right in
            switch nodeSort {
            case .tokens: left.attributedTokens > right.attributedTokens
            case .sessions: left.sessionCount > right.sessionCount
            case .turns: left.turnCount > right.turnCount
            case .label: left.label.localizedStandardCompare(right.label) == .orderedAscending
            }
        }
    }

    private func refresh() async {
        isLoading = true
        defer {
            if !Task.isCancelled { isLoading = false }
        }
        do {
            let next = try await RegistryQueryLocal.shared.contextLab(query)
            try Task.checkCancellation()
            snapshot = next
            errorMessage = nil
            if findNode(selectedNodeId, in: next.aggregateRoot) == nil {
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

    private func applyRepoDraft() {
        let path = repoDraft.trimmingCharacters(in: .whitespaces)
        repoDraft = path
        query.repoPaths = path.isEmpty ? [] : [WaveOrigin.resolve(path)]
    }

    private func clearFilters() {
        query = defaultQuery
        query.startedBefore = Int64(Date().timeIntervalSince1970)
        query.startedAfter = query.startedBefore - 30 * 24 * 60 * 60
        repoDraft = defaultQuery.repoPaths.first ?? ""
    }

    private func saveCurrentView() {
        let saved = ContextLabSavedView(query: query, mode: mode)
        savedViews.removeAll { $0 == saved }
        savedViews.append(saved)
        persistSavedViews()
    }

    private func apply(_ saved: ContextLabSavedView) {
        query = saved.query
        repoDraft = query.repoPaths.first ?? ""
        mode = saved.mode
    }

    private func delete(_ saved: ContextLabSavedView) {
        savedViews.removeAll { $0 == saved }
        persistSavedViews()
    }

    private func persistSavedViews() {
        guard let data = try? JSONEncoder().encode(savedViews) else { return }
        UserDefaults.standard.set(data, forKey: "contextLabSavedViews")
    }

    private static func loadSavedViews() -> [ContextLabSavedView] {
        guard let data = UserDefaults.standard.data(forKey: "contextLabSavedViews"),
              let views = try? JSONDecoder().decode([ContextLabSavedView].self, from: data)
        else { return [] }
        var seen = Set<ContextLabSavedView>()
        return views.filter { seen.insert($0).inserted }
    }

    private var filtersAreEmpty: Bool {
        var filters = query
        filters.startedAfter = defaultQuery.startedAfter
        filters.startedBefore = defaultQuery.startedBefore
        return windowDays == 30 && filters == defaultQuery
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
    laterCompleteCaptures: UInt64
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
    return nil
}

private func displayKind(_ kind: ContextAssetKind?) -> String {
    kind?.rawValue.replacingOccurrences(of: "_", with: " ").capitalized ?? "Session set"
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
