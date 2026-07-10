import Foundation

public enum AttemptFailureState: Equatable, Sendable {
    case failed
    case retryPending
    case retrying
    case recoveredOnRetry
}

public struct AttemptFailurePresentation: Equatable, Sendable {
    public let state: AttemptFailureState
    public let reason: String
    public let flow: String
    public let step: String

    public var title: String {
        switch state {
        case .failed: return "Attempt failed"
        case .retryPending: return "Attempt failed · retry pending"
        case .retrying: return "Attempt failed · retrying"
        case .recoveredOnRetry: return "Attempt failed · recovered on retry"
        }
    }
}

private struct StepKey: Hashable {
    let invocationID: String
    let stepIndex: Int
    let iteration: Int

    init(_ body: BodyProvenance) {
        invocationID = body.invocationId
        stepIndex = body.stepIndex
        iteration = body.iteration
    }

    init(_ step: PlayheadStepRef) {
        invocationID = step.invocationId
        stepIndex = step.index
        iteration = step.iteration
    }
}

public func attemptFailurePresentations(
    turns: [ChatTurn],
    playhead: PlayheadView?,
    loopState: WaveLoopState
) -> [String: AttemptFailurePresentation] {
    var presentations: [String: AttemptFailurePresentation] = [:]

    for (index, turn) in turns.enumerated() {
        guard turn.role == .assistant, turn.status == .failed, let body = turn.body else {
            continue
        }
        let key = StepKey(body)
        let laterAttempts = turns.dropFirst(index + 1).filter { later in
            guard later.role == .assistant, let laterBody = later.body else { return false }
            return laterBody.bodyId != body.bodyId && StepKey(laterBody) == key
        }

        let state: AttemptFailureState
        if laterAttempts.contains(where: { $0.status == .completed }) {
            state = .recoveredOnRetry
        } else if laterAttempts.contains(where: { $0.status == .running })
            || playhead?.active.map({ $0.bodyId != body.bodyId && StepKey($0) == key }) == true {
            state = .retrying
        } else if playhead?.active == nil,
                  playhead?.now.map(StepKey.init) == key,
                  loopState != .failed {
            state = .retryPending
        } else {
            state = .failed
        }

        let reason = body.terminationReason.flatMap { recorded in
            recorded.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : recorded
        } ?? "No failure reason was recorded."
        presentations[turn.id] = AttemptFailurePresentation(
            state: state,
            reason: reason,
            flow: body.flow,
            step: body.step
        )
    }

    return presentations
}
