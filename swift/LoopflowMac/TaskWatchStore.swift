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
    private(set) var outputWindowTrimmed = false
    private var outputObservation = 0
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

    var outputGroups: [TaskWatchOutputGroup] {
        let rows = visibleOutput.flatMap { output in output.rows.map { (output.source, $0) } }
            .sorted { $0.1.position < $1.1.position }
        var groups: [TaskWatchOutputGroup] = []
        for (source, row) in rows {
            if let last = groups.last, last.source.runId == source.runId,
               last.source.source == source.source, last.history == row.position.history {
                groups[groups.count - 1].rows.append(row)
            } else {
                groups.append(TaskWatchOutputGroup(source: source, rows: [row]))
            }
        }
        return groups
    }

    var latestOutputRow: TaskWatchOutputRowReference? {
        output.flatMap { output in
            output.rows.compactMap { row in
                row.lastLiveObservation.map { ($0, TaskWatchOutputRowReference(source: output.id, row: row.id)) }
            }
        }.max { $0.0 < $1.0 }?.1
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
    func readOutput(issue: String, query: RegistryQuery, history: Bool = false, reload: Bool = false,
                    restartHistory: Bool = false) async {
        let request = UUID()
        outputRequest = request
        defer { if outputRequest == request { outputRequest = nil } }
        if reload {
            liveCursor = nil
            historyCursor = nil
            output = []
            outputObservation = 0
            outputWindowTrimmed = false
            historyGaps = []
            liveGaps = []
            needsOutputReload = false
        }
        guard !needsOutputReload else { return }
        do {
            // Seed first: output arriving during historical paging must remain readable.
            if liveCursor == nil || (!history && !restartHistory) {
                let page = try await query.taskOutput(issue: issue, cursor: liveCursor, tail: liveCursor == nil, cwd: nil)
                try Task.checkCancellation()
                guard outputRequest == request else { return }
                guard accept(page, history: false) else { return }
                liveCursor = page.nextCursor
            }
            if historyCursor == nil || history || restartHistory {
                let page = try await query.taskOutput(issue: issue, cursor: restartHistory ? nil : historyCursor, cwd: nil)
                try Task.checkCancellation()
                guard outputRequest == request else { return }
                guard accept(page, history: true, restartHistory: restartHistory) else { return }
                historyCursor = page.nextCursor
            }
            outputError = nil
        } catch is CancellationError {
            // Retain the last accepted page and its matching continuation.
        } catch {
            if outputRequest == request, !Task.isCancelled { outputError = error.localizedDescription }
        }
    }

    private func accept(_ page: TaskOutputPage, history: Bool, restartHistory: Bool = false) -> Bool {
        if page.sources.contains(where: \.reset) {
            needsOutputReload = true
            outputError = "An output source changed. Reload output to read the new source; the previous output is retained below."
            return false
        }
        if restartHistory {
            for index in output.indices { output[index].restartHistory() }
            outputObservation = 0
            outputWindowTrimmed = false
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
                output[index].merge(source, history: history, observation: &outputObservation)
            } else {
                var group = TaskWatchOutput(source: source)
                group.merge(source, history: history, observation: &outputObservation)
                output.append(group)
            }
        }
        trimOutputWindow()
        outputRevision += 1
        return true
    }

    private func trimOutputWindow() {
        let maximumRecords = 4_096
        let maximumBytes = 16 * 1_024 * 1_024
        let records = output.indices.flatMap { index in
            output[index].retainedRecords.map { (source: index, id: $0.id, observation: $0.observation, bytes: $0.bytes) }
        }.sorted { $0.observation < $1.observation }
        var removals: [Int: Set<String>] = [:]
        // An oversized new record must not evict every smaller readable record.
        let fitting = records.filter { record in
            if record.bytes > maximumBytes {
                removals[record.source, default: []].insert(record.id)
                return false
            }
            return true
        }
        var count = fitting.count
        var bytes = fitting.reduce(0) { $0 + $1.bytes }
        for record in fitting {
            guard count > maximumRecords || bytes > maximumBytes else { break }
            removals[record.source, default: []].insert(record.id)
            count -= 1
            bytes -= record.bytes
        }
        for (index, ids) in removals { output[index].removeRecords(ids) }
        if !removals.isEmpty { outputWindowTrimmed = true }
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
