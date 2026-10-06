import Foundation
import Loopflow
import SwiftUI

/// Complete Task-attributed Session history, disclosed on demand. The list comes from the
/// shared `lf usage --days 0 --task ID --json` reader; loading begins on expansion. Provider outcomes
/// remain separate from recorded input completion and command exit. Each step also
/// shows what its submitted input was made of, from `lf usage --context`.
struct TaskHistoryView: View {
    let model: PodiumModel
    let task: RoadmapTask
    let wave: WaveSnapshot
    @Environment(\.palette) private var palette

    private var reading: PodiumReading<[SessionHistory]> { model.sessionHistory[task.id] }
    private var history: [SessionHistory]? { reading.value }
    private var expanded: Bool { model.navigation.expandedHistory.contains(task.id) }
    private var inFlight: Bool { model.sessionHistory.inFlight.contains(task.id) }
    private var context: ContextReport? { model.taskContext[task.id].value }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            WorkspaceDisclosureHeading(
                title: "Session history",
                count: expanded ? history?.count : nil,
                expanded: expanded,
                inFlight: inFlight,
                failure: expanded && reading.errorMessage != nil ? history != nil : nil,
                identifier: "task-history",
                accessibilityLabel: "Session history, \(expanded ? "expanded" : "collapsed")",
                help: "Complete recorded Session history for \(task.task.identifier)",
                toggle: {
                    let navigation = model.navigation
                    if navigation.expandedHistory.remove(task.id) == nil {
                        navigation.expandedHistory.insert(task.id)
                        load()
                    }
                },
                retry: load
            )
            if expanded {
                if let history { list(history) } else { statusText }
            }
        }
    }

    private func load() {
        Task { await model.loadSessionHistory(task: task, wave: wave) }
        Task { await model.loadTaskContext(task: task, wave: wave) }
    }

    @ViewBuilder
    private var statusText: some View {
        Text(reading.errorMessage.map { "History could not be read: \($0)" } ?? "Reading history…")
            .font(Typography.body(12))
            .foregroundStyle(palette.textSecondary)
            .textSelection(.enabled)
            .accessibilityIdentifier("task-history-status")
    }

    private func list(_ history: [SessionHistory]) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            if let reason = reading.errorMessage {
                Text("Latest read failed: \(reason)")
                    .font(Typography.body(12))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                    .padding(.bottom, Spacing.xs)
                    .accessibilityIdentifier("task-history-status")
            }
            if history.isEmpty {
                Text("No Session history recorded for this Task.")
                    .font(Typography.body(12))
                    .foregroundStyle(palette.textSecondary)
                    .accessibilityIdentifier("task-history-empty")
            }
            if let totals = context?.totals, let line = Self.sources(totals) {
                contextLine("Task context  \(line)", flagged: totals.contains(where: \.overBudget))
                    .padding(.bottom, Spacing.xs)
                    .accessibilityIdentifier("task-context-totals")
            }
            ForEach(history) { input in
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Text(input.skill ?? input.harness)
                        .font(Typography.code(11.5))
                        .foregroundStyle(palette.text)
                    Text(Self.agent(input))
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                    Spacer(minLength: Spacing.sm)
                    Text(Self.started(input.observedAt))
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textTertiary)
                    Text(input.status)
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                        .frame(minWidth: 72, alignment: .trailing)
                    Text(Self.shortId(input.id))
                        .font(Typography.code(10.5))
                        .foregroundStyle(palette.textTertiary)
                        .textSelection(.enabled)
                        .help(input.id)
                }
                .padding(.vertical, 5)
                .accessibilityElement(children: .combine)
                .accessibilityIdentifier("task-history-input-\(input.id)")
                if let step = context?.steps.first(where: { $0.input == (input.artifactKey ?? input.sessionId) }) {
                    contextLine(
                        Self.summary(step),
                        flagged: step.overAssembledBudget || step.sources.contains(where: \.overBudget)
                    )
                    .padding(.bottom, 5)
                    .accessibilityIdentifier("task-history-input-context-\(input.id)")
                }
            }
        }
        .accessibilityIdentifier("task-history-list")
    }

    private func contextLine(_ text: String, flagged: Bool) -> some View {
        Text(text)
            .font(Typography.code(10.5))
            .foregroundStyle(flagged ? palette.text : palette.textTertiary)
            .textSelection(.enabled)
            .help("Submitted input by source, in tokens. ! marks a source over its budget; ? is unrecorded.")
    }

    /// Measured sources joined on one line; nil when nothing was recorded.
    static func sources(_ sources: [ContextSourceUsage]) -> String? {
        let parts = sources.compactMap { usage -> String? in
            guard let tokens = usage.tokens else { return nil }
            guard tokens > 0 || usage.overBudget else { return nil }
            let count = usage.source == "steers" ? usage.count.map { " (\($0))" } ?? " (?)" : ""
            return "\(usage.source.replacingOccurrences(of: "_", with: " ")) \(tokenCount(tokens))\(count)\(usage.overBudget ? "!" : "")"
        }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    static func summary(_ step: StepContext) -> String {
        var parts = [sources(step.sources) ?? "context not recorded"]
        if let assembled = step.assembledTokens {
            parts.append("assembled \(tokenCount(assembled))\(step.overAssembledBudget ? "!" : "")")
        }
        if let peak = step.peakRequestTokens {
            parts.append("peak \(tokenCount(peak))")
        }
        return parts.joined(separator: " · ")
    }

    static func tokenCount(_ tokens: Int) -> String {
        tokens >= 1000 ? String(format: "%.1fk", Double(tokens) / 1000) : String(tokens)
    }

    static func agent(_ input: SessionHistory) -> String {
        input.model.map { "\(input.harness):\($0)" } ?? input.harness
    }

    static func started(_ unix: Int) -> String {
        Date(timeIntervalSince1970: TimeInterval(unix)).formatted(date: .abbreviated, time: .shortened)
    }

    static func shortId(_ id: String) -> String {
        let body = id.hasPrefix("run_") ? String(id.dropFirst(4)) : id
        return String(body.prefix(8))
    }
}
