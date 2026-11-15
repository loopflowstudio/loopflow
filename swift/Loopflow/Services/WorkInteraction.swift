import Foundation
import Observation

/// Process-wide presentation for one Machine. Query leases belong to windows;
/// commands keep their original destination independently of those leases.
@MainActor @Observable
public final class WorkInteraction {
    public struct Failure: Identifiable {
        public let record: SubmittedOperation?
        public let message: String
        let saved: Bool
        public let id: String
        fileprivate let intent: Intent?
    }
    private struct Lease {
        let key: String
        let receive: ([ProjectedWorkPlan]?) -> Void
    }
    fileprivate struct Intent {
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
    public private(set) var creations: [CreationOperation] = []
    public private(set) var machine: String?
    public private(set) var failures: [Failure] = []
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
                           select: (CreationOperation) -> Void) -> CreationOperation {
        let id = UUID().uuidString.lowercased()
        PerfClock.interaction(.taskCreate, .gesture, id: id)
        let command = ServedWrite.taskCreate(directory: directory, requestID: id, title: nil,
            request: request, wave: wave, graph: graph, session: nil, run: true)
        let intent = Intent(id: id, payload: try! JSONEncoder().encode(command), submittedAt: Date(), draftID: draftID,
            destination: machine, effect: nil)
        let operation = CreationOperation(id: id, request: request, directory: directory, wave: wave,
            graph: graph, submittedAt: intent.submittedAt, destination: machine)
        creations.append(operation)
        intents.append(intent)
        select(operation)
        PerfClock.interaction(.taskCreate, .statePublished, id: id)
        start()
        enqueue(intent)
        return operation
    }

    public func presented(_ id: String) async throws { try await store.presented(id) }

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
        enqueue(intent)
        return id
    }

    public func canEditCreation(_ id: String) -> Bool {
        failures.contains { $0.id == id && !$0.saved } && creations.contains { $0.id == id }
    }

    public func editCreation(_ id: String, edit: (CreationOperation) -> Void) {
        guard canEditCreation(id), let operation = creations.first(where: { $0.id == id }) else { return }
        failures.removeAll { $0.id == id }
        creations.removeAll { $0.id == id }
        edit(operation)
    }

    public func retry(_ id: String) {
        guard let failure = failures.first(where: { $0.id == id }) else { return }
        if failure.saved, let record = failure.record {
            failures.removeAll { $0.id == id }
            deliver(record)
            return
        }
        guard let intent = failure.intent else { return }
        failures.removeAll { $0.id == id }
        creations.first { $0.id == id }?.clearFailure()
        intents.append(intent)
        publish()
        enqueue(intent)
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
                creations.first { $0.id == intent.id }?.markSaved()
                guard !Task.isCancelled else { return }
                if let index = intents.firstIndex(where: { $0.id == intent.id }) {
                    intents[index].savedRevision = revision
                }
                await refreshFromStore()
                deliver(record)
            } catch {
                intents.removeAll { $0.id == intent.id }
                failures.append(Failure(record: intent.record, message: error.localizedDescription, saved: false, id: intent.id, intent: intent))
                creations.first { $0.id == intent.id }?.fail(error.localizedDescription)
                publish()
            }
        }
    }

    private func resumeSaved() async {
        guard machine != nil, let records = try? await store.pending() else { return }
        for record in records where record.destination == machine {
            guard deliveries[record.id] == nil, !failures.contains(where: { $0.id == record.id }),
                  let command = try? JSONDecoder().decode(JSONValue.self, from: record.payload),
                  case .object(let fields) = command, (fields["kind"] == .string("task.complete") || fields["kind"] == .string("task.create")) else { continue }
            deliver(record)
        }
    }

    private func deliver(_ record: SubmittedOperation) {
        guard deliveries[record.id] == nil else { return }
        deliveries[record.id] = Task { [weak self, store] in
            defer { self?.deliveries[record.id] = nil }
            do {
                _ = try await store.reconcile(record)
                let command = try? JSONDecoder().decode(JSONValue.self, from: record.payload)
                PerfClock.interaction(command?["kind"] == .string("task.create") ? .taskCreate : .taskComplete,
                    .commandReturned, id: record.id)
                try await store.observeSaved(record)
            } catch {
                guard !Task.isCancelled else { return }
                self?.creations.first { $0.id == record.id }?.fail(error.localizedDescription)
                self?.failures.append(Failure(record: record, message: error.localizedDescription, saved: true, id: record.id, intent: nil))
            }
        }
    }

    private func project(_ base: ClientWorkQuery) -> [ProjectedWorkPlan] {
        base.applying(intents.compactMap(\.effect)).projection
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
        commandPayloads = saved.map(\.payload) + intents.filter { intent in !saved.contains(where: { $0.id == intent.id }) }.map(\.payload)
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
        for value in update.creations {
            let operation: CreationOperation
            if let existing = creations.first(where: { $0.id == value.record.id }) { operation = existing }
            else {
                operation = CreationOperation(value)
                creations.append(operation)
            }
            operation.receive(value)
        }
        creations.removeAll { operation in
            !savedIDs.contains(operation.id) && !intents.contains(where: { $0.id == operation.id })
                && !failures.contains(where: { $0.id == operation.id })
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
