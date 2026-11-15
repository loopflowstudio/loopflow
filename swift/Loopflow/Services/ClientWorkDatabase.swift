import Foundation
import SQLite3

/// Confined to ClientWorkStore's actor. One transaction imports released journal
/// files, retaining them until commit. A crash can repeat import without loss.
final class ClientWorkDatabase {
    struct State: Codable {
        let record: SubmittedOperation
        var sentEpoch: String?
        var receiptEpoch: String?
        var observedEpoch: String?
        var receipt: OperationReceipt?
        var presentedAt: Date?
        var receiptUnavailable: Bool?
        var departedEpochs: [String]?
    }
    private let connection: OpaquePointer

    init(directory: URL) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
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
        guard var state = try state(id) else { return false }
        if state.observedEpoch == epoch { return true }
        guard !(state.departedEpochs ?? []).contains(epoch) else { return false }
        if let previous = state.observedEpoch {
            state.departedEpochs = (state.departedEpochs ?? []) + [previous]
        }
        state.observedEpoch = epoch
        state.receiptUnavailable = true
        try update(state)
        return true
    }

    func unavailable(_ id: String) throws {
        guard var state = try state(id) else { return }
        state.receiptUnavailable = true
        try update(state)
    }

    func presented(_ id: String) throws {
        guard var state = try state(id) else { return }
        guard state.receipt?.result != nil || state.receipt?.commits.contains(where: { $0.result != .null }) == true else {
            throw RegistryQueryError("No saved result is available to present")
        }
        guard state.presentedAt == nil else { return }
        state.presentedAt = Date()
        try update(state)
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
