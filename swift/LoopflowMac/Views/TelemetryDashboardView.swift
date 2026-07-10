import Charts
import Loopflow
import SwiftUI

/// Where the tokens went. Every chart here reads one payload — `lf usage --json`,
/// which applies the cumulative-diff rule so each boundary row carries what that
/// skill (or that inline run) actually spent. The rows are additive: they sum to
/// the totals `lf usage` prints. If a chart disagrees with that table, the chart
/// is wrong.
struct TelemetryDashboardView: View {
    @Environment(\.palette) private var palette

    private static let windowDays = 30

    @State private var spend: [TraceSpan] = []
    @State private var doctor: DoctorReport?
    @State private var codebase: CodeNode?
    @State private var growth: [CodeSnapshot] = []
    @State private var selectedRepo: String?
    @State private var errorMessage: String?
    @State private var isLoading = true

    /// Repos the ledger has seen, by absolute path. The codebase charts measure
    /// one repo at a time; the spend charts are machine-wide.
    private var repos: [String] {
        Array(Set(spend.compactMap(\.repo))).sorted()
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            ScrollView {
                LazyVStack(alignment: .leading, spacing: Spacing.xl) {
                    ledgerHealth

                    chartCard(
                        "Tokens by skill · \(Self.windowDays) days",
                        subtitle: "Input + output, per day. A run with no skill is inline work."
                    ) {
                        DailyTokensChart(spend: spend, key: Self.skillKey)
                            .frame(height: 260)
                    }

                    chartCard(
                        "Tokens by model · \(Self.windowDays) days",
                        subtitle: "provider:model — the harness and the model it drove"
                    ) {
                        DailyTokensChart(spend: spend, key: { $0.agent })
                            .frame(height: 260)
                    }

                    chartCard(
                        "Codebase over time · \(Self.windowDays) days",
                        subtitle: "What a model pays to read this repo, per top-level directory"
                    ) {
                        CodebaseGrowthChart(snapshots: growth)
                            .frame(height: 260)
                    }

                    chartCard(
                        codebaseTitle,
                        subtitle: "Files on disk. Width is tokens; a lockfile is cheap in lines and ruinous here."
                    ) {
                        CodeFlame(root: codebase)
                            .frame(minHeight: 200)
                    }

                    chartCard(
                        "Cache-hit ratio",
                        subtitle: "cache read / (input + cache read)"
                    ) {
                        CacheRatioChart(spend: spend)
                            .frame(height: 220)
                    }
                }
                .padding(Spacing.xxl)
            }
        }
        .background(palette.background)
        .task { await refresh() }
        .onChange(of: selectedRepo) { _, _ in
            Task { await loadCodebase() }
        }
    }

    /// A boundary with no skill is a run that never entered one — an inline
    /// prompt. Naming it keeps the series honest rather than dropping the spend.
    private static func skillKey(_ span: TraceSpan) -> String {
        span.skill ?? "(inline)"
    }

    private var header: some View {
        HStack(spacing: Spacing.lg) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Telemetry")
                    .font(Typography.heroTitle(26))
                    .foregroundStyle(palette.text)
                Text("Where the tokens went, and whether the ledger heard it.")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            Spacer()
            if repos.count > 1 {
                Picker("Repo", selection: $selectedRepo) {
                    ForEach(repos, id: \.self) { repo in
                        Text(shortRepoName(repo)).tag(Optional(repo))
                    }
                }
                .labelsHidden()
                .frame(maxWidth: 220)
            }
            Text(totalLabel)
                .font(Typography.code(12))
                .foregroundStyle(palette.textSecondary)
            Button {
                Task { await refresh() }
            } label: {
                Image(systemName: "arrow.clockwise")
            }
            .buttonStyle(.plain)
            .disabled(isLoading)
            .help("Refresh telemetry")
        }
        .padding(.horizontal, Spacing.xxl)
        .padding(.vertical, Spacing.lg)
    }

    private var codebaseTitle: String {
        guard let codebase else { return "Codebase flame" }
        return "Codebase flame · \(codebase.lines.formatted()) lines · \(compactTokens(codebase.tokens)) tokens"
    }

    private func shortRepoName(_ repo: String) -> String {
        repo.split(separator: "/").last.map(String.init) ?? repo
    }

    private var totalLabel: String {
        let tokens = spend.reduce(0) { $0 + $1.totalTokens }
        let cost = spend.reduce(0.0) { $0 + ($1.costUsd ?? 0) }
        return "\(tokens.formatted()) tokens · \(cost.formatted(.currency(code: "USD")))"
    }

    @ViewBuilder
    private var ledgerHealth: some View {
        if let errorMessage {
            Text(errorMessage)
                .font(Typography.caption())
                .foregroundStyle(Color.statusError)
        } else if let doctor {
            HStack(spacing: Spacing.sm) {
                ForEach(doctor.checks) { check in
                    HStack(spacing: Spacing.xs) {
                        Circle()
                            .fill(healthColor(check.status))
                            .frame(width: 7, height: 7)
                        Text(check.name)
                            .font(Typography.caption(11))
                            .foregroundStyle(palette.text)
                    }
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xs)
                    .background(palette.surfaceMuted, in: Capsule())
                    .help(check.detail)
                }
                Spacer()
                Text("\(doctor.rows) events")
                    .font(Typography.code(11))
                    .foregroundStyle(palette.textSecondary)
            }
        }
    }

    private func chartCard<Content: View>(
        _ title: String,
        subtitle: String,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(Typography.sectionTitle(18))
                    .foregroundStyle(palette.text)
                Text(subtitle)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }
            content()
        }
        .padding(Spacing.lg)
        .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.lg))
        .overlay {
            RoundedRectangle(cornerRadius: CornerRadius.lg)
                .stroke(palette.border, lineWidth: 1)
        }
    }

    private func refresh() async {
        isLoading = true
        defer { isLoading = false }
        do {
            spend = try await RegistryQueryLocal.shared.spend(days: Self.windowDays)
            doctor = try? await RegistryQueryLocal.shared.doctor()
            errorMessage = nil
        } catch {
            errorMessage = error.localizedDescription
        }
        if selectedRepo == nil || !repos.contains(selectedRepo ?? "") {
            selectedRepo = repos.first
        }
        await loadCodebase()
    }

    /// A repo the ledger remembers may no longer exist on disk (a worktree that
    /// was removed). That is a missing chart, not a failed dashboard.
    private func loadCodebase() async {
        guard let selectedRepo else {
            codebase = nil
            growth = []
            return
        }
        codebase = try? await RegistryQueryLocal.shared.codebase(repoPath: selectedRepo)
        growth = (try? await RegistryQueryLocal.shared.codebaseHistory(
            repoPath: selectedRepo, days: Self.windowDays
        )) ?? []
    }

    private func healthColor(_ status: String) -> Color {
        switch status {
        case "ok": .statusSuccess
        case "warn": .statusWarning
        default: .statusError
        }
    }
}

private func compactTokens(_ tokens: Int) -> String {
    switch tokens {
    case 1_000_000...: return "\(tokens / 1_000_000)M"
    case 1_000...: return "\(tokens / 1_000)k"
    default: return "\(tokens)"
    }
}

// MARK: - Daily tokens, stacked by a chosen dimension

private struct DailyBucket: Identifiable {
    let id: String
    let day: Date
    let series: String
    let tokens: Int
}

/// One stacked bar per day. `key` picks the dimension — skill, or provider:model.
private struct DailyTokensChart: View {
    @Environment(\.palette) private var palette
    let spend: [TraceSpan]
    let key: (TraceSpan) -> String

    private var buckets: [DailyBucket] {
        var totals: [String: DailyBucket] = [:]
        for span in spend where span.totalTokens > 0 {
            let day = Calendar.current.startOfDay(
                for: Date(timeIntervalSince1970: TimeInterval(span.startedAt))
            )
            let series = key(span)
            let id = "\(day.timeIntervalSince1970)-\(series)"
            let running = totals[id]?.tokens ?? 0
            totals[id] = DailyBucket(
                id: id, day: day, series: series, tokens: running + span.totalTokens
            )
        }
        return totals.values.sorted {
            ($0.day, $0.series) < ($1.day, $1.series)
        }
    }

    var body: some View {
        if buckets.isEmpty {
            EmptyChartHint()
        } else {
            Chart(buckets) { bucket in
                BarMark(
                    x: .value("Day", bucket.day, unit: .day),
                    y: .value("Tokens", bucket.tokens)
                )
                .foregroundStyle(by: .value("Series", bucket.series))
            }
            .chartLegend(position: .bottom, spacing: Spacing.sm)
            .chartYAxis {
                AxisMarks { value in
                    AxisGridLine().foregroundStyle(palette.border)
                    AxisValueLabel {
                        if let tokens = value.as(Int.self) {
                            Text(compactTokens(tokens))
                        }
                    }
                }
            }
        }
    }
}

// MARK: - Codebase: growth over time, and the flame on disk

/// Stacked by top-level directory. Lines are what a human counts; tokens are
/// what a run costs, and they disagree — a lockfile is cheap in lines and
/// ruinous here. This plots the number the context budget spends.
private struct CodebaseGrowthChart: View {
    @Environment(\.palette) private var palette
    let snapshots: [CodeSnapshot]

    private struct Point: Identifiable {
        let id: String
        let date: Date
        let path: String
        let tokens: Int
    }

    private var points: [Point] {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        return snapshots.flatMap { snapshot -> [Point] in
            guard let date = formatter.date(from: snapshot.date) else { return [] }
            return snapshot.slices.map { slice in
                Point(
                    id: "\(snapshot.commit)-\(slice.path)",
                    date: date,
                    path: slice.path,
                    tokens: slice.tokens
                )
            }
        }
    }

    var body: some View {
        if points.isEmpty {
            EmptyChartHint(
                message: "No git history in this window",
                hint: "`lf tokens --days 30` walks the repo's commits"
            )
        } else {
            Chart(points) { point in
                AreaMark(
                    x: .value("Day", point.date, unit: .day),
                    y: .value("Tokens", point.tokens)
                )
                .foregroundStyle(by: .value("Directory", point.path))
            }
            .chartLegend(position: .bottom, spacing: Spacing.sm)
            .chartYAxis {
                AxisMarks { value in
                    AxisGridLine().foregroundStyle(palette.border)
                    AxisValueLabel {
                        if let tokens = value.as(Int.self) {
                            Text(compactTokens(tokens))
                        }
                    }
                }
            }
        }
    }
}

/// An icicle over the files on disk: repo on top, each directory partitioned by
/// the tokens its subtree costs. Width is tokens, never lines and never time.
private struct CodeFlame: View {
    let root: CodeNode?

    var body: some View {
        if let root, root.tokens > 0 {
            VStack(alignment: .leading, spacing: 2) {
                CodeFlameRow(node: root, total: root.tokens, depth: 0)
            }
        } else {
            EmptyChartHint(
                message: "No codebase measured",
                hint: "The repo may no longer exist on disk"
            )
        }
    }
}

/// Depth is capped: below three levels the bars are thinner than their labels,
/// and a chart nobody can read is worse than one that stops.
private let maxFlameDepth = 3

private struct CodeFlameRow: View {
    @Environment(\.palette) private var palette
    let node: CodeNode
    let total: Int
    let depth: Int

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            GeometryReader { geometry in
                let fraction = total > 0 ? Double(node.tokens) / Double(total) : 0
                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: CornerRadius.sm)
                        .fill(depthColor.opacity(0.85))
                        .frame(width: max(2, geometry.size.width * fraction))
                    Text("\(node.name)  \(compactTokens(node.tokens))")
                        .font(Typography.code(11))
                        .foregroundStyle(palette.text)
                        .padding(.leading, Spacing.xs)
                        .lineLimit(1)
                }
            }
            .frame(height: 22)
            .help("\(node.path.isEmpty ? node.name : node.path): \(node.tokens.formatted()) tokens · \(node.lines.formatted()) lines")

            if depth < maxFlameDepth {
                ForEach(node.children.prefix(12)) { child in
                    CodeFlameRow(node: child, total: total, depth: depth + 1)
                        .padding(.leading, Spacing.md)
                }
            }
        }
    }

    private var depthColor: Color {
        let ramp: [Color] = [.loopflowBurgundy, .statusInfo, .statusSuccess, .statusWarning]
        return ramp[min(depth, ramp.count - 1)]
    }
}

// MARK: - Cache-hit ratio

private struct CachePoint: Identifiable {
    let id: String
    let date: Date
    let ratio: Double
    let agent: String
}

private struct CacheRatioChart: View {
    @Environment(\.palette) private var palette
    let spend: [TraceSpan]

    private var points: [CachePoint] {
        spend
            .compactMap { span in
                let denominator = (span.inputTokens ?? 0) + (span.cacheReadTokens ?? 0)
                guard denominator > 0 else { return nil }
                return CachePoint(
                    id: span.id,
                    date: Date(timeIntervalSince1970: TimeInterval(span.startedAt)),
                    ratio: Double(span.cacheReadTokens ?? 0) / Double(denominator),
                    agent: span.agent
                )
            }
            .sorted { $0.date < $1.date }
    }

    var body: some View {
        if points.isEmpty {
            EmptyChartHint()
        } else {
            Chart(points) { point in
                PointMark(
                    x: .value("When", point.date),
                    y: .value("Cache hit", point.ratio)
                )
                .foregroundStyle(by: .value("Agent", point.agent))
                .opacity(0.8)
            }
            .chartYScale(domain: 0...1)
            .chartYAxis {
                AxisMarks { value in
                    AxisGridLine().foregroundStyle(palette.border)
                    AxisValueLabel {
                        if let ratio = value.as(Double.self) {
                            Text(ratio.formatted(.percent.precision(.fractionLength(0))))
                        }
                    }
                }
            }
            .chartLegend(position: .bottom, spacing: Spacing.sm)
        }
    }
}

private struct EmptyChartHint: View {
    @Environment(\.palette) private var palette

    var message: String = "No recorded spend in this window"
    var hint: String = "Run `lf doctor` to check the ledger is being written"

    var body: some View {
        VStack(spacing: Spacing.xs) {
            Text(message)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
            Text(hint)
                .font(Typography.code(11))
                .foregroundStyle(palette.textSecondary)
        }
        .frame(maxWidth: .infinity, minHeight: 120)
    }
}
