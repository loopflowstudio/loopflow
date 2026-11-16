import Foundation
import Observation

/// A Task or Session change someone made that the Machine has not shown yet.
/// The client never decides list membership beyond this table; the next
/// reading that covers the change replaces whatever it shows.
public enum PendingCommand: Equatable, Sendable {
    /// Gone from every list; the next row is selected.
    case complete(task: String)
    case abandon(task: String)
    /// Working if it was started, else Backlog.
    case reopen(task: String)
    /// Working if it was started.
    case claim(task: String)
    /// Backlog.
    case unassign(task: String)
    /// Leaves every list filtered to another Wave.
    case move(task: String, wave: String)
    case renameSession(session: String, title: String)

    public var task: String? {
        switch self {
        case .complete(let task), .abandon(let task), .reopen(let task), .claim(let task), .unassign(let task),
             .move(let task, _): task
        case .renameSession: nil
        }
    }
}

/// One Machine's changes shown before its answer, keyed by request ID. A change
/// shows on every reading until a reading at or past the revisions its answer
/// advanced; it is dropped once every live reading of the Machine is past it.
/// Never by comparing values, never on a timer.
@MainActor
@Observable
public final class PendingChanges {
    public struct Change: Equatable, Sendable {
        public let requestID: String
        public let command: PendingCommand
        /// What the Machine's answer advanced; nil until it answers.
        public fileprivate(set) var answer: Revisions?
    }

    public private(set) var changes: [Change] = []
    /// Each live subscription's last `read_at`; nil when it had no store.
    @ObservationIgnored private var readings: [String: Revisions?] = [:]

    public init() {}

    /// Show `command` now. The returned request ID names it on the wire.
    @discardableResult
    public func add(_ command: PendingCommand, requestID: String = UUID().uuidString) -> String {
        changes.append(Change(requestID: requestID, command: command, answer: nil))
        return requestID
    }

    /// The Machine answered: keep the change until a reading covers `revisions`.
    /// A partly done change (an error that advanced revisions) is answered too.
    /// An answer that advanced nothing (or a transport that cannot say) has no
    /// reading to wait for: the rows show the readings as they are.
    public func answered(_ requestID: String, revisions: Revisions) {
        guard let index = changes.firstIndex(where: { $0.requestID == requestID }) else { return }
        guard !revisions.domains.isEmpty else {
            changes.remove(at: index)
            return
        }
        changes[index].answer = revisions
        prune()
    }

    /// The Machine refused: nothing changed, so the rows come back now.
    public func refused(_ requestID: String) {
        changes.removeAll { $0.requestID == requestID }
    }

    /// `subscription` delivered a reading at `readAt`.
    public func read(_ subscription: String, at readAt: Revisions?) {
        readings[subscription] = .some(readAt)
        prune()
    }

    /// `subscription` ended; its last reading no longer holds changes back.
    public func ended(_ subscription: String) {
        readings.removeValue(forKey: subscription)
        prune()
    }

    /// Changes still unanswered, to send again under their request IDs.
    public var unanswered: [Change] { changes.filter { $0.answer == nil } }

    /// The changes a reading at `readAt` does not show yet.
    public func shown(at readAt: Revisions?) -> [Change] {
        changes.filter { change in
            guard let answer = change.answer, let readAt else { return true }
            return !readAt.covers(answer)
        }
    }

    /// Whether `task` is gone from lists read at `readAt`.
    public func removes(task: String, at readAt: Revisions?) -> Bool {
        shown(at: readAt).contains { change in
            switch change.command {
            case .complete(let id), .abandon(let id): id == task
            default: false
            }
        }
    }

    /// `plan` as it looks with this Machine's changes made. `wave` is the
    /// Wave the list is filtered to; nil lists every Wave. `identifier` names a
    /// row's Task as commands do, for rows the reading left unnamed.
    public func apply(to plan: WorkViewPlan, readAt: Revisions?, wave: String? = nil,
                      identifier: (WorkViewPlan.Content) -> String? = \.identifier) -> WorkViewPlan {
        var plan = plan
        for change in shown(at: readAt) {
            guard let task = change.command.task else { continue }
            let matches = plan.contents.filter { $0.work.kind == .task && ($0.work.id == task || identifier($0) == task) }.map(\.work)
            guard !matches.isEmpty else { continue }
            switch change.command {
            case .complete, .abandon:
                plan.remove(matches)
            case .move(_, let target):
                if let wave, wave != target { plan.remove(matches) }
            case .reopen:
                plan.updateStates(of: matches) { states in
                    states.subtracting([.done]).union([.open, Self.started(states) ? .working : .backlog])
                }
            case .claim:
                plan.updateStates(of: matches) { states in Self.started(states) ? states.union([.working]) : states }
            case .unassign:
                plan.updateStates(of: matches) { $0.union([.backlog]) }
            case .renameSession:
                break
            }
        }
        return plan
    }

    /// `sessions` with pending renames made.
    public func apply(to sessions: [SessionRecord], readAt: Revisions?) -> [SessionRecord] {
        let renames = shown(at: readAt).compactMap { change -> (String, String)? in
            guard case .renameSession(let session, let title) = change.command else { return nil }
            return (session, title)
        }
        guard !renames.isEmpty else { return sessions }
        let titles = Dictionary(renames, uniquingKeysWith: { _, newer in newer })
        return sessions.map { session in
            guard let title = titles[session.id] else { return session }
            var renamed = session
            renamed.title = title
            return renamed
        }
    }

    private static func started(_ states: Set<WorkSelectionState>) -> Bool {
        !states.isDisjoint(with: [.idle, .active, .moving, .shipping])
    }

    private func prune() {
        guard !readings.isEmpty else { return }
        changes.removeAll { change in
            guard let answer = change.answer else { return false }
            return readings.values.allSatisfy { readAt in readAt?.covers(answer) ?? false }
        }
    }
}

extension WorkViewPlan {
    fileprivate mutating func remove(_ members: [WorkWorkspaceKey]) {
        let gone = Set(members)
        projection.visible.removeAll { gone.contains($0.work) }
        resolution.members.removeAll { gone.contains($0.work) }
        contents.removeAll { gone.contains($0.work) }
    }

    /// The section a change moves a Task to replaces its old one; a covering
    /// reading's own states replace these.
    fileprivate mutating func updateStates(of members: [WorkWorkspaceKey],
                                           _ update: (Set<WorkSelectionState>) -> Set<WorkSelectionState>) {
        let sections: Set<WorkSelectionState> = [.backlog, .working, .autonomous, .done]
        for index in contents.indices where members.contains(contents[index].work) {
            let current = Set(contents[index].states)
            let next = update(current)
            let section = next.subtracting(current).intersection(sections)
            let kept = section.isEmpty ? next : next.subtracting(sections.subtracting(section))
            contents[index].states = WorkSelectionState.allOrdered.filter(kept.contains)
        }
    }
}

extension WorkSelectionState {
    /// Declaration order, so patched states keep a stable order.
    fileprivate static let allOrdered: [WorkSelectionState] = [
        .open, .available, .busy, .blocked, .unstarted, .idle, .active, .moving, .shipping,
        .backlog, .working, .autonomous, .done,
    ]
}
