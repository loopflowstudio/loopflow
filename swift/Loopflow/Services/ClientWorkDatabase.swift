import CryptoKit
import Foundation
import SQLite3

/// Confined to ClientWorkStore's actor. One transaction imports released journal
/// files, retaining them until commit. A crash can repeat import without loss.
final class ClientWorkDatabase {
    struct State: Codable {
        let record: SubmittedOperation
        var sentEpoch: String?
        var receiptEpoch: String?
        // The saved receipt cursor; work_epochs owns the Machine's active lifetime.
        var observedEpoch: String?
        var receipt: OperationReceipt?
        var presentedAt: Date?
        var receiptUnavailable: Bool?
        var departedEpochs: [String]?
        var legacyOutcome: JSONValue?
        var legacySource: JSONValue?
        var retiredAt: Date?
        var imported: Bool?
        var sourceReleased: Bool?
    }
    private struct RetiredIdentity: Codable {
        let id: String
        let draftID: String?
        let digest: String
        let retiredAt: Date
    }

    private func digest(_ record: SubmittedOperation) throws -> String {
        let command = try JSONDecoder().decode(JSONValue.self, from: record.payload)
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        var hash = SHA256()
        hash.update(data: Data(record.destination.utf8))
        hash.update(data: Data([0]))
        hash.update(data: try encoder.encode(command))
        return hash.finalize().map { String(format: "%02x", $0) }.joined()
    }

    private func retired(_ id: String) throws -> RetiredIdentity? {
        try rows("SELECT state FROM retired_operations WHERE id=?", id: id).first.map {
            try JSONDecoder().decode(RetiredIdentity.self, from: $0)
        }
    }

    private func retainIdentity(_ state: State) throws {
        let identity = RetiredIdentity(id: state.record.id, draftID: state.record.draftID,
            digest: try digest(state.record), retiredAt: state.retiredAt ?? Date())
        try execute("INSERT OR REPLACE INTO retired_operations(id,state) VALUES(?,?)",
            id: identity.id, data: JSONEncoder().encode(identity))
        try execute("DELETE FROM operations WHERE id=?", id: identity.id)
    }

    private struct Epochs: Codable {
        var current: String
        var departed: Set<String>
    }
    private struct QueryRequirement: Codable {
        var fence: CommitFence
        let directory: String?
        let kind: String?

        func requiredFence(in reading: ClientWorkQuery) -> CommitFence? {
            guard let directory, let kind else { return fence }
            let declarations = reading.plans.filter { $0.repositoryPath == directory }.compactMap { $0.membership.effects[kind] }
            guard !declarations.isEmpty else { return nil }
            let domains = Array(Set(declarations.flatMap(\.domains)))
            // A missing domain cannot weaken the stored requirement.
            return fence.requiring(domains) ?? fence
        }
    }
    static let retainedEffectLimit = 64
    private var retainedQueries: [String: Set<String>] = [:]
    private let connection: OpaquePointer

    init(directory: URL, prepareDirectory: (URL) throws -> Void = { _ in }) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try prepareDirectory(directory)
        var connection: OpaquePointer?
        let code = sqlite3_open_v2(directory.appendingPathComponent("work.sqlite").path, &connection,
                                   SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_FULLMUTEX, nil)
        guard code == SQLITE_OK, let connection else {
            let message = connection.map { String(cString: sqlite3_errmsg($0)) } ?? "Cannot open client store"
            if let connection { sqlite3_close(connection) }
            throw RegistryQueryError(message)
        }
        // Deinit owns the handle even if a later initialization step throws.
        self.connection = connection
        try execute("PRAGMA journal_mode=WAL")
        try execute("PRAGMA synchronous=FULL")
        try execute("CREATE TABLE IF NOT EXISTS operations (sequence INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT NOT NULL UNIQUE, state BLOB NOT NULL)")
        try execute("CREATE UNIQUE INDEX IF NOT EXISTS operations_draft ON operations(json_extract(CAST(state AS TEXT),'$.record.draftID'))")
        try execute("CREATE INDEX IF NOT EXISTS operations_pending ON operations(sequence) WHERE json_extract(CAST(state AS TEXT),'$.retiredAt') IS NULL")
        try execute("CREATE INDEX IF NOT EXISTS operations_sources ON operations(sequence) WHERE (json_extract(CAST(state AS TEXT),'$.imported')=1 OR json_extract(CAST(state AS TEXT),'$.record.draftID') IS NOT NULL) AND json_extract(CAST(state AS TEXT),'$.sourceReleased') IS NOT 1")
        try execute("CREATE TABLE IF NOT EXISTS retired_operations (id TEXT PRIMARY KEY, state BLOB NOT NULL)")
        try execute("CREATE UNIQUE INDEX IF NOT EXISTS retired_operations_draft ON retired_operations(json_extract(CAST(state AS TEXT),'$.draftID'))")
        // Compact the earlier branch format before any source-file import.
        try transaction {
            for data in try rows("SELECT state FROM operations WHERE json_extract(CAST(state AS TEXT),'$.retiredAt') IS NOT NULL") {
                try retainIdentity(JSONDecoder().decode(State.self, from: data))
            }
        }
        try execute("CREATE TABLE IF NOT EXISTS work_queries (id TEXT PRIMARY KEY, state BLOB NOT NULL)")
        try execute("CREATE INDEX IF NOT EXISTS work_queries_with_effects ON work_queries(id) WHERE json_array_length(CAST(state AS TEXT),'$.effects')>0")
        try execute("CREATE TABLE IF NOT EXISTS work_epochs (id TEXT PRIMARY KEY, state BLOB NOT NULL)")
        try execute("CREATE TABLE IF NOT EXISTS work_requirements (id TEXT PRIMARY KEY, state BLOB NOT NULL)")
        // Confirmed caches are replaceable. The outbox and its reload floors
        // survive a membership vocabulary change; never reinterpret an old plan.
        try execute("DELETE FROM work_queries WHERE EXISTS (SELECT 1 FROM json_each(CAST(work_queries.state AS TEXT),'$.plans') WHERE json_type(value,'$.membership.domains') IS NULL)")
        let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "json" }
        let records = try files.map { try JSONDecoder().decode(SubmittedOperation.self, from: Data(contentsOf: $0)) }
            .sorted { ($0.submittedAt, $0.id) < ($1.submittedAt, $1.id) }
        try importLegacy(records.map { LegacyOperation(record: $0, outcome: nil) })
        for file in files { try FileManager.default.removeItem(at: file) }
        try releaseSources(requests: records.map(\.id), draftID: nil)
    }

    deinit { sqlite3_close(connection) }

    func pending() throws -> [SubmittedOperation] {
        try rows("SELECT state FROM operations WHERE json_extract(CAST(state AS TEXT),'$.retiredAt') IS NULL ORDER BY sequence").map { try JSONDecoder().decode(State.self, from: $0).record }
    }

    func draftSubmitted(_ id: String) throws -> Bool {
        try !rows("SELECT CAST(1 AS BLOB) FROM operations WHERE json_extract(CAST(state AS TEXT),'$.record.draftID')=?1 UNION ALL SELECT CAST(1 AS BLOB) FROM retired_operations WHERE json_extract(CAST(state AS TEXT),'$.draftID')=?1 LIMIT 1", id: id).isEmpty
    }

    func state(_ id: String) throws -> State? {
        try rows("SELECT state FROM operations WHERE id=?", id: id).first.map { try JSONDecoder().decode(State.self, from: $0) }
    }

    func save(_ record: SubmittedOperation, sentEpoch: String? = nil) throws {
        if let identity = try retired(record.id) {
            guard identity.digest == (try digest(record)) else {
                throw RegistryQueryError("This retired operation identity already has another submitted command")
            }
            return
        }
        if let draftID = record.draftID, try draftSubmitted(draftID), try state(record.id) == nil {
            throw RegistryQueryError("This editor revision was already submitted")
        }
        if let existing = try state(record.id) {
            guard try existing.record.hasSameCommand(as: record) else {
                throw RegistryQueryError("This operation identity already has another submitted command")
            }
            return
        }
        let state = State(record: record, sentEpoch: sentEpoch, receiptEpoch: nil, observedEpoch: nil, receipt: nil)
        try execute("INSERT INTO operations(id,state) VALUES(?,?)", id: record.id, data: JSONEncoder().encode(state))
    }

    func importLegacy(_ values: [LegacyOperation]) throws {
        try transaction {
            for value in values {
                try save(value.record, sentEpoch: "")
                if try retired(value.record.id) != nil { continue }
                guard var state = try state(value.record.id) else {
                    throw RegistryQueryError("The imported operation disappeared")
                }
                if let outcome = value.outcome, let receipt = state.receipt {
                    try validateLegacy(outcome, receipt: receipt)
                }
                if let previous = state.legacyOutcome, let outcome = value.outcome, previous != outcome {
                    throw RegistryQueryError("This operation identity already has another saved outcome")
                }
                if let previous = state.legacySource, let source = value.source, previous != source {
                    throw RegistryQueryError("This operation identity already has another saved submission")
                }
                state.imported = true
                if state.legacyOutcome == nil { state.legacyOutcome = value.outcome }
                if state.legacySource == nil { state.legacySource = value.source }
                try update(state)
            }
        }
    }

    private func validateLegacy(_ outcome: JSONValue, receipt: OperationReceipt) throws {
        guard receipt.validation == .bound else { return }
        let result = receipt.result ?? receipt.commits.last(where: { $0.stage == "task.filed" })?.result
        if case .string(let original) = outcome["task_id"],
           case .string(let current) = result?["task"]["id"], original != current {
            throw RegistryQueryError("The current receipt names another Task; the original saved outcome is retained")
        }
        if case .string(let original) = outcome["receipt"]["session_id"],
           case .string(let current) = result?["session_id"], original != current {
            throw RegistryQueryError("The current receipt names another Session; the original saved outcome is retained")
        }
    }

    /// Epoch departure survives reconnect and restart; an old subscription can
    /// never restore its former evidence just by being the first to answer.
    func observeEpoch(_ epoch: String, id: String) throws -> Bool {
        guard var state = try state(id), !(state.departedEpochs ?? []).contains(epoch) else { return false }
        return try transaction {
            guard try observeMachineEpoch(epoch, machine: state.record.destination) else { return false }
            if state.observedEpoch != epoch {
                if let previous = state.observedEpoch { state.departedEpochs = (state.departedEpochs ?? []) + [previous] }
                state.observedEpoch = epoch
                state.receiptUnavailable = true
                try update(state)
            }
            return true
        }
    }

    func unavailable(_ id: String) throws {
        guard var state = try state(id) else { return }
        state.receiptUnavailable = true
        try update(state)
    }

    func presented(_ id: String) throws {
        try transaction {
            guard var state = try state(id) else { return }
            guard state.receipt?.result != nil || state.receipt?.commits.contains(where: { $0.result != .null }) == true else {
                throw RegistryQueryError("No saved result is available to present")
            }
            if state.presentedAt == nil {
                state.presentedAt = Date()
                try update(state)
            }
            try retire(id)
        }
    }

    func markSent(_ id: String, epoch: String) throws {
        guard var state = try state(id) else { throw RegistryQueryError("The operation is not saved") }
        state.sentEpoch = epoch
        try update(state)
    }

    func receive(_ receipt: OperationReceipt, epoch: String, id: String) throws {
        guard var state = try state(id) else { return }
        // A replacement database cannot rewrite the original result. Its facts
        // need domain recovery before they can confirm this saved operation.
        guard state.receiptEpoch == nil || state.receiptEpoch == epoch else { return }
        if state.receiptEpoch == epoch, let previous = state.receipt, previous.revision > receipt.revision { return }
        if let outcome = state.legacyOutcome { try validateLegacy(outcome, receipt: receipt) }
        state.receiptUnavailable = false
        if state.receiptEpoch != epoch || state.receipt?.revision != receipt.revision {
            state.receipt = receipt
        }
        state.receiptEpoch = epoch
        // Learning the receipt's lifetime makes an imported journal recoverable.
        state.sentEpoch = epoch
        try update(state)
    }

    /// A single transaction owns presentation, query invalidation and retirement.
    /// Required reload fences outlive the command, including for unopened queries.
    func retireIfConverged(_ id: String) throws {
        try transaction { try retire(id) }
    }

    private func retire(_ id: String) throws {
        guard let state = try state(id),
              state.receiptUnavailable != true,
              let epoch = try activeEpoch(machine: state.record.destination), state.receiptEpoch == epoch,
              let receipt = state.receipt,
              let declaration = OperationDeclaration(kind: receipt.kind),
              let resolutions = declaration.resolutions(in: receipt) else { return }
        if declaration.requiresPresentation && state.presentedAt == nil { return }
        let recovery = OperationRecovery(receipt: receipt, planning: try currentPlanning(state),
            resultPresented: true, receiptAvailable: true)
        guard !recovery.needsInspection else { return }
        guard resolutions.allSatisfy({
            $0.fence.machine == state.record.destination && $0.fence.epoch == epoch
                && $0.fence.schema == ServerProtocol.revision && !$0.fence.revisions.isEmpty
        }) else { return }
        // Only the reusable working set keeps an exact ordered effect. Closed
        // bases become unavailable under the compact floors below, atomically
        // with retirement. A long-held query also has a finite effect budget.
        let membershipEffect = try membershipEffect(state)
        if let effect = membershipEffect {
            for key in (retainedQueries[state.record.destination] ?? []).sorted() {
                guard let base = try queryState(key) else { continue }
                var reading = base
                guard reading.stamp.machine == state.record.destination,
                      reading.stamp.epoch == epoch,
                      reading.plans.contains(where: { $0.repositoryPath == effect.directory && $0.membership.effects[effect.kind] != nil }),
                      !effect.covered(by: reading.stamp, plans: reading.plans) else { continue }
                guard try usable(reading), (reading.effects ?? []).count < Self.retainedEffectLimit else {
                    try execute("DELETE FROM work_queries WHERE id=?", id: key)
                    continue
                }
                var effects = reading.effects ?? []
                effects.removeAll { $0.operationID == effect.operationID }
                effects.append(effect)
                reading.effects = effects
                try execute("UPDATE work_queries SET state=?2 WHERE id=?1", id: key, data: JSONEncoder().encode(reading))
            }
        }
        var requirements = try requirements(state.record.destination)
        for resolution in resolutions {
            let fence = resolution.fence
            // Bound requirements by lifetime, repository and effect declaration,
            // independent of the number of completed commands.
            if let index = requirements.firstIndex(where: {
                $0.fence.machine == fence.machine && $0.fence.epoch == fence.epoch && $0.fence.schema == fence.schema
                    && $0.directory == membershipEffect?.directory && $0.kind == membershipEffect?.kind
            }) {
                var revisions = requirements[index].fence.revisions
                for (domain, revision) in fence.revisions { revisions[domain] = max(revisions[domain] ?? 0, revision) }
                requirements[index].fence = CommitFence(machine: fence.machine, epoch: fence.epoch,
                    schema: fence.schema, revisions: revisions)
            } else { requirements.append(QueryRequirement(fence: fence, directory: membershipEffect?.directory, kind: membershipEffect?.kind)) }
        }
        try execute("INSERT OR REPLACE INTO work_requirements(id,state) VALUES(?,?)",
            id: state.record.destination, data: JSONEncoder().encode(requirements))
        if state.sourceReleased != true && (state.imported == true || state.record.draftID != nil) {
            try retainIdentity(state)
        } else { try execute("DELETE FROM operations WHERE id=?", id: id) }
    }

    /// Called after the source rewrite commits. A crash before this call retains
    /// only a digest; a crash before the rewrite still cannot resurrect work.
    func releaseSources(requests: [String], draftID: String?) throws {
        try transaction {
            var ids = Set(requests)
            if let draftID {
                for data in try rows("SELECT state FROM operations WHERE json_extract(CAST(state AS TEXT),'$.record.draftID')=?", id: draftID) {
                    ids.insert(try JSONDecoder().decode(State.self, from: data).record.id)
                }
                try execute("DELETE FROM retired_operations WHERE json_extract(CAST(state AS TEXT),'$.draftID')=?", id: draftID)
            }
            for id in ids {
                if var state = try state(id) {
                    state.sourceReleased = true
                    try update(state)
                }
                try execute("DELETE FROM retired_operations WHERE id=?", id: id)
            }
        }
    }

    func retainCreationSource(draftID: String?, before: Date) throws {
        try transaction {
            // A phone has one editor file. Once it names a new revision, older
            // editor and imported-file identities have no remaining source.
            for data in try rows("SELECT state FROM retired_operations") {
                let identity = try JSONDecoder().decode(RetiredIdentity.self, from: data)
                if identity.retiredAt <= before && (identity.draftID == nil || identity.draftID != draftID) {
                    try execute("DELETE FROM retired_operations WHERE id=?", id: identity.id)
                }
            }
            for data in try rows("SELECT state FROM operations WHERE (json_extract(CAST(state AS TEXT),'$.imported')=1 OR json_extract(CAST(state AS TEXT),'$.record.draftID') IS NOT NULL) AND json_extract(CAST(state AS TEXT),'$.sourceReleased') IS NOT 1") {
                var state = try JSONDecoder().decode(State.self, from: data)
                let record = state.record
                guard record.submittedAt <= before, record.draftID == nil || record.draftID != draftID else { continue }
                state.sourceReleased = true
                try update(state)
            }
        }
    }

    func retainQueries(machine: String, keys: Set<String>) throws {
        let ids = Set(keys.map { queryID(machine: machine, query: $0) })
        guard retainedQueries[machine] != ids else { return }
        let prefix = queryID(machine: machine, query: "")
        // Read IDs only: old membership bodies are not decoded on navigation.
        // Existing transferred effects always have a persisted reload floor.
        try transaction {
            let stored = try rows("SELECT CAST(id AS BLOB) FROM work_queries WHERE id>=?1 AND id<(?1 || char(1114111)) AND json_array_length(CAST(state AS TEXT),'$.effects')>0", id: prefix)
            for data in stored {
                let id = String(decoding: data, as: UTF8.self)
                if !ids.contains(id) { try execute("DELETE FROM work_queries WHERE id=?", id: id) }
            }
        }
        retainedQueries[machine] = ids
    }

    func currentPlanning(_ state: State) throws -> OperationPlanningReading? {
        guard state.receiptUnavailable != true,
              let epoch = try activeEpoch(machine: state.record.destination), state.receiptEpoch == epoch,
              let reading = state.receipt?.planning,
              reading.fence.machine == state.record.destination,
              reading.fence.epoch == epoch,
              reading.fence.schema == ServerProtocol.revision else { return nil }
        return reading
    }

    private func requirements(_ machine: String) throws -> [QueryRequirement] {
        guard let data = try rows("SELECT state FROM work_requirements WHERE id=?", id: machine).first else { return [] }
        // Upgrade the previous Machine-wide floors in place. Missing scope
        // retains its conservative meaning until a new epoch replaces it.
        if let previous = try? JSONDecoder().decode([CommitFence].self, from: data) {
            return previous.map { QueryRequirement(fence: $0, directory: nil, kind: nil) }
        }
        return try JSONDecoder().decode([QueryRequirement].self, from: data)
    }

    func cache(_ reading: ClientWorkQuery, key: String) throws -> ClientWorkQuery? {
        let stamp = reading.stamp
        guard stamp.complete, stamp.schema == ServerProtocol.revision, stamp.query != nil else {
            throw RegistryQueryError("A confirmed Work base requires a complete stamped query")
        }
        return try transaction {
            guard try observeMachineEpoch(stamp.epoch, machine: stamp.machine) else {
                return try cached(machine: stamp.machine, key: key)
            }
            let id = queryID(machine: stamp.machine, query: key)
            let previous = try queryState(id)
            if let previous, previous.stamp.epoch == stamp.epoch {
                let domains = Set((previous.plans + reading.plans).flatMap { $0.membership.domains })
                if !stamp.covers(previous.stamp, domains: domains) { return try cached(machine: stamp.machine, key: key) }
            }
            var reading = reading
            if previous?.stamp.epoch == stamp.epoch {
                reading.effects = previous?.effects?.filter { !$0.covered(by: stamp, plans: reading.plans) }
            }
            try execute("INSERT OR REPLACE INTO work_queries(id,state) VALUES(?,?)", id: id,
                data: JSONEncoder().encode(reading))
            return try cached(machine: stamp.machine, key: key)
        }
    }

    func cached(machine: String, key: String) throws -> ClientWorkQuery? {
        guard var reading = try queryState(queryID(machine: machine, query: key)), try usable(reading) else { return nil }
        var effects = reading.effects ?? []
        for record in try pending() where record.destination == machine {
            guard let state = try state(record.id), let effect = try membershipEffect(state),
                  !effect.covered(by: reading.stamp, plans: reading.plans) else { continue }
            effects.removeAll { $0.operationID == effect.operationID }
            effects.append(effect)
        }
        reading.effects = effects
        reading.prepareProjection()
        return reading
    }

    private func usable(_ reading: ClientWorkQuery) throws -> Bool {
        guard let query = reading.stamp.query,
              try machineEpochs(reading.stamp.machine)?.current == reading.stamp.epoch else { return false }
        return try requirements(reading.stamp.machine).filter { $0.fence.epoch == reading.stamp.epoch }.allSatisfy {
            guard let fence = $0.requiredFence(in: reading) else { return true }
            return fence.isCovered(by: reading.stamp, query: query) || effectsCover(fence, in: reading)
        }
    }

    private func membershipEffect(_ state: State) throws -> WorkQueryEffect? {
        let command = try JSONDecoder().decode(JSONValue.self, from: state.record.payload)
        guard case .object(let fields) = command, fields["kind"] == .string("task.complete"),
              case .string(let directory) = fields["directory"],
              case .string(let issue) = fields["issue"] else { return nil }
        let sequence = try rows("SELECT CAST(sequence AS BLOB) FROM operations WHERE id=?", id: state.record.id).first
            .flatMap { Int64(String(decoding: $0, as: UTF8.self)) }
        guard let sequence else { return nil }
        let receipt = state.receiptEpoch == (try activeEpoch(machine: state.record.destination)) ? state.receipt : nil
        let resolution = receipt?.commits.first { $0.stage == "task.completed" }
        return WorkQueryEffect(operationID: state.record.id, sequence: sequence, directory: directory,
            kind: "task.complete", issue: issue, resolution: resolution)
    }

    private func effectsCover(_ fence: CommitFence, in reading: ClientWorkQuery) -> Bool {
        // Every domain in the reload floor must be covered by this base or an
        // effect with its exact semantic resolution. A pending request is not
        // evidence that an arbitrary later frontier is safe.
        fence.revisions.allSatisfy { domain, revision in
            let single = CommitFence(machine: fence.machine, epoch: fence.epoch, schema: fence.schema, revisions: [domain: revision])
            if let query = reading.stamp.query, single.isCovered(by: reading.stamp, query: query) { return true }
            return (reading.effects ?? []).contains {
                guard let resolution = $0.resolution else { return false }
                return resolution.fence.machine == fence.machine && resolution.fence.epoch == fence.epoch
                    && resolution.fence.schema == fence.schema && (resolution.fence.revisions[domain] ?? -1) >= revision
            }
        }
    }

    func forgetQueries(machine: String) throws {
        let prefix = queryID(machine: machine, query: "")
        try execute("DELETE FROM work_queries WHERE substr(id,1,length(?1))=?1", id: prefix)
    }

    private func queryID(machine: String, query: String) -> String { "\(machine.utf8.count):\(machine)\(query)" }

    private func queryState(_ id: String) throws -> ClientWorkQuery? {
        try rows("SELECT state FROM work_queries WHERE id=?", id: id).first
            .map { try JSONDecoder().decode(ClientWorkQuery.self, from: $0) }
    }

    func activeEpoch(machine: String) throws -> String? { try machineEpochs(machine)?.current }

    private func machineEpochs(_ machine: String) throws -> Epochs? {
        try rows("SELECT state FROM work_epochs WHERE id=?", id: machine).first
            .map { try JSONDecoder().decode(Epochs.self, from: $0) }
    }

    private func observeMachineEpoch(_ epoch: String, machine: String) throws -> Bool {
        let previous = try machineEpochs(machine)
        if previous?.current == epoch { return true }
        // Import the saved receipt cursors' known departures when introducing
        // the Machine-wide epoch owner. No submitted command is changed.
        let historical: [String]
        if previous == nil {
            historical = try rows("SELECT state FROM operations").map {
                try JSONDecoder().decode(State.self, from: $0)
            }.filter { $0.record.destination == machine }.flatMap { $0.departedEpochs ?? [] }
        } else { historical = [] }
        var epochs = previous ?? Epochs(current: epoch, departed: Set(historical))
        guard !epochs.departed.contains(epoch) else { return false }
        if epochs.current != epoch {
            epochs.departed.insert(epochs.current)
            epochs.current = epoch
        }
        try execute("INSERT OR REPLACE INTO work_epochs(id,state) VALUES(?,?)", id: machine,
            data: JSONEncoder().encode(epochs))
        let activeRequirements = try requirements(machine).filter { $0.fence.epoch == epoch }
        try execute("INSERT OR REPLACE INTO work_requirements(id,state) VALUES(?,?)", id: machine,
            data: JSONEncoder().encode(activeRequirements))
        return true
    }

    private func transaction<T>(_ body: () throws -> T) throws -> T {
        try execute("BEGIN IMMEDIATE")
        do {
            let result = try body()
            try execute("COMMIT")
            return result
        } catch {
            try? execute("ROLLBACK")
            throw error
        }
    }

    private func update(_ state: State) throws {
        try execute("UPDATE operations SET state=?2 WHERE id=?1", id: state.record.id, data: JSONEncoder().encode(state))
    }

    private func statement(_ sql: String, id: String?, data: Data?) throws -> OpaquePointer {
        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(connection, sql, -1, &statement, nil) == SQLITE_OK, let statement else { throw failure() }
        // SQLite copies each binding before the temporary Swift buffer expires.
        let copy = unsafeBitCast(-1, to: sqlite3_destructor_type.self)
        if let id, sqlite3_bind_text(statement, 1, id, -1, copy) != SQLITE_OK {
            sqlite3_finalize(statement)
            throw failure()
        }
        if let data {
            let code = data.withUnsafeBytes { sqlite3_bind_blob(statement, 2, $0.baseAddress, Int32($0.count), copy) }
            guard code == SQLITE_OK else { sqlite3_finalize(statement); throw failure() }
        }
        return statement
    }

    private func execute(_ sql: String, id: String? = nil, data: Data? = nil) throws {
        let statement = try statement(sql, id: id, data: data)
        defer { sqlite3_finalize(statement) }
        var code = sqlite3_step(statement)
        while code == SQLITE_ROW { code = sqlite3_step(statement) }
        guard code == SQLITE_DONE else { throw failure() }
    }

    private func rows(_ sql: String, id: String? = nil) throws -> [Data] {
        let statement = try statement(sql, id: id, data: nil)
        defer { sqlite3_finalize(statement) }
        var result: [Data] = []
        var code = sqlite3_step(statement)
        while code == SQLITE_ROW {
            result.append(Data(bytes: sqlite3_column_blob(statement, 0), count: Int(sqlite3_column_bytes(statement, 0))))
            code = sqlite3_step(statement)
        }
        guard code == SQLITE_DONE else { throw failure() }
        return result
    }

    private func failure() -> RegistryQueryError { RegistryQueryError(String(cString: sqlite3_errmsg(connection))) }
}
