import Charts
import LoopflowCore
import SwiftUI

private enum AnalyticsLens: String, CaseIterable, Identifiable {
    case work = "Work"
    case prompt = "Prompt"

    var id: String { rawValue }
}

private enum AnalyticsPeriod: Int, CaseIterable, Identifiable {
    case days7 = 7
    case days30 = 30
    case days90 = 90

    var id: Int { rawValue }

    var label: String { "\(rawValue)d" }
}

private struct WorkPoint: Identifiable {
    let id = UUID()
    let date: Date
    let group: String
    let totals: TokenTotals
    let cost: CostEstimate
}

private struct PromptSegment: Identifiable {
    let id = UUID()
    let date: Date
    let source: String
    let inputTokens: Int
}

struct AnalyticsDashboardView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    @State private var lens: AnalyticsLens = .work
    @State private var period: AnalyticsPeriod = .days30
    @State private var workGroupBy: UsageGroupBy = .wave
    @State private var selectedWave: String?
    @State private var selectedFlow: String?
    @State private var selectedStep: String?
    @State private var selectedWorkDate: Date?
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var workPoints: [WorkPoint] = []
    @State private var promptSegments: [PromptSegment] = []

    private let workGroupOptions: [UsageGroupBy] = [.wave, .flow, .step, .model]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.lg) {
                header

                Picker("Lens", selection: $lens) {
                    ForEach(AnalyticsLens.allCases) { lens in
                        Text(lens.rawValue).tag(lens)
                    }
                }
                .pickerStyle(.segmented)

                controls
                content
            }
            .padding(Spacing.lg)
        }
        .background(palette.background)
        .task(id: reloadKey) {
            await loadAnalytics()
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text("Analytics")
                .font(Typography.sectionTitle(24))
                .foregroundStyle(palette.text)
            Text("Estimated cost and prompt composition across your waves")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
        }
    }

    @ViewBuilder
    private var controls: some View {
        HStack(spacing: Spacing.md) {
            Picker("Period", selection: $period) {
                ForEach(AnalyticsPeriod.allCases) { period in
                    Text(period.label).tag(period)
                }
            }
            .pickerStyle(.segmented)
            .frame(maxWidth: 240)

            if lens == .work {
                Picker("Group", selection: $workGroupBy) {
                    ForEach(workGroupOptions, id: \.self) { option in
                        Text(option.rawValue.capitalized).tag(option)
                    }
                }
                .pickerStyle(.menu)
            } else {
                filterPicker(
                    label: "Wave",
                    selection: $selectedWave,
                    values: waveFilterOptions
                )
                filterPicker(
                    label: "Flow",
                    selection: $selectedFlow,
                    values: flowFilterOptions
                )
                filterPicker(
                    label: "Step",
                    selection: $selectedStep,
                    values: stepFilterOptions
                )
            }
            Spacer()
        }
    }

    @ViewBuilder
    private var content: some View {
        if let errorMessage {
            Text(errorMessage)
                .font(Typography.caption())
                .foregroundStyle(Color.statusError)
        }

        if isLoading {
            ProgressView("Loading analytics…")
        } else if lens == .work {
            workChart
        } else {
            promptChart
        }
    }

    // MARK: - Work lens

    @ViewBuilder
    private var workChart: some View {
        if workPoints.isEmpty {
            ContentUnavailableView("No usage data", systemImage: "chart.line.uptrend.xyaxis")
        } else {
            Chart(workPoints) { point in
                LineMark(
                    x: .value("Date", point.date),
                    y: .value("Cost", point.cost.total)
                )
                .foregroundStyle(by: .value("Group", point.group))

                PointMark(
                    x: .value("Date", point.date),
                    y: .value("Cost", point.cost.total)
                )
                .foregroundStyle(by: .value("Group", point.group))
            }
            .frame(minHeight: 280)
            .chartLegend(position: .bottom)
            .chartXSelection(value: $selectedWorkDate)
            .chartYAxis {
                AxisMarks { value in
                    AxisGridLine()
                    AxisValueLabel {
                        if let cost = value.as(Double.self) {
                            Text(formatCost(cost))
                                .font(Typography.code(10))
                                .monospacedDigit()
                        }
                    }
                }
            }

            workBreakdown
        }
    }

    @ViewBuilder
    private var workBreakdown: some View {
        if let selectedWorkDate {
            let dailyPoints = points(for: selectedWorkDate)
            if !dailyPoints.isEmpty {
                let anyEstimate = dailyPoints.contains { $0.cost.isEstimate }

                VStack(alignment: .leading, spacing: Spacing.sm) {
                    HStack(spacing: Spacing.xs) {
                        Text("Breakdown · \(displayDateFormatter.string(from: selectedWorkDate))")
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                        if anyEstimate {
                            Text("~ estimated")
                                .font(Typography.code(10))
                                .foregroundStyle(palette.textSecondary)
                        }
                    }

                    ForEach(dailyPoints, id: \.group) { point in
                        HStack(alignment: .top) {
                            Text(point.group)
                                .font(Typography.caption())
                                .frame(minWidth: 80, alignment: .leading)
                            Spacer()
                            VStack(alignment: .trailing, spacing: 2) {
                                Text(formatCost(point.cost.total))
                                    .font(Typography.code(12))
                                    .monospacedDigit()
                                HStack(spacing: Spacing.sm) {
                                    costLabel("non-cached", point.cost.nonCached)
                                    costLabel("cached", point.cost.cached)
                                }
                            }
                        }
                        .foregroundStyle(palette.text)
                    }
                }
            }
        }
    }

    private func costLabel(_ label: String, _ value: Double) -> some View {
        HStack(spacing: 2) {
            Text(label)
            Text(formatCost(value))
                .monospacedDigit()
        }
        .font(Typography.code(10))
        .foregroundStyle(palette.textSecondary)
    }

    // MARK: - Prompt lens

    @ViewBuilder
    private var promptChart: some View {
        if promptSegments.isEmpty {
            ContentUnavailableView("No prompt data", systemImage: "chart.bar.xaxis")
        } else {
            Chart(promptSegments) { segment in
                BarMark(
                    x: .value("Date", segment.date),
                    y: .value("Input Tokens", segment.inputTokens)
                )
                .foregroundStyle(by: .value("Source", segment.source))
            }
            .frame(minHeight: 280)
            .chartLegend(position: .bottom)
        }
    }

    // MARK: - Data loading

    private var reloadKey: String {
        [
            lens.rawValue,
            "\(period.rawValue)",
            workGroupBy.rawValue,
            selectedWave ?? "all",
            selectedFlow ?? "all",
            selectedStep ?? "all",
            repoState.isConnected ? "connected" : "disconnected",
        ].joined(separator: "|")
    }

    private var waveFilterOptions: [(id: String, label: String)] {
        repoState.waves
            .sorted { $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending }
            .map { ($0.id, $0.displayName) }
    }

    private var flowFilterOptions: [(id: String, label: String)] {
        repoState.flows
            .filter { $0.type == .flow }
            .map { ($0.name, $0.name) }
            .sorted { $0.label.localizedCaseInsensitiveCompare($1.label) == .orderedAscending }
    }

    private var stepFilterOptions: [(id: String, label: String)] {
        let standaloneSteps = repoState.flows
            .filter { $0.type == .step }
            .map(\.name)
        let nestedSteps = repoState.flows
            .filter { $0.type == .flow }
            .flatMap(\.steps)
            .map(\.prompt)
        let unique = Set(standaloneSteps + nestedSteps)
        return unique
            .map { ($0, $0) }
            .sorted { $0.label.localizedCaseInsensitiveCompare($1.label) == .orderedAscending }
    }

    @ViewBuilder
    private func filterPicker(
        label: String,
        selection: Binding<String?>,
        values: [(id: String, label: String)]
    ) -> some View {
        Picker(label, selection: selection) {
            Text("All \(label)s").tag(String?.none)
            ForEach(values, id: \.id) { option in
                Text(option.label).tag(Optional(option.id))
            }
        }
        .pickerStyle(.menu)
    }

    private func loadAnalytics() async {
        guard repoState.isConnected else {
            workPoints = []
            promptSegments = []
            errorMessage = nil
            return
        }

        isLoading = true
        errorMessage = nil
        defer { isLoading = false }

        do {
            switch lens {
            case .work:
                let timeseries = try await repoState.usageTimeseries(
                    filters: UsageAnalyticsFilters(
                        from: startDate,
                        to: Date()
                    ),
                    bucket: .day,
                    groupBy: workGroupBy
                )
                workPoints = makeWorkPoints(from: timeseries)
                selectedWorkDate = nil
            case .prompt:
                let timeseries = try await repoState.usageTimeseries(
                    filters: UsageAnalyticsFilters(
                        wave: selectedWave,
                        flow: selectedFlow,
                        step: selectedStep,
                        from: startDate,
                        to: Date()
                    ),
                    bucket: .day,
                    groupBy: .source
                )
                promptSegments = makePromptSegments(from: timeseries)
            }
        } catch {
            errorMessage = error.localizedDescription
            workPoints = []
            promptSegments = []
        }
    }

    private var startDate: Date {
        Calendar.current.date(byAdding: .day, value: -period.rawValue, to: Date()) ?? Date()
    }

    private func makeWorkPoints(from timeseries: UsageTimeseries) -> [WorkPoint] {
        timeseries.buckets.flatMap { bucket in
            guard let date = parsePeriodDate(bucket.period) else { return [WorkPoint]() }
            return bucket.groups.map { group in
                let model = workGroupBy == .model ? group.key : nil
                return WorkPoint(
                    date: date,
                    group: group.key,
                    totals: group.tokens,
                    cost: CostEstimator.estimateCost(group.tokens, model: model)
                )
            }
        }
    }

    private func makePromptSegments(from timeseries: UsageTimeseries) -> [PromptSegment] {
        timeseries.buckets.flatMap { bucket in
            guard let date = parsePeriodDate(bucket.period) else { return [PromptSegment]() }
            return bucket.groups.map { group in
                PromptSegment(
                    date: date,
                    source: group.key,
                    inputTokens: group.tokens.input
                )
            }
        }
    }

    private func parsePeriodDate(_ value: String) -> Date? {
        if let day = dayFormatter.date(from: value) {
            return day
        }
        if let month = monthFormatter.date(from: value) {
            return month
        }
        return nil
    }

    private func points(for date: Date) -> [WorkPoint] {
        workPoints
            .filter { Calendar.current.isDate($0.date, inSameDayAs: date) }
            .sorted { $0.cost.total > $1.cost.total }
    }
}

// MARK: - Formatting

private func formatCost(_ value: Double) -> String {
    if value < 0.005 { return "<$0.01" }
    return String(format: "$%.2f", value)
}

private func posixFormatter(_ format: String) -> DateFormatter {
    let formatter = DateFormatter()
    formatter.calendar = Calendar(identifier: .gregorian)
    formatter.locale = Locale(identifier: "en_US_POSIX")
    formatter.timeZone = TimeZone(secondsFromGMT: 0)
    formatter.dateFormat = format
    return formatter
}

private let dayFormatter = posixFormatter("yyyy-MM-dd")
private let monthFormatter = posixFormatter("yyyy-MM")

private let displayDateFormatter: DateFormatter = {
    let formatter = DateFormatter()
    formatter.dateStyle = .medium
    formatter.timeStyle = .none
    return formatter
}()
