import Foundation
import Observation

/// Process-wide presentation for one Machine. Query leases belong to windows;
/// commands keep their original destination independently of those leases.
@MainActor @Observable
public final class WorkInteraction {
    private struct Lease {
        let key: String
        let receive: ([ProjectedWorkPlan]?) -> Void
    }
    private struct Intent {
        let id: String
        let payload: Data
        let submittedAt: Date
        let draftID: String?
        var destination: String?
        var record: SubmittedOperation? {
            destination.map { SubmittedOperation(requestID: id, destination: $0, payload: payload,
                version: ServerProtocol.revision, submittedAt: submittedAt, draftID: draftID, resource: resource) }
        }
        let effect: WorkQueryEffect?
        var resource: OperationResource?
        var savedRevision: UInt64?
    }
    public private(set) var operations: [PendingOperation] = []
    public var creations: [PendingOperation] { operations.filter(\.isCreation) }
    public var listedOperations: [PendingOperation] { operations.filter(\.isListed) }
    public private(set) var machine: String?
    public private(set) var waitingForConnection = false
    public private(set) var cacheError: String?
    private var completing: Set<TaskTarget> = []
    private struct TaskTarget: Hashable { let issue: String; let directory: String }
    private var confirmedNames: [String: SessionNameFact] = [:]
    private var projectedNames: [String: (title: String, source: SessionTitleSource)] = [:]
    @ObservationIgnored private var saved: [SubmittedOperation] = []
    @ObservationIgnored private let store: ClientWorkStore
    @ObservationIgnored private var leases: [UUID: Lease] = [:]
    // Live queries are pinned; recent unleased answers share one bounded LRU.
    static let reusableQueryLimit = 8
    @ObservationIgnored private var recent: [String] = []
    @ObservationIgnored private var generation: UInt64 = 0
    @ObservationIgnored private var reconciled: Set<String> = []
    @ObservationIgnored private var reads: [String: ServedRead] = [:]
    @ObservationIgnored private var bases: [String: ClientWorkQuery] = [:]
    @ObservationIgnored private var intents: [Intent] = []
    @ObservationIgnored private var revision: UInt64 = 0
    @ObservationIgnored private var changes: Task<Void, Never>?
    @ObservationIgnored private var saving: Task<Void, Never>?
    @ObservationIgnored private var deliveries: [String: Task<Void, Never>] = [:]
    @ObservationIgnored private var refreshes: [UUID: Task<Void, Never>] = [:]
    @ObservationIgnored private var retiring: [UUID: Task<Void, Never>] = [:]
    @ObservationIgnored private var stopped: Task<Void, Never>?

    public init(machine: String?, store: ClientWorkStore) {
        self.machine = machine
        self.store = store
        if let machine { store.rememberMachine(machine) }
    }

    deinit {
        changes?.cancel()
        saving?.cancel()
        for task in deliveries.values { task.cancel() }
    }

    /// A local endpoint may reveal its Machine after a gesture. Binding keeps
    /// every mount on the existing unsaved intents; a known destination stays fixed.
    public func bindMachine(_ machine: String) {
        guard self.machine == nil else { return }
        self.machine = machine
        store.rememberMachine(machine)
    }

    public func open(_ read: ServedRead, receive: @escaping ([ProjectedWorkPlan]?) -> Void) -> UUID {
        let id = UUID()
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        // ServedRead encodes only finite enums, strings and integers.
        let key = String(decoding: try! encoder.encode(read), as: UTF8.self)
        reads[key] = read
        recent.removeAll { $0 == key }; recent.append(key)
        leases[id] = Lease(key: key, receive: receive)
        prune()
        receive(reconciled.contains(key) ? bases[key].map(project) : nil)
        if changes == nil { start() }
        else { refresh() }
        return id
    }

    public func close(_ id: UUID) {
        leases[id] = nil
        prune()
        if leases.isEmpty {
            generation &+= 1
            reconciled = []
        }
        refresh()
    }

    private func refresh() {
        let id = UUID()
        refreshes[id] = Task { [weak self] in
            defer { self?.refreshes[id] = nil }
            guard !Task.isCancelled else { return }
            await self?.refreshFromStore()
        }
    }

    /// Command observation belongs to the Machine, independently of query leases.
    public func start() {
        guard changes == nil else { return }
        stopped = nil
        changes = Task { [weak self, store] in
            for await _ in await store.changes() {
                guard !Task.isCancelled else { return }
                await self?.refreshFromStore()
                guard !Task.isCancelled else { return }
                await self?.resumeSaved()
            }
        }
    }

    /// Freeze and publish the operation before Machine discovery, save or send.
    @discardableResult
    public func createTask(request: String, directory: String, wave: String? = nil, graph: String? = nil, draftID: String? = nil,
                           select: (PendingOperation) -> Void) -> PendingOperation {
        let id = UUID().uuidString.lowercased()
        PerfClock.interaction(.taskCreate, .gesture, id: id)
        let command = ServedWrite.taskCreate(directory: directory, requestID: id, title: nil,
            request: request, wave: wave, graph: graph, session: nil, run: true)
        let intent = Intent(id: id, payload: try! JSONEncoder().encode(command), submittedAt: Date(), draftID: draftID,
            destination: machine, effect: nil)
        let operation = PendingOperation(id: id, payload: intent.payload, kind: "task.create", request: request, directory: directory, wave: wave,
            graph: graph, submittedAt: intent.submittedAt, destination: machine)
        operations.append(operation)
        intents.append(intent)
        select(operation)
        PerfClock.interaction(.taskCreate, .statePublished, id: id)
        start()
        enqueue(intent)
        return operation
    }

    public enum Resource: Hashable {
        case task(String), session(String)
    }

    public func presented(_ id: String) async throws {
        try await store.presented(id)
        await refreshFromStore()
    }

    /// Every mounted route acknowledges the same result, including ordinary rows.
    public func presented(_ resource: Resource) async throws {
        let ids = operations.filter { operation in
            switch resource {
            case .task(let id): operation.taskID == id
            case .session(let id): operation.sessionID == id
            }
        }.map(\.id)
        for id in ids { try await presented(id) }
    }

    private func prune() {
        let pinned = Set(leases.values.map(\.key))
        let reusable = recent.filter { !pinned.contains($0) }.suffix(Self.reusableQueryLimit)
        let retained = pinned.union(reusable)
        recent.removeAll { !retained.contains($0) }
        reads = reads.filter { retained.contains($0.key) }
        bases = bases.filter { retained.contains($0.key) }
        reconciled.formIntersection(retained)
    }

    public func receive(_ plans: [WorkViewPlan], stamp: ReadingStamp, for read: ServedRead, receivingLink: ReceivingLink? = nil) async throws {
        guard stamp.machine == machine else { throw RegistryQueryError("Work belongs to another Machine") }
        _ = try await store.receivePlans(plans, stamp: stamp, for: read, receivingLink: receivingLink)
        await refreshFromStore()
    }

    public func receiveNames(_ records: [SessionRecord], stamp: ReadingStamp) async throws {
        guard stamp.machine == machine else { throw RegistryQueryError("Session names belong to another Machine") }
        start()
        try await store.receiveNames(records, stamp: stamp)
        await refreshFromStore()
    }

    public func sessionTitle(id: String, fallback: String) -> String {
        projectedNames[id]?.title ?? fallback
    }

    public func named(_ record: SessionRecord) -> SessionRecord {
        guard let name = projectedNames[record.id] else { return record }
        var record = record
        record.title = name.title
        record.titleSource = name.source
        return record
    }

    /// Freeze the name and close the editor in the same turn as publication.
    @discardableResult
    public func renameSession(id session: String, name: String, directory: String) -> String {
        // A newer edit replaces a name that never saved. It cannot later retry
        // behind that newer edit and reverse the requested order.
        let replaced = Set(operations.filter {
            $0.kind == "session.rename" && $0.targetSession == session && !$0.saved && $0.error != nil
        }.map(\.id))
        operations.removeAll { replaced.contains($0.id) }
        intents.removeAll { replaced.contains($0.id) }
        let id = UUID().uuidString.lowercased()
        let command = ServedWrite.sessionRename(directory: directory, id: session, name: name)
        let payload = try! JSONEncoder().encode(command)
        let intent = Intent(id: id, payload: payload, submittedAt: Date(), draftID: nil, destination: machine, effect: nil, resource: .session(session))
        operations.append(PendingOperation(id: id, payload: payload, kind: "session.rename", request: name,
            directory: directory, wave: nil, graph: nil, submittedAt: intent.submittedAt, destination: machine))
        intents.append(intent)
        publish()
        start()
        enqueue(intent)
        return id
    }

    /// Publication and origin navigation happen in this call, before disk or
    /// Machine discovery. Receipt and save callbacks never navigate.
    @discardableResult
    public func complete(_ work: WorkWorkspaceKey, issue: String, directory: String,
                         selected: WorkWorkspaceKey?, visible: [WorkWorkspaceKey],
                         select: (WorkWorkspaceKey?) -> Void) -> String {
        leave(work, command: .taskComplete(directory: directory, issue: issue), kind: "task.complete",
            issue: issue, directory: directory, selected: selected, visible: visible, select: select)
    }

    @discardableResult
    public func abandon(_ work: WorkWorkspaceKey, issue: String, directory: String,
                        selected: WorkWorkspaceKey?, visible: [WorkWorkspaceKey], select: (WorkWorkspaceKey?) -> Void) -> String {
        leave(work, command: .taskAbandon(directory: directory, issue: issue), kind: "task.abandon",
            issue: issue, directory: directory, selected: selected, visible: visible, select: select)
    }

    /// Parent intent changes every cached membership without changing navigation.
    @discardableResult
    public func moveTask(_ work: WorkWorkspaceKey, issue: String, directory: String, wave: String?) -> String {
        let replaced = Set(intents.filter { intent in
            intent.resource == .task(work) && intent.effect?.kind == "task.move"
                && operations.contains { $0.id == intent.id && !$0.saved && $0.error != nil }
        }.map(\.id))
        intents.removeAll { replaced.contains($0.id) }
        operations.removeAll { replaced.contains($0.id) }
        let id = UUID().uuidString.lowercased()
        let command = ServedWrite.taskMove(directory: directory, issue: issue, wave: wave, machine: nil)
        let payload = try! JSONEncoder().encode(command)
        let effect = WorkQueryEffect(operationID: id, sequence: 0, directory: directory,
            kind: "task.move", issue: issue, wave: wave, work: work, resolution: nil)
        let intent = Intent(id: id, payload: payload, submittedAt: Date(), draftID: nil, destination: machine, effect: effect, resource: .task(work))
        operations.append(PendingOperation(id: id, payload: payload, kind: "task.move", request: "",
            directory: directory, wave: wave, graph: nil, submittedAt: intent.submittedAt, destination: machine))
        intents.append(intent)
        publish()
        start()
        enqueue(intent)
        return id
    }

    public enum TaskEdit { case claim, unassign, reopen }

    /// Membership edits publish synchronously and keep navigation and native owners.
    @discardableResult
    public func editTask(_ work: WorkWorkspaceKey, issue: String, directory: String, edit: TaskEdit) -> String {
        let command: ServedWrite
        let kind: String
        switch edit {
        case .claim: command = .taskClaim(directory: directory, issue: issue); kind = "task.claim"
        case .unassign: command = .taskUnassign(directory: directory, issue: issue); kind = "task.unassign"
        case .reopen: command = .taskReopen(directory: directory, issue: issue); kind = "task.reopen"
        }
        let id = UUID().uuidString.lowercased()
        let payload = try! JSONEncoder().encode(command)
        let effect = WorkQueryEffect(operationID: id, sequence: 0, directory: directory, kind: kind, issue: issue, work: work, resolution: nil)
        let intent = Intent(id: id, payload: payload, submittedAt: Date(), draftID: nil, destination: machine, effect: effect, resource: .task(work))
        operations.append(PendingOperation(id: id, payload: payload, kind: kind, request: "", directory: directory,
            wave: nil, graph: nil, submittedAt: intent.submittedAt, destination: machine))
        intents.append(intent)
        publish()
        start()
        enqueue(intent)
        return id
    }

    private func leave(_ work: WorkWorkspaceKey, command: ServedWrite, kind: String, issue: String, directory: String,
                       selected: WorkWorkspaceKey?, visible: [WorkWorkspaceKey], select: (WorkWorkspaceKey?) -> Void) -> String {
        let id = UUID().uuidString.lowercased()
        let journey: PerfClock.Journey = kind == "task.abandon" ? .taskAbandon : .taskComplete
        PerfClock.interaction(journey, .gesture, id: id)
        // ServedWrite's finite strings/enums cannot fail encoding.
        let payload = try! JSONEncoder().encode(command)
        let effect = WorkQueryEffect(operationID: id, sequence: 0, directory: directory,
            kind: kind, issue: issue, work: work, resolution: nil)
        let intent = Intent(id: id, payload: payload, submittedAt: Date(), draftID: nil, destination: machine, effect: effect, resource: .task(work))
        operations.append(PendingOperation(id: id, payload: payload, kind: kind, request: "",
            directory: directory, wave: nil, graph: nil, submittedAt: intent.submittedAt, destination: machine))
        intents.append(intent)
        publish()
        if selected == work {
            var seen = Set<WorkWorkspaceKey>()
            let order = visible.filter { seen.insert($0).inserted }
            let index = order.firstIndex(of: work) ?? order.endIndex
            let following = order.dropFirst(min(index + 1, order.count)) + order.prefix(index)
            select(following.first { $0 != work && $0.kind == .task })
        }
        PerfClock.interaction(journey, .statePublished, id: id)
        start()
        enqueue(intent)
        return id
    }

    public func canEditCreation(_ id: String) -> Bool {
        creations.first { $0.id == id }?.presentation().actions.contains(.edit) == true
    }

    public func editCreation(_ id: String, edit: (PendingOperation) -> Void) {
        guard canEditCreation(id), let operation = creations.first(where: { $0.id == id }) else { return }
        intents.removeAll { $0.id == id }
        operations.removeAll { $0.id == id }
        edit(operation)
    }

    public func retry(_ id: String) {
        guard let operation = operations.first(where: { $0.id == id }) else { return }
        operation.clearFailure()
        if let record = saved.first(where: { $0.id == id }) {
            // A read-only observer holds the same store recovery lease. Retire
            // it before a deliberate Check request, without resubmitting blindly.
            operation.deliveryFailed = true
            let previous = deliveries[id]
            previous?.cancel()
            let attempt = UUID()
            operation.deliveryAttempt = attempt
            deliveries[id] = Task { [weak self] in
                await previous?.value
                guard let self, !Task.isCancelled, operation.deliveryAttempt == attempt else { return }
                deliveries[id] = nil
                operation.deliveryFailed = false
                deliver(record)
            }
        } else if let intent = intents.first(where: { $0.id == id }) {
            publish()
            enqueue(intent)
        }
    }

    private func enqueue(_ intent: Intent) {
        let previous = saving
        saving = Task { [weak self, store] in
            await previous?.value
            guard let self, !Task.isCancelled else { return }
            var intent = intent
            do {
                if intent.destination == nil {
                    let destination: String
                    if let machine { destination = machine }
                    else { destination = try await store.resolveMachine() }
                    intent.destination = destination
                    bindMachine(destination)
                    if let index = intents.firstIndex(where: { $0.id == intent.id }) { intents[index] = intent }
                }
                guard let record = intent.record else { return }
                let revision = try await store.save(record)
                operations.first { $0.id == intent.id }?.markSaved()
                guard !Task.isCancelled else { return }
                if let index = intents.firstIndex(where: { $0.id == intent.id }) {
                    intents[index].savedRevision = revision
                }
                await refreshFromStore()
                guard !Task.isCancelled else { return }
                deliver(record)
            } catch {
                if let operation = operations.first(where: { $0.id == intent.id }) {
                    operation.fail(error.localizedDescription)
                    if operation.kind == "session.rename", let index = operations.firstIndex(where: { $0.id == intent.id }),
                       operations.dropFirst(index + 1).contains(where: { $0.kind == "session.rename" && $0.targetSession == operation.targetSession }) {
                        operations.removeAll { $0.id == intent.id }
                        intents.removeAll { $0.id == intent.id }
                    }
                }
                // A held older save can fail after a later Move was published.
                // Its original ID must never become retryable behind that edit.
                if intent.effect?.kind == "task.move", let index = intents.firstIndex(where: { $0.id == intent.id }),
                   intents.dropFirst(index + 1).contains(where: { $0.effect?.kind == "task.move" && $0.resource == intent.resource }) {
                    intents.removeAll { $0.id == intent.id }
                    operations.removeAll { $0.id == intent.id }
                }
                publish()
            }
        }
    }

    private func resumeSaved() async {
        guard changes != nil, !Task.isCancelled, machine != nil, let records = try? await store.pending() else { return }
        for record in records where record.destination == machine {
            guard deliveries[record.id] == nil,
                  operations.first(where: { $0.id == record.id })?.deliveryFailed != true,
                  !(await store.isRecovering(record.id)) else { continue }
            deliver(record)
        }
    }

    private func deliver(_ record: SubmittedOperation) {
        guard changes != nil, !Task.isCancelled, deliveries[record.id] == nil else { return }
        let attempt = UUID()
        let operation = operations.first { $0.id == record.id }
        operation?.deliveryAttempt = attempt
        deliveries[record.id] = Task { [weak self, store] in
            defer {
                if operation?.deliveryAttempt == attempt { self?.deliveries[record.id] = nil }
            }
            do {
                _ = try await store.reconcile(record)
                let command = try? JSONDecoder().decode(JSONValue.self, from: record.payload)
                if command?["kind"] == .string("task.create") {
                    PerfClock.interaction(.taskCreate, .commandReturned, id: record.id)
                } else if command?["kind"] == .string("task.complete") {
                    PerfClock.interaction(.taskComplete, .commandReturned, id: record.id)
                }
                try await store.observeSaved(record)
            } catch {
                guard !Task.isCancelled else { return }
                if let operation = self?.operations.first(where: { $0.id == record.id }) {
                    operation.deliveryFailed = true
                    operation.fail(error.localizedDescription)
                }
            }
        }
    }

    private func project(_ base: ClientWorkQuery) -> [ProjectedWorkPlan] {
        let effects = intents.filter { intent in
            operations.first(where: { $0.id == intent.id })?.error == nil
        }.compactMap(\.effect)
        guard !effects.isEmpty else { return base.projection }
        return base.applying(effects).projection
    }

    public func isCompleting(issue: String, directory: String) -> Bool {
        completing.contains(TaskTarget(issue: issue, directory: directory))
    }

    private func publish() {
        var names = confirmedNames.mapValues { (title: $0.title, source: $0.source) }
        for operation in operations where operation.kind == "session.rename" && operation.projectsEffect {
            guard let command = try? JSONDecoder().decode(JSONValue.self, from: operation.payload),
                  case .string(let session) = command["id"], case .string(let title) = command["name"] else { continue }
            if operation.evidence?.validation == .bound, let commit = operation.evidence?.commits.first(where: { $0.stage == "session.renamed" }),
               commit.resolution == .committed, confirmedNames[session]?.covers(commit.fence) == true { continue }
            names[session] = (title, .human)
        }
        projectedNames = names
        var pending = Set<TaskTarget>()
        for operation in operations where operation.projectsEffect {
            guard let issue = operation.targetIssue else { continue }
            let target = TaskTarget(issue: issue, directory: operation.directory)
            if operation.kind == "task.complete" { pending.insert(target) }
            if operation.kind == "task.reopen" || operation.kind == "task.abandon" { pending.remove(target) }
        }
        completing = pending
        for lease in Array(leases.values) { lease.receive(reconciled.contains(lease.key) ? bases[lease.key].map(project) : nil) }
    }

    /// Finish the ordered local saves, including when the application backgrounds.
    public func persistPending() async { await saving?.value }

    public func refreshFromStore() async {
        let queries = leases.isEmpty ? [:] : reads
        let generation = generation
        guard let machine else { return }
        do {
            let update = try await store.presentation(machine: machine, queries: queries,
                reusable: leases.isEmpty ? [] : Set(queries.keys))
            guard generation == self.generation, changes != nil else { return }
            apply(update)
            cacheError = nil
        } catch {
            // Keep the published projection while offering the cache error.
            cacheError = error.localizedDescription
        }
    }

    func apply(_ update: ClientWorkStore.Presentation) {
        guard update.revision >= revision else { return }
        revision = update.revision
        waitingForConnection = update.waitingForConnection
        confirmedNames = update.names
        let savedIDs = Set(update.records.map(\.id))
        for (id, task) in deliveries where !savedIDs.contains(id) {
            task.cancel()
            deliveries[id] = nil
            let retirement = UUID()
            retiring[retirement] = Task { [weak self] in
                await task.value
                self?.retiring[retirement] = nil
            }
        }
        intents.removeAll { savedIDs.contains($0.id) || $0.savedRevision.map { $0 <= update.revision } == true }
        saved = update.records
        for value in update.operations {
            let operation: PendingOperation
            if let existing = operations.first(where: { $0.id == value.record.id }) { operation = existing }
            else {
                operation = PendingOperation(value)
                operations.append(operation)
            }
            operation.receive(value)
        }
        operations.removeAll { operation in
            !savedIDs.contains(operation.id) && !intents.contains(where: { $0.id == operation.id })
        }
        for key in update.requested where reads[key] != nil {
            bases[key] = update.queries[key]
            reconciled.insert(key)
        }
        publish()
    }

    /// Application/test teardown. Closing a query alone does not cancel writes.
    @discardableResult
    public func shutdown() -> Task<Void, Never> {
        if let stopped { return stopped }
        let tasks = [changes, saving].compactMap { $0 } + Array(deliveries.values)
            + Array(refreshes.values) + Array(retiring.values)
        for task in tasks { task.cancel() }
        changes = nil
        saving = nil
        deliveries = [:]
        refreshes = [:]
        retiring = [:]
        leases = [:]
        generation &+= 1
        reconciled = []
        let stopped = Task {
            for task in tasks { await task.value }
        }
        self.stopped = stopped
        return stopped
    }
}
