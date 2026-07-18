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

/// One turn, as a human reads it. `conclusion` is what the thread shows; `steps`
/// is operational narration the harness tagged as `commentary`, curated behind a
/// disclosure so the wave's decision reads without the process around it. `prose`
/// stays the full speech (conclusion + steps) so failure rollups and freshness
/// checks that ask "did this turn say anything" are unchanged.
public struct TurnPresentation: Equatable, Sendable {
    public let prose: String
    public let conclusion: String
    public let steps: [String]

    public init(prose: String, conclusion: String, steps: [String]) {
        self.prose = prose
        self.conclusion = conclusion
        self.steps = steps
    }

    /// A turn whose whole speech is its conclusion, with no curated steps.
    public init(prose: String) {
        self.init(prose: prose, conclusion: prose, steps: [])
    }

    public var hasProse: Bool {
        !prose.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    public var hasSteps: Bool { !steps.isEmpty }
}

/// Project one wire turn into the one human-facing conversation.
///
/// Prose (`turn.text`) is the conclusion — streamed fragments plus any
/// `final_answer` or untagged message, joined by the fold (`ChatTurn.absorbing`).
/// A `.message` item the provider tagged `commentary` is operational narration:
/// the fold keeps it out of `text` and in `items`, and here it becomes a curated
/// step. A turn that is *only* commentary keeps that text as its conclusion
/// rather than hiding all of itself. A pure token-streamed turn carries no
/// commentary tag, so it reads whole — the honest boundary until a harness tags
/// its streamed segments.
public func turnPresentation(_ turn: ChatTurn) -> TurnPresentation {
    var conclusion = turn.text
    var steps: [String] = []

    for item in turn.items {
        guard case let .message(_, text, phase) = item else { continue }
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { continue }
        if phase == "commentary" {
            steps.append(text)
        } else {
            conclusion = conclusion.isEmpty ? text : conclusion + "\n\n" + text
        }
    }

    if conclusion.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, !steps.isEmpty {
        conclusion = steps.joined(separator: "\n\n")
        steps = []
    }

    let prose = steps.isEmpty
        ? conclusion
        : ([conclusion] + steps).filter { !$0.isEmpty }.joined(separator: "\n\n")

    return TurnPresentation(prose: prose, conclusion: conclusion, steps: steps)
}

/// Child activity a human reads versus lifecycle churn the Wave summarizes.
public func isConversational(_ activity: ChildControlActivity) -> Bool {
    switch activity.kind {
    case .stateChanged: false
    case .prOpened, .completed, .failed: true
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
