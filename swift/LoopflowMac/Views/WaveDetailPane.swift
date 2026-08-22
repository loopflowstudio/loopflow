#if os(macOS)
import SwiftUI
import Loopflow

struct WaveComposerPrefill: Equatable {
    let id: UUID
    let text: String
}

struct WaveWorkSelection: Equatable {
    let kind: ChildActivitySubject
    let id: String
}

struct WaveDetailReading {
    private(set) var snapshot: WaveDetailSnapshot?
    private(set) var errorMessage: String?

    mutating func update(_ snapshot: WaveDetailSnapshot) {
        self.snapshot = snapshot
        errorMessage = nil
    }

    mutating func recordFailure(_ error: Error) {
        snapshot = nil
        errorMessage = "Wave status unavailable: \(error.localizedDescription)"
    }

    mutating func clear() {
        snapshot = nil
        errorMessage = nil
    }
}

/// One Wave surface: current Project/Task state beside the durable conversation.
/// `lf status` supplies the work map; the Wave listener streams ordered chat and
/// child activity from its journal.
struct WaveDetailPane: View {
    let wave: WaveViewModel
    let repoPath: String
    let onClose: () -> Void

    @Environment(\.palette) private var palette
    @Environment(\.openWindow) private var openWindow
    @State private var selection: WaveWorkSelection?
    @State private var prefill: WaveComposerPrefill?
    @State private var workRefresh: UInt64 = 0
    // A shared singleton is externally owned, so it observes as an @ObservedObject.
    // Wrapping it in @StateObject installs StateObject's create-and-own lifecycle
    // during the first body pass, which fires the singleton's publisher mid-eval —
    // an AttributeGraph dependency cycle at cold invocation and sheet presentation.
    @ObservedObject private var terminalStore = TaskTerminalStore.shared

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            HSplitView {
                WavePlanView(
                    plan: wave.plan ?? WavePlan(objective: ""),
                    wave: wave,
                    repoPath: repoPath,
                    selection: $selection,
                    refreshSignal: workRefresh,
                    onTellWave: tellWave,
                    terminalStore: terminalStore
                )
                .frame(minWidth: 230, idealWidth: 320, maxWidth: 440, maxHeight: .infinity)

                WaveChatView(
                    repoPath: repoPath,
                    waveName: wave.name,
                    prefill: prefill,
                    onSelectChild: { selection = $0 },
                    onChildActivity: { workRefresh &+= 1 }
                )
                    .frame(minWidth: 340, maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }

    private func tellWave(_ selection: WaveWorkSelection) {
        self.selection = selection
        let noun = selection.kind == .project ? "Project" : "Task"
        prefill = WaveComposerPrefill(
            id: UUID(),
            text: "Regarding \(noun) \(selection.id): "
        )
    }

    private var header: some View {
        HStack(spacing: Spacing.sm) {
            WaveLensView(lens: wave.lens)
            Text(wave.displayName)
                .font(Typography.sectionTitle())
                .foregroundStyle(palette.text)

            Spacer()

            Button {
                openWindow(
                    id: "context-lab",
                    value: ContextLabRoute.wave(repoPath: repoPath, wave: wave.name)
                )
            } label: {
                Label("Context Lab", systemImage: "text.magnifyingglass")
                    .font(Typography.caption())
            }
            .buttonStyle(.borderless)
            .help("Study the instructions seen by this Wave's invocations")
            .accessibilityIdentifier("wave-context-lab")

            Button {
                onClose()
            } label: {
                Image(systemName: "xmark")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            .buttonStyle(.plain)
            .help("Close wave")
            .accessibilityLabel("Close wave")
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.md)
    }
}

private struct WavePlanView: View {
    let plan: WavePlan
    let wave: WaveViewModel
    let repoPath: String
    @Binding var selection: WaveWorkSelection?
    let refreshSignal: UInt64
    let onTellWave: (WaveWorkSelection) -> Void
    @ObservedObject var terminalStore: TaskTerminalStore

    @Environment(\.palette) private var palette
    @State private var reading = WaveDetailReading()
    // True until the first live read resolves. It gates the loading affordance,
    // so an empty projects area during the pre-snapshot window reads as
    // "loading" rather than "no projects".
    @State private var isAwaitingDetail = true

    private var identity: String { "\(repoPath)|\(wave.id)" }
    private var refreshIdentity: String { "\(identity)|\(refreshSignal)" }
    private var workMap: WaveWorkMap? { reading.snapshot?.workMap }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.xl) {
                objective
                if let portfolio = reading.snapshot?.metricPortfolio {
                    WaveMetricPortfolioView(
                        portfolio: portfolio,
                        projectNames: Dictionary(uniqueKeysWithValues:
                            (workMap?.projects ?? []).map { ($0.project.id, $0.project.name) }
                        )
                    )
                }
                projects
                if let selection, let workMap {
                    WaveWorkInspector(
                        selection: selection,
                        workMap: workMap,
                        repoPath: repoPath,
                        onTellWave: onTellWave,
                        terminalStore: terminalStore
                    )
                }
                liveStatusFooter
            }
            .padding(Spacing.xl)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(palette.background)
        .task(id: refreshIdentity) {
            while !Task.isCancelled {
                await refreshDetail()
                try? await Task.sleep(for: .seconds(30))
            }
        }
    }

    private var objectiveText: String { workMap?.objective ?? plan.objective }

    /// Lead with one sentence, prominent. The full objective is disclosure, not
    /// clipped prose — and the lead is a deterministic excerpt, never a
    /// generated summary that could disagree with GOAL.md.
    private var objective: some View {
        let full = objectiveText.trimmingCharacters(in: .whitespacesAndNewlines)
        let lead = Self.firstSentence(full)
        return VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(full.isEmpty ? "No objective written yet." : lead)
                .font(Typography.sectionTitle(20))
                .foregroundStyle(palette.text)
                .lineSpacing(3)
                .textSelection(.enabled)
                .accessibilityIdentifier("wave-objective-lead")

            if !full.isEmpty, full != lead {
                DisclosureGroup {
                    Text(full)
                        .font(Typography.body(13))
                        .foregroundStyle(palette.textSecondary)
                        .lineSpacing(3)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.top, Spacing.xs)
                } label: {
                    Text("Full objective")
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                }
                .tint(palette.accent)
            }
        }
    }

    /// Deterministic one-sentence excerpt: flatten newlines, then cut at the
    /// first sentence terminator followed by a space or end of text.
    static func firstSentence(_ text: String) -> String {
        let flat = text.split(whereSeparator: \.isNewline)
            .joined(separator: " ")
            .trimmingCharacters(in: .whitespaces)
        guard !flat.isEmpty else { return "" }
        let terminators: Set<Character> = [".", "!", "?"]
        let chars = Array(flat)
        var result = ""
        for (i, c) in chars.enumerated() {
            result.append(c)
            guard terminators.contains(c) else { continue }
            let next = i + 1 < chars.count ? chars[i + 1] : " "
            if next == " " { return result.trimmingCharacters(in: .whitespaces) }
        }
        return result.trimmingCharacters(in: .whitespaces)
    }

    private var projects: some View {
        let projectCount = workMap?.projects.count ?? plan.projects.count
        return VStack(alignment: .leading, spacing: Spacing.md) {
            HStack(spacing: Spacing.sm) {
                Text("Projects")
                    .font(Typography.caption(10))
                    .fontWeight(.medium)
                    .foregroundStyle(palette.textSecondary)

                Text("\(projectCount)")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xxs)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
            }

            if let workMap, !workMap.projects.isEmpty {
                LazyVStack(alignment: .leading, spacing: Spacing.md) {
                    ForEach(workMap.projects) { project in
                        WaveProjectWorkView(
                            project: project,
                            selection: $selection
                        )
                    }
                }
            } else if plan.projects.isEmpty {
                if isAwaitingDetail {
                    HStack(spacing: Spacing.sm) {
                        ProgressView().controlSize(.small)
                        Text("Loading live detail…")
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                    }
                    .accessibilityIdentifier("wave-detail-loading")
                } else {
                    Text("No projects yet.")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                }
            } else {
                LazyVStack(alignment: .leading, spacing: Spacing.md) {
                    ForEach(plan.projects) { project in
                        WaveProjectView(project: project)
                    }
                }
            }
        }
        .accessibilityIdentifier("wave-projects")
    }

    /// Live-status failures are operational detail, not primary hierarchy: a
    /// quiet footer says the authored plan is showing cached and hides the raw
    /// reason behind disclosure. Volatile status and metrics never survive a
    /// failed refresh; the plan above still renders from the cached `WavePlan`.
    @ViewBuilder
    private var liveStatusFooter: some View {
        if let errorMessage = reading.errorMessage {
            DisclosureGroup {
                Text(errorMessage)
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.top, Spacing.xxs)
            } label: {
                Label("Showing cached plan · live status unavailable", systemImage: "arrow.triangle.2.circlepath")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
            }
            .tint(palette.textSecondary)
            .accessibilityIdentifier("wave-live-status-footer")
        }
    }

    private func refreshDetail() async {
        if AppTestMode.current() == .mockWaves {
            applyMockDetail()
            return
        }
        guard wave.isRegistered else {
            reading.clear()
            isAwaitingDetail = false
            return
        }
        do {
            let snapshot = try await RegistryQueryLocal.shared.status(
                wave: wave.name,
                cwd: repoPath
            )
            guard !Task.isCancelled else { return }
            reading.update(snapshot)
        } catch {
            guard !Task.isCancelled else { return }
            reading.recordFailure(error)
        }
        isAwaitingDetail = false
    }

    /// The `mock-waves` detail rendering: the fixture owns the state→reading
    /// decision (see `MockWaveFixture.detailReading`); the view just applies it.
    private func applyMockDetail() {
        let outcome = MockWaveFixture.detailReading(
            waveName: wave.name,
            state: MockWaveFixture.detailState
        )
        reading = outcome.reading
        isAwaitingDetail = outcome.awaitingFirstRead
    }
}

struct WaveMetricPortfolioView: View {
    let portfolio: MetricPortfolio
    let projectNames: [String: String]

    @Environment(\.palette) private var palette

    private var presentation: WaveMetricPortfolioPresentation {
        WaveMetricPortfolioPresentation(portfolio: portfolio)
    }

    private var official: [MetricReading] {
        portfolio.metrics
            .filter { $0.stage == .graduated }
            .sorted { left, right in
                let priority = left.evidence.displayPriority - right.evidence.displayPriority
                return priority == 0 ? left.name < right.name : priority < 0
            }
    }

    private var candidates: [MetricReading] {
        portfolio.metrics
            .filter { $0.stage == .installed }
            .sorted { $0.name < $1.name }
    }

    var body: some View {
        if !portfolio.metrics.isEmpty || !portfolio.contractIssues.isEmpty {
            VStack(alignment: .leading, spacing: Spacing.lg) {
                portfolioHeader

                if !official.isEmpty {
                    VStack(alignment: .leading, spacing: Spacing.md) {
                        portfolioSectionLabel("Official measures", count: official.count)
                        metricGroups(official)
                    }
                    .accessibilityIdentifier("wave-metric-official")
                }

                if !candidates.isEmpty {
                    VStack(alignment: .leading, spacing: Spacing.md) {
                        VStack(alignment: .leading, spacing: Spacing.xxs) {
                            portfolioSectionLabel("Candidates", count: candidates.count)
                            Text("Installed contracts still proving their instruments.")
                                .font(Typography.caption(10))
                                .foregroundStyle(palette.textSecondary)
                        }
                        metricGroups(candidates)
                    }
                    .padding(Spacing.md)
                    .background(palette.surfaceMuted.opacity(0.48))
                    .overlay {
                        RoundedRectangle(cornerRadius: CornerRadius.md)
                            .stroke(palette.border.opacity(0.8), lineWidth: 1)
                    }
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                    .accessibilityIdentifier("wave-metric-candidates")
                }

                if !portfolio.contractIssues.isEmpty {
                    VStack(alignment: .leading, spacing: Spacing.sm) {
                        HStack(spacing: Spacing.sm) {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .foregroundStyle(Color.statusWarning)
                            portfolioSectionLabel(
                                "Contract issues",
                                count: portfolio.contractIssues.count
                            )
                        }

                        VStack(alignment: .leading, spacing: Spacing.sm) {
                            ForEach(
                                Array(portfolio.contractIssues.enumerated()),
                                id: \.offset
                            ) { _, issue in
                                Text(issue.summary)
                                    .font(Typography.caption(10))
                                    .foregroundStyle(palette.text)
                                    .textSelection(.enabled)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                            }
                        }
                    }
                    .padding(Spacing.md)
                    .background(Color.statusWarning.opacity(0.08))
                    .overlay {
                        RoundedRectangle(cornerRadius: CornerRadius.md)
                            .stroke(Color.statusWarning.opacity(0.25), lineWidth: 1)
                    }
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                    .accessibilityIdentifier("wave-metric-contract-issues")
                }
            }
            .accessibilityIdentifier("wave-metric-portfolio")
        }
    }

    private var portfolioHeader: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                Text("Metrics")
                    .font(Typography.sectionTitle(18))
                    .foregroundStyle(palette.text)

                Spacer()

                if presentation.needsAttentionCount > 0 {
                    Label(
                        countLabel(
                            presentation.needsAttentionCount,
                            singular: "measure needs attention",
                            plural: "measures need attention"
                        ),
                        systemImage: "exclamationmark.circle.fill"
                    )
                    .font(Typography.caption(10))
                    .fontWeight(.medium)
                    .foregroundStyle(Color.statusError)
                }
            }

            Text(presentation.headline)
                .font(Typography.body(12))
                .foregroundStyle(palette.textSecondary)

            HStack(spacing: Spacing.sm) {
                summaryPill(countLabel(
                    presentation.officialCount,
                    singular: "official measure",
                    plural: "official measures"
                ))
                summaryPill(countLabel(
                    presentation.candidateCount,
                    singular: "candidate",
                    plural: "candidates"
                ))
                if presentation.contractIssueCount > 0 {
                    summaryPill(
                        countLabel(
                            presentation.contractIssueCount,
                            singular: "issue",
                            plural: "issues"
                        ),
                        color: .statusWarning
                    )
                }
            }
        }
        .accessibilityIdentifier("wave-metric-summary")
    }

    private func countLabel(_ count: Int, singular: String, plural: String) -> String {
        "\(count) \(count == 1 ? singular : plural)"
    }

    private func summaryPill(_ text: String, color: Color? = nil) -> some View {
        Text(text)
            .font(Typography.caption(9))
            .fontWeight(.medium)
            .foregroundStyle(color ?? palette.textSecondary)
            .padding(.horizontal, Spacing.sm)
            .padding(.vertical, Spacing.xs)
            .background((color ?? palette.textSecondary).opacity(0.09))
            .clipShape(Capsule())
    }

    private func portfolioSectionLabel(_ text: String, count: Int) -> some View {
        HStack(spacing: Spacing.sm) {
            Text(text.uppercased())
                .font(Typography.caption(9))
                .fontWeight(.semibold)
                .tracking(0.8)
                .foregroundStyle(palette.textSecondary)
            Text("\(count)")
                .font(Typography.caption(9))
                .foregroundStyle(palette.textSecondary)
        }
    }

    @ViewBuilder
    private func metricGroups(_ metrics: [MetricReading]) -> some View {
        let projectIds = Array(Set(metrics.map(\.projectId))).sorted {
            (projectNames[$0] ?? $0) < (projectNames[$1] ?? $1)
        }
        ForEach(projectIds, id: \.self) { projectId in
            VStack(alignment: .leading, spacing: Spacing.sm) {
                HStack(spacing: Spacing.sm) {
                    Rectangle()
                        .fill(palette.accent)
                        .frame(width: 12, height: 2)
                    Text(projectNames[projectId] ?? projectId)
                        .font(Typography.body(11))
                        .fontWeight(.semibold)
                        .foregroundStyle(palette.text)
                }
                ForEach(metrics.filter { $0.projectId == projectId }) { metric in
                    WaveMetricCard(
                        metric: metric,
                        owner: projectNames[projectId] ?? projectId
                    )
                }
            }
        }
    }
}

struct WaveMetricPortfolioPresentation: Equatable {
    let officialCount: Int
    let candidateCount: Int
    let holdingCount: Int
    let needsAttentionCount: Int
    let contractIssueCount: Int

    init(portfolio: MetricPortfolio) {
        let official = portfolio.metrics.filter { $0.stage == .graduated }
        officialCount = official.count
        candidateCount = portfolio.metrics.count - official.count
        holdingCount = official.count { $0.evidence.isHealthy }
        needsAttentionCount = official.count - holdingCount
        contractIssueCount = portfolio.contractIssues.count
    }

    var headline: String {
        switch (officialCount, holdingCount) {
        case (0, _):
            return "No official measures yet. Candidates remain visible while their evidence matures."
        case (1, 1):
            return "The official measure currently holds."
        case (1, 0):
            return "The official measure needs attention."
        default:
            return "\(holdingCount) of \(officialCount) official measures currently hold."
        }
    }
}

struct WaveMetricRowPresentation: Equatable {
    let name: String
    let description: String
    let state: String
    let owner: String
    let instrumentState: String
    let value: String
    let target: String
    let window: String
    let freshness: String
    let reason: String?

    init(metric: MetricReading, owner: String) {
        name = metric.name
        description = metric.description
        state = metric.evidence.label
        self.owner = owner
        instrumentState = metric.instrumented ? "Instrumented" : "Awaiting instrument"
        value = metric.evidence.value.map { metric.format($0) } ?? "—"
        target = metric.target.display(unit: metric.unit)
        window = metric.window
        freshness = metric.freshness.summary
        reason = metric.evidence.reason
    }
}

private struct WaveMetricCard: View {
    let metric: MetricReading
    let owner: String

    @Environment(\.palette) private var palette

    var body: some View {
        let presentation = WaveMetricRowPresentation(metric: metric, owner: owner)
        HStack(spacing: 0) {
            Rectangle()
                .fill(metric.evidence.stateColor)
                .frame(width: 3)

            VStack(alignment: .leading, spacing: Spacing.sm) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Text(presentation.name)
                        .font(Typography.body(13))
                        .fontWeight(.semibold)
                        .foregroundStyle(palette.text)
                    Spacer(minLength: Spacing.xs)
                    stateBadge(presentation.state)
                }

                Text(presentation.description)
                    .font(Typography.body(11))
                    .foregroundStyle(palette.textSecondary)
                    .lineSpacing(2)
                    .fixedSize(horizontal: false, vertical: true)

                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Text(presentation.value)
                        .font(Typography.sectionTitle(18))
                        .foregroundStyle(palette.text)
                    Text("target \(presentation.target)")
                        .font(Typography.caption(9))
                        .foregroundStyle(palette.textSecondary)
                    Spacer()
                    Text("\(presentation.window) window")
                        .font(Typography.caption(9))
                        .foregroundStyle(palette.textSecondary)
                }

                HStack(spacing: Spacing.xs) {
                    Text(presentation.instrumentState)
                        .font(Typography.caption(9))
                        .fontWeight(.medium)
                        .foregroundStyle(metric.instrumented ? palette.textSecondary : Color.statusWarning)
                    Text("·")
                        .foregroundStyle(palette.textSecondary)
                    Text(presentation.freshness)
                        .font(Typography.caption(9))
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(1)
                }

                if let reason = presentation.reason {
                    Text(reason)
                        .font(Typography.caption(10))
                        .foregroundStyle(metric.evidence.stateColor)
                        .textSelection(.enabled)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .padding(Spacing.md)
        }
        .background(palette.surface)
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.md)
                .stroke(palette.border.opacity(0.85), lineWidth: 1)
        }
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .accessibilityIdentifier("wave-metric")
        .accessibilityElement(children: .combine)
    }

    private func stateBadge(_ state: String) -> some View {
        Text(state.uppercased())
            .font(Typography.caption(8))
            .fontWeight(.bold)
            .tracking(0.5)
            .foregroundStyle(metric.evidence.stateColor)
            .padding(.horizontal, Spacing.sm)
            .padding(.vertical, Spacing.xs)
            .background(metric.evidence.stateColor.opacity(0.10))
            .clipShape(Capsule())
    }
}

private extension MetricReading {
    func format(_ value: Double) -> String {
        if unit == "ratio" {
            return value.formatted(.percent.precision(.fractionLength(0 ... 2)))
        }
        return "\(value.formatted(.number.precision(.fractionLength(0 ... 3)))) \(unit)"
    }
}

private extension MetricTarget {
    func display(unit: String) -> String {
        switch self {
        case let .atLeast(value): return "≥ \(formatted(value, unit: unit))"
        case let .atMost(value): return "≤ \(formatted(value, unit: unit))"
        }
    }

    private func formatted(_ value: Double, unit: String) -> String {
        if unit == "ratio" {
            return value.formatted(.percent.precision(.fractionLength(0 ... 2)))
        }
        return "\(value.formatted(.number.precision(.fractionLength(0 ... 3)))) \(unit)"
    }
}

private extension MetricFreshness {
    var summary: String {
        switch self {
        case .never: return "Never observed"
        case let .fresh(_, expiresAt): return "Fresh until \(expiresAt)"
        case let .stale(_, expiresAt): return "Stale since \(expiresAt)"
        }
    }
}

private extension MetricEvidence {
    var stateColor: Color {
        switch self {
        case .met: return .statusSuccess
        case .missed: return .statusError
        case .unknown: return .statusNeutral
        case .unavailable: return .statusWarning
        }
    }

    var displayPriority: Int {
        switch self {
        case .missed, .unavailable: return 0
        case .unknown: return 1
        case .met: return 2
        }
    }

    var label: String {
        switch self {
        case .met: return "Met"
        case .missed: return "Missed"
        case .unknown: return "Unknown"
        case .unavailable: return "Unavailable"
        }
    }

    var isHealthy: Bool {
        if case .met = self { return true }
        return false
    }

    var value: Double? {
        switch self {
        case let .met(value, _, _), let .missed(value, _, _): return value
        case let .unknown(cause): return cause.value
        case .unavailable: return nil
        }
    }

    var reason: String? {
        switch self {
        case .met, .missed: return nil
        case let .unknown(cause): return cause.summary
        case let .unavailable(reason, sourceAsOf): return "\(reason) · source time \(sourceAsOf)"
        }
    }
}

private extension MetricUnknownCause {
    var value: Double? {
        switch self {
        case let .incomplete(value, _, _),
             let .windowMismatch(value, _, _),
             let .staleObservation(value, _, _): return value
        case .never, .revisionMismatch, .staleUnavailable: return nil
        }
    }

    var summary: String {
        switch self {
        case .never: return "No observation has arrived."
        case let .revisionMismatch(expected, observed, sourceTime):
            return "Evidence at \(sourceTime) measured revision \(observed), not \(expected)."
        case .incomplete: return "The latest source window is incomplete."
        case .windowMismatch: return "The latest source window does not match the contract."
        case .staleObservation: return "The latest observation is stale."
        case let .staleUnavailable(reason, sourceAsOf):
            return "The last source failure is stale: \(reason) · source time \(sourceAsOf)"
        }
    }
}

private extension MetricContractIssue {
    var summary: String {
        switch self {
        case let .malformedContract(path, message): return "\(path): \(message)"
        case let .unresolvedOwner(waveId, metricId, projectId):
            return "\(waveId)/\(metricId) names unknown Project \(projectId)."
        case let .instrumentMismatch(waveId, metricId, contractInstrument, registeredInstrument):
            return "\(waveId)/\(metricId) declares \(contractInstrument), but \(registeredInstrument) is registered."
        case let .invalidGraduation(waveId, metricId, _, reason):
            return "\(waveId)/\(metricId) cannot graduate: \(reason)."
        }
    }
}

private struct WaveProjectWorkView: View {
    let project: WaveProjectWork
    @Binding var selection: WaveWorkSelection?

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                WaveLensView(lens: projectLens, diameter: 10, accessibilityId: "project-lens")
                    .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 2 }

                Text(project.project.name)
                    .font(Typography.sectionTitle(17))
                    .foregroundStyle(palette.text)

                Text(openTaskLabel)
                    .font(Typography.caption(10))
                    .fontWeight(.medium)
                    .foregroundStyle(palette.textSecondary)
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xxs)
                    .background(palette.surfaceMuted)
                    .clipShape(Capsule())
                    .accessibilityIdentifier("project-open-tasks")

                Spacer()

                if let status = project.runtime?.current.state.label {
                    Text(status)
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                }
            }

            if !project.project.definition.isEmpty {
                Text(project.project.definition)
                    .font(Typography.body(13))
                    .foregroundStyle(palette.textSecondary)
                    .lineSpacing(2)
                    .textSelection(.enabled)
            }

            if !project.project.krs.isEmpty {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(project.project.krs) { kr in
                        proofRow(text: kr.text, holds: kr.holds)
                    }
                }
            }

            VStack(alignment: .leading, spacing: Spacing.xs) {
                ForEach(project.tasks) { task in
                    WaveTaskWorkView(
                        task: task,
                        selection: $selection
                    )
                }
            }

            if let failure = project.runtime?.lastFailure {
                ProjectFailureHistoryView(failure: failure)
            }

            Text("Next: \(project.nextMove.owner.rawValue) · \(project.nextMove.reason)")
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
            if let directive = project.directive {
                directiveStatus(directive)
            }
        }
        .padding(Spacing.md)
        .background(palette.surfaceMuted.opacity(0.65))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.md)
                .stroke(isSelected ? palette.accent : Color.clear, lineWidth: 1)
        }
        .contentShape(Rectangle())
        .accessibilityIdentifier("wave-project")
        .onTapGesture {
            selection = WaveWorkSelection(kind: .project, id: project.project.slug)
        }
    }

    private var isSelected: Bool {
        selection == WaveWorkSelection(kind: .project, id: project.project.slug)
    }

    /// The Project's lens, derived from its shared runtime and its Tasks'
    /// attention evidence — the same grammar the Wave and Task rows use.
    private var projectLens: WaveLens {
        WaveLens.forProject(runtime: project.runtime, tasks: project.tasks)
    }

    private var openTaskLabel: String {
        let open = project.tasks.filter { !$0.task.completed }.count
        return open == 1 ? "1 open task" : "\(open) open tasks"
    }

    private func directiveStatus(_ directive: WorkDirectiveSnapshot) -> some View {
        Text("Direction v\(directive.version) · \(directive.incorporatedAt == nil ? "pending incorporation" : "incorporated")")
            .font(Typography.caption(10))
            .foregroundStyle(directive.incorporatedAt == nil ? palette.textSecondary : palette.accent)
    }

    private func proofRow(text: String, holds: Bool) -> some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Image(systemName: holds ? "checkmark.circle.fill" : "circle")
                .font(Typography.caption(11))
                .foregroundStyle(holds ? palette.accent : palette.textSecondary)
                .frame(width: 14)
                .accessibilityHidden(true)
            Text(text)
                .font(Typography.caption(12))
                .foregroundStyle(palette.text)
                .lineSpacing(2)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(text)
        .accessibilityValue(holds ? "Holds" : "Open")
        .accessibilityIdentifier("project-key-result")
    }
}

struct ProjectFailureHistoryView: View {
    let failure: HistoricalFailure

    @Environment(\.palette) private var palette

    var body: some View {
        Text("Last failure at \(failure.occurredAt): \(failure.message)")
            .font(Typography.caption(10))
            .foregroundStyle(palette.textSecondary)
            .textSelection(.enabled)
            .accessibilityIdentifier("project-last-failure")
    }
}

private struct WaveTaskWorkView: View {
    let task: WaveTaskWork
    @Binding var selection: WaveWorkSelection?

    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            WaveLensView(lens: WaveLens.forTask(task.attention), diameter: 9, accessibilityId: "task-lens")
                .frame(width: 14)
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                    Text(task.task.identifier)
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                    Text(task.task.name)
                        .font(Typography.caption(12))
                        .foregroundStyle(palette.text)
                        .lineLimit(2)
                }
                Text("\(task.runtime?.current.state.label ?? "unstarted") · next: \(task.nextMove.owner.rawValue)")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                if let directive = task.directive {
                    Text("direction v\(directive.version) · \(directive.incorporatedAt == nil ? "pending" : "incorporated")")
                        .font(Typography.caption(10))
                        .foregroundStyle(directive.incorporatedAt == nil ? palette.textSecondary : palette.accent)
                }
                ForEach(task.prs) { pr in
                    PrLink(pr: pr)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Spacing.sm)
        .background(palette.background.opacity(0.55))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.sm)
                .stroke(isSelected ? palette.accent : Color.clear, lineWidth: 1)
        }
        .contentShape(Rectangle())
        .accessibilityIdentifier("wave-task")
        .onTapGesture {
            selection = WaveWorkSelection(kind: .task, id: task.task.identifier)
        }
    }

    private var isSelected: Bool {
        selection == WaveWorkSelection(kind: .task, id: task.task.identifier)
    }
}

private struct WaveWorkInspector: View {
    let selection: WaveWorkSelection
    let workMap: WaveWorkMap
    let repoPath: String
    let onTellWave: (WaveWorkSelection) -> Void
    @ObservedObject var terminalStore: TaskTerminalStore

    @Environment(\.palette) private var palette
    @State private var showsTaskWorkspace = false

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack {
                Text("Selected work")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                Spacer()
                Button("Tell Wave about this") { onTellWave(selection) }
                    .buttonStyle(.borderless)
                    .font(Typography.caption(10))
            }
            if let project {
                Text(project.project.name)
                    .font(Typography.sectionTitle(15))
                    .foregroundStyle(palette.text)
                details(
                    directive: project.directive,
                    status: project.runtime?.current.state.label ?? "unstarted",
                    reason: project.nextMove.reason,
                    provider: project.runtime?.provider,
                    location: nil,
                    prs: []
                )
            } else if let task {
                Text("\(task.task.identifier) · \(task.task.name)")
                    .font(Typography.sectionTitle(15))
                    .foregroundStyle(palette.text)
                details(
                    directive: task.directive,
                    status: task.runtime?.current.state.label ?? "unstarted",
                    reason: task.attention.reason,
                    provider: task.runtime?.provider,
                    location: taskLocation,
                    prs: task.prs
                )
                if task.reference.workspace != nil {
                    Button("Open Task workspace") { showsTaskWorkspace = true }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.small)
                }
            }
        }
        .padding(Spacing.md)
        .background(palette.surfaceMuted)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .sheet(isPresented: $showsTaskWorkspace) {
            if let task {
                TaskWorkspaceView(
                    task: task.task,
                    reference: task.reference,
                    runtime: task.runtime,
                    attention: task.attention,
                    repoPath: repoPath,
                    terminalStore: terminalStore,
                    initialSection: .changes
                )
            }
        }
    }

    private var project: WaveProjectWork? {
        guard selection.kind == .project else { return nil }
        return workMap.projects.first { $0.project.slug == selection.id || $0.project.id == selection.id }
    }

    private var task: WaveTaskWork? {
        guard selection.kind == .task else { return nil }
        return workMap.projects
            .flatMap(\.tasks)
            .first { $0.task.identifier == selection.id || $0.task.id == selection.id }
    }

    private var taskLocation: String? {
        guard let workspace = task?.reference.workspace else { return nil }
        guard let branch = workspace.branch else { return workspace.worktree }
        return "\(workspace.worktree)\n\(branch)"
    }

    @ViewBuilder
    private func details(
        directive: WorkDirectiveSnapshot?,
        status: String,
        reason: String,
        provider: String?,
        location: String?,
        prs: [PrSnapshot]
    ) -> some View {
        Text("\(status) · \(reason)")
            .font(Typography.caption(11))
            .foregroundStyle(palette.textSecondary)
        if let directive {
            Text("Direction v\(directive.version)")
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
            Text(directive.text)
                .font(Typography.body(12))
                .foregroundStyle(palette.text)
                .textSelection(.enabled)
            Text(directive.incorporatedAt == nil ? "Awaiting incorporation" : "Incorporated")
                .font(Typography.caption(10))
                .foregroundStyle(directive.incorporatedAt == nil ? palette.textSecondary : palette.accent)
        }
        if let provider {
            Text("Provider · \(provider)")
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
        }
        if let location {
            Text(location)
                .font(.system(size: 10, design: .monospaced))
                .foregroundStyle(palette.textSecondary)
                .textSelection(.enabled)
        }
        ForEach(prs) { pr in
            PrLink(pr: pr)
        }
    }
}

private struct PrLink: View {
    let pr: PrSnapshot

    @Environment(\.palette) private var palette

    var body: some View {
        if let github = pr.publication?.github {
            Link(
                "PR #\(github.number) · \(pr.phase.rawValue)\(pr.publication?.merge?.afterMerge == .completeTask ? " · completes Task" : "")",
                destination: github.url
            )
            .font(Typography.caption(10))
        } else {
            Text("PR \(pr.sequence) · \(pr.phase.rawValue)\(pr.publication?.merge?.afterMerge == .completeTask ? " · completes Task" : "") · \(pr.branch)")
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
        }
    }
}

private struct WaveProjectView: View {
    let project: WaveProject

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(project.title)
                .font(Typography.sectionTitle(17))
                .foregroundStyle(palette.text)

            if let definition = project.definition {
                Text(definition)
                    .font(Typography.body(13))
                    .foregroundStyle(palette.textSecondary)
                    .lineSpacing(2)
                    .textSelection(.enabled)
            }

            if !project.krs.isEmpty {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(project.krs) { kr in
                        HStack(alignment: .top, spacing: Spacing.sm) {
                            Image(systemName: kr.proof == .holds ? "checkmark.circle.fill" : "circle")
                                .font(Typography.caption(11))
                                .foregroundStyle(kr.proof == .holds ? palette.accent : palette.textSecondary)
                                .frame(width: 14)
                                .accessibilityHidden(true)

                            Text(kr.text)
                                .font(Typography.caption(12))
                                .foregroundStyle(palette.text)
                                .lineSpacing(2)
                                .textSelection(.enabled)
                        }
                        .accessibilityElement(children: .combine)
                        .accessibilityLabel(kr.text)
                        .accessibilityValue(kr.proof == .holds ? "Holds" : "Open")
                    }
                }
                .padding(.top, Spacing.xs)
            }
        }
        .padding(Spacing.md)
        .background(palette.surfaceMuted.opacity(0.65))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }
}

#endif
