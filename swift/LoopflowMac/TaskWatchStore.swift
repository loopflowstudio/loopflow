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
        stepIndex = target.stepIndex
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
            if invocation == nil {
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
