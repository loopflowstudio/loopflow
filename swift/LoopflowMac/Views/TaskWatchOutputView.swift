#if os(macOS)
import Loopflow
import SwiftUI

struct TaskWatchOutputView: View {
    @Bindable var store: TaskWatchStore
    let onHistory: () -> Void
    let onFollow: () -> Void
    let onReload: () -> Void
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(store.filtersStage ? "Stage output" : "All output")
                    .font(Typography.sectionTitle(13))
                if let run = store.runId {
                    Text(run).font(Typography.code(10)).lineLimit(1).help(run)
                }
                Spacer()
                if store.isReadingOutput { ProgressView().controlSize(.small) }
                Button("Load history", action: onHistory)
                    .disabled(store.isRefreshing || store.isReadingOutput || store.needsOutputReload)
                Button("Follow live", systemImage: "arrow.down.to.line", action: onFollow)
                    .disabled(store.isRefreshing || store.isReadingOutput || store.needsOutputReload)
            }
            .padding(Spacing.md)
            HStack {
                Text("Updates on refresh · arrivals in observation order")
                Spacer()
                Text(store.followsOutput ? "Following output" : "Scroll paused")
            }
            .font(Typography.caption(10))
            .foregroundStyle(palette.textSecondary)
            .padding(.horizontal, Spacing.md)
            .padding(.bottom, Spacing.sm)
            if let error = store.outputError {
                HStack {
                    Label("\(store.output.isEmpty ? "Output unavailable" : "Output stale"): \(error)", systemImage: "exclamationmark.triangle")
                    if store.needsOutputReload { Button("Reload output", action: onReload) }
                }
                .font(Typography.caption(11))
                .padding(Spacing.md)
            }
            ForEach(store.outputGaps, id: \.self) { gap in
                Label(gap.message, systemImage: "exclamationmark.triangle")
                    .font(Typography.caption(11)).padding(.horizontal, Spacing.md)
            }
            Divider()
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: Spacing.lg) {
                        if store.visibleOutput.isEmpty {
                            Text(store.output.isEmpty
                                 ? "No output sources observed yet. Refresh to read available evidence."
                                 : "No loaded output matches this selection. Load history or Follow live.")
                                .font(Typography.body(12))
                                .foregroundStyle(palette.textSecondary)
                        }
                        if !store.visibleOutput.isEmpty && store.outputGroups.isEmpty {
                            Text("No output in the pages read. Expand Sources for availability, or Load history.")
                                .font(Typography.body(12))
                                .foregroundStyle(palette.textSecondary)
                        }
                        if !store.visibleOutput.isEmpty {
                            DisclosureGroup {
                                ForEach(store.visibleOutput) { output in sourceEvidence(output) }
                            } label: {
                                HStack {
                                    Text("Sources (\(store.visibleOutput.count))")
                                    let unavailable = store.visibleOutput.filter { !$0.source.available }.count
                                    let warnings = store.visibleOutput.filter { !$0.source.gaps.isEmpty }.count
                                    if unavailable > 0 {
                                        Label("\(unavailable) unavailable", systemImage: "exclamationmark.triangle")
                                    }
                                    if warnings > 0 {
                                        Label("\(warnings) with warnings", systemImage: "exclamationmark.triangle")
                                    }
                                }
                                .font(Typography.caption(11))
                            }
                        }
                        ForEach(store.outputGroups) { group in
                            VStack(alignment: .leading, spacing: Spacing.sm) {
                                sourceHeader(group.source)
                                Text(group.history ? "Loaded history" : "Live arrivals")
                                    .font(Typography.caption(10))
                                    .foregroundStyle(palette.textSecondary)
                                ForEach(group.rows) { row in
                                    TaskWatchOutputRowView(row: row)
                                        .id(TaskWatchOutputRowReference(source: group.sourceId, row: row.id))
                                }
                            }
                            .padding(Spacing.md)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(palette.surfaceMuted)
                            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                        }
                        Color.clear.frame(height: 1).id("output-bottom")
                    }
                    .padding(Spacing.md)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .accessibilityIdentifier("task-watch-output")
                .onChange(of: store.outputRevision) { _, _ in
                    if store.followsOutput { follow(proxy) }
                }
                .onChange(of: store.followsOutput) { _, follows in
                    if follows { follow(proxy) }
                }
                .onScrollPhaseChange { _, phase in
                    if phase == .tracking || phase == .interacting { store.followsOutput = false }
                }
            }
        }
    }

    private func follow(_ proxy: ScrollViewProxy) {
        if let row = store.latestOutputRow {
            proxy.scrollTo(row, anchor: .bottom)
        } else {
            proxy.scrollTo("output-bottom", anchor: .bottom)
        }
    }

    private func sourceHeader(_ source: TaskOutputSource) -> some View {
        HStack {
            Button { store.inspectRun(source.runId) } label: {
                Text("\(source.provider) · \(source.runId)")
                    .font(Typography.code(11))
            }
            .buttonStyle(.link)
            Spacer()
            if let stage = source.stage {
                Button("Stage \(stage.stepIndex + 1) · attempt iteration \(stage.iteration)") {
                    store.selectStage(stage)
                }
                .help(stage.invocationId)
            } else {
                Text("Unassigned Run").foregroundStyle(palette.textSecondary)
            }
        }
        .font(Typography.caption(11))
    }

    private func sourceEvidence(_ output: TaskWatchOutput) -> some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            sourceHeader(output.source)
            if !output.source.available {
                Label("Output source unavailable", systemImage: "exclamationmark.triangle")
            } else if output.rows.isEmpty {
                Text("No new output in the pages read. Load history to inspect earlier output.")
            }
            if output.historyHasMore { Text("More history remains for this Run.") }
            if output.liveHasMore { Text("More arriving output remains; Refresh continues reading.") }
            ForEach(output.source.gaps, id: \.self) { gap in
                Label(gap.message, systemImage: "exclamationmark.triangle")
            }
        }
        .font(Typography.caption(11))
        .textSelection(.enabled)
        .padding(Spacing.md)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(palette.surfaceMuted)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }
}

private struct TaskWatchOutputRowView: View {
    let row: TaskWatchOutputRow
    @State private var expanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text(row.title).font(Typography.caption(10).weight(.semibold))
            if !row.text.isEmpty {
                Text(expanded ? row.text : String(row.text.prefix(2_000)))
                    .font(row.code ? Typography.code(11) : Typography.body(12))
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                if row.text.count > 2_000 {
                    Button(expanded ? "Show less" : "Show full output") { expanded.toggle() }
                        .buttonStyle(.link)
                }
            }
        }
    }
}
#endif
