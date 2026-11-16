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
    static let retainedEffectLimit = 64
    private var retainedQueries: [String: Set<String>] = [:]
    // Coverage is connection-local. Reopening SQLite or reconnecting, even to
    // the same source, requires another captured reading for external fields.
    private var fieldConnections: [String: UUID] = [:]
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
        try execute("""
            CREATE INDEX IF NOT EXISTS operations_field_stages ON operations(json_extract(CAST(state AS TEXT),'$.record.destination'))
            WHERE json_extract(CAST(state AS TEXT),'$.receipt.validation')='bound'
            """)
        try execute("DROP INDEX IF EXISTS operations_sources")
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
        try execute("CREATE TABLE IF NOT EXISTS session_names (id TEXT PRIMARY KEY, state BLOB NOT NULL)")
        try execute("CREATE INDEX IF NOT EXISTS session_names_machine ON session_names(json_extract(CAST(state AS TEXT),'$.fence.machine'))")
        try execute("CREATE TABLE IF NOT EXISTS work_epochs (id TEXT PRIMARY KEY, state BLOB NOT NULL)")
        try execute("CREATE TABLE IF NOT EXISTS work_requirements (id TEXT PRIMARY KEY, state BLOB NOT NULL)")
        try execute("CREATE TABLE IF NOT EXISTS work_contents (id TEXT PRIMARY KEY, state BLOB NOT NULL)")
        try execute("CREATE TABLE IF NOT EXISTS work_fields (id TEXT PRIMARY KEY, state BLOB NOT NULL)")
        try execute("CREATE INDEX IF NOT EXISTS work_fields_resource ON work_fields(json_extract(CAST(state AS TEXT),'$.work.id'))")
        // Confirmed caches are replaceable. The outbox and its reload floors
        // survive a membership vocabulary change; never reinterpret an old plan.
        try execute("DELETE FROM work_queries WHERE EXISTS (SELECT 1 FROM json_each(CAST(work_queries.state AS TEXT),'$.plans') WHERE json_type(value,'$.membership.domains') IS NULL)")
        // Old cache stamps cannot acquire evidence from a later connection.
        // Keep exact outbox requests, receipts and reload requirements intact.
        try execute("DELETE FROM work_queries WHERE json_type(CAST(state AS TEXT),'$.stamp.observations') IS NULL")
        // Aggregate lifecycle caches cannot recover the inputs terminal state masked.
        // Keep exact commands, receipts and reload floors while replacing cached bodies.
        try execute("""
            DELETE FROM work_queries WHERE EXISTS (
                SELECT 1 FROM json_each(CAST(work_queries.state AS TEXT),'$.plans') AS plan,
                    json_each(plan.value,'$.membership.facts') AS fact
                WHERE json_type(fact.value,'$.parent') IS NOT 'object'
                    OR json_type(fact.value,'$.activity') IS NOT 'object'
                    OR json_type(fact.value,'$.ownership') IS NOT 'object'
                    OR json_type(fact.value,'$.publication') IS NOT 'object'
                    OR json_type(fact.value,'$.execution') IS NOT 'object'
                    OR json_type(fact.value,'$.disposition') IS NOT 'object'
                    OR json_type(fact.value,'$.disposition.value.completed_at') IS NULL)
            """)
        try transaction {
            // An aggregate cannot reconstruct masked inputs. Its support still
            // bounds the first replacement reading, so an older body cannot
            // revive an already-incorporated effect after this cache upgrade.
            var replacements: [String: [WorkQueryRequirement]] = [:]
            for data in try rows("SELECT CAST(json_extract(CAST(state AS TEXT),'$.lifecycle.support') AS BLOB) FROM work_fields WHERE json_type(CAST(state AS TEXT),'$.lifecycle.support')='object'") {
                guard let support = try? JSONDecoder().decode(WorkMembershipPlan.FieldSupport.self, from: data),
                      support.schema == ServerProtocol.revision, !support.revisions.isEmpty else { continue }
                let floor = WorkQueryRequirement(fence: CommitFence(machine: support.machine, epoch: support.epoch,
                    schema: support.schema, revisions: support.revisions), directory: nil, kind: nil)
                if replacements[support.machine] == nil { replacements[support.machine] = try requirements(support.machine) }
                var floors = replacements[support.machine] ?? []
                guard !floors.contains(where: { $0.covers(floor) }) else { continue }
                floors.removeAll { floor.covers($0) }
                floors.append(floor)
                replacements[support.machine] = floors
            }
            for (machine, floors) in replacements {
                try execute("INSERT OR REPLACE INTO work_requirements(id,state) VALUES(?,?)", id: machine, data: JSONEncoder().encode(floors))
            }
            // Parent facts and their incorporation survive. Old activity had
            // already suppressed terminal Tasks; only a new raw capture replaces it.
            try execute("UPDATE work_fields SET state=json_remove(CAST(state AS TEXT),'$.lifecycle','$.activity') WHERE json_type(CAST(state AS TEXT),'$.lifecycle') IS NOT NULL")
            try execute("UPDATE work_fields SET state=json_remove(CAST(state AS TEXT),'$.activity') WHERE json_extract(CAST(state AS TEXT),'$.activity.support.derivation')='work.activity/1'")
            try execute("UPDATE work_fields SET state=json_remove(CAST(state AS TEXT),'$.disposition') WHERE json_type(CAST(state AS TEXT),'$.disposition')='object' AND json_type(CAST(state AS TEXT),'$.disposition.value.value.completed_at') IS NULL")
        }
        // Fixed-candidate programs cannot support arrivals or acquire new proof.
        try execute("DELETE FROM work_queries WHERE EXISTS (SELECT 1 FROM json_tree(CAST(state AS TEXT),'$.plans') WHERE key='kind' AND value='state' AND json_type(CAST(state AS TEXT),substr(fullkey,1,length(fullkey)-5) || '.members') IS NOT NULL)")
        let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "json" }
        let records = try files.map { try JSONDecoder().decode(SubmittedOperation.self, from: Data(contentsOf: $0)) }
            .sorted { ($0.submittedAt, $0.id) < ($1.submittedAt, $1.id) }
        try importLegacy(records.map { LegacyOperation(record: $0, outcome: nil) })
        for file in files { try FileManager.default.removeItem(at: file) }
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
        let accepted = state.receipt ?? receipt
        try transaction {
            try update(state)
            if accepted.validation == .bound, let declaration = OperationDeclaration(kind: accepted.kind) {
                for commit in accepted.commits where declaration.stages.contains(commit.stage) {
                    for field in declaration.fields { try incorporate(commit, field: field, machine: state.record.destination) }
                }
            }
            for commit in accepted.commits where accepted.kind == "session.rename" && accepted.validation == .bound
                && commit.stage == "session.renamed" && commit.resolution == .committed {
                guard commit.fence.machine == state.record.destination, commit.fence.epoch == epoch,
                      case .string(let session) = commit.result["id"],
                      case .string(let title) = commit.result["title"],
                      case .string(let source) = commit.result["title_source"],
                      let source = SessionTitleSource(rawValue: source) else {
                    throw RegistryQueryError("The Session rename result is missing its name or provenance")
                }
                let command = try JSONDecoder().decode(JSONValue.self, from: state.record.payload)
                guard case .string(let target) = command["id"], StructuredIdentity.session(target) == session,
                      commit.resources.contains(session) else {
                    throw RegistryQueryError("The Session rename result names another Session")
                }
                try receiveName(SessionNameFact(id: session, title: title, source: source, fence: commit.fence))
            }
        }
    }

    func earlierResourcePending(_ record: SubmittedOperation) throws -> Bool {
        let command = try JSONDecoder().decode(JSONValue.self, from: record.payload)
        let rename = command["kind"] == .string("session.rename")
        guard case .string(let resource) = command[rename ? "id" : "issue"] else { return false }
        let epoch = try activeEpoch(machine: record.destination)
        for earlier in try pending() {
            if earlier.id == record.id { break }
            guard earlier.destination == record.destination else { continue }
            let prior = try JSONDecoder().decode(JSONValue.self, from: earlier.payload)
            let sameResource: Bool
            if let resource = record.resource, let previous = earlier.resource { sameResource = resource == previous }
            else {
                sameResource = rename
                    ? prior["kind"] == .string("session.rename") && prior["id"] == .string(resource)
                    : prior["directory"] == command["directory"] && prior["issue"] == .string(resource)
            }
            guard sameResource, let state = try state(earlier.id) else { continue }
            if state.receiptUnavailable == true || state.receiptEpoch != epoch { return true }
            guard let receipt = state.receipt, receipt.validation == .bound,
                  OperationDeclaration(kind: receipt.kind)?.resolutions(in: receipt) != nil else { return true }
        }
        return false
    }

    func receiveNames(_ records: [SessionRecord], stamp: ReadingStamp) throws {
        try transaction {
            guard stamp.schema == ServerProtocol.revision,
                  try observeMachineEpoch(stamp.epoch, machine: stamp.machine) else { return }
            let fence = CommitFence(machine: stamp.machine, epoch: stamp.epoch, schema: stamp.schema,
                revisions: ["sessions": stamp.revisions.sessions])
            for record in records where record.titleSource != .unavailable {
                try receiveName(SessionNameFact(id: record.id, title: record.title, source: record.titleSource, fence: fence))
            }
        }
    }

    private func receiveName(_ fact: SessionNameFact) throws {
        guard fact.fence.schema == ServerProtocol.revision, fact.fence.revisions["sessions"] != nil,
              try activeEpoch(machine: fact.fence.machine) == fact.fence.epoch else { return }
        let key = queryID(machine: fact.fence.machine, query: fact.id)
        if let previous = try rows("SELECT state FROM session_names WHERE id=?", id: key).first
            .map({ try JSONDecoder().decode(SessionNameFact.self, from: $0) }),
           previous.fence.epoch == fact.fence.epoch, !fact.covers(previous.fence) { return }
        try execute("INSERT OR REPLACE INTO session_names(id,state) VALUES(?,?)", id: key, data: JSONEncoder().encode(fact))
    }

    func names(machine: String) throws -> [String: SessionNameFact] {
        let epoch = try activeEpoch(machine: machine)
        let facts = try rows("SELECT state FROM session_names WHERE json_extract(CAST(state AS TEXT),'$.fence.machine')=?", id: machine)
            .map { try JSONDecoder().decode(SessionNameFact.self, from: $0) }.filter { $0.fence.epoch == epoch }
        return Dictionary(facts.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
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
                      !reading.plans.isEmpty else { continue }
                guard try usable(reading), (reading.effects ?? []).count < Self.retainedEffectLimit else {
                    try execute("DELETE FROM work_queries WHERE id=?", id: key)
                    continue
                }
                var effects = reading.effects ?? []
                effects.removeAll { $0.operationID == effect.operationID }
                if try !incorporated(effect, machine: state.record.destination) { effects.append(effect) }
                reading.effects = effects
                var acknowledged = reading.reconciledRequirements ?? []
                for resolution in resolutions {
                    let requirement = WorkQueryRequirement(fence: resolution.fence, directory: effect.directory, kind: effect.kind)
                    acknowledged.removeAll { requirement.covers($0) }
                    acknowledged.append(requirement)
                }
                reading.reconciledRequirements = acknowledged
                try execute("UPDATE work_queries SET state=?2 WHERE id=?1", id: key, data: JSONEncoder().encode(reading))
            }
        }
        var requirements = try requirements(state.record.destination)
        // Session names have one normalized field owner; the committed result
        // already joined it in the receipt transaction. No membership reload.
        for resolution in resolutions where receipt.kind != "session.rename" && resolution.resolution != .definitelyRefused {
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
            } else { requirements.append(WorkQueryRequirement(fence: fence, directory: membershipEffect?.directory, kind: membershipEffect?.kind)) }
        }
        try execute("INSERT OR REPLACE INTO work_requirements(id,state) VALUES(?,?)",
            id: state.record.destination, data: JSONEncoder().encode(requirements))
        if state.imported == true || state.record.draftID != nil {
            try retainIdentity(state)
        } else { try execute("DELETE FROM operations WHERE id=?", id: id) }
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

    private func requirements(_ machine: String) throws -> [WorkQueryRequirement] {
        guard let data = try rows("SELECT state FROM work_requirements WHERE id=?", id: machine).first else { return [] }
        // Upgrade the previous Machine-wide floors in place. Missing scope
        // retains its conservative meaning until a new epoch replaces it.
        if let previous = try? JSONDecoder().decode([CommitFence].self, from: data) {
            return previous.map { WorkQueryRequirement(fence: $0, directory: nil, kind: nil) }
        }
        return try JSONDecoder().decode([WorkQueryRequirement].self, from: data)
    }

    func cache(_ reading: ClientWorkQuery, key: String,
               observation: MachineConnection.ObservationConnection? = nil) throws -> ClientWorkQuery? {
        let stamp = reading.stamp
        guard stamp.schema == ServerProtocol.revision, stamp.query != nil else {
            throw RegistryQueryError("Work fields require a supported stamped query")
        }
        return try transaction {
            guard try observeMachineEpoch(stamp.epoch, machine: stamp.machine) else {
                return try cached(machine: stamp.machine, key: key, observation: observation)
            }
            if stamp.complete {
                for content in reading.plans.flatMap(\.contents) { try receiveContent(content, stamp: stamp) }
            }
            let stages = try fieldStages(machine: stamp.machine)
            for fact in reading.plans.flatMap({ $0.membership.facts }) {
                try receiveFields(fact, stamp: stamp, observation: observation, stages: stages[fact.work.id] ?? [])
            }
            // Field ingestion is useful even when this body cannot replace the
            // exact-query base. Incompleteness cannot certify filtered absence.
            guard stamp.complete else { return try cached(machine: stamp.machine, key: key, observation: observation) }
            let id = queryID(machine: stamp.machine, query: key)
            let previous = try queryState(id)
            if let previous, previous.stamp.epoch == stamp.epoch {
                let domains = Set((previous.plans + reading.plans).flatMap { $0.membership.domains })
                if !stamp.covers(previous.stamp, domains: domains) { return try cached(machine: stamp.machine, key: key, observation: observation) }
            }
            var reading = reading
            if previous?.stamp.epoch == stamp.epoch {
                reading.effects = try previous?.effects?.filter { try !incorporated($0, machine: stamp.machine) }
                reading.reconciledRequirements = previous?.reconciledRequirements
            }
            try execute("INSERT OR REPLACE INTO work_queries(id,state) VALUES(?,?)", id: id,
                data: JSONEncoder().encode(reading))
            return try cached(machine: stamp.machine, key: key, observation: observation)
        }
    }

    private func fieldID(machine: String, work: WorkWorkspaceKey) throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        return queryID(machine: machine, query: String(decoding: try encoder.encode(work), as: UTF8.self))
    }

    private func fields(_ id: String) throws -> WorkFieldFacts? {
        try rows("SELECT state FROM work_fields WHERE id=?", id: id).first.map {
            try JSONDecoder().decode(WorkFieldFacts.self, from: $0)
        }
    }

    private func receiveFields(_ fact: WorkMembershipPlan.Facts, stamp: ReadingStamp,
                               observation: MachineConnection.ObservationConnection?,
                               stages: [(WorkField, OperationCommit)]) throws {
        let id = try fieldID(machine: stamp.machine, work: fact.work)
        let previous = try fields(id)
        var facts = previous ?? WorkFieldFacts(machine: stamp.machine, work: fact.work)
        facts.parent = join(fact.parent.wave, support: fact.parent.support, previous: facts.parent,
            stamp: stamp, observation: observation, key: id + "/parent")
        var disposition = fact.disposition
        disposition.support = nil
        facts.disposition = join(disposition, support: fact.disposition.support, previous: facts.disposition,
            stamp: stamp, observation: observation, key: id + "/disposition")
        var ownership = fact.ownership
        ownership.support = nil
        facts.ownership = join(ownership, support: fact.ownership.support, previous: facts.ownership,
            stamp: stamp, observation: observation, key: id + "/ownership")
        var publication = fact.publication
        publication.support = nil
        facts.publication = join(publication, support: fact.publication.support, previous: facts.publication,
            stamp: stamp, observation: observation, key: id + "/publication")
        var execution = fact.execution
        execution.support = nil
        facts.execution = join(execution, support: fact.execution.support, previous: facts.execution,
            stamp: stamp, observation: observation, key: id + "/execution")
        facts.activity = join(WorkFieldStates(fact.activity), support: fact.activity.support, previous: facts.activity,
            stamp: stamp, observation: observation, key: id + "/activity")
        for (field, commit) in stages { facts.incorporate(commit, field: field) }
        guard facts != previous, facts.parent != nil || facts.disposition != nil || facts.ownership != nil || facts.publication != nil || facts.execution != nil || facts.activity != nil else { return }
        try execute("INSERT OR REPLACE INTO work_fields(id,state) VALUES(?,?)", id: id, data: JSONEncoder().encode(facts))
    }

    private func join<Value>(_ value: Value, support: WorkMembershipPlan.FieldSupport?,
                             previous: WorkFieldFact<Value>?, stamp: ReadingStamp,
                             observation: MachineConnection.ObservationConnection?, key: String) -> WorkFieldFact<Value>? {
        // Unattributed or departed-link input cannot revoke a current external
        // capture, including when the old producer reported it unavailable.
        if observation == nil, let previous, case .captured = previous.support.observations,
           support?.observations != .sqlite { return previous }
        if let support {
            guard support.belongs(to: stamp) else { return previous }
            if case .captured(let evidence) = support.observations,
               evidence.source != observation?.source { return previous }
        }
        if support == nil, let previous, previous.support.machine == stamp.machine,
           previous.support.epoch == stamp.epoch, previous.support.schema == stamp.schema,
           previous.support.observations == .sqlite {
            let incoming = stamp.fieldFence.revisions
            let older = previous.support.revisions.allSatisfy { domain, revision in incoming[domain].map { $0 <= revision } == true }
                && previous.support.revisions.contains { domain, revision in incoming[domain].map { $0 < revision } == true }
            if older { return previous }
        }
        guard let support, support.belongs(to: stamp), support.isAvailable(on: observation) else {
            fieldConnections[key] = nil
            guard var previous else { return nil }
            previous.needsReconciliation = true
            return previous
        }
        return joinConfirmed(value, support: support, previous: previous, observation: observation, key: key)
    }

    private func joinConfirmed<Value>(_ value: Value, support: WorkMembershipPlan.FieldSupport,
                                      previous: WorkFieldFact<Value>?,
                                      observation: MachineConnection.ObservationConnection?, key: String) -> WorkFieldFact<Value>? {
        if var previous, previous.support.epoch == support.epoch, previous.support.schema == support.schema {
            guard support.covers(previous.support) else {
                // An ordered older reply cannot revoke newer coverage. Crossed
                // or otherwise incomparable supports require reconciliation.
                if !previous.support.covers(support) {
                    previous.reconcile(support)
                }
                return previous
            }
            if support == previous.support && value != previous.value {
                previous.reconcile(support)
                return previous
            }
            if !previous.conflictingSupports.isEmpty,
               support == previous.support || !previous.conflictingSupports.allSatisfy({ support.covers($0) }) {
                // A repeated value cannot resolve a conflict at equal support,
                // nor can a reading cover just one of two crossed supports.
                previous.reconcile(support)
                return previous
            }
        }
        if case .captured = support.observations { fieldConnections[key] = observation?.generation }
        else { fieldConnections[key] = nil }
        return WorkFieldFact(value: value, support: support, needsReconciliation: false,
            incorporated: previous?.incorporated?.mapValues { fences in
                fences.filter { $0.epoch == support.epoch && $0.schema == support.schema }
            })
    }

    private func incorporate(_ commit: OperationCommit, field: WorkField, machine: String) throws {
        guard commit.fence.machine == machine, commit.fence.schema == ServerProtocol.revision,
              try activeEpoch(machine: machine) == commit.fence.epoch else { return }
        for resource in Set(commit.resources) {
            for data in try rows("SELECT state FROM work_fields WHERE json_extract(CAST(state AS TEXT),'$.work.id')=?", id: resource) {
                var facts = try JSONDecoder().decode(WorkFieldFacts.self, from: data)
                guard facts.machine == machine, facts.work.kind == .task else { continue }
                let previous = facts
                if field == .parent, commit.resolution == .committed, case .object(let result) = commit.result,
                   let actual = result["wave_id"], actual == .null || { if case .string = actual { return true }; return false }() {
                    let support = WorkMembershipPlan.FieldSupport(machine: machine, epoch: commit.fence.epoch,
                        schema: commit.fence.schema, derivation: "work.parent/1", revisions: commit.fence.revisions,
                        observations: .sqlite)
                    let parent: String? = { if case .string(let value) = actual { return value }; return nil }()
                    facts.parent = joinConfirmed(parent, support: support, previous: facts.parent,
                        observation: nil, key: try fieldID(machine: machine, work: facts.work) + "/parent")
                }
                facts.incorporate(commit, field: field)
                if facts != previous {
                    try execute("UPDATE work_fields SET state=?2 WHERE id=?1",
                        id: fieldID(machine: machine, work: facts.work), data: JSONEncoder().encode(facts))
                }
            }
        }
    }

    /// Reuse the outbox and retained effects as stage owners. Incorporation is
    /// not another receipt ledger and never turns an unknown send into a retry.
    private func fieldStages(machine: String) throws -> [String: [(WorkField, OperationCommit)]] {
        let epoch = try activeEpoch(machine: machine)
        var stages: [(WorkField, OperationCommit)] = []
        let pendingStages = try rows("""
            SELECT state FROM operations WHERE json_extract(CAST(state AS TEXT),'$.record.destination')=?
                AND json_extract(CAST(state AS TEXT),'$.receipt.validation')='bound'
            """, id: machine)
        for data in pendingStages {
            let state = try JSONDecoder().decode(State.self, from: data)
            guard state.receiptEpoch == epoch,
                  state.receiptUnavailable != true, let receipt = state.receipt, receipt.validation == .bound,
                  let declaration = OperationDeclaration(kind: receipt.kind) else { continue }
            for field in declaration.fields { stages.append(contentsOf: receipt.commits.filter { declaration.stages.contains($0.stage) }.map { (field, $0) }) }
        }
        let prefix = queryID(machine: machine, query: "")
        for data in try rows("SELECT state FROM work_queries WHERE id>=?1 AND id<(?1 || char(1114111)) AND json_array_length(CAST(state AS TEXT),'$.effects')>0", id: prefix) {
            let query = try JSONDecoder().decode(ClientWorkQuery.self, from: data)
            for effect in query.effects ?? [] {
                guard let declaration = OperationDeclaration(kind: effect.kind),
                      let commit = effect.resolution, declaration.stages.contains(commit.stage),
                      commit.fence.machine == machine, commit.fence.epoch == epoch else { continue }
                for field in declaration.fields { stages.append((field, commit)) }
            }
        }
        var resources: [String: [(WorkField, OperationCommit)]] = [:]
        for (field, commit) in stages {
            for resource in Set(commit.resources) { resources[resource, default: []].append((field, commit)) }
        }
        return resources
    }

    func fieldReading(machine: String, work: WorkWorkspaceKey,
                      observation: MachineConnection.ObservationConnection? = nil) throws -> WorkFieldReading? {
        let id = try fieldID(machine: machine, work: work)
        guard let facts = try fields(id), let epoch = try activeEpoch(machine: machine) else { return nil }
        func current<Value>(_ fact: WorkFieldFact<Value>?, _ field: String) -> Bool {
            guard let fact, !fact.needsReconciliation, fact.support.epoch == epoch,
                  fact.support.schema == ServerProtocol.revision else { return false }
            switch fact.support.observations {
            case .sqlite: return true
            case .unavailable: return false
            case .captured:
                return fact.support.isAvailable(on: observation)
                    && observation.map { fieldConnections[id + "/" + field] == $0.generation } == true
            }
        }
        return WorkFieldReading(facts: facts, parentCurrent: current(facts.parent, "parent"),
            dispositionCurrent: facts.disposition?.value.known == true && current(facts.disposition, "disposition"),
            ownershipCurrent: facts.ownership?.value.known == true && current(facts.ownership, "ownership"),
            publicationCurrent: facts.publication?.value.known == true && current(facts.publication, "publication"),
            executionCurrent: facts.execution?.value.known == true && current(facts.execution, "execution"),
            activityCurrent: facts.activity?.value.known == true && current(facts.activity, "activity"))
    }

    func cached(machine: String, key: String,
                observation: MachineConnection.ObservationConnection? = nil) throws -> ClientWorkQuery? {
        let id = queryID(machine: machine, query: key)
        guard var reading = try queryState(id), try usable(reading) else { return nil }
        var persisted = reading
        let previous = reading.answer
        var effects = reading.effects ?? []
        for record in try pending() where record.destination == machine {
            guard let state = try state(record.id), let effect = try membershipEffect(state) else { continue }
            effects.removeAll { $0.operationID == effect.operationID }
            effects.append(effect)
        }
        reading.effects = try effects.filter { try !incorporated($0, machine: machine) }
        var fields: [WorkWorkspaceKey: WorkFieldReading] = [:]
        for work in Set(reading.plans.flatMap { $0.membership.facts.map(\.work) }) {
            fields[work] = try fieldReading(machine: machine, work: work, observation: observation)
        }
        var contents: [WorkWorkspaceKey: WorkViewPlan.Content] = [:]
        var unavailableContent = Set<WorkWorkspaceKey>()
        for key in Set(reading.plans.flatMap { $0.membership.facts.map(\.work) }) {
            if let fact = try content(machine: machine, work: key, epoch: reading.stamp.epoch) {
                contents[key] = fact.value
                if !fact.conflicts.isEmpty { unavailableContent.insert(key) }
            }
        }
        reading.prepareProjection(fields: fields, contents: contents, unavailableContent: unavailableContent)
        // Rows and their consumed coverage are one replaceable cache value.
        // Recompute after restart and on connection changes; a persisted capture
        // never becomes current merely because its observer returned.
        if reading.answer != previous {
            persisted.answer = reading.answer
            try execute("UPDATE work_queries SET state=?2 WHERE id=?1", id: id, data: JSONEncoder().encode(persisted))
        }
        return reading
    }

    private func usable(_ reading: ClientWorkQuery) throws -> Bool {
        guard let query = reading.stamp.query,
              try machineEpochs(reading.stamp.machine)?.current == reading.stamp.epoch else { return false }
        return try requirements(reading.stamp.machine).filter { $0.fence.epoch == reading.stamp.epoch }.allSatisfy { requirement in
            guard let fence = requirement.requiredFence(in: reading) else { return true }
            return fence.isCovered(by: reading.stamp, query: query)
                || (reading.reconciledRequirements ?? []).contains(where: { $0.covers(requirement) })
        }
    }

    private func membershipEffect(_ state: State) throws -> WorkQueryEffect? {
        let command = try JSONDecoder().decode(JSONValue.self, from: state.record.payload)
        guard case .object(let fields) = command, case .string(let kind) = fields["kind"],
              let declaration = OperationDeclaration(kind: kind), declaration.changesMembership,
              case .string(let directory) = fields["directory"],
              case .string(let issue) = fields["issue"] else { return nil }
        if kind == "task.move", let machine = fields["machine"], machine != .null { return nil }
        let sequence = try rows("SELECT CAST(sequence AS BLOB) FROM operations WHERE id=?", id: state.record.id).first
            .flatMap { Int64(String(decoding: $0, as: UTF8.self)) }
        guard let sequence else { return nil }
        let receipt = state.receiptEpoch == (try activeEpoch(machine: state.record.destination)) ? state.receipt : nil
        let work: WorkWorkspaceKey? = { if case .task(let key) = state.record.resource { return key }; return nil }()
        return declaration.effect(id: state.record.id, sequence: sequence, directory: directory, issue: issue, wave: { if case .string(let wave) = fields["wave"] { return wave }; return nil }(), work: work, receipt: receipt)
    }

    private func incorporated(_ effect: WorkQueryEffect, machine: String) throws -> Bool {
        guard let commit = effect.resolution else { return false }
        if commit.resolution == .definitelyRefused { return true }
        guard let declaration = OperationDeclaration(kind: effect.kind), !declaration.fields.isEmpty, !commit.resources.isEmpty else { return false }
        for resource in commit.resources {
            let records = try rows("SELECT state FROM work_fields WHERE json_extract(CAST(state AS TEXT),'$.work.id')=?", id: resource)
                .map { try JSONDecoder().decode(WorkFieldFacts.self, from: $0) }.filter { $0.machine == machine }
            if !declaration.fields.allSatisfy({ field in records.contains { $0.incorporates(commit, field: field) } }) { return false }
        }
        return true
    }

    private struct ContentFact: Codable {
        let stamp: ReadingStamp
        let value: WorkViewPlan.Content
        var conflicts: [CommitFence]
    }

    private func sameContent(_ first: WorkViewPlan.Content, _ second: WorkViewPlan.Content) -> Bool {
        // Membership fields have their own provenance. Native diagnostics and
        // attachment locations cannot make real Task content disappear.
        first.work == second.work && first.title == second.title && first.identifier == second.identifier
            && first.primarySession == second.primarySession && first.paneRevision == second.paneRevision && first.face == second.face
    }

    private func receiveContent(_ content: WorkViewPlan.Content, stamp: ReadingStamp) throws {
        let id = try fieldID(machine: stamp.machine, work: content.work)
        if let data = try rows("SELECT state FROM work_contents WHERE id=?", id: id).first {
            let previous = try JSONDecoder().decode(ContentFact.self, from: data)
            if previous.stamp.epoch == stamp.epoch {
                let incoming = stamp.fieldFence
                let current = previous.stamp.fieldFence
                let newer = incoming.covers(current)
                let older = current.covers(incoming)
                if !newer && older { return }
                if !newer || (older && !sameContent(previous.value, content))
                    || (!previous.conflicts.isEmpty && (older || !previous.conflicts.allSatisfy { incoming.covers($0) && !$0.covers(incoming) })) {
                    var conflict = previous
                    if !conflict.conflicts.contains(where: { $0.covers(incoming) }) {
                        conflict.conflicts.removeAll { incoming.covers($0) }
                        conflict.conflicts.append(incoming)
                    }
                    try execute("UPDATE work_contents SET state=?2 WHERE id=?1", id: id, data: JSONEncoder().encode(conflict))
                    return
                }
            }
        }
        try execute("INSERT OR REPLACE INTO work_contents(id,state) VALUES(?,?)", id: id,
            data: JSONEncoder().encode(ContentFact(stamp: stamp, value: content, conflicts: [])))
    }

    private func content(machine: String, work: WorkWorkspaceKey, epoch: String) throws -> ContentFact? {
        guard let data = try rows("SELECT state FROM work_contents WHERE id=?", id: fieldID(machine: machine, work: work)).first else { return nil }
        let fact = try JSONDecoder().decode(ContentFact.self, from: data)
        return fact.stamp.epoch == epoch ? fact : nil
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
        let previousConnections = fieldConnections
        try execute("BEGIN IMMEDIATE")
        do {
            let result = try body()
            try execute("COMMIT")
            return result
        } catch {
            try? execute("ROLLBACK")
            fieldConnections = previousConnections
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
