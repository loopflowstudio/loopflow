import Foundation

// The wave thread is a conversation, not a build log. The wire carries
// everything the loop did — every tool call, every shell command, every file
// edit, and the flow step (`task_clarify`, `task_pursue`, `task_mutate`) that
// produced each assistant span. Rendering all of it, flat, buries the two
// things a human actually reads for: what the wave SAID, and what needs a
// decision.
//
// This is the projection that decides what reaches the eye. It is
// surface-only: nothing here changes the wire, the journal, or the runtime.
// The durable record remains available through the ledger; chat has one job:
// make the Wave understandable and steerable.
//
// The rule: **prose and decisions are the conversation.** Execution evidence
// stays in the journal. A failed turn or child remains visible through its
// human-level failure presentation, never by rebuilding the shell log here.

/// One turn, as a human reads it.
public struct TurnPresentation: Equatable, Sendable {
    /// The turn's speech: its streamed text plus any `.message` items (prose
    /// the harness emitted as an item rather than as turn text).
    public let prose: String
    public var hasProse: Bool {
        !prose.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

/// Project one wire turn into the one human-facing conversation.
public func turnPresentation(_ turn: ChatTurn) -> TurnPresentation {
    var prose = turn.text

    for item in turn.items {
        switch item {
        case let .message(_, text, _):
            // Harness prose that arrived as an item is speech, so it reads
            // with the turn's text rather than as a backend card.
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { continue }
            prose = prose.isEmpty ? text : prose + "\n\n" + text
        case .command, .file, .tool, .thought, .unknown:
            break
        }
    }

    return TurnPresentation(prose: prose)
}

/// Child activity a human reads (a decision, delivery, completion, or failure)
/// versus lifecycle churn the Wave should summarize in its own prose.
public func isConversational(_ activity: ChildControlActivity) -> Bool {
    switch activity.kind {
    case .stateChanged, .controlApplied, .directed, .incorporated:
        return false
    case .controlUncertain, .decisionRequired, .decisionResolved,
         .prOpened, .completed, .failed:
        return true
    }
}

/// The turns a human sees. Empty completed spans at flow-step boundaries carry
/// nothing to read and are dropped. A running span stays visible as the one
/// transient "Working…" row; a failed span gets a human-level failure badge.
public func isVisibleTurn(_ turn: ChatTurn) -> Bool {
    if let activity = turn.activity { return isConversational(activity) }
    if turn.role == .user { return true }
    if turn.status == .running || turn.status == .failed { return true }
    return turnPresentation(turn).hasProse
}
