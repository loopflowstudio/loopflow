import Foundation

public enum AttemptFailureState: Equatable, Sendable {
    case failed
    case retrying
    case recoveredOnRetry
}

public struct AttemptFailureDetail: Equatable, Sendable, Identifiable {
    public let turnId: String
    public let bodyId: String
    public let reason: String
    public let createdAt: String

    public var id: String { turnId }
}

public struct AttemptFailurePresentation: Equatable, Sendable {
    public let state: AttemptFailureState
    public let reason: String
    public let flow: String?
    public let step: String
    public let attempts: [AttemptFailureDetail]

    public var count: Int { attempts.count }

    public var title: String {
        let failure = count == 1 ? "Attempt failed" : "\(count) attempts failed"
        switch state {
        case .failed: return failure
        case .retrying: return "\(failure) · retrying"
        case .recoveredOnRetry: return "\(failure) · recovered on retry"
        }
    }
}

/// Only historical bodies carry a saved logical step that identifies retries.
private struct StepKey: Hashable {
    let invocationID: String
    let stepIndex: Int
    let iteration: Int

    init?(_ body: BodyProvenance) {
        guard let invocationId = body.invocationId,
              let index = body.stepIndex,
              let iteration = body.iteration else { return nil }
        invocationID = invocationId
        stepIndex = index
        self.iteration = iteration
    }
}

private struct EquivalentFailureKey: Hashable {
    let flow: String?
    let step: String
    let reason: String
}

public func attemptFailurePresentations(
    turns: [ChatTurn]
) -> [String: AttemptFailurePresentation] {
    var presentations: [String: AttemptFailurePresentation] = [:]
    var equivalentFailures: [EquivalentFailureKey: [String]] = [:]

    for (index, turn) in turns.enumerated() {
        guard turn.role == .assistant, turn.status == .failed, let body = turn.body else {
            continue
        }
        let key = StepKey(body)
        let laterAttempts = turns.dropFirst(index + 1).filter { later in
            guard later.role == .assistant, let laterBody = later.body else { return false }
            return key != nil && laterBody.bodyId != body.bodyId && StepKey(laterBody) == key
        }

        let state: AttemptFailureState
        if laterAttempts.contains(where: { $0.status == .completed }) {
            state = .recoveredOnRetry
        } else if laterAttempts.contains(where: { $0.status == .running }) {
            state = .retrying
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
            step: body.step,
            attempts: [AttemptFailureDetail(
                turnId: turn.id,
                bodyId: body.bodyId,
                reason: reason,
                createdAt: turn.createdAt
            )]
        )
        if !turnPresentation(turn).hasProse {
            equivalentFailures[
                EquivalentFailureKey(flow: body.flow, step: body.step, reason: reason),
                default: []
            ].append(turn.id)
        }
    }

    for ids in equivalentFailures.values where ids.count > 1 {
        let attempts = ids.compactMap { presentations[$0]?.attempts.first }
        guard let latestId = ids.last,
              let latest = presentations[latestId] else { continue }
        for id in ids.dropLast() {
            presentations.removeValue(forKey: id)
        }
        presentations[latestId] = AttemptFailurePresentation(
            state: latest.state,
            reason: latest.reason,
            flow: latest.flow,
            step: latest.step,
            attempts: attempts
        )
    }

    return presentations
}

/// Visible conversation after equivalent operational-only failures roll up.
/// Authored failure prose never collapses; its words remain in chronology.
public func visibleConversationTurns(
    _ turns: [ChatTurn],
    failures: [String: AttemptFailurePresentation]
) -> [ChatTurn] {
    turns.filter { turn in
        if turn.role == .assistant,
           turn.status == .failed,
           turn.body != nil,
           !turnPresentation(turn).hasProse {
            return failures[turn.id] != nil
        }
        return isVisibleTurn(turn)
    }
}
