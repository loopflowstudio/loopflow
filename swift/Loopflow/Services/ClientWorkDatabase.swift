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
    }
    private struct Epochs: Codable {
        var current: String
        var departed: Set<String>
    }
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
        try execute("CREATE TABLE IF NOT EXISTS work_queries (id TEXT PRIMARY KEY, state BLOB NOT NULL)")
        try execute("CREATE TABLE IF NOT EXISTS work_epochs (id TEXT PRIMARY KEY, state BLOB NOT NULL)")
        try execute("CREATE TABLE IF NOT EXISTS work_requirements (id TEXT PRIMARY KEY, state BLOB NOT NULL)")
        let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "json" }
        let records = try files.map { try JSONDecoder().decode(SubmittedOperation.self, from: Data(contentsOf: $0)) }
            .sorted { ($0.submittedAt, $0.id) < ($1.submittedAt, $1.id) }
        try execute("BEGIN IMMEDIATE")
        do {
            for record in records {
                // Released journals did not record a send epoch. Absence after
                // migration cannot prove that their effect never happened.
                try save(record, sentEpoch: "")
            }
            try execute("COMMIT")
        } catch {
            try? execute("ROLLBACK")
            throw error
        }
        for file in files { try FileManager.default.removeItem(at: file) }
    }

    deinit { sqlite3_close(connection) }

    func pending() throws -> [SubmittedOperation] {
        try rows("SELECT state FROM operations ORDER BY sequence").map { try JSONDecoder().decode(State.self, from: $0).record }
    }

    func state(_ id: String) throws -> State? {
        try rows("SELECT state FROM operations WHERE id=?", id: id).first.map { try JSONDecoder().decode(State.self, from: $0) }
    }

    func save(_ record: SubmittedOperation, sentEpoch: String? = nil) throws {
        if let existing = try state(record.id) {
            guard try existing.record.hasSameCommand(as: record) else {
                throw RegistryQueryError("This operation identity already has another submitted command")
            }
            return
        }
        let state = State(record: record, sentEpoch: sentEpoch, receiptEpoch: nil, observedEpoch: nil, receipt: nil)
        try execute("INSERT INTO operations(id,state) VALUES(?,?)", id: record.id, data: JSONEncoder().encode(state))
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
        var requirements = try requirements(state.record.destination)
        for resolution in resolutions {
            let fence = resolution.fence
            guard fence.machine == state.record.destination, fence.epoch == epoch,
                  fence.schema == ServerProtocol.revision, !fence.revisions.isEmpty else { return }
            // One revision frontier per lifetime, independent of the number
            // of completed commands or queries ever opened.
            if let index = requirements.firstIndex(where: {
                $0.machine == fence.machine && $0.epoch == fence.epoch && $0.schema == fence.schema
            }) {
                var revisions = requirements[index].revisions
                for (domain, revision) in fence.revisions { revisions[domain] = max(revisions[domain] ?? 0, revision) }
                requirements[index] = CommitFence(machine: fence.machine, epoch: fence.epoch,
                    schema: fence.schema, revisions: revisions)
            } else { requirements.append(fence) }
        }
        try execute("INSERT OR REPLACE INTO work_requirements(id,state) VALUES(?,?)",
            id: state.record.destination, data: JSONEncoder().encode(requirements))
        try execute("DELETE FROM operations WHERE id=?", id: id)
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

    private func requirements(_ machine: String) throws -> [CommitFence] {
        try rows("SELECT state FROM work_requirements WHERE id=?", id: machine).first
            .map { try JSONDecoder().decode([CommitFence].self, from: $0) } ?? []
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
            if let previous = try queryState(id), previous.stamp.epoch == stamp.epoch,
               !stamp.covers(previous.stamp) { return try cached(machine: stamp.machine, key: key) }
            try execute("INSERT OR REPLACE INTO work_queries(id,state) VALUES(?,?)", id: id,
                data: JSONEncoder().encode(reading))
            return try cached(machine: stamp.machine, key: key)
        }
    }

    func cached(machine: String, key: String) throws -> ClientWorkQuery? {
        guard let reading = try queryState(queryID(machine: machine, query: key)), let query = reading.stamp.query else { return nil }
        let epochs = try machineEpochs(machine)
        guard epochs?.current == reading.stamp.epoch,
              try requirements(machine).filter({ $0.epoch == reading.stamp.epoch }).allSatisfy({
                  $0.isCovered(by: reading.stamp, query: query)
              }) else { return nil }
        return reading
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
        let activeRequirements = try requirements(machine).filter { $0.epoch == epoch }
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
