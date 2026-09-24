#if os(macOS)
import Loopflow
import SwiftUI

struct TaskWatchView: View {
    let issue: String
    @Bindable var store: TaskWatchStore

    @Environment(\.palette) private var palette
    @State private var refresh = 0
    @State private var loadHistory = false
    @State private var reloadOutput = false

    var body: some View {
        VSplitView {
            plan.frame(minHeight: 180, idealHeight: 310)
            TaskWatchOutputView(store: store, onHistory: {
                store.followsOutput = false
                loadHistory = true
                refresh += 1
            }, onFollow: {
                store.followLive()
                refresh += 1
            }, onReload: {
                reloadOutput = true
                refresh += 1
            })
            .frame(minHeight: 180, idealHeight: 350)
        }
        .foregroundStyle(palette.text)
        .background(palette.background)
        .task(id: refresh) {
            let history = loadHistory
            let reload = reloadOutput
            loadHistory = false
            reloadOutput = false
            await store.refresh(issue: issue, query: RegistryQueryLocal.shared)
            guard !Task.isCancelled else { return }
            await store.readOutput(issue: issue, query: RegistryQueryLocal.shared, history: history, reload: reload)
        }
    }

    private var plan: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Recorded plan")
                    .font(Typography.sectionTitle(13))
                Spacer()
                if store.isRefreshing { ProgressView().controlSize(.small) }
                Button("Refresh", systemImage: "arrow.clockwise") { refresh += 1 }
                    .disabled(store.isRefreshing || store.isReadingOutput)
                    .accessibilityLabel("Refresh Task Watch")
            }
            .padding(Spacing.md)
            if let error = store.error {
                evidence(
                    store.snapshot == nil ? "Watch unavailable" : "Refresh failed — showing the last snapshot",
                    detail: error
                )
            }
            if let snapshot = store.snapshot {
                if !snapshot.gaps.isEmpty {
                    DisclosureGroup("Incomplete evidence (\(snapshot.gaps.count))") {
                        ForEach(Array(snapshot.gaps.enumerated()), id: \.offset) { _, gap in
                            Text(gap.message).frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                    .font(Typography.caption(11))
                    .padding(.horizontal, Spacing.md)
                    .padding(.bottom, Spacing.sm)
                }
                if snapshot.invocations.isEmpty {
                    ContentUnavailableView(
                        "No recorded plan",
                        systemImage: "point.3.connected.trianglepath.dotted",
                        description: Text("Runs without a retained plan remain listed below.")
                    )
                } else {
                    HStack {
                        Picker("Invocation", selection: Binding(
                            get: { store.invocationId },
                            set: {
                                store.selectInvocation($0)
                                store.inspectStage(store.stepIndex)
                            }
                        )) {
                            ForEach(snapshot.invocations) { invocation in
                                Text("\(invocation.flow) · \(invocation.id) · \(invocation.settlement?.rawValue ?? "unsettled")")
                                    .tag(Optional(invocation.id))
                            }
                        }
                    }
                    .padding(.horizontal, Spacing.md)
                    .padding(.bottom, Spacing.sm)
                    Divider()
                    HSplitView {
                        stages.frame(minWidth: 260, idealWidth: 310, maxWidth: 390)
                        stageDetail.frame(minWidth: 350, maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
                Divider()
                DisclosureGroup("All Task Runs (\(snapshot.runs.count))") {
                    ScrollView {
                        VStack(alignment: .leading, spacing: Spacing.xs) {
                            ForEach(snapshot.runs, id: \.runId) { run in
                                HStack {
                                    Button { store.inspectRun(run.runId) } label: {
                                        runLabel(run.runId, provider: run.provider)
                                    }
                                    .buttonStyle(.link)
                                    Spacer()
                                    if let stage = run.stage {
                                        Button("Stage \(stage.stepIndex + 1) · iteration \(stage.iteration)") {
                                            store.selectStage(stage)
                                        }
                                    } else {
                                        Text("Unassigned").foregroundStyle(palette.textSecondary)
                                    }
                                }
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .frame(maxHeight: 160)
                }
                .font(Typography.caption(11))
                .padding(Spacing.md)
            } else if store.error == nil {
                ProgressView("Reading retained Task history…")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                Spacer()
            }
        }
    }

    private var stages: some View {
        List(selection: Binding(get: { store.stepIndex }, set: { store.inspectStage($0) })) {
            if let invocation = store.invocation {
                ForEach(Array(invocation.stages.enumerated()), id: \.element.stepIndex) { index, stage in
                    VStack(alignment: .leading, spacing: Spacing.xs) {
                        if index > 0 {
                            Image(systemName: "arrow.down")
                                .foregroundStyle(palette.textSecondary)
                                .accessibilityHidden(true)
                        }
                        Label("\(stage.stepIndex + 1). \(stage.name)", systemImage: icon(stage))
                            .font(Typography.body(12).weight(.semibold))
                        Text("\(stage.human ? "Human checkpoint" : stage.kind.rawValue) · \(stage.attempts.count) \(stage.attempts.count == 1 ? "attempt" : "attempts")")
                            .font(Typography.caption(10))
                        HStack {
                            if isActive(stage) {
                                Label("Current stage", systemImage: "location.fill")
                            }
                            Text(stage.attempts.last?.state.rawValue ?? "Not entered")
                        }
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                    }
                    .padding(.vertical, Spacing.xs)
                    .tag(stage.stepIndex)
                    .accessibilityElement(children: .combine)
                }
            }
        }
        .listStyle(.sidebar)
        .accessibilityLabel("Recorded stages")
    }

    @ViewBuilder
    private var stageDetail: some View {
        if let stage = store.stage, let invocation = store.invocation {
            ScrollView {
                VStack(alignment: .leading, spacing: Spacing.md) {
                    Label("\(stage.stepIndex + 1). \(stage.name)", systemImage: icon(stage))
                        .font(Typography.sectionTitle(18))
                    Text(stage.flowParents.joined(separator: " → "))
                        .foregroundStyle(palette.textSecondary)
                    if let node = stage.nodeId {
                        Text(node).font(Typography.code(11))
                    }
                    if stage.attempts.isEmpty {
                        Text("This stage has no recorded attempt.")
                    }
                    ForEach(Array(stage.attempts.enumerated()), id: \.offset) { index, attempt in
                        VStack(alignment: .leading, spacing: Spacing.sm) {
                            Text("Attempt \(index + 1) · iteration \(attempt.iteration) · \(attempt.state.rawValue)")
                                .font(Typography.body(12).weight(.semibold))
                            if let run = attempt.runId {
                                Button { store.inspectRun(run) } label: {
                                    runLabel(run, provider: store.snapshot?.runs.first { $0.runId == run }?.provider)
                                }
                                .buttonStyle(.link)
                            }
                            if let summary = attempt.readySummary { Text(summary) }
                            if let failure = attempt.failure {
                                Label(failure.reason, systemImage: "exclamationmark.triangle")
                                Text("Observed \(failure.observedAt)")
                                    .font(Typography.caption(10))
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(Spacing.md)
                        .background(palette.surfaceMuted)
                        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                    }
                    ForEach(Array(invocation.transitions.enumerated()), id: \.offset) { _, edge in
                        if edge.from.stepIndex == stage.stepIndex {
                            Button {
                                store.selectStage(edge.to)
                            } label: {
                                Label(
                                    "\(edge.reason.rawValue.capitalized): stage \(edge.from.stepIndex + 1), iteration \(edge.from.iteration) → stage \(edge.to.stepIndex + 1), iteration \(edge.to.iteration)",
                                    systemImage: edge.reason == .iterated ? "arrow.uturn.backward" : "arrow.right"
                                )
                            }
                            .buttonStyle(.link)
                        }
                    }
                }
                .font(Typography.body(12))
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(Spacing.lg)
            }
        } else {
            ContentUnavailableView("Select a recorded stage", systemImage: "list.bullet")
        }
    }

    private func isActive(_ stage: TaskWatchStage) -> Bool {
        store.snapshot?.activeStage?.invocationId == store.invocationId
            && store.snapshot?.activeStage?.stepIndex == stage.stepIndex
    }

    private func icon(_ stage: TaskWatchStage) -> String {
        if stage.human { return "person.crop.circle" }
        return stage.kind == .op ? "terminal" : "square.stack.3d.up"
    }

    private func runLabel(_ runId: String, provider: String?) -> some View {
        Text("\(provider ?? "Unknown provider") · \(runId)")
            .font(Typography.code(10))
            .textSelection(.enabled)
    }

    private func evidence(_ title: String, detail: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Label(title, systemImage: "exclamationmark.triangle")
                .font(Typography.body(12).weight(.semibold))
            Text(detail).font(Typography.caption(11)).textSelection(.enabled)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Spacing.md)
        .background(palette.surfaceMuted)
    }
}
#endif
