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
    @State private var errorMessage: String?
    @State private var isLoading = true

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
                        "Token flame · by repo",
                        subtitle: "repo → wave → flow → skill. Width is tokens, not time."
                    ) {
                        TokenFlame(spend: spend)
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

// MARK: - Token flame

/// A node in the repo → wave → flow → skill rollup. Width is tokens.
private struct FlameNode: Identifiable {
    let id: String
    let label: String
    let tokens: Int
    var children: [FlameNode]
}

/// An icicle chart: the repo on top, each level below partitioned by the tokens
/// its children spent. Width is tokens, never time — a fast skill can be the
/// widest thing on the page, which is exactly what a cost chart should say.
private struct TokenFlame: View {
    let spend: [TraceSpan]

    private var roots: [FlameNode] { Self.build(spend) }

    var body: some View {
        if roots.isEmpty {
            EmptyChartHint()
        } else {
            VStack(alignment: .leading, spacing: 2) {
                ForEach(roots) { root in
                    FlameRow(node: root, total: root.tokens, depth: 0)
                }
            }
        }
    }

    /// Group by the dimensions each row carries, skipping the ones it doesn't.
    static func build(_ spend: [TraceSpan]) -> [FlameNode] {
        let rows = spend.filter { $0.totalTokens > 0 }
        let byRepo = Dictionary(grouping: rows) { $0.repo.map(shortRepo) ?? "(unattributed)" }
        return byRepo
            .map { repo, repoRows in
                FlameNode(
                    id: repo,
                    label: repo,
                    tokens: repoRows.reduce(0) { $0 + $1.totalTokens },
                    children: level(repoRows, prefix: repo, path: [\.wave, \.flow, \.skill])
                )
            }
            .sorted { $0.tokens > $1.tokens }
    }

    private static func level(
        _ rows: [TraceSpan],
        prefix: String,
        path: [KeyPath<TraceSpan, String?>]
    ) -> [FlameNode] {
        guard let keyPath = path.first else { return [] }
        let rest = Array(path.dropFirst())
        // A dimension nothing in this subtree carries adds a row of noise; skip it.
        guard rows.contains(where: { $0[keyPath: keyPath] != nil }) else {
            return level(rows, prefix: prefix, path: rest)
        }
        let groups = Dictionary(grouping: rows) { $0[keyPath: keyPath] ?? "—" }
        return groups
            .map { label, groupRows in
                FlameNode(
                    id: "\(prefix)/\(label)",
                    label: label,
                    tokens: groupRows.reduce(0) { $0 + $1.totalTokens },
                    children: level(groupRows, prefix: "\(prefix)/\(label)", path: rest)
                )
            }
            .sorted { $0.tokens > $1.tokens }
    }

    private static func shortRepo(_ repo: String) -> String {
        repo.split(separator: "/").last.map(String.init) ?? repo
    }
}

private struct FlameRow: View {
    @Environment(\.palette) private var palette
    let node: FlameNode
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
                    Text("\(node.label)  \(compactTokens(node.tokens))")
                        .font(Typography.code(11))
                        .foregroundStyle(palette.text)
                        .padding(.leading, Spacing.xs)
                        .lineLimit(1)
                }
            }
            .frame(height: 22)
            .help("\(node.label): \(node.tokens.formatted()) tokens")

            ForEach(node.children) { child in
                FlameRow(node: child, total: total, depth: depth + 1)
                    .padding(.leading, Spacing.md)
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

    var body: some View {
        VStack(spacing: Spacing.xs) {
            Text("No recorded spend in this window")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
            Text("Run `lf doctor` to check the ledger is being written")
                .font(Typography.code(11))
                .foregroundStyle(palette.textSecondary)
        }
        .frame(maxWidth: .infinity, minHeight: 120)
    }
}
