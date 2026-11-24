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
    /// `owner` is the repository reading's `viewer`: who the Machine names. Working
    /// if it was started; with no viewer, the section moves only.
    case claim(task: String, owner: TaskOwner?)
    case pause(task: String)
    case unpause(task: String, owned: Bool)
    /// Backlog.
    case unassign(task: String)
    /// Leaves every list filtered to another Wave; nil leaves it in none.
    case move(task: String, wave: String?)
    case renameSession(session: String, title: String)

    public var task: String? {
        switch self {
        case .complete(let task), .abandon(let task), .reopen(let task), .claim(let task, _), .unassign(let task),
             .move(let task, _), .pause(let task), .unpause(let task, _): task
        case .renameSession: nil
        }
    }
}

/// One Machine's changes shown before its answer, keyed by request ID, and the
/// link to that Machine. A change shows on every reading until a reading at or
/// past the revisions its answer advanced; it is dropped once every live
/// reading of the Machine is past it. Never by comparing values, never on a timer.
@MainActor
@Observable
public final class PendingChanges {
    /// Sends a write under its request ID; answers the revisions it advanced.
    public typealias Send = @MainActor (ServedWrite, String) async throws -> Revisions?

    public struct Change: Equatable, Sendable {
        public let requestID: String
        public let command: PendingCommand
        /// What the Machine's answer advanced; nil until it answers.
        public fileprivate(set) var answer: Revisions?
    }

    /// The link to the Machine: a blip keeps outstanding changes in memory and
    /// sends them again; away disables change controls until the link returns.
    public enum Link: Equatable, Sendable { case connected, blip, away }

    public static let awayAfter = Duration.seconds(10)
    public static let awayNotice = "machine-away"
    public static let awayText = "The Machine is away; showing what was last read"

    public private(set) var changes: [Change] = []
    public private(set) var link = Link.connected
    /// Whether change controls are enabled: not while the Machine is away.
    public var acceptsChanges: Bool { link != .away }

    @ObservationIgnored private let send: Send
    @ObservationIgnored private let describe: ((any Error) -> String)?
    /// Each live subscription's last `read_at`; nil when it had no store.
    @ObservationIgnored private var readings: [String: Revisions?] = [:]
    @ObservationIgnored private var centers: [WeakNoticeCenter] = []
    @ObservationIgnored private var away: Task<Void, Never>?

    /// `describe` words a failure that is not the Machine's; by default its description.
    public init(send: @escaping Send, describe: ((any Error) -> String)? = nil) {
        self.send = send
        self.describe = describe
    }

    // MARK: Making changes

    /// The Machine is settling this request. A second click waits for its answer.
    public func pauseRequest(for issue: String) -> String? {
        for change in changes where change.answer == nil && change.command.task == issue {
            switch change.command {
            case .pause: return "Pause requested"
            case .unpause: return "Unpause requested"
            default: continue
            }
        }
        return nil
    }

    public func setPause(_ action: TaskActions.Pause, row: RoadmapTask, directory: String, notices: NoticeCenter,
                         show: Notice.Action?, answered: @escaping @MainActor () async -> Void) {
        let issue = row.task.identifier
        let change: (command: PendingCommand, write: ServedWrite, verb: String) = switch action {
        case .pause:
            (.pause(task: issue), .taskPause(directory: directory, issue: issue), "paused")
        case .unpause:
            (.unpause(task: issue, owned: row.runtime?.owner != nil), .taskUnpause(directory: directory, issue: issue, run: false), "unpaused")
        }
        make(change.command, subject: issue, verb: change.verb, write: change.write,
             notices: notices, show: show, answered: answered)
    }

    /// Show `command` on every list now and send `write` under its request ID,
    /// again while the link is lost. The change stays until a reading covers what
    /// the answer advanced. A refusal says "LOO-4 couldn't be completed: … · Show",
    /// `show` going to the Task, and removes the change, or keeps it until a
    /// reading covers what moved while the refused write ran.
    /// `answered` runs after an answer, to ask for the readings that show it.
    @discardableResult
    public func make(_ command: PendingCommand, subject: String, verb: String, write: ServedWrite, notices: NoticeCenter,
                     show: Notice.Action?, answered: @escaping @MainActor () async -> Void = {}) -> String {
        let requestID = UUID().uuidString
        changes.append(Change(requestID: requestID, command: command, answer: nil))
        Task {
            switch await deliver(write, requestID: requestID) {
            case .answered(let revisions):
                self.answered(requestID, revisions: revisions)
                await answered()
            case .refused(let reason, let revisions):
                notices.post(Notice("\(subject) couldn't be \(verb): \(reason)", key: requestID, action: show))
                // Domains moved while it failed, by it or by another write: the
                // reading that covers them shows which.
                guard let revisions else {
                    changes.removeAll { $0.requestID == requestID }
                    return
                }
                self.answered(requestID, revisions: revisions)
                await answered()
            }
        }
        return requestID
    }

    /// Start, Run and the two Auto-Ship requests change nothing on screen: the
    /// Notice says "LOO-5 Auto-Ship requested" in the click and the Machine's readings
    /// show the rest. A refusal posts over it.
    public func request(_ name: String, subject: String, write: ServedWrite, notices: NoticeCenter,
                        undo: Notice.Action? = nil, answered: @escaping @MainActor () async -> Void = {},
                        refused: @escaping @MainActor () -> Void = {}) {
        let requestID = UUID().uuidString
        notices.post(Notice("\(subject) \(name) requested", key: requestID, action: undo))
        Task {
            switch await deliver(write, requestID: requestID) {
            case .answered:
                await answered()
            case .refused(let reason, _):
                notices.post(Notice("\(subject) \(name) couldn't be requested: \(reason)", key: requestID))
                refused()
            }
        }
    }

    private enum Delivery {
        case answered(Revisions)
        /// `revisions` are the domains that advanced while the write ran; the
        /// Machine cannot say whether this write moved them.
        case refused(String, revisions: Revisions?)
    }

    /// Send `write` until the Machine answers it. A lost link sends it again under
    /// the same request ID, so the Machine runs it once.
    private func deliver(_ write: ServedWrite, requestID: String) async -> Delivery {
        var delay = Duration.milliseconds(250)
        while true {
            do {
                let revisions = try await send(write, requestID)
                linkReturned()
                return .answered(revisions ?? Revisions([:]))
            } catch let error as ServerError {
                linkReturned()
                return .refused(error.message, revisions: error.revisions)
            } catch where ServerLink.lostLink(error) && !Task.isCancelled {
                linkLost()
                try? await Task.sleep(for: delay)
                delay = min(delay * 2, .seconds(4))
            } catch {
                return .refused(describe?(error) ?? error.localizedDescription, revisions: nil)
            }
        }
    }

    /// The Machine answered: keep the change until a reading covers `revisions`.
    /// An answer that advanced nothing (or a transport that cannot say) has no
    /// reading to wait for: the rows show the readings as they are.
    private func answered(_ requestID: String, revisions: Revisions) {
        guard let index = changes.firstIndex(where: { $0.requestID == requestID }) else { return }
        guard !revisions.domains.isEmpty else {
            changes.remove(at: index)
            return
        }
        changes[index].answer = revisions
        prune()
    }

    // MARK: The link

    /// `notices` shows this Machine's away condition.
    public func report(to notices: NoticeCenter) {
        centers.removeAll { $0.center == nil || $0.center === notices }
        centers.append(WeakNoticeCenter(center: notices))
        if link == .away { notices.post(Self.awayCondition) }
    }

    /// A write or subscription lost the Machine: a blip, away after ten seconds.
    public func linkLost() {
        guard link == .connected else { return }
        link = .blip
        away = Task { [weak self] in
            try? await Task.sleep(for: Self.awayAfter)
            guard let self, !Task.isCancelled, self.link == .blip else { return }
            self.link = .away
            for center in self.centers.compactMap(\.center) { center.post(Self.awayCondition) }
        }
    }

    /// The Machine answered again.
    public func linkReturned() {
        away?.cancel()
        away = nil
        guard link != .connected else { return }
        if link == .away {
            for center in centers.compactMap(\.center) { center.clear(key: Self.awayNotice) }
        }
        link = .connected
    }

    private static var awayCondition: Notice {
        Notice(awayText, key: awayNotice, kind: .condition)
    }

    // MARK: Readings

    /// `subscription` delivered `reading`.
    public func read<Value>(_ subscription: String, _ reading: Reading<Value>) {
        readings[subscription] = .some(reading.readAt)
        prune()
    }

    /// `subscription` ended; its last reading no longer holds changes back.
    public func ended(_ subscription: String) {
        readings.removeValue(forKey: subscription)
        prune()
    }

    /// The changes a reading at `readAt` does not show yet.
    private func patches(at readAt: Revisions?) -> [Change] {
        changes.filter { change in
            guard let answer = change.answer, let readAt else { return true }
            return !readAt.covers(answer)
        }
    }

    /// `reading` as it looks with this Machine's changes made. `wave` is the
    /// Wave the list is filtered to; nil lists every Wave. `identifier` names a
    /// row's Task as commands do, for rows the reading left unnamed.
    public func apply(to reading: Reading<WorkViewPlan>, wave: String? = nil,
                      identifier: (WorkViewPlan.Content) -> String? = \.identifier) -> WorkViewPlan? {
        guard var plan = reading.value else { return nil }
        let gone = patch(&plan.contents, readAt: reading.readAt, wave: wave, identifier: identifier)
        if !gone.isEmpty {
            plan.projection.visible.removeAll { gone.contains($0.work) }
            plan.resolution.members.removeAll { gone.contains($0.work) }
        }
        return plan
    }

    /// One reading's Task rows with this Machine's changes made.
    public func apply(to contents: [WorkViewPlan.Content], readAt: Revisions?, wave: String? = nil) -> [WorkViewPlan.Content] {
        var contents = contents
        _ = patch(&contents, readAt: readAt, wave: wave, identifier: \.identifier)
        return contents
    }

    /// Patches `contents` in place; answers the rows it removed.
    private func patch(_ contents: inout [WorkViewPlan.Content], readAt: Revisions?, wave: String?,
                       identifier: (WorkViewPlan.Content) -> String?) -> Set<WorkWorkspaceKey> {
        var gone = Set<WorkWorkspaceKey>()
        for change in patches(at: readAt) {
            guard let task = change.command.task else { continue }
            let matches = Set(contents.filter { $0.work.kind == .task && ($0.work.id == task || identifier($0) == task) }.map(\.work))
            guard !matches.isEmpty else { continue }
            switch change.command {
            case .complete, .abandon:
                gone.formUnion(matches)
            case .move(_, let target):
                if let wave, wave != target { gone.formUnion(matches) }
            case .reopen:
                contents.updateStates(of: matches) { states in
                    states.inSection(Self.started(states) ? .working : .backlog).union([.open])
                }
            case .pause:
                contents.updateStates(of: matches) {
                    $0.inSection(.paused).subtracting([.moving, .active, .idle, .busy, .available, .blocked])
                }
            case .unpause(_, let owned):
                contents.updateStates(of: matches) { $0.inSection(owned ? .working : .backlog).union([.idle]) }
            case .claim:
                contents.updateStates(of: matches) { states in
                    Self.started(states) ? states.inSection(.working) : states
                }
            case .unassign:
                contents.updateStates(of: matches) { states in
                    let backlog = states.inSection(.backlog)
                    return states.contains(.paused) ? backlog.union([.idle]) : backlog
                }
            case .renameSession:
                break
            }
            contents.removeAll { gone.contains($0.work) }
        }
        return gone
    }

    /// Planning rows read at `readAt` with this Machine's changes made, for every
    /// command in the table. `wave` is the Wave the rows are filtered to; a Task
    /// moved into it comes from `inventory`.
    public func apply(to tasks: [RoadmapTask], readAt: Revisions?, wave: String? = nil,
                      inventory: [RoadmapTask] = []) -> [RoadmapTask] {
        let shown = patches(at: readAt)
        guard !shown.isEmpty else { return tasks }
        var tasks = tasks
        for change in shown {
            guard let issue = change.command.task else { continue }
            if case .move(_, let target) = change.command, let wave, wave == target,
               !tasks.contains(where: { $0.task.identifier == issue }),
               let moved = inventory.first(where: { $0.task.identifier == issue }) {
                tasks.append(moved)
            }
            guard let index = tasks.firstIndex(where: { $0.task.identifier == issue }) else { continue }
            switch change.command {
            case .complete:
                tasks[index].lifecycle = .done
                tasks[index].section = .done
            case .abandon:
                tasks.remove(at: index)
            case .reopen:
                // A finished Task was started when it has a checkout.
                let checkedOut = tasks[index].reference.workspace != nil
                tasks[index].lifecycle = checkedOut ? .idle : .unstarted
                tasks[index].section = checkedOut && tasks[index].runtime?.owner != nil ? .working : .backlog
            case .pause:
                tasks[index].lifecycle = .paused
                tasks[index].section = .paused
            case .unpause(_, let owned):
                tasks[index].lifecycle = .idle
                tasks[index].section = owned ? .working : .backlog
            case .claim(_, let owner):
                if let owner { tasks[index].runtime?.owner = owner }
                if tasks[index].lifecycle.isWorking { tasks[index].section = .working }
            case .unassign:
                if tasks[index].lifecycle == .paused { tasks[index].lifecycle = .idle }
                tasks[index].runtime?.owner = nil
                tasks[index].section = .backlog
            case .move(_, let target):
                tasks[index].task.waveId = target
                if let wave, wave != target { tasks.remove(at: index) }
            case .renameSession:
                break
            }
        }
        return tasks
    }

    /// `sessions` with pending renames made.
    public func apply(to sessions: [SessionRecord], readAt: Revisions?) -> [SessionRecord] {
        let renames = patches(at: readAt).compactMap { change -> (String, String)? in
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
        !states.isDisjoint(with: [.idle, .active, .moving])
    }

    private func prune() {
        guard !readings.isEmpty else { return }
        changes.removeAll { change in
            guard let answer = change.answer else { return false }
            return readings.values.allSatisfy { readAt in readAt?.covers(answer) ?? false }
        }
    }
}

private struct WeakNoticeCenter {
    weak var center: NoticeCenter?
}

private extension Set<WorkSelectionState> {
    func inSection(_ section: WorkSelectionState) -> Self {
        subtracting([.backlog, .working, .paused, .done]).union([section])
    }
}

private extension [WorkViewPlan.Content] {
    mutating func updateStates(of members: Set<WorkWorkspaceKey>,
                               _ update: (Set<WorkSelectionState>) -> Set<WorkSelectionState>) {
        for index in indices where members.contains(self[index].work) {
            let next = update(Set(self[index].states))
            self[index].states = WorkSelectionState.allCases.filter(next.contains)
        }
    }
}
