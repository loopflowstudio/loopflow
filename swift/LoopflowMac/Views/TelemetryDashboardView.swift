import Charts
import Loopflow
import SwiftUI

struct TelemetryDashboardView: View {
    @Environment(\.palette) private var palette

    @State private var runs: [RunLedgerEntry] = []
    @State private var spans: [TraceSpan] = []
    @State private var doctor: DoctorReport?
    @State private var selectedRunID: String?
    @State private var errorMessage: String?
    @State private var isLoading = true

    private var traces: [RunLedgerEntry] {
        var seen = Set<String>()
        return runs
            .sorted { $0.started > $1.started }
            .filter { seen.insert($0.runId).inserted }
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            ScrollView {
                LazyVStack(alignment: .leading, spacing: Spacing.xl) {
                    ledgerHealth
                    chartCard("Run flamechart", subtitle: "Wall-clock width · own process cost") {
                        RunFlamechart(spans: spans)
                            .frame(minHeight: 120)
                    }
                    chartCard("Cost waterfall", subtitle: "One additive bar per process") {
                        CostWaterfall(spans: spans)
                            .frame(height: 220)
                    }
                    chartCard("Cache-hit ratio", subtitle: "cache read / (input + cache read)") {
                        CacheRatioChart(runs: runs)
                            .frame(height: 220)
                    }
                    chartCard("Ledger silence · 7 days", subtitle: "Black means no recorded run activity") {
                        SilenceRibbon(runs: runs)
                            .frame(height: 44)
                    }
                }
                .padding(Spacing.xxl)
            }
        }
        .background(palette.background)
        .task { await refresh() }
        .onChange(of: selectedRunID) { _, runID in
            guard let runID else { return }
            Task { await loadTrace(runID) }
        }
    }

    private var header: some View {
        HStack(spacing: Spacing.lg) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Telemetry")
                    .font(Typography.heroTitle(26))
                    .foregroundStyle(palette.text)
                Text("What moved, what it cost, and whether the ledger heard it.")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            Spacer()
            if !traces.isEmpty {
                Picker("Run", selection: $selectedRunID) {
                    ForEach(traces) { run in
                        Text("\(run.wave ?? "—") · \(run.label)")
                            .tag(Optional(run.runId))
                    }
                }
                .labelsHidden()
                .frame(maxWidth: 300)
            }
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
            runs = try await RegistryQueryLocal.shared.recentRuns()
            doctor = try? await RegistryQueryLocal.shared.doctor()
            errorMessage = nil
            let available = Set(runs.map(\.runId))
            if selectedRunID == nil || !available.contains(selectedRunID ?? "") {
                selectedRunID = traces.first?.runId
            } else if let selectedRunID {
                await loadTrace(selectedRunID)
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func loadTrace(_ runID: String) async {
        do {
            spans = try await RegistryQueryLocal.shared.trace(runID: runID)
            errorMessage = nil
        } catch {
            spans = []
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

private struct RunFlamechart: View {
    let spans: [TraceSpan]

    @Environment(\.palette) private var palette

    private var ordered: [(span: TraceSpan, depth: Int)] {
        let ids = Set(spans.map(\.processId))
        return spans
            .sorted { $0.startedAt < $1.startedAt }
            .map { span in
                var depth = 0
                var parent = span.parentProcessId
                var visited = Set<String>()
                while let value = parent, ids.contains(value), visited.insert(value).inserted {
                    depth += 1
                    parent = spans.first { $0.processId == value }?.parentProcessId
                }
                return (span, depth)
            }
    }

    var body: some View {
        if spans.isEmpty {
            empty("Select a recorded run")
        } else {
            let first = spans.map(\.startedAt).min() ?? 0
            let lastRecorded = spans.flatMap { [$0.startedAt, $0.endedAt ?? $0.startedAt] }.max() ?? first
            let range = max(lastRecorded - first, 1)
            let maxCost = max(spans.compactMap(\.costUsd).max() ?? 0, 0.01)
            VStack(spacing: Spacing.xs) {
                ForEach(ordered, id: \.span.processId) { item in
                    HStack(spacing: Spacing.sm) {
                        Text(item.span.name ?? "unknown process")
                            .font(Typography.code(10))
                            .foregroundStyle(palette.text)
                            .lineLimit(1)
                            .padding(.leading, CGFloat(item.depth) * 12)
                            .frame(width: 230, alignment: .leading)
                        GeometryReader { geometry in
                            let start = CGFloat(item.span.startedAt - first) / CGFloat(range)
                            let recordedEnd = item.span.endedAt ?? lastRecorded
                            let width = CGFloat(max(recordedEnd - item.span.startedAt, 0)) / CGFloat(range)
                            let intensity = (item.span.costUsd ?? 0) / maxCost
                            RoundedRectangle(cornerRadius: 3)
                                .fill(Color.loopflowBurgundy.opacity(0.3 + 0.7 * intensity))
                                .overlay {
                                    if item.span.endedAt == nil {
                                        HatchPattern()
                                            .clipShape(RoundedRectangle(cornerRadius: 3))
                                    }
                                }
                                .frame(width: max(6, geometry.size.width * width))
                                .offset(x: geometry.size.width * start)
                                .help(spanHelp(item.span))
                        }
                        .frame(height: 20)
                    }
                }
            }
        }
    }

    private func spanHelp(_ span: TraceSpan) -> String {
        let cost = span.costUsd.map { String(format: "$%.2f", $0) } ?? "—"
        let agent = [span.provider, span.model].compactMap { $0 }.joined(separator: ":")
        return "\(span.status) · \(cost) · \(agent.isEmpty ? "unknown agent" : agent)"
    }

    private func empty(_ text: String) -> some View {
        Text(text)
            .font(Typography.caption())
            .foregroundStyle(palette.textSecondary)
            .frame(maxWidth: .infinity, minHeight: 100)
    }
}

private struct HatchPattern: View {
    var body: some View {
        Canvas { context, size in
            var path = Path()
            for offset in stride(from: -size.height, through: size.width, by: 6) {
                path.move(to: CGPoint(x: offset, y: size.height))
                path.addLine(to: CGPoint(x: offset + size.height, y: 0))
            }
            context.stroke(path, with: .color(.white.opacity(0.6)), lineWidth: 1)
        }
    }
}

private struct CostWaterfall: View {
    let spans: [TraceSpan]

    @Environment(\.palette) private var palette

    var body: some View {
        if spans.contains(where: { ($0.costUsd ?? 0) > 0 }) {
            Chart(spans) { span in
                BarMark(
                    x: .value("Process", shortName(span.name)),
                    y: .value("Cost", span.costUsd ?? 0)
                )
                .foregroundStyle(Color.loopflowBurgundy.gradient)
                .annotation(position: .top) {
                    if let cost = span.costUsd {
                        Text(String(format: "$%.2f", cost))
                            .font(Typography.code(9))
                            .foregroundStyle(palette.textSecondary)
                    }
                }
            }
            .chartYAxisLabel("USD")
        } else {
            Text("No cost recorded for this run.")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func shortName(_ name: String?) -> String {
        guard let name else { return "unknown" }
        let parts = name.split(separator: " ")
        return parts.suffix(2).joined(separator: " ")
    }
}

private struct CacheRatioChart: View {
    let runs: [RunLedgerEntry]

    private var points: [CachePoint] {
        runs.compactMap { run in
            let denominator = run.inputTokens + run.cacheReadTokens
            guard denominator > 0 else { return nil }
            return CachePoint(
                id: run.processId,
                date: Date(timeIntervalSince1970: TimeInterval(run.started)),
                ratio: Double(run.cacheReadTokens) / Double(denominator),
                provider: run.provider ?? "unknown"
            )
        }
        .sorted { $0.date < $1.date }
    }

    var body: some View {
        Chart(points) { point in
            LineMark(
                x: .value("Time", point.date),
                y: .value("Cache hit", point.ratio)
            )
            .foregroundStyle(by: .value("Provider", point.provider))
            PointMark(
                x: .value("Time", point.date),
                y: .value("Cache hit", point.ratio)
            )
            .foregroundStyle(by: .value("Provider", point.provider))
        }
        .chartYScale(domain: 0...1)
        .chartYAxis {
            AxisMarks(format: Decimal.FormatStyle.Percent.percent.scale(100))
        }
    }
}

private struct CachePoint: Identifiable {
    let id: String
    let date: Date
    let ratio: Double
    let provider: String
}

private struct SilenceRibbon: View {
    let runs: [RunLedgerEntry]

    private let binCount = 28

    private var bins: [Bool] {
        let end = Date()
        let start = end.addingTimeInterval(-7 * 24 * 3600)
        let width = end.timeIntervalSince(start) / Double(binCount)
        return (0..<binCount).map { index in
            let binStart = start.addingTimeInterval(Double(index) * width)
            let binEnd = binStart.addingTimeInterval(width)
            return runs.contains { run in
                let runStart = Date(timeIntervalSince1970: TimeInterval(run.started))
                let runEnd = Date(timeIntervalSince1970: TimeInterval(run.ended ?? run.started))
                return runStart < binEnd && runEnd >= binStart
            }
        }
    }

    var body: some View {
        HStack(spacing: 2) {
            ForEach(Array(bins.enumerated()), id: \.offset) { _, active in
                Rectangle()
                    .fill(active ? Color.loopflowBurgundy : Color.black)
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
        .accessibilityLabel("Seven day ledger coverage")
        .accessibilityValue("\(bins.filter { $0 }.count) of \(binCount) intervals recorded activity")
    }
}
