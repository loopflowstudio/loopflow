import Foundation
import Observation

/// Process-wide presentation for one Machine. Query leases belong to windows;
/// commands keep their original destination independently of those leases.
@MainActor @Observable
public final class WorkInteraction {
    public struct Failure: Identifiable {
        public let record: SubmittedOperation
        public let message: String
        let saved: Bool
        public var id: String { record.id }
    }
    private struct Lease {
        let key: String
        let read: ServedRead
        let receive: ([ProjectedWorkPlan]?) -> Void
    }
    private struct Intent {
        let record: SubmittedOperation
        let effect: WorkQueryEffect
        var savedRevision: UInt64?
    }
    public let machine: String
    public private(set) var failures: [Failure] = []
    public private(set) var cacheError: String?
    public private(set) var pending: [SubmittedOperation] = []
    @ObservationIgnored private var saved: [SubmittedOperation] = []
    @ObservationIgnored private let store: ClientWorkStore
    @ObservationIgnored private var leases: [UUID: Lease] = [:]
    @ObservationIgnored private var bases: [String: ClientWorkQuery] = [:]
    @ObservationIgnored private var intents: [Intent] = []
    @ObservationIgnored private var revision: UInt64 = 0
    @ObservationIgnored private var resumed = false
    @ObservationIgnored private var changes: Task<Void, Never>?
    @ObservationIgnored private var saving: Task<Void, Never>?
    @ObservationIgnored private var deliveries: [String: Task<Void, Never>] = [:]

    public init(machine: String, store: ClientWorkStore) {
        self.machine = machine
        self.store = store
    }

    public func open(_ read: ServedRead, receive: @escaping ([ProjectedWorkPlan]?) -> Void) throws -> UUID {
        let id = UUID()
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        let key = String(decoding: try encoder.encode(read), as: UTF8.self)
        leases[id] = Lease(key: key, read: read, receive: receive)
        if let base = bases[key] { receive(project(base)) }
        if changes == nil {
            changes = Task { [weak self, store] in
                for await _ in await store.changes() {
                    guard !Task.isCancelled else { return }
                    await self?.refreshFromStore()
                    await self?.resumeSaved()
                }
            }
        } else { Task { [weak self] in await self?.refreshFromStore() } }
        return id
    }

    public func close(_ id: UUID) {
        let key = leases.removeValue(forKey: id)?.key
        if let key, !leases.values.contains(where: { $0.key == key }) { bases[key] = nil }
        if leases.isEmpty { changes?.cancel(); changes = nil }
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
                         select: (WorkWorkspaceKey?) -> Void) throws -> String {
        let id = UUID().uuidString.lowercased()
        PerfClock.interaction(.taskComplete, .gesture, id: id)
        let record = SubmittedOperation(requestID: id, destination: machine,
            payload: try JSONEncoder().encode(ServedWrite.taskComplete(directory: directory, issue: issue)),
            version: ServerProtocol.revision, submittedAt: Date())
        let effect = WorkQueryEffect(operationID: id, sequence: 0, directory: directory,
            kind: "task.complete", issue: issue, resolution: nil)
        let intent = Intent(record: record, effect: effect)
        intents.append(intent)
        publish()
        if selected == work {
            let index = visible.firstIndex(of: work) ?? visible.endIndex
            let following = visible.dropFirst(min(index + 1, visible.count)) + visible.prefix(index)
            select(following.first { $0 != work && $0.kind == .task })
        }
        PerfClock.interaction(.taskComplete, .statePublished, id: id)
        enqueue(intent)
        return id
    }

    public func retry(_ id: String) {
        guard let failure = failures.first(where: { $0.id == id }) else { return }
        if failure.saved {
            failures.removeAll { $0.id == id }
            deliver(failure.record)
            return
        }
        guard let command = try? JSONDecoder().decode(JSONValue.self, from: failure.record.payload),
              case .object(let fields) = command,
              case .string(let directory) = fields["directory"],
              case .string(let issue) = fields["issue"] else { return }
        failures.removeAll { $0.id == id }
        let intent = Intent(record: failure.record, effect: WorkQueryEffect(operationID: id, sequence: 0,
            directory: directory, kind: "task.complete", issue: issue, resolution: nil))
        intents.append(intent)
        publish()
        enqueue(intent)
    }

    private func enqueue(_ intent: Intent) {
        let previous = saving
        saving = Task { [weak self, store] in
            await previous?.value
            guard let self, !Task.isCancelled else { return }
            do {
                let revision = try await store.save(intent.record)
                guard !Task.isCancelled else { return }
                if let index = intents.firstIndex(where: { $0.record.id == intent.record.id }) {
                    intents[index].savedRevision = revision
                }
                await refreshFromStore()
                deliver(intent.record)
            } catch {
                intents.removeAll { $0.record.id == intent.record.id }
                failures.append(Failure(record: intent.record, message: error.localizedDescription, saved: false))
                publish()
            }
        }
    }

    private func resumeSaved() async {
        guard !resumed else { return }
        resumed = true
        guard let records = try? await store.pending() else { resumed = false; return }
        for record in records where record.destination == machine {
            guard deliveries[record.id] == nil,
                  let command = try? JSONDecoder().decode(JSONValue.self, from: record.payload),
                  case .object(let fields) = command, fields["kind"] == .string("task.complete") else { continue }
            deliver(record)
        }
    }

    private func deliver(_ record: SubmittedOperation) {
        guard deliveries[record.id] == nil else { return }
        deliveries[record.id] = Task { [weak self, store] in
            defer { self?.deliveries[record.id] = nil }
            do {
                _ = try await store.reconcile(record)
                PerfClock.interaction(.taskComplete, .commandReturned, id: record.id)
                try await store.observeSaved(record)
            } catch {
                guard !Task.isCancelled else { return }
                self?.failures.append(Failure(record: record, message: error.localizedDescription, saved: true))
            }
        }
    }

    private func project(_ base: ClientWorkQuery) -> [ProjectedWorkPlan] {
        base.applying(intents.map(\.effect)).projection
    }

    public func isCompleting(issue: String, directory: String) -> Bool {
        pending.contains { record in
            guard let command = try? JSONDecoder().decode(JSONValue.self, from: record.payload),
                  case .object(let fields) = command else { return false }
            return fields["kind"] == .string("task.complete") && fields["directory"] == .string(directory)
                && fields["issue"] == .string(issue)
        }
    }

    private func publish() {
        pending = saved + intents.filter { intent in !saved.contains(where: { $0.id == intent.record.id }) }.map(\.record)
        for lease in Array(leases.values) { lease.receive(bases[lease.key].map(project)) }
    }

    /// Finish the ordered local saves, including when the application backgrounds.
    public func persistPending() async { await saving?.value }

    public func refreshFromStore() async {
        let queries = Dictionary(leases.values.map { ($0.key, $0.read) }, uniquingKeysWith: { first, _ in first })
        do {
            apply(try await store.presentation(machine: machine, queries: queries))
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
        intents.removeAll { savedIDs.contains($0.record.id) || $0.savedRevision.map { $0 <= update.revision } == true }
        saved = update.records
        let mounted = Set(leases.values.map(\.key))
        for key in update.requested.intersection(mounted) { bases[key] = update.queries[key] }
        publish()
    }

    /// Application/test teardown. Closing a query alone does not cancel writes.
    public func shutdown() {
        changes?.cancel(); changes = nil
        saving?.cancel(); saving = nil
        for task in deliveries.values { task.cancel() }
        deliveries = [:]
        leases = [:]
    }
}
