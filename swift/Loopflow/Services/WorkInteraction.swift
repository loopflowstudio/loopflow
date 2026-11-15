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
                version: ServerProtocol.revision, submittedAt: submittedAt, draftID: draftID) }
        }
        let effect: WorkQueryEffect?
        var savedRevision: UInt64?
    }
    public private(set) var operations: [PendingOperation] = []
    public var creations: [PendingOperation] { operations.filter(\.isCreation) }
    public var listedOperations: [PendingOperation] { operations.filter(\.isListed) }
    public private(set) var machine: String?
    public private(set) var cacheError: String?
    private var commandPayloads: [Data] = []
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

    public init(machine: String?, store: ClientWorkStore) {
        self.machine = machine
        self.store = store
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
        else { Task { [weak self] in await self?.refreshFromStore() } }
        return id
    }

    public func close(_ id: UUID) {
        leases[id] = nil
        prune()
        if leases.isEmpty {
            generation &+= 1
            reconciled = []
        }
        Task { [weak self] in await self?.refreshFromStore() }
    }

    /// Command observation belongs to the Machine, independently of query leases.
    public func start() {
        guard changes == nil else { return }
        changes = Task { [weak self, store] in
            for await _ in await store.changes() {
                guard !Task.isCancelled else { return }
                await self?.refreshFromStore()
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

    public func receive(_ plans: [WorkViewPlan], stamp: ReadingStamp, for read: ServedRead) async throws {
        guard stamp.machine == machine else { throw RegistryQueryError("Work belongs to another Machine") }
        _ = try await store.receivePlans(plans, stamp: stamp, for: read)
        await refreshFromStore()
    }

    /// Publication and origin navigation happen in this call, before disk or
    /// Machine discovery. Receipt and save callbacks never navigate.
    @discardableResult
    public func complete(_ work: WorkWorkspaceKey, issue: String, directory: String,
                         selected: WorkWorkspaceKey?, visible: [WorkWorkspaceKey],
                         select: (WorkWorkspaceKey?) -> Void) -> String {
        let id = UUID().uuidString.lowercased()
        PerfClock.interaction(.taskComplete, .gesture, id: id)
        // ServedWrite's finite strings/enums cannot fail encoding.
        let payload = try! JSONEncoder().encode(ServedWrite.taskComplete(directory: directory, issue: issue))
        let effect = WorkQueryEffect(operationID: id, sequence: 0, directory: directory,
            kind: "task.complete", issue: issue, resolution: nil)
        let intent = Intent(id: id, payload: payload, submittedAt: Date(), draftID: nil, destination: machine, effect: effect)
        operations.append(PendingOperation(id: id, payload: payload, kind: "task.complete", request: "",
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
        PerfClock.interaction(.taskComplete, .statePublished, id: id)
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
                deliver(record)
            } catch {
                operations.first { $0.id == intent.id }?.fail(error.localizedDescription)
                publish()
            }
        }
    }

    private func resumeSaved() async {
        guard machine != nil, let records = try? await store.pending() else { return }
        for record in records where record.destination == machine {
            guard deliveries[record.id] == nil,
                  operations.first(where: { $0.id == record.id })?.deliveryFailed != true,
                  !(await store.isRecovering(record.id)) else { continue }
            deliver(record)
        }
    }

    private func deliver(_ record: SubmittedOperation) {
        guard deliveries[record.id] == nil else { return }
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
        base.applying(intents.filter { intent in
            operations.first(where: { $0.id == intent.id })?.error == nil
        }.compactMap(\.effect)).projection
    }

    public func isCompleting(issue: String, directory: String) -> Bool {
        commandPayloads.contains { payload in
            guard let command = try? JSONDecoder().decode(JSONValue.self, from: payload),
                  case .object(let fields) = command else { return false }
            return fields["kind"] == .string("task.complete") && fields["directory"] == .string(directory)
                && fields["issue"] == .string(issue)
        }
    }

    private func publish() {
        commandPayloads = saved.map(\.payload) + intents.filter { intent in
            !saved.contains(where: { $0.id == intent.id }) && operations.first(where: { $0.id == intent.id })?.error == nil
        }.map(\.payload)
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
        let savedIDs = Set(update.records.map(\.id))
        for (id, task) in deliveries where !savedIDs.contains(id) {
            task.cancel()
            deliveries[id] = nil
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
    public func shutdown() {
        changes?.cancel(); changes = nil
        saving?.cancel(); saving = nil
        for task in deliveries.values { task.cancel() }
        deliveries = [:]
        leases = [:]
        generation &+= 1
        reconciled = []
    }
}
