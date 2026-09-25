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

    func plan(cached: WavePlan) -> WavePlan {
        guard let snapshot else { return cached }
        return WavePlan(objective: snapshot.workMap.objective, chapter: snapshot.workMap.chapter)
    }

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

/// One Wave surface: chapter plan and Tasks beside the durable conversation.
/// `lf status` supplies the work map; the Wave listener streams ordered chat and
/// child activity from its journal.
struct WaveDetailPane: View {
    let wave: WaveViewModel
    let repoPath: String
    let onClose: () -> Void

    @Environment(\.palette) private var palette
    @State private var selection: WaveWorkSelection?
    @State private var showHistory = false
    @State private var historyReference: String?
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
            Button("Chapter history") { showHistory = true }.padding(.bottom, 8)
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
                    onSelectChild: { reference in
                        if reference.kind == .project {
                            historyReference = reference.id
                            if wave.plan?.chapter?.sourceProjectId != reference.id && wave.plan?.chapter?.sourceProjectSlug != reference.id && wave.plan?.chapter?.sourceWorkId != reference.id { showHistory = true }
                            selection = nil
                        } else { selection = reference }
                    },
                    onChildActivity: { workRefresh &+= 1 }
                )
                    .frame(minWidth: 340, maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .sheet(isPresented: $showHistory) {
            ChapterHistoryView(wave: wave.name, repo: repoPath, sourceReference: historyReference)
        }
    }

    private func tellWave(_ selection: WaveWorkSelection) {
        self.selection = selection
        let noun = "Task"
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
    // so an empty plan during the pre-snapshot window reads as loading.
    @State private var isAwaitingDetail = true

    private var identity: String { "\(repoPath)|\(wave.id)" }
    private var refreshIdentity: String { "\(identity)|\(refreshSignal)" }
    private var workMap: WaveWorkMap? { reading.snapshot?.workMap }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.xl) {
                objective
                chapterAndTasks
                if let portfolio = reading.snapshot?.metricPortfolio {
                    WaveMetricPortfolioView(
                        portfolio: portfolio
                    )
                }
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

    private var displayedPlan: WavePlan { reading.plan(cached: plan) }
    private var objectiveText: String { displayedPlan.objective }

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

    private var chapterAndTasks: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            if let chapter = displayedPlan.chapter { WaveChapterView(chapter: chapter) }
            Text("Tasks").font(Typography.sectionTitle(17))
            if isAwaitingDetail {
                ProgressView("Loading Tasks…").accessibilityIdentifier("wave-detail-loading")
            } else if let workMap {
                switch workMap.tasks {
                case .unavailable(let reason): Text(reason).foregroundStyle(Color.statusWarning)
                case .available(let tasks, _):
                    if tasks.isEmpty { Text("No Tasks in this chapter.").foregroundStyle(palette.textSecondary) }
                    ForEach(tasks) { task in WaveTaskWorkView(task: task, selection: $selection) }
                }
            } else { Text("Task status unavailable.").foregroundStyle(palette.textSecondary) }
            ForEach(reading.snapshot?.unavailableTasks ?? [], id: \.taskId) { task in
                Text("\(task.taskIdentifier): \(task.reason) · \(task.recovery)")
                    .foregroundStyle(Color.statusWarning)
            }
        }.accessibilityIdentifier("wave-tasks")
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

                if presentation.requiresWorkCount > 0 {
                    Label(
                        countLabel(
                            presentation.requiresWorkCount,
                            singular: "measure needs work",
                            plural: "measures need work"
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
        ForEach(metrics) { metric in WaveMetricCard(metric: metric, owner: "Wave") }
    }
}

struct WaveMetricPortfolioPresentation: Equatable {
    let officialCount: Int
    let candidateCount: Int
    let holdingCount: Int
    let targetedCount: Int
    let requiresWorkCount: Int
    let contractIssueCount: Int
    let chapterUnavailable: Bool

    init(portfolio: MetricPortfolio) {
        let official = portfolio.metrics.filter { $0.stage == .graduated }
        officialCount = official.count
        candidateCount = portfolio.metrics.count - official.count
        targetedCount = official.count { $0.target != nil }
        holdingCount = official.count { $0.target != nil && $0.evidence.isHealthy }
        requiresWorkCount = targetedCount - holdingCount
        contractIssueCount = portfolio.contractIssues.count
        chapterUnavailable = portfolio.metrics.isEmpty && portfolio.contractIssues.contains {
            if case .chapterUnavailable = $0 { return true }
            return false
        }
    }

    var headline: String {
        if chapterUnavailable { return "Chapter targets unavailable." }
        if officialCount > 0 && targetedCount == 0 { return "No targets set for this chapter." }
        switch (targetedCount, holdingCount) {
        case (0, _):
            return "No official measures yet. Candidates remain visible while their evidence matures."
        case (1, 1):
            return "The official measure currently holds."
        case (1, 0):
            return "The official measure needs work."
        default:
            return "\(holdingCount) of \(targetedCount) chapter targets currently hold."
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
        target = metric.target?.display(unit: metric.unit) ?? "unset for this chapter"
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
        case .unknown, .untargeted: return .statusNeutral
        case .unavailable: return .statusWarning
        }
    }

    var displayPriority: Int {
        switch self {
        case .missed, .unavailable: return 0
        case .unknown, .untargeted: return 1
        case .met: return 2
        }
    }

    var label: String {
        switch self {
        case .untargeted: return "No target"
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
        case let .untargeted(value, _, _), let .met(value, _, _), let .missed(value, _, _): return value
        case let .unknown(cause): return cause.value
        case .unavailable: return nil
        }
    }

    var reason: String? {
        switch self {
        case .met, .missed, .untargeted: return nil
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
        case let .chapterUnavailable(waveId, reason): return "\(waveId): chapter targets unavailable: \(reason)"
        case let .unresolvedTarget(waveId, metricId): return "\(waveId)/\(metricId): chapter target has no readable instrument contract"
        case let .malformedContract(path, message): return "\(path): \(message)"
        case let .instrumentMismatch(waveId, metricId, contractInstrument, registeredInstrument):
            return "\(waveId)/\(metricId) declares \(contractInstrument), but \(registeredInstrument) is registered."
        case let .invalidGraduation(waveId, metricId, _, reason):
            return "\(waveId)/\(metricId) cannot graduate: \(reason)."
        }
    }
}

private struct WaveTaskWorkView: View {
    let task: WaveTaskWork
    @Binding var selection: WaveWorkSelection?

    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            WaveLensView(lens: WaveLens.forTask(task.condition), diameter: 9, accessibilityId: "task-lens")
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
                Text("\(task.runtime?.status.label ?? "unstarted") · next: \(task.nextMove.owner.rawValue)")
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
            if let task {
                Text("\(task.task.identifier) · \(task.task.name)")
                    .font(Typography.sectionTitle(15))
                    .foregroundStyle(palette.text)
                details(
                    directive: task.directive,
                    status: task.runtime?.status.label ?? "unstarted",
                    reason: task.condition.reason,
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
                    repoPath: repoPath,
                    terminalStore: terminalStore,
                    initialSection: .changes
                )
            }
        }
    }

    private var task: WaveTaskWork? {
        guard selection.kind == .task else { return nil }
        return workMap.tasks.items
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

struct WaveChapterView: View {
    let chapter: ChapterSummary
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text("Chapter \(chapter.id)").font(Typography.caption(11)).foregroundStyle(palette.textSecondary)

            ForEach(chapter.krs) { kr in
                Label(kr.text, systemImage: kr.holds ? "checkmark.circle.fill" : "circle")
                    .font(Typography.body(12)).textSelection(.enabled)
                    .accessibilityValue(kr.holds ? "Holds" : "Open")
            }
            if chapter.phase != "complete" {
                Text("Chapter transition in progress").foregroundStyle(Color.statusWarning)
            }
            if let error = chapter.error { Text(error).foregroundStyle(Color.statusWarning).textSelection(.enabled) }
        }.accessibilityIdentifier("wave-chapter")
    }
}

#endif
