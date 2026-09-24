import Foundation
import Loopflow
import Observation

/// Presentation state only. Rust owns the retained plan and attempt projection.
@MainActor
@Observable
final class TaskWatchStore {
    private(set) var snapshot: TaskWatchSnapshot?
    private(set) var error: String?
    private var refreshId: UUID?
    var invocationId: String?
    var stepIndex: UInt32?
    var filtersStage = false
    var runId: String?
    var followsOutput = true
    private(set) var output: [TaskWatchOutput] = []
    private var historyGaps: [OutputGap] = []
    private var liveGaps: [OutputGap] = []
    var outputGaps: [OutputGap] {
        Array(Set(historyGaps + liveGaps)).sorted { $0.message < $1.message }
    }
    private(set) var outputError: String?
    private(set) var needsOutputReload = false
    private(set) var outputRevision = 0
    private(set) var latestOutputSource: TaskWatchOutput.ID?
    private var liveCursor: String?
    private var historyCursor: String?
    private var outputRequest: UUID?

    var isReadingOutput: Bool { outputRequest != nil }
    var visibleOutput: [TaskWatchOutput] {
        output.filter { source in
            (runId == nil || source.source.runId == runId)
                && (!filtersStage || (source.source.stage?.invocationId == invocationId
                    && source.source.stage?.stepIndex == stepIndex))
        }
    }

    func inspectStage(_ index: UInt32?) {
        stepIndex = index
        filtersStage = true
        runId = nil
        followsOutput = false
    }

    func inspectRun(_ id: String) {
        runId = id
        followsOutput = false
    }

    func followLive() {
        filtersStage = false
        runId = nil
        followsOutput = true
        selectInvocation(snapshot?.activeStage?.invocationId ?? snapshot?.invocations.last?.id)
    }

    var isRefreshing: Bool { refreshId != nil }

    var invocation: TaskWatchInvocation? {
        snapshot?.invocations.first { $0.id == invocationId }
    }

    var stage: TaskWatchStage? {
        invocation?.stages.first { $0.stepIndex == stepIndex }
    }

    func selectInvocation(_ id: String?) {
        invocationId = id
        let active = snapshot?.activeStage
        stepIndex = active?.invocationId == id ? active?.stepIndex : invocation?.stages.first?.stepIndex
    }

    func selectStage(_ target: TaskFlowStage) {
        invocationId = target.invocationId
        inspectStage(target.stepIndex)
    }

    /// Both continuations belong to this retained presentation, never to a provider.
    func readOutput(issue: String, query: RegistryQuery, history: Bool = false, reload: Bool = false) async {
        let request = UUID()
        outputRequest = request
        defer { if outputRequest == request { outputRequest = nil } }
        if reload {
            liveCursor = nil
            historyCursor = nil
            output = []
            latestOutputSource = nil
            historyGaps = []
            liveGaps = []
            needsOutputReload = false
        }
        guard !needsOutputReload else { return }
        do {
            // Seed first: output arriving during historical paging must remain readable.
            if liveCursor == nil || !history {
                let page = try await query.taskOutput(issue: issue, cursor: liveCursor, tail: liveCursor == nil, cwd: nil)
                try Task.checkCancellation()
                guard outputRequest == request else { return }
                guard accept(page, history: false) else { return }
                liveCursor = page.nextCursor
            }
            if historyCursor == nil || history {
                let page = try await query.taskOutput(issue: issue, cursor: historyCursor, cwd: nil)
                try Task.checkCancellation()
                guard outputRequest == request else { return }
                guard accept(page, history: true) else { return }
                historyCursor = page.nextCursor
            }
            outputError = nil
        } catch is CancellationError {
            // Retain the last accepted page and its matching continuation.
        } catch {
            if outputRequest == request, !Task.isCancelled { outputError = error.localizedDescription }
        }
    }

    private func accept(_ page: TaskOutputPage, history: Bool) -> Bool {
        if page.sources.contains(where: \.reset) {
            needsOutputReload = true
            outputError = "An output source changed. Reload output to read the new source; the previous output is retained below."
            return false
        }
        if history {
            historyGaps = page.gaps
        } else {
            // Failed seeding changes where this continuation starts. Preserve
            // that evidence until reload, even after subsequent quiet pages.
            liveGaps = page.gaps + liveGaps.filter { $0.code == "tail_unavailable" }
        }
        for source in page.sources {
            if let index = output.firstIndex(where: { $0.source.runId == source.runId && $0.source.source == source.source }) {
                if !history, source.records.contains(where: { !output[index].containsRevision($0) }) {
                    latestOutputSource = output[index].id
                }
                output[index].merge(source, history: history)
            } else {
                var group = TaskWatchOutput(source: source)
                group.merge(source, history: history)
                if !history, !source.records.isEmpty { latestOutputSource = group.id }
                output.append(group)
            }
        }
        outputRevision += 1
        return true
    }

    func refresh(issue: String, query: RegistryQuery) async {
        let request = UUID()
        refreshId = request
        defer {
            if refreshId == request { refreshId = nil }
        }
        do {
            // Watch is a registry read; a deleted worktree must not become cwd.
            let next = try await query.taskWatch(issue: issue, cwd: nil)
            try Task.checkCancellation()
            guard refreshId == request else { return }
            snapshot = next
            error = nil
            if followsOutput || invocation == nil {
                selectInvocation(next.activeStage?.invocationId ?? next.invocations.last?.id)
            } else if stage == nil {
                stepIndex = invocation?.stages.first?.stepIndex
            }
        } catch is CancellationError {
            // Hiding Watch leaves the last observed snapshot intact.
        } catch {
            if refreshId == request, !Task.isCancelled {
                self.error = error.localizedDescription
            }
        }
    }

}
