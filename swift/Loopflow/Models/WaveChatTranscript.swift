import Foundation

// The wave thread is a conversation, not a build log. The wire carries
// everything the loop did — every tool call, every shell command, every file
// edit, and the flow step (`task_clarify`, `task_pursue`, `task_mutate`) that
// produced each assistant span. Rendering all of it, flat, buries the two
// things a human actually reads for: what the wave SAID, and what needs a
// decision.
//
// This is the projection that decides what reaches the eye. It is
// surface-only: nothing here changes the wire, the journal, or the runtime,
// and the audit mode returns the full record unchanged — the same shape as
// `AttemptFailurePresentation`.
//
// The rule: **prose, failures, and decisions are the conversation; everything
// else is evidence.** Evidence collapses to a count and stays one disclosure
// away.

/// Evidence a turn produced, coalesced to counts. One row, whatever the turn
/// did — items growing changes this row's label, never the number of rows.
public struct TurnActivity: Equatable, Sendable {
    public let commands: Int
    public let files: Int
    public let tools: Int
    public let isRunning: Bool

    public var total: Int { commands + files + tools }

    /// What the row says. A running turn that has produced no evidence yet
    /// still reads as motion ("Working…"), so a tool-only turn is never a
    /// blank space where the wave went quiet.
    public var label: String {
        var parts: [String] = []
        if commands > 0 { parts.append(count(commands, "command")) }
        if files > 0 { parts.append("\(count(files, "file")) edited") }
        if tools > 0 { parts.append(count(tools, "tool call")) }

        if parts.isEmpty {
            return isRunning ? "Working…" : "No activity"
        }
        let summary = parts.joined(separator: " · ")
        return isRunning ? "Working — \(summary)" : summary
    }

    private func count(_ n: Int, _ noun: String) -> String {
        "\(n) \(noun)\(n == 1 ? "" : "s")"
    }
}

/// One turn, as a human reads it.
public struct TurnPresentation: Equatable, Sendable {
    /// The turn's speech: its streamed text, plus — in the conversation only —
    /// any `.message` items (prose the harness emitted as an item rather than
    /// as turn text). In audit those items stay in `auditItems` as their own
    /// cards and are not folded here, so the words render exactly once.
    public let prose: String
    /// Coalesced evidence, or nil when the turn produced none.
    public let activity: TurnActivity?
    /// Items that failed. Never collapsed, in either mode — an actionable
    /// error is conversation, not evidence.
    public let failures: [ConversationItem]
    /// Every item, in wire order: the body of the audit disclosure.
    public let auditItems: [ConversationItem]
    /// The `flow / step` boundary above an assistant span. Backend phase
    /// structure — audit only.
    public let showsBoundary: Bool

    public var hasProse: Bool {
        !prose.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

/// Project one turn for the given mode. `audit == true` is today's view: every
/// item as its own card, the flow-step boundary restored.
public func turnPresentation(_ turn: ChatTurn, audit: Bool) -> TurnPresentation {
    var prose = turn.text
    var commands = 0
    var files = 0
    var tools = 0
    var failures: [ConversationItem] = []

    for item in turn.items {
        switch item {
        case let .message(_, text, _):
            // Harness prose that arrived as an item. In the conversation it is
            // speech, so it reads with the turn's text rather than as a card.
            // In audit it stays in its own card and must NOT also be folded
            // here — audit restores the prior execution-log shape exactly, and
            // folding would print the same words twice.
            guard !audit else { continue }
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else { continue }
            prose = prose.isEmpty ? text : prose + "\n\n" + text
        case .command:
            commands += 1
        case .file:
            files += 1
        case .tool:
            tools += 1
        case .thought, .unknown:
            break
        }
        if isActionableFailure(item) {
            failures.append(item)
        }
    }

    let auditItems = audit ? turn.items.filter(\.isVisibleInConversation) : []
    let activity: TurnActivity?
    if audit {
        // The cards carry their own status; a count row on top would just
        // restate them.
        activity = nil
    } else if commands + files + tools > 0 || turn.status == .running {
        activity = TurnActivity(
            commands: commands,
            files: files,
            tools: tools,
            isRunning: turn.status == .running
        )
    } else {
        activity = nil
    }

    return TurnPresentation(
        prose: prose,
        activity: activity,
        // In audit mode the failing item is already on screen as its own card.
        failures: audit ? [] : failures,
        auditItems: auditItems,
        showsBoundary: audit
    )
}

/// An item a human has to act on: the loop hit something that didn't work. A
/// nonzero exit is a failure even when the harness marked the command
/// `completed` — the command ran, and it said no.
public func isActionableFailure(_ item: ConversationItem) -> Bool {
    switch item {
    case let .command(_, _, _, status, _, exitCode, _):
        if let exitCode, exitCode != 0 { return true }
        return status == .failed
    case let .file(_, _, status):
        return status == .failed
    case let .tool(_, _, status, _, _):
        return status == .failed
    case .message, .thought, .unknown:
        return false
    }
}

/// Child activity a human reads (a decision to make, a PR to look at, a
/// project that finished or failed) versus lifecycle churn the wave emits as
/// it drives its children. Churn is audit-only.
public func isConversational(_ activity: ChildControlActivity) -> Bool {
    switch activity.kind {
    case .stateChanged, .controlApplied:
        return false
    case .controlUncertain, .directed, .incorporated, .decisionRequired,
         .decisionResolved, .pullRequestOpened, .completed, .failed:
        return true
    }
}

/// The turns a human sees. Assistant turns with neither speech nor evidence —
/// the empty spans a flow produces at a step boundary — carry nothing to read
/// and are dropped, unless a failure or audit mode makes them worth a row.
public func isVisibleTurn(_ turn: ChatTurn, audit: Bool) -> Bool {
    if audit { return true }
    if let activity = turn.activity { return isConversational(activity) }
    if turn.role == .user { return true }
    if turn.status == .failed { return true }
    let presentation = turnPresentation(turn, audit: false)
    return presentation.hasProse
        || presentation.activity != nil
        || !presentation.failures.isEmpty
}
