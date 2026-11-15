import Foundation
import SQLite3
import XCTest
@testable import Loopflow

/// The same persisted-query cases run in the Desktop and phone test targets.
@MainActor
final class ClientWorkQueryTests: XCTestCase {
    func stop(_ owner: WorkInteraction) {
        let stopped = owner.shutdown()
        addTeardownBlock { await stopped.value }
    }

    func plan() throws -> WorkViewPlan {
        let url = Bundle(for: Self.self).url(forResource: "view_plan", withExtension: "json")
            ?? URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/view_plan.json")
        return try JSONDecoder().decode(WorkViewPlan.self, from: Data(contentsOf: url))
    }

    func stamp(_ query: String, revision: Int64, epoch: String = "epoch") -> ReadingStamp {
        // All fields, including nullable query, exercise the wire decoder.
        try! JSONDecoder().decode(ReadingStamp.self, from: JSONSerialization.data(withJSONObject: [
            "machine": "machine", "epoch": epoch, "schema": ServerProtocol.revision, "query": query,
            "complete": true, "observations": NSNull(), "revisions": ["planning": revision, "sessions": revision, "graphs": revision,
                "processes": revision, "usage": revision]]))
    }

    func receipt(kind: String = "session.new", stage: String = "session.input_replaced",
                         planning: OperationCommit.PlanningEffect = .none, diagnostic: String? = nil,
                         resolution: OperationCommit.Resolution = .committed) throws -> OperationReceipt {
        let declaration: [String: Any] = ["resolution": resolution.rawValue, "stage": stage, "resources": ["session"],
            "fence": ["machine": "machine", "epoch": "epoch", "schema": ServerProtocol.revision,
                      "revisions": ["planning": 7, "sessions": 7]],
            "result": ["id": "session"], "planning_effect": planning.rawValue, "planning_changes": []]
        return try JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: [
            "request_id": "saved", "kind": kind, "fingerprint": NSNull(), "submitted_fingerprint": "hash",
            "validation": "bound", "delivery": "finished", "revision": 2,
            "result": ["id": "session"], "error": diagnostic as Any? ?? NSNull(),
            "commits": [declaration], "planning": NSNull(), "created_at": 1, "updated_at": 2]))
    }

    func store(_ directory: URL) -> ClientWorkStore {
        let fixture = ClientWorkStore(directory: directory, machine: { "machine" }, read: { _ in "null" }, observe: { _ in
            ServerWatch(updates: AsyncThrowingStream { $0.finish() }, checkpoint: { _ in }, cancel: {})
        }, send: { _ in XCTFail("Cache and retirement must not send commands") })
        addTeardownBlock { await fixture.close() }
        return fixture
    }

    func save(_ directory: URL, receipt: OperationReceipt) throws {
        let database = try ClientWorkDatabase(directory: directory)
        let command: ServedWrite = receipt.kind == "task.update"
            ? .taskUpdate(directory: "/repo", issue: "task", notes: "Original words")
            : .sessionNew(directory: "/repo", requestID: "saved", id: "session", agent: nil)
        try database.save(SubmittedOperation(requestID: "saved", destination: "machine",
            payload: JSONEncoder().encode(command),
            version: ServerProtocol.revision, submittedAt: Date()))
        XCTAssertTrue(try database.observeEpoch("epoch", id: "saved"))
        try database.receive(receipt, epoch: "epoch", id: "saved")
    }

    func testCreationRetiresOnlyAfterItsExactExportResolvesAndItsResultWasPresented() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let url = Bundle(for: Self.self).url(forResource: "operation_creation_planning", withExtension: "json", subdirectory: "server")
            ?? URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("rust/loopflow/tests/fixtures/dto/server/operation_creation_planning.json")
        let result = try XCTUnwrap(try RawJSON(Data(contentsOf: url)).fields()["result"])
        let pending = try result.decode(OperationReceipt.self)
        let fence = try XCTUnwrap(pending.planning?.fence)
        let database = try ClientWorkDatabase(directory: directory)
        try database.save(SubmittedOperation(requestID: pending.requestID, destination: fence.machine,
            payload: JSONEncoder().encode(ServedWrite.taskCreate(directory: "/repo", requestID: pending.requestID,
                title: nil, request: "Original words", wave: nil, graph: nil, session: nil, run: false)),
            version: ServerProtocol.revision, submittedAt: Date()))
        XCTAssertTrue(try database.observeEpoch(fence.epoch, id: pending.requestID))
        try database.receive(pending, epoch: fence.epoch, id: pending.requestID)
        try database.presented(pending.requestID)
        XCTAssertEqual(try database.pending().count, 1)
        XCTAssertEqual(pending.commits[0].planningChanges[0].kind, .taskCreation)
        var resolved = try XCTUnwrap(JSONSerialization.jsonObject(with: result.bytes) as? [String: Any])
        resolved["revision"] = pending.revision + 1
        var status = try XCTUnwrap(resolved["planning"] as? [String: Any])
        var changes = try XCTUnwrap(status["changes"] as? [[String: Any]])
        changes[0]["state"] = "delivered"
        status["changes"] = changes
        resolved["planning"] = status
        let delivered = try JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: resolved))
        XCTAssertEqual(delivered.result, pending.result)
        try database.receive(delivered, epoch: fence.epoch, id: pending.requestID)
        try database.retireIfConverged(pending.requestID)
        XCTAssertTrue(try ClientWorkDatabase(directory: directory).pending().isEmpty)
    }

    func testDeclaredPlanningInapplicabilityClearsThePromptAndRetiresWithReloadFences() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let client = store(directory)
        let base = try plan()
        let list = ServedRead.desktopViewGet(directory: "/repo", expression: "state:working")
        let count = ServedRead.viewSelect(expression: "state:working")
        _ = try await client.receivePlans([base], stamp: stamp("list", revision: 6), for: list)
        _ = try await client.receivePlans([base], stamp: stamp("count", revision: 7), for: count)
        try save(directory, receipt: receipt())
        let recovery = try await client.recovery("saved")
        XCTAssertEqual(recovery?.planning, .inapplicable)
        XCTAssertFalse(recovery?.needsInspection == true)
        let pending = try await client.pending()
        XCTAssertEqual(pending.count, 1) // unseen result remains available
        try await client.presented("saved")
        let retired = try await client.pending()
        XCTAssertTrue(retired.isEmpty)

        let restarted = store(directory)
        let lagging = try await restarted.cachedPlans(machine: "machine", for: list)
        let covered = try await restarted.cachedPlans(machine: "machine", for: count)
        XCTAssertNil(lagging)
        XCTAssertEqual(covered?.plans, [base])
        // A lagging reply, including for a query first opened after retirement,
        // cannot erase the invalidation requirement.
        let rejected = try await restarted.receivePlans([base], stamp: stamp("list", revision: 6), for: list)
        let future = try await restarted.receivePlans([base], stamp: stamp("future", revision: 6), for: .viewSelect(expression: "state:available"))
        XCTAssertNil(rejected)
        XCTAssertNil(future)
        let current = try await restarted.receivePlans([base], stamp: stamp("list", revision: 7), for: list)
        XCTAssertEqual(current?.plans, [base])
    }

    func testAResolvedEditDoesNotRequireAnUnrelatedCreationPresentation() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        try save(directory, receipt: receipt(kind: "task.update", stage: "task.updated"))
        let database = try ClientWorkDatabase(directory: directory)
        try database.retireIfConverged("saved")
        XCTAssertTrue(try database.pending().isEmpty)
    }

    func testMissingCorrelationAndUnclassifiedDiagnosticsRetainTheExactRequest() async throws {
        for (planning, diagnostic) in [(OperationCommit.PlanningEffect.correlated, nil), (.none, "Attachment interrupted")] {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
            try save(directory, receipt: receipt(planning: planning, diagnostic: diagnostic))
            let client = store(directory)
            try await client.presented("saved")
            let restarted = store(directory)
            let pending = try await restarted.pending()
            let recovery = try await restarted.recovery("saved")
            XCTAssertEqual(pending.count, 1)
            XCTAssertEqual(recovery?.resultPresented, true)
            XCTAssertEqual(recovery?.needsInspection, true)
        }
    }

    func testQueryEpochReplacementInvalidatesEveryOperationAndRejectsDepartedRepliesAfterRestart() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        try save(directory, receipt: receipt(planning: .correlated))
        let base = try plan()
        let query = ServedRead.viewSelect(expression: "state:working")
        let client = store(directory)
        try await client.presented("saved")
        _ = try await client.receivePlans([base], stamp: stamp("list", revision: 1, epoch: "replacement"), for: query)
        let restarted = store(directory)
        let recovery = try await restarted.recovery("saved")
        XCTAssertEqual(recovery?.resultPresented, true)
        XCTAssertEqual(recovery?.receiptAvailable, false)
        XCTAssertEqual(recovery?.planning, .unavailable)
        let delayed = try await restarted.receivePlans([base], stamp: stamp("list", revision: 99), for: query)
        XCTAssertEqual(delayed?.stamp.epoch, "replacement")
        let pending = try await restarted.pending()
        XCTAssertEqual(pending.count, 1)
    }

    func testOlderQueryCannotReplaceThePersistedBodyAfterRestart() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let query = ServedRead.viewSelect(expression: "state:working")
        let base = try plan()
        _ = try await store(directory).receivePlans([base], stamp: stamp("list", revision: 12), for: query)
        let delayed = try await store(directory).receivePlans([], stamp: stamp("list", revision: 11), for: query)
        XCTAssertEqual(delayed?.plans, [base])
        XCTAssertEqual(delayed?.stamp.revisions.planning, 12)
    }

    func workingPlan(_ ids: [String], explicit: Bool = false) throws -> WorkViewPlan {
        var body = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(plan())) as? [String: Any])
        let repository = try XCTUnwrap(body["repo"] as? String)
        body["repo_path"] = "/repo"
        func key(_ id: String) -> [String: String] { ["repository": repository, "kind": "task", "id": id] }
        func member(_ id: String, _ source: String) -> [String: Any] { ["source": ["root", source], "work": key(id)] }
        var nodes: [[String: Any]] = ["first", "second"].map { source in
            ["kind": "state", "state": "working", "members": ids.map { member($0, source) }]
        }
        if explicit { nodes.append(["kind": "members", "members": [member("a", "retained")]]) }
        var membership = try XCTUnwrap(body["membership"] as? [String: Any])
        membership["root"] = ["kind": "compose", "items": nodes]
        membership["facts"] = ids.map { ["work": key($0), "name": $0, "parent": ["wave": NSNull(), "support": NSNull()], "lifecycle": ["states": ["open", "working"], "known": true, "support": NSNull()], "activity": ["states": ["available"], "known": true, "support": NSNull()]] as [String: Any] }
        body["membership"] = membership
        let original = try XCTUnwrap((body["contents"] as? [[String: Any]])?.first)
        body["contents"] = ids.map { id in
            var content = original; content["work"] = key(id); content["title"] = id
            content["identifier"] = id; content["states"] = ["open", "working", "available"]
            return content
        }
        // Confirmed wire membership and geometry match the compiled program.
        let compiled = try JSONDecoder().decode(WorkMembershipPlan.self, from: JSONSerialization.data(withJSONObject: membership))
        let members = compiled.resolve().members
        body["resolution"] = try JSONSerialization.jsonObject(with: JSONEncoder().encode(compiled.resolve()))
        body["arrangement"] = try JSONSerialization.jsonObject(with: JSONEncoder().encode(WorkArrangement(order: members, hidden: [], mode: .auto)))
        body["projection"] = ["visible": ids.map { member($0, "first") }, "suppressed": ids.map { ["member": member($0, "second"), "winner": member($0, "first")] }]
        return try JSONDecoder().decode(WorkViewPlan.self, from: JSONSerialization.data(withJSONObject: body))
    }

    func complete(_ directory: URL) throws -> ClientWorkDatabase {
        let database = try ClientWorkDatabase(directory: directory)
        try database.save(SubmittedOperation(requestID: "saved", destination: "machine",
            payload: JSONEncoder().encode(ServedWrite.taskComplete(directory: "/repo", issue: "a")),
            version: ServerProtocol.revision, submittedAt: Date()))
        XCTAssertTrue(try database.observeEpoch("epoch", id: "saved"))
        return database
    }

    private func fieldStamp(_ query: String = "fields", planning: Int64 = 7, sessions: Int64 = 7,
                            complete: Bool = true, epoch: String = "epoch") -> ReadingStamp {
        try! JSONDecoder().decode(ReadingStamp.self, from: JSONSerialization.data(withJSONObject: [
            "machine": "machine", "epoch": epoch, "schema": ServerProtocol.revision, "query": query,
            "complete": complete, "observations": NSNull(), "revisions": ["planning": planning,
                "sessions": sessions, "graphs": 7, "processes": 7, "usage": 7]]))
    }

    private func fieldPlan(_ stamp: ReadingStamp, wave: String? = "a", lifecycle: [WorkSelectionState] = [.working],
                           activity: [WorkSelectionState] = [.busy], clock: UInt64? = 19,
                           source: String = "observer", activityKnown: Bool = true) throws -> WorkViewPlan {
        let plan = try workingPlan(["a"])
        func support(_ derivation: String, _ revisions: [String: Int64],
                     _ observations: WorkMembershipPlan.FieldSupport.Observations) -> WorkMembershipPlan.FieldSupport {
            .init(machine: stamp.machine, epoch: stamp.epoch, schema: stamp.schema,
                derivation: derivation, revisions: revisions, observations: observations)
        }
        let observations: WorkMembershipPlan.FieldSupport.Observations = clock.map {
            .captured(.init(source: source, validatedAt: $0, dependencies: [.init(fact: "process", token: $0)]))
        } ?? .unavailable
        let old = try XCTUnwrap(plan.membership.facts.first)
        var membership = plan.membership
        membership.facts = [.init(work: old.work, name: old.name,
            parent: .init(wave: wave, support: support("work.parent/1", ["planning": stamp.revisions.planning], .sqlite)),
            lifecycle: .init(states: lifecycle, known: true, support: support("work.lifecycle/1",
                ["planning": stamp.revisions.planning, "graphs": 7, "processes": 7], .sqlite)),
            activity: .init(states: activity, known: activityKnown, support: support("work.activity/1",
                ["planning": stamp.revisions.planning, "sessions": stamp.revisions.sessions, "graphs": 7, "processes": 7], observations)))]
        var wire = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(plan)) as? [String: Any])
        wire["membership"] = try JSONSerialization.jsonObject(with: JSONEncoder().encode(membership))
        return try JSONDecoder().decode(WorkViewPlan.self, from: JSONSerialization.data(withJSONObject: wire))
    }

    func testFieldIngestionPrecedesIncompleteAndIncomparableQueryReplacement() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let database = try complete(directory)
        let connection = MachineConnection.ObservationConnection(source: "observer", generation: UUID())
        let held = fieldStamp(sessions: 9)
        let plan = try fieldPlan(held)
        let work = try XCTUnwrap(plan.membership.facts.first?.work)
        _ = try database.cache(ClientWorkQuery(stamp: held, plans: [plan]), key: "held", observation: connection)
        for complete in [false, true] {
            let other = fieldStamp(planning: 9, sessions: 8, complete: complete)
            _ = try database.cache(ClientWorkQuery(stamp: other,
                plans: [fieldPlan(other, wave: "c", lifecycle: [.done], clock: nil)]), key: "held", observation: connection)
            let fields = try XCTUnwrap(database.fieldReading(machine: "machine", work: work, observation: connection))
            XCTAssertEqual(fields.facts.parent?.value, "c")
            XCTAssertEqual(fields.facts.lifecycle?.value.states, [.done])
            XCTAssertEqual(fields.facts.activity?.value.states, [.busy], "Invalid capture keeps the last-known activity")
            XCTAssertTrue(fields.parentCurrent)
            XCTAssertTrue(fields.lifecycleCurrent)
            XCTAssertFalse(fields.activityCurrent)
            XCTAssertTrue(fields.facts.parent?.covers(CommitFence(machine: "machine", epoch: "epoch",
                schema: UInt32(ServerProtocol.revision), revisions: ["planning": 8])) == true)
            XCTAssertEqual(try database.cached(machine: "machine", key: "held")?.stamp, held,
                "Independently newer fields do not manufacture a coherent query snapshot")
        }
        XCTAssertEqual(try database.pending().map(\.id), ["saved"], "Field incorporation never settles unknown delivery")
        let restarted = try ClientWorkDatabase(directory: directory)
        let restored = try XCTUnwrap(restarted.fieldReading(machine: "machine", work: work))
        XCTAssertEqual(restored.facts.parent?.value, "c")
        XCTAssertTrue(restored.parentCurrent)
        XCTAssertFalse(restored.activityCurrent)
        let repaired = fieldStamp("other", planning: 9, sessions: 10)
        _ = try restarted.cache(ClientWorkQuery(stamp: repaired,
            plans: [fieldPlan(repaired, wave: "c", lifecycle: [.done], activity: [], clock: 21)]), key: "other", observation: connection)
        let fresh = try XCTUnwrap(restarted.fieldReading(machine: "machine", work: work, observation: connection))
        XCTAssertTrue(fresh.activityCurrent)
        XCTAssertEqual(fresh.facts.activity?.value.states, [])
        XCTAssertEqual(fresh.facts.parent?.value, "c")
    }

    func testFieldObservationReconnectRequiresReadingEvenWithTheSameSource() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let database = try ClientWorkDatabase(directory: directory)
        let first = MachineConnection.ObservationConnection(source: "observer", generation: UUID())
        let stamp = fieldStamp()
        let plan = try fieldPlan(stamp)
        let reading = ClientWorkQuery(stamp: stamp, plans: [plan])
        let work = try XCTUnwrap(plan.membership.facts.first?.work)
        _ = try database.cache(reading, key: "held", observation: first)
        XCTAssertTrue(try XCTUnwrap(database.fieldReading(machine: "machine", work: work, observation: first)).activityCurrent)
        XCTAssertFalse(try XCTUnwrap(database.fieldReading(machine: "machine", work: work)).activityCurrent)
        let reconnected = MachineConnection.ObservationConnection(source: "observer", generation: UUID())
        XCTAssertFalse(try XCTUnwrap(database.fieldReading(machine: "machine", work: work, observation: reconnected)).activityCurrent)
        _ = try database.cache(reading, key: "held", observation: reconnected)
        XCTAssertTrue(try XCTUnwrap(database.fieldReading(machine: "machine", work: work, observation: reconnected)).activityCurrent)
        let restarted = try ClientWorkDatabase(directory: directory)
        XCTAssertFalse(try XCTUnwrap(restarted.fieldReading(machine: "machine", work: work, observation: reconnected)).activityCurrent)
        XCTAssertTrue(try XCTUnwrap(restarted.fieldReading(machine: "machine", work: work)).parentCurrent)
        let replacement = MachineConnection.ObservationConnection(source: "replacement", generation: UUID())
        _ = try restarted.cache(ClientWorkQuery(stamp: stamp,
            plans: [fieldPlan(stamp, activity: [], clock: 1, source: "replacement")]), key: "held", observation: replacement)
        XCTAssertTrue(try XCTUnwrap(restarted.fieldReading(machine: "machine", work: work, observation: replacement)).activityCurrent)
        _ = try restarted.cache(reading, key: "held", observation: replacement)
        let stale = try XCTUnwrap(restarted.fieldReading(machine: "machine", work: work, observation: replacement))
        XCTAssertEqual(stale.facts.activity?.value.states, [], "A delayed old source cannot elect itself")
        XCTAssertTrue(stale.activityCurrent, "A departed-source frame cannot invalidate the current capture")
    }

    func testFieldStoreCannotBorrowCurrentConnectionForAnUnattributedReading() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let carrier = CompletionCarrier()
        let connection = MachineConnection {
            try await ServerLink.open(ServerEndpoint(client: .init(kind: "test", platform: "test", name: "test", version: "test"),
                control: { carrier }, ticketed: { carrier }))
        }
        let link = try await connection.link()
        addTeardownBlock { await link.close() }
        let disk = ClientWorkStore(directory: directory, connection: connection, machine: { "machine" }, read: { _ in "null" },
            observe: { _ in ServerWatch(updates: AsyncThrowingStream { $0.finish() }, checkpoint: { _ in }, cancel: {}) },
            send: { _ in XCTFail("Field ingestion never sends") })
        addTeardownBlock { await disk.close() }
        let stamp = fieldStamp()
        let plan = try fieldPlan(stamp, source: "fixture-observer")
        _ = try await disk.receivePlans([plan], stamp: stamp, for: .viewSelect(expression: "state:working"))
        let work = try XCTUnwrap(plan.membership.facts.first?.work)
        let fields = try await disk.fieldReading(machine: "machine", work: work)
        XCTAssertTrue(try XCTUnwrap(fields).parentCurrent)
        XCTAssertNil(fields?.facts.activity, "A matching source alone cannot attribute a buffered reading to this connection")
        XCTAssertFalse(try XCTUnwrap(fields).activityCurrent)
    }

    func testFieldOrderingRejectsStaleCrossedAndEqualConflictingEvidence() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let database = try ClientWorkDatabase(directory: directory)
        let connection = MachineConnection.ObservationConnection(source: "observer", generation: UUID())
        let stamp = fieldStamp(planning: 8)
        let plan = try fieldPlan(stamp, wave: "c", activity: [], clock: 20)
        let work = try XCTUnwrap(plan.membership.facts.first?.work)
        _ = try database.cache(ClientWorkQuery(stamp: stamp, plans: [plan]), key: "first", observation: connection)
        let older = fieldStamp(planning: 7)
        _ = try database.cache(ClientWorkQuery(stamp: older, plans: [fieldPlan(older)]), key: "older", observation: connection)
        var facts = try XCTUnwrap(database.fieldReading(machine: "machine", work: work, observation: connection))
        XCTAssertEqual(facts.facts.parent?.value, "c")
        XCTAssertTrue(facts.activityCurrent)
        // Newer SQL with older observations covers neither computation.
        let crossed = fieldStamp(planning: 9)
        _ = try database.cache(ClientWorkQuery(stamp: crossed, plans: [fieldPlan(crossed, wave: "d", clock: 19)]), key: "crossed", observation: connection)
        facts = try XCTUnwrap(database.fieldReading(machine: "machine", work: work, observation: connection))
        XCTAssertEqual(facts.facts.parent?.value, "d")
        XCTAssertEqual(facts.facts.activity?.value.states, [])
        XCTAssertTrue(facts.parentCurrent)
        XCTAssertFalse(facts.activityCurrent)
        _ = try database.cache(ClientWorkQuery(stamp: crossed, plans: [fieldPlan(crossed, wave: "wrong", clock: 21)]), key: "conflict", observation: connection)
        facts = try XCTUnwrap(database.fieldReading(machine: "machine", work: work, observation: connection))
        XCTAssertEqual(facts.facts.parent?.value, "d")
        XCTAssertFalse(facts.parentCurrent)
        XCTAssertTrue(facts.activityCurrent)
        // Equal support with the retained value is still not reconciliation.
        _ = try database.cache(ClientWorkQuery(stamp: crossed, plans: [fieldPlan(crossed, wave: "d", clock: 21)]), key: "repeat", observation: connection)
        XCTAssertFalse(try XCTUnwrap(database.fieldReading(machine: "machine", work: work)).parentCurrent)
        let newer = fieldStamp(planning: 10)
        _ = try database.cache(ClientWorkQuery(stamp: newer, plans: [fieldPlan(newer, wave: "e", clock: 22)]), key: "newer", observation: connection)
        facts = try XCTUnwrap(database.fieldReading(machine: "machine", work: work, observation: connection))
        XCTAssertEqual(facts.facts.parent?.value, "e")
        XCTAssertTrue(facts.parentCurrent)
        _ = try database.cache(ClientWorkQuery(stamp: newer,
            plans: [fieldPlan(newer, wave: "e", activity: [], clock: 24, activityKnown: false)]), key: "unknown", observation: connection)
        _ = try database.cache(ClientWorkQuery(stamp: newer,
            plans: [fieldPlan(newer, wave: "e", clock: 23)]), key: "old known", observation: connection)
        facts = try XCTUnwrap(database.fieldReading(machine: "machine", work: work, observation: connection))
        XCTAssertFalse(facts.activityCurrent)
        XCTAssertEqual(facts.facts.activity?.value.known, false, "An older known answer cannot overwrite a captured unknown transition")
    }

    func testFieldSupportMustMatchItsResourceAndCapturedDependencies() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let database = try ClientWorkDatabase(directory: directory)
        let connection = MachineConnection.ObservationConnection(source: "observer", generation: UUID())
        let stamp = fieldStamp()
        let plan = try fieldPlan(stamp)
        let work = try XCTUnwrap(plan.membership.facts.first?.work)
        _ = try database.cache(ClientWorkQuery(stamp: stamp, plans: [plan]), key: "first", observation: connection)
        let candidate = try fieldPlan(stamp, wave: "wrong", activity: [], clock: 20)
        for (field, value) in [("machine", "other" as Any), ("epoch", "other"), ("schema", 0),
                               ("revisions", ["planning": 8]), ("derivation", "")] {
            var wire = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(candidate)) as? [String: Any])
            var membership = try XCTUnwrap(wire["membership"] as? [String: Any])
            var facts = try XCTUnwrap(membership["facts"] as? [[String: Any]])
            var parent = try XCTUnwrap(facts[0]["parent"] as? [String: Any])
            var support = try XCTUnwrap(parent["support"] as? [String: Any])
            support[field] = value
            parent["support"] = support
            facts[0]["parent"] = parent
            membership["facts"] = facts
            wire["membership"] = membership
            let invalid = try JSONDecoder().decode(WorkViewPlan.self, from: JSONSerialization.data(withJSONObject: wire))
            _ = try database.cache(ClientWorkQuery(stamp: stamp, plans: [invalid]), key: field, observation: connection)
            let fields = try XCTUnwrap(database.fieldReading(machine: "machine", work: work, observation: connection))
            XCTAssertEqual(fields.facts.parent?.value, "a", field)
            XCTAssertTrue(fields.parentCurrent, field)
            XCTAssertTrue(fields.activityCurrent, "Invalid parent support does not discard independent activity")
        }
        let newer = fieldStamp(planning: 8)
        var wire = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(fieldPlan(newer, wave: "b"))) as? [String: Any])
        var membership = try XCTUnwrap(wire["membership"] as? [String: Any])
        var facts = try XCTUnwrap(membership["facts"] as? [[String: Any]])
        var activity = try XCTUnwrap(facts[0]["activity"] as? [String: Any])
        var support = try XCTUnwrap(activity["support"] as? [String: Any])
        support["observations"] = ["kind": "captured", "evidence": ["source": "observer", "validated_at": 20,
            "dependencies": [["fact": "process", "token": 19], ["fact": "process", "token": 21]]]]
        activity["support"] = support
        facts[0]["activity"] = activity
        membership["facts"] = facts
        wire["membership"] = membership
        _ = try database.cache(ClientWorkQuery(stamp: newer,
            plans: [JSONDecoder().decode(WorkViewPlan.self, from: JSONSerialization.data(withJSONObject: wire))]), key: "invalid capture", observation: connection)
        let fields = try XCTUnwrap(database.fieldReading(machine: "machine", work: work, observation: connection))
        XCTAssertEqual(fields.facts.parent?.value, "b")
        XCTAssertTrue(fields.parentCurrent)
        XCTAssertFalse(fields.activityCurrent)
    }

    func testFieldAndQueryWritesRollbackTogetherAndFieldLookupIsKeyedAtTenThousandRows() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let database = try ClientWorkDatabase(directory: directory)
        let connection = MachineConnection.ObservationConnection(source: "observer", generation: UUID())
        let stamp = fieldStamp()
        let plan = try fieldPlan(stamp)
        let work = try XCTUnwrap(plan.membership.facts.first?.work)
        _ = try database.cache(ClientWorkQuery(stamp: stamp, plans: [plan]), key: "first", observation: connection)
        var sqlite: OpaquePointer?
        XCTAssertEqual(sqlite3_open(directory.appendingPathComponent("work.sqlite").path, &sqlite), SQLITE_OK)
        defer { sqlite3_close(sqlite) }
        XCTAssertEqual(sqlite3_exec(sqlite, "CREATE TRIGGER reject_query BEFORE INSERT ON work_queries BEGIN SELECT RAISE(ABORT,'disk failure'); END", nil, nil, nil), SQLITE_OK)
        let newer = fieldStamp(planning: 8)
        XCTAssertThrowsError(try database.cache(ClientWorkQuery(stamp: newer,
            plans: [fieldPlan(newer, wave: "b", clock: 20)]), key: "failed", observation: connection))
        let fields = try XCTUnwrap(database.fieldReading(machine: "machine", work: work, observation: connection))
        XCTAssertEqual(fields.facts.parent?.value, "a")
        XCTAssertTrue(fields.parentCurrent)
        XCTAssertFalse(fields.activityCurrent, "A rolled back transaction cannot leave a revalidation proof")
        XCTAssertNil(try database.cached(machine: "machine", key: "failed"))
        XCTAssertEqual(sqlite3_exec(sqlite, "WITH RECURSIVE ids(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM ids WHERE n<10000) INSERT INTO work_fields(id,state) SELECT 'fixture-'||n,'{}' FROM ids", nil, nil, nil), SQLITE_OK)
        var statement: OpaquePointer?
        XCTAssertEqual(sqlite3_prepare_v2(sqlite, "EXPLAIN QUERY PLAN SELECT state FROM work_fields WHERE id=?", -1, &statement, nil), SQLITE_OK)
        defer { sqlite3_finalize(statement) }
        XCTAssertEqual(sqlite3_step(statement), SQLITE_ROW)
        let detail = String(cString: sqlite3_column_text(statement, 3))
        XCTAssertTrue(detail.contains("SEARCH work_fields USING INDEX"), detail)
    }

    func testFieldProtocolUpgradeRequiresFreshSupportWithoutReinterpretingStoredFacts() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let database = try ClientWorkDatabase(directory: directory)
        let stamp = fieldStamp()
        let plan = try fieldPlan(stamp)
        let work = try XCTUnwrap(plan.membership.facts.first?.work)
        _ = try database.cache(ClientWorkQuery(stamp: stamp, plans: [plan]), key: "first")
        var sqlite: OpaquePointer?
        XCTAssertEqual(sqlite3_open(directory.appendingPathComponent("work.sqlite").path, &sqlite), SQLITE_OK)
        defer { sqlite3_close(sqlite) }
        XCTAssertEqual(sqlite3_exec(sqlite, "UPDATE work_fields SET state=CAST(json_set(CAST(state AS TEXT),'$.parent.support.schema',0) AS BLOB)", nil, nil, nil), SQLITE_OK)
        let restarted = try ClientWorkDatabase(directory: directory)
        XCTAssertFalse(try XCTUnwrap(restarted.fieldReading(machine: "machine", work: work)).parentCurrent)
        _ = try restarted.cache(ClientWorkQuery(stamp: stamp, plans: [fieldPlan(stamp, wave: "b")]), key: "fresh")
        let fields = try XCTUnwrap(restarted.fieldReading(machine: "machine", work: work))
        XCTAssertTrue(fields.parentCurrent)
        XCTAssertEqual(fields.facts.parent?.value, "b")
    }

    func testHeldListKeepsItsRemovalAfterCommandRetirementAndRestartRegardlessOfCountOrder() async throws {
        for countFirst in [false, true] {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
            let client = store(directory)
            let list = ServedRead.desktopViewGet(directory: "/repo", expression: "state:working")
            let count = ServedRead.viewSelect(expression: "state:working")
            let base = try workingPlan(["a", "b"])
            _ = try await client.receivePlans([base], stamp: stamp("list", revision: 6), for: list)
            _ = try await client.receivePlans([base], stamp: stamp("count", revision: 6), for: count)
            let database = try complete(directory)
            let pending = try await client.cachedPlans(machine: "machine", for: list)
            XCTAssertEqual(pending?.projection.first?.projection.visible.map(\.work.id), ["b"])
            XCTAssertEqual(pending?.projection.first?.projection.suppressed.count, 1)
            XCTAssertEqual(pending?.projection.first?.resolution.attention.map(\.id), ["b"])
            XCTAssertEqual(pending?.projection.first?.taskCount, 1)
            if countFirst {
                _ = try await client.receivePlans([workingPlan(["b"])], stamp: stamp("count", revision: 7), for: count)
            }
            try database.retainQueries(machine: "machine", keys: [key(list), key(count)])
            try database.receive(receipt(kind: "task.complete", stage: "task.completed"), epoch: "epoch", id: "saved")
            try database.retireIfConverged("saved")
            XCTAssertTrue(try database.pending().isEmpty)
            let restarted = store(directory)
            if !countFirst {
                _ = try await restarted.receivePlans([workingPlan(["b"])], stamp: stamp("count", revision: 7), for: count)
            }
            let lagging = try await restarted.cachedPlans(machine: "machine", for: list)
            let counted = try await restarted.cachedPlans(machine: "machine", for: count)
            XCTAssertEqual(lagging?.plans, [base]) // confirmed facts were never relabeled
            XCTAssertEqual(lagging?.projection.first?.projection.visible.map(\.work.id), ["b"])
            XCTAssertEqual(lagging?.projection.first?.taskCount, 1)
            XCTAssertEqual(counted?.projection.first?.taskCount, 1)
            let covering = try await restarted.receivePlans([workingPlan(["b"])], stamp: stamp("list", revision: 7), for: list)
            XCTAssertEqual(covering?.projection.first?.taskCount, 1)
            XCTAssertTrue(covering?.effects?.isEmpty != false)
            let delayed = try await restarted.receivePlans([base], stamp: stamp("list", revision: 6), for: list)
            XCTAssertEqual(delayed?.projection.first?.taskCount, 1)
        }
    }

    func testCompletionRetainsExplicitDestinationAndDoesNotCountItsOtherOccurrencesTwice() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let client = store(directory)
        let query = ServedRead.viewSelect(expression: "state:working or task:a")
        _ = try await client.receivePlans([workingPlan(["a", "b"], explicit: true)], stamp: stamp("list", revision: 6), for: query)
        _ = try complete(directory)
        let projected = try await client.cachedPlans(machine: "machine", for: query)?.projection.first
        XCTAssertEqual(projected?.projection.visible.map(\.work.id), ["b", "a"])
        XCTAssertEqual(projected?.taskCount, 2)
        XCTAssertEqual(projected?.projection.visible.last?.source, ["root", "retained"])
        XCTAssertEqual(projected?.states.first { $0.key.id == "a" }?.value, [.done])
    }
    func testRequestFenceCannotIncorporateCompletionAndSupersessionWaitsForReplacementFacts() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let query = ServedRead.viewSelect(expression: "state:working")
        let base = try workingPlan(["a", "b"])
        let client = store(directory)
        _ = try await client.receivePlans([base], stamp: stamp("list", revision: 6), for: query)
        let database = try complete(directory)
        try database.receive(receipt(kind: "task.complete", stage: "task.completion_requested"), epoch: "epoch", id: "saved")
        try database.retireIfConverged("saved")
        XCTAssertEqual(try database.pending().count, 1)
        let requestReading = try await client.receivePlans([base], stamp: stamp("list", revision: 7), for: query)
        XCTAssertEqual(requestReading?.projection.first?.taskCount, 1)
        // The withdrawal resolution requires a replacement reading even when
        // the old base still contains A. Receipt arrival never restores a row.
        _ = try await client.receivePlans([base], stamp: stamp("list", revision: 6), for: query)
        var wire = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(
            receipt(kind: "task.complete", stage: "task.completed", resolution: .superseded))) as? [String: Any])
        wire["revision"] = 3
        var commits = try XCTUnwrap(wire["commits"] as? [[String: Any]])
        var fence = try XCTUnwrap(commits[0]["fence"] as? [String: Any])
        fence["revisions"] = ["planning": 8]
        commits[0]["fence"] = fence
        wire["commits"] = commits
        try database.retainQueries(machine: "machine", keys: [key(query)])
        try database.receive(JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: wire)), epoch: "epoch", id: "saved")
        try database.retireIfConverged("saved")
        XCTAssertTrue(try database.pending().isEmpty)
        let restarted = store(directory)
        let held = try await restarted.cachedPlans(machine: "machine", for: query)
        XCTAssertEqual(held?.projection.first?.taskCount, 1)
        let replacement = try await restarted.receivePlans([base], stamp: stamp("list", revision: 8), for: query)
        XCTAssertEqual(replacement?.projection.first?.taskCount, 2)
        let delayed = try await restarted.receivePlans([workingPlan(["b"])], stamp: stamp("list", revision: 7), for: query)
        XCTAssertEqual(delayed?.projection.first?.taskCount, 2)
    }

    func testReplacementEpochRetainsUnansweredIntentAndCannotBeReversedByOldListOrReceipt() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let query = ServedRead.viewSelect(expression: "state:working")
        let base = try workingPlan(["a", "b"])
        let client = store(directory)
        _ = try await client.receivePlans([base], stamp: stamp("list", revision: 6), for: query)
        let database = try complete(directory)
        try database.receive(receipt(kind: "task.complete", stage: "task.completed", diagnostic: "Unclassified failure"), epoch: "epoch", id: "saved")
        _ = try await client.receivePlans([base], stamp: stamp("list", revision: 1, epoch: "replacement"), for: query)
        let restarted = store(directory)
        let old = try await restarted.receivePlans([base], stamp: stamp("list", revision: 99), for: query)
        XCTAssertEqual(old?.stamp.epoch, "replacement")
        XCTAssertEqual(old?.projection.first?.taskCount, 1)
        let recovery = try await restarted.recovery("saved")
        XCTAssertEqual(recovery?.receiptAvailable, false)
        XCTAssertEqual(try database.pending().count, 1)
        try database.retireIfConverged("saved")
        XCTAssertEqual(try database.pending().count, 1)
    }

    func testRetiredCompletionDoesNotInvalidateAnUnrelatedRepository() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let client = store(directory)
        let query = ServedRead.viewSelect(expression: "state:working")
        let unrelated = try plan() // /src/fixture, whereas the command is /repo
        _ = try await client.receivePlans([unrelated], stamp: stamp("other", revision: 6), for: query)
        let database = try complete(directory)
        try database.receive(receipt(kind: "task.complete", stage: "task.completed"), epoch: "epoch", id: "saved")
        try database.retireIfConverged("saved")
        let persisted = try await store(directory).cachedPlans(machine: "machine", for: query)
        XCTAssertEqual(persisted?.plans, [unrelated])
        XCTAssertEqual(persisted?.projection.first?.projection.visible, unrelated.projection.visible)
    }

    func testDefiniteRefusalWithdrawsOnlyItsEffectWithoutWaitingForAnotherList() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let query = ServedRead.viewSelect(expression: "state:working")
        let client = store(directory)
        _ = try await client.receivePlans([workingPlan(["a", "b"])], stamp: stamp("list", revision: 6), for: query)
        let database = try complete(directory)
        try database.save(SubmittedOperation(requestID: "later", destination: "machine",
            payload: JSONEncoder().encode(ServedWrite.taskComplete(directory: "/repo", issue: "b")),
            version: ServerProtocol.revision, submittedAt: Date()))
        let hidden = try await client.cachedPlans(machine: "machine", for: query)
        XCTAssertEqual(hidden?.projection.first?.taskCount, 0)
        try database.retainQueries(machine: "machine", keys: [key(query)])
        try database.receive(receipt(kind: "task.complete", stage: "task.completed", resolution: .definitelyRefused), epoch: "epoch", id: "saved")
        try database.retireIfConverged("saved")
        let refused = try await store(directory).cachedPlans(machine: "machine", for: query)
        XCTAssertEqual(refused?.projection.first?.projection.visible.map(\.work.id), ["a"])
        XCTAssertEqual(refused?.projection.first?.taskCount, 1)
        XCTAssertEqual(try database.pending().map(\.id), ["later"])
    }

    func testUnknownActivityElsewhereDoesNotRetainACompletedOccurrenceOrInventAnExactCount() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        var body = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(workingPlan(["a", "b"]))) as? [String: Any])
        var membership = try XCTUnwrap(body["membership"] as? [String: Any])
        var facts = try XCTUnwrap(membership["facts"] as? [[String: Any]])
        facts[1]["activity"] = ["states": [], "known": false, "support": NSNull()]
        membership["facts"] = facts
        let members = try XCTUnwrap((body["resolution"] as? [String: Any])?["members"] as? [[String: Any]])
        membership["root"] = ["kind": "state", "state": "available", "members": members]
        body["membership"] = membership
        let compiled = try JSONDecoder().decode(WorkMembershipPlan.self, from: JSONSerialization.data(withJSONObject: membership))
        body["resolution"] = try JSONSerialization.jsonObject(with: JSONEncoder().encode(compiled.resolve()))
        let base = try JSONDecoder().decode(WorkViewPlan.self, from: JSONSerialization.data(withJSONObject: body))
        let client = store(directory)
        let query = ServedRead.viewSelect(expression: "state:available")
        _ = try await client.receivePlans([base], stamp: stamp("list", revision: 6), for: query)
        _ = try complete(directory)
        let projected = try await client.cachedPlans(machine: "machine", for: query)?.projection.first
        XCTAssertEqual(projected?.projection.visible.map(\.work.id), ["b"])
        XCTAssertNil(projected?.taskCount)
        XCTAssertFalse(projected?.resolution.unavailable.isEmpty == true)
        // The confirmation stamp and its unknown native fact remain untouched.
        XCTAssertFalse(projected?.confirmed.membership.facts[1].activityKnown == true)
    }

    func testCompletionIncorporationUsesItsDeclaredDomains() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let client = store(directory)
        let query = ServedRead.viewSelect(expression: "state:working")
        _ = try await client.receivePlans([workingPlan(["a", "b"])], stamp: stamp("list", revision: 6), for: query)
        let database = try complete(directory)
        try database.receive(receipt(kind: "task.complete", stage: "task.completed"), epoch: "epoch", id: "saved")
        try database.retireIfConverged("saved")
        var wire = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(stamp("list", revision: 6))) as? [String: Any])
        var revisions = try XCTUnwrap(wire["revisions"] as? [String: Any])
        revisions["planning"] = 7 // Session revision remains below the receipt's unrelated counter.
        wire["revisions"] = revisions
        let covering = try JSONDecoder().decode(ReadingStamp.self, from: JSONSerialization.data(withJSONObject: wire))
        let result = try await client.receivePlans([workingPlan(["b"])], stamp: covering, for: query)
        XCTAssertEqual(result?.projection.first?.taskCount, 1)
        XCTAssertTrue(result?.effects?.isEmpty != false)
    }

    func testUnrelatedUsageRevisionsCannotPreventADeclaredWorkReplacement() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let query = ServedRead.viewSelect(expression: "state:working")
        let client = store(directory)
        _ = try await client.receivePlans([workingPlan(["a", "b"])], stamp: stamp("list", revision: 6), for: query)
        var wire = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(stamp("list", revision: 7))) as? [String: Any])
        var revisions = try XCTUnwrap(wire["revisions"] as? [String: Any])
        revisions["usage"] = 1 // Work plans do not read usage.
        wire["revisions"] = revisions
        let replacement = try JSONDecoder().decode(ReadingStamp.self, from: JSONSerialization.data(withJSONObject: wire))
        let result = try await client.receivePlans([workingPlan(["b"])], stamp: replacement, for: query)
        XCTAssertEqual(result?.projection.first?.taskCount, 1)
        XCTAssertEqual(result?.stamp.revisions.usage, 1)
    }

    func testNewlySelectedCandidateWithoutContentStaysUnknownUntilItsCoveringRead() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        var body = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(workingPlan(["a", "b"]))) as? [String: Any])
        var membership = try XCTUnwrap(body["membership"] as? [String: Any])
        let refs = try XCTUnwrap((body["resolution"] as? [String: Any])?["members"])
        membership["root"] = ["kind": "state", "state": "done", "members": refs]
        body["membership"] = membership
        body["contents"] = []
        body["resolution"] = ["members": [], "attention": [], "unavailable": []]
        body["arrangement"] = ["order": [], "hidden": [], "mode": "auto"]
        body["projection"] = ["visible": [], "suppressed": []]
        let base = try JSONDecoder().decode(WorkViewPlan.self, from: JSONSerialization.data(withJSONObject: body))
        let query = ServedRead.viewSelect(expression: "state:done")
        let client = store(directory)
        _ = try await client.receivePlans([base], stamp: stamp("done", revision: 6), for: query)
        _ = try complete(directory)
        let projected = try await client.cachedPlans(machine: "machine", for: query)?.projection.first
        XCTAssertNil(projected?.taskCount)
        XCTAssertTrue(projected?.resolution.unavailable.contains("Work a content awaits confirmation") == true)
    }

    func key(_ query: ServedRead) throws -> String {
        let encoder = JSONEncoder(); encoder.outputFormatting = .sortedKeys
        return String(decoding: try encoder.encode(query), as: UTF8.self)
    }

    func testQueryReopenAndIndependentLeasesProjectHeldIntentOverOlderPreparedAnswers() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let a = ServedRead.desktopViewGet(directory: "/repo", expression: "state:working")
        let b = ServedRead.desktopViewGet(directory: "/repo", expression: "state:available")
        let base = try workingPlan(["a", "b"])
        let seed = store(directory)
        _ = try await seed.receivePlans([base], stamp: stamp("a", revision: 6), for: a)
        _ = try await seed.receivePlans([base], stamp: stamp("b", revision: 6), for: b)
        let prepared = try await seed.presentation(machine: "machine", queries: [key(a): a, key(b): b])
        let hold = HeldIntentDisk()
        let disk = ClientWorkStore(directory: directory, prepareDirectory: { _ in hold.enter() }, machine: { "machine" },
            read: { _ in "null" }, observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) },
            send: { _ in XCTFail("No send while saving") })
        addTeardownBlock { await disk.close() }
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { hold.release(); stop(owner) }
        var first: ProjectedWorkPlan?
        var second: ProjectedWorkPlan?
        let firstLease = owner.open(a) { first = $0?.first }
        _ = owner.open(a) { second = $0?.first }
        let bLease = owner.open(b) { _ in }
        owner.apply(prepared)
        var entered = hold.entered.makeAsyncIterator()
        _ = await entered.next()
        var selected: WorkWorkspaceKey? = base.contents[0].work
        let id = owner.complete(base.contents[0].work, issue: "a", directory: "/repo", selected: selected,
            visible: base.contents.map(\.work)) { selected = $0 }
        XCTAssertEqual(selected?.id, "b")
        selected = WorkWorkspaceKey(repository: base.repository, kind: .task, id: "deliberate")
        owner.close(firstLease)
        owner.close(bLease)
        var reopened: ProjectedWorkPlan?
        _ = owner.open(b) { reopened = $0?.first }
        XCTAssertEqual(reopened?.taskCount, 1) // First cached publication, before the actor can answer.
        owner.apply(prepared) // Prepared before the gesture, delivered after it.
        XCTAssertEqual(first?.taskCount, 1)
        XCTAssertEqual(second?.taskCount, 1)
        XCTAssertEqual(reopened?.projection.visible.map(\.work.id), ["b"])
        XCTAssertEqual(selected?.id, "deliberate")
        hold.release()
        await owner.persistPending()
        let records = try await disk.pending()
        XCTAssertEqual(records.map(\.id), [id])
        XCTAssertEqual(second?.taskCount, 1)
    }

    func testMachineDiscoveryFailureRetainsTheUnsavedIntentForSameIdentityRetry() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let fault = IntentDiskFault()
        let disk = ClientWorkStore(directory: directory, machine: { try fault.check(); return "machine" },
            read: { _ in "null" }, observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) },
            send: { _ in })
        addTeardownBlock { await disk.close() }
        let owner = WorkInteraction(machine: nil, store: disk)
        defer { stop(owner) }
        let a = WorkWorkspaceKey(repository: "repo", kind: .task, id: "a")
        let b = WorkWorkspaceKey(repository: "repo", kind: .task, id: "b")
        var selected: WorkWorkspaceKey? = a
        let id = owner.complete(a, issue: "a", directory: "/repo", selected: a, visible: [a, b]) { selected = $0 }
        XCTAssertEqual(selected, b)
        await owner.persistPending()
        XCTAssertEqual(owner.listedOperations.filter { $0.error != nil }.map(\.id), [id])
        XCTAssertNil(owner.operations.first?.destination)
        XCTAssertFalse(owner.isCompleting(issue: "a", directory: "/repo"))
        selected = a
        fault.recover()
        owner.retry(id)
        await owner.persistPending()
        let records = try await disk.pending()
        XCTAssertEqual(records.map(\.id), [id])
        XCTAssertEqual(records.first?.destination, "machine")
        XCTAssertEqual(selected, a)
    }

    func testCompletePublishesBeforeMachineDiscoveryAndSavesTheSameIdentity() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let discovery = HeldIntentMachine()
        let disk = ClientWorkStore(directory: directory, machine: { await discovery.read() }, read: { _ in "null" },
            observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) }, send: { _ in })
        addTeardownBlock { await disk.close() }
        let owner = WorkInteraction(machine: nil, store: disk)
        defer { stop(owner) }
        let a = WorkWorkspaceKey(repository: "repo", kind: .task, id: "a")
        let b = WorkWorkspaceKey(repository: "repo", kind: .task, id: "b")
        var selected: WorkWorkspaceKey? = a
        let id = owner.complete(a, issue: "a", directory: "/repo", selected: a, visible: [a, b]) { selected = $0 }
        XCTAssertEqual(selected, b)
        XCTAssertTrue(owner.isCompleting(issue: "a", directory: "/repo"))
        var entered = discovery.entered.makeAsyncIterator()
        _ = await entered.next()
        XCTAssertNil(owner.machine)
        owner.bindMachine("machine")
        XCTAssertTrue(owner.isCompleting(issue: "a", directory: "/repo"))
        selected = a
        await discovery.release()
        await owner.persistPending()
        let records = try await disk.pending()
        XCTAssertEqual(records.map(\.id), [id])
        XCTAssertEqual(records.first?.destination, "machine")
        XCTAssertEqual(selected, a)
    }

    func testCompletePublishesAndNavigatesWhileDiskAndNetworkAreHeld() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let query = ServedRead.viewSelect(expression: "state:working")
        let base = try workingPlan(["a", "b"])
        let seed = store(directory)
        _ = try await seed.receivePlans([base], stamp: stamp("list", revision: 6), for: query)
        let prepared = try await seed.presentation(machine: "machine", queries: [key(query): query])
        let hold = HeldIntentDisk()
        let disk = ClientWorkStore(directory: directory, prepareDirectory: { _ in hold.enter() }, machine: { "machine" },
            read: { _ in "null" }, observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) },
            send: { _ in XCTFail("No send before save or receipt lookup") })
        addTeardownBlock { await disk.close() }
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { hold.release(); stop(owner) }
        var first: [ProjectedWorkPlan] = []
        let lease = owner.open(query) { first = $0 ?? [] }
        owner.apply(prepared)
        var entered = hold.entered.makeAsyncIterator()
        _ = await entered.next() // The store actor is now blocked opening disk.
        var selected: WorkWorkspaceKey? = base.contents[0].work
        let work = try XCTUnwrap(selected)
        let visible = base.contents.map(\.work)
        let id = owner.complete(work, issue: "a", directory: "/repo", selected: selected, visible: visible) { selected = $0 }
        XCTAssertEqual(selected?.id, "b")
        XCTAssertEqual(first.first?.projection.visible.map(\.work.id), ["b"])
        XCTAssertEqual(first.first?.taskCount, 1)
        XCTAssertEqual(first.first?.confirmed, base)
        var second: [ProjectedWorkPlan] = []
        let secondLease = owner.open(query) { second = $0 ?? [] }
        XCTAssertEqual(second.first?.taskCount, 1, "Another mounted feed needs no new Machine frame")
        owner.close(lease)
        owner.apply(prepared) // A prepared base crosses the gesture/save boundary.
        XCTAssertEqual(second.first?.taskCount, 1)
        selected = work // Later deliberate navigation must survive save and replies.
        owner.close(secondLease) // Every view closes while this save is still held.
        var resumed: [ProjectedWorkPlan]?
        let resumedLease = owner.open(query) { resumed = $0 }
        XCTAssertNil(resumed, "Suspended observation cannot republish its old prepared answer")
        hold.release()
        await owner.persistPending()
        await owner.refreshFromStore()
        XCTAssertEqual(resumed?.first?.taskCount, 1, "Durable adoption after all leases close must not remove twice")
        XCTAssertEqual(selected, work)
        let saved = try await disk.pending()
        XCTAssertEqual(saved.map(\.id), [id])
        owner.close(resumedLease)
        let restarted = store(directory)
        let restored = try await restarted.cachedPlans(machine: "machine", for: query)
        XCTAssertEqual(restored?.projection.first?.projection.visible.map(\.work.id), ["b"])
        XCTAssertEqual(restored?.projection.first?.taskCount, 1)
    }

    func testProductionIntentReconcilesEveryReceiptListCountOrderWithoutReselecting() async throws {
        let cases = [OperationCommit.Resolution.committed, .superseded, .definitelyRefused].flatMap { disposition in
            [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]].map { (disposition, $0) }
        }
        for (disposition, order) in cases {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
            let server = HeldCompletionMachine()
            let disk = ClientWorkStore(directory: directory, machine: { "machine" }, read: { _ in "null" },
                observe: { server.observe($0) }, send: { server.send($0) })
            addTeardownBlock { await disk.close() }
            let owner = WorkInteraction(machine: "machine", store: disk)
            defer { stop(owner) }
            let list = ServedRead.viewSelect(expression: "state:working")
            let count = ServedRead.desktopViewGet(directory: "/repo", expression: "state:working")
            var shown: [ProjectedWorkPlan] = []
            var counted: [ProjectedWorkPlan] = []
            _ = owner.open(list) { shown = $0 ?? [] }
            _ = owner.open(count) { counted = $0 ?? [] }
            let base = try workingPlan(["a", "b"])
            try await owner.receive([base], stamp: stamp("list", revision: 6), for: list)
            try await owner.receive([base], stamp: stamp("count", revision: 6), for: count)
            var selection: WorkWorkspaceKey? = base.contents[0].work
            var navigations = 0
            let id = owner.complete(base.contents[0].work, issue: "a", directory: "/repo", selected: selection,
                visible: base.contents.map(\.work)) { selection = $0; navigations += 1 }
            XCTAssertEqual(shown.first?.taskCount, 1)
            XCTAssertEqual(counted.first?.taskCount, 1)
            await owner.persistPending()
            var observing = server.observations.makeAsyncIterator()
            _ = await observing.next()
            server.publish(try completionFrame(nil, epoch: "epoch"))
            var sent = server.sends.makeAsyncIterator()
            _ = await sent.next()
            var received = Set<Int>()
            for step in order {
                received.insert(step)
                if step == 0 {
                    var result = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(
                        receipt(kind: "task.complete", stage: "task.completed", resolution: disposition))) as? [String: Any])
                    result["request_id"] = id
                    let resolved = try JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: result))
                    server.publish(try completionFrame(resolved, epoch: "epoch"))
                    for await _ in await disk.changes() {
                        if try await disk.pending().isEmpty { break }
                    }
                    await owner.refreshFromStore()
                } else {
                    let query = step == 1 ? list : count
                    try await owner.receive([disposition == .committed ? workingPlan(["b"]) : base], stamp: stamp(step == 1 ? "list" : "count", revision: 7), for: query)
                }
                func expected(_ queryStep: Int) -> Int {
                    disposition != .committed && received.contains(0)
                        && (disposition == .definitelyRefused || received.contains(queryStep)) ? 2 : 1
                }
                XCTAssertEqual(shown.first?.taskCount, expected(1), "\(disposition), order \(order), step \(step)")
                XCTAssertEqual(counted.first?.taskCount, expected(2))
                XCTAssertEqual(selection?.id, "b")
                XCTAssertEqual(navigations, 1)
                let restored = try await store(directory).cachedPlans(machine: "machine", for: list)
                XCTAssertEqual(restored?.projection.first?.taskCount, expected(1))
            }
            XCTAssertEqual(server.submissions.count, 1)
        }
    }

    func testSaveFailureWithdrawsOnlyItsLayerAndRetainsOriginalCommandForRetry() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let query = ServedRead.viewSelect(expression: "state:working")
        let base = try workingPlan(["a", "b"])
        let seed = store(directory)
        _ = try await seed.receivePlans([base], stamp: stamp("list", revision: 6), for: query)
        let prepared = try await seed.presentation(machine: "machine", queries: [key(query): query])
        let fault = IntentDiskFault()
        let disk = ClientWorkStore(directory: directory, prepareDirectory: { _ in try fault.check() }, machine: { "machine" },
            read: { _ in "null" }, observe: { _ in ServerWatch(updates: AsyncThrowingStream { $0.finish() }, checkpoint: { _ in }, cancel: {}) },
            send: { _ in XCTFail("No missing lookup authorizes a send") })
        addTeardownBlock { await disk.close() }
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { stop(owner) }
        var shown: [ProjectedWorkPlan] = []
        _ = owner.open(query) { shown = $0 ?? [] }
        owner.apply(prepared)
        var selection: WorkWorkspaceKey? = base.contents[0].work
        let failed = owner.complete(base.contents[0].work, issue: "a", directory: "/repo", selected: selection,
            visible: base.contents.map(\.work)) { selection = $0 }
        await owner.persistPending()
        await owner.refreshFromStore()
        XCTAssertEqual(owner.cacheError, "Disk unavailable")
        XCTAssertEqual(shown.first?.taskCount, 2)
        XCTAssertEqual(owner.listedOperations.first?.id, failed)
        XCTAssertEqual(selection?.id, "b")
        fault.recover()
        _ = owner.complete(base.contents[1].work, issue: "b", directory: "/repo", selected: selection,
            visible: base.contents.map(\.work)) { selection = $0 }
        XCTAssertEqual(shown.first?.projection.visible.map(\.work.id), ["a"])
        await owner.persistPending()
        XCTAssertEqual(shown.first?.taskCount, 1)
        let retained = try XCTUnwrap(owner.operations.first { $0.id == failed })
        XCTAssertEqual(retained.destination, "machine")
        owner.retry(failed)
        XCTAssertEqual(shown.first?.taskCount, 0)
        await owner.persistPending()
        let saved = try await disk.pending()
        XCTAssertEqual(saved.count, 2)
        XCTAssertEqual(saved.last?.payload, retained.payload)
        XCTAssertEqual(selection?.id, "a", "Retry and save failure cannot navigate again")
    }

    func testDispatchedCompleteSurvivesReplacementEpochWithoutReplayAfterRestart() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let server = HeldCompletionMachine()
        let disk = ClientWorkStore(directory: directory, machine: { "machine" }, read: { _ in "null" },
            observe: { server.observe($0) }, send: { server.send($0) })
        addTeardownBlock { await disk.close() }
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { stop(owner) }
        let query = ServedRead.viewSelect(expression: "state:working")
        var shown: [ProjectedWorkPlan] = []
        _ = owner.open(query) { shown = $0 ?? [] }
        let base = try workingPlan(["a", "b"])
        try await owner.receive([base], stamp: stamp("list", revision: 6), for: query)
        let id = owner.complete(base.contents[0].work, issue: "a", directory: "/repo", selected: nil,
            visible: base.contents.map(\.work)) { _ in XCTFail("An unselected Complete cannot navigate") }
        await owner.persistPending()
        var observing = server.observations.makeAsyncIterator()
        _ = await observing.next()
        server.publish(try completionFrame(nil, epoch: "epoch"))
        var sent = server.sends.makeAsyncIterator()
        _ = await sent.next()
        try await owner.receive([base], stamp: stamp("list", revision: 1, epoch: "replacement"), for: query)
        XCTAssertEqual(shown.first?.taskCount, 1)
        await owner.shutdown().value
        let replacementServer = HeldCompletionMachine()
        replacementServer.publish(try completionFrame(nil, epoch: "replacement"))
        let replacement = ClientWorkStore(directory: directory, machine: { "machine" }, read: { _ in "null" },
            observe: { replacementServer.observe($0) }, send: { replacementServer.send($0) })
        addTeardownBlock { await replacement.close() }
        let restarted = WorkInteraction(machine: "machine", store: replacement)
        defer { stop(restarted) }
        _ = restarted.open(query) { shown = $0 ?? [] }
        var reopened = replacementServer.observations.makeAsyncIterator()
        _ = await reopened.next()
        for await _ in await replacement.changes() {
            if !(await replacement.isRecovering(id)) { break }
        }
        await restarted.refreshFromStore()
        XCTAssertEqual(shown.first?.taskCount, 1)
        XCTAssertTrue(replacementServer.submissions.isEmpty)
        try await restarted.receive([base], stamp: stamp("list", revision: 99), for: query)
        XCTAssertEqual(shown.first?.taskCount, 1)
        let pending = try await replacement.pending()
        XCTAssertEqual(pending.map(\.id), [id])
        let cached = try await replacement.cachedPlans(machine: "machine", for: query)
        XCTAssertEqual(cached?.stamp.epoch, "replacement")
    }



    func testSuspendedAnswerReconcilesBeforeReopenAfterClosedSettlementAndRestart() async throws {
        for disposition in [OperationCommit.Resolution.committed, .superseded, .definitelyRefused] {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
            let disk = store(directory)
            let owner = WorkInteraction(machine: "machine", store: disk)
            defer { stop(owner) }
            let query = ServedRead.viewSelect(expression: "state:working")
            let base = try workingPlan(["a", "b"])
            var frames: [[ProjectedWorkPlan]?] = []
            let lease = owner.open(query) { frames.append($0) }
            try await owner.receive([base], stamp: stamp("list", revision: 6), for: query)
            XCTAssertEqual(frames.last??.first?.taskCount, 2)
            owner.close(lease)
            await owner.refreshFromStore() // Drain close and release disk retention.
            let database = try complete(directory)
            try database.receive(receipt(kind: "task.complete", stage: "task.completed", resolution: disposition), epoch: "epoch", id: "saved")
            try database.retireIfConverged("saved")
            XCTAssertTrue(try database.pending().isEmpty)
            frames = []
            let reopened = owner.open(query) { frames.append($0) }
            XCTAssertTrue(frames.allSatisfy { $0 == nil }, "A suspended prepared answer cannot publish before disk reconciliation")
            await owner.refreshFromStore()
            XCTAssertNil(frames.last ?? nil, "The closed base requires a covering reading")
            let restarted = store(directory)
            let restored = try await restarted.cachedPlans(machine: "machine", for: query)
            XCTAssertNil(restored)
            // A saved Dashboard can change scope while the same query is held.
            let unrelated = try plan()
            let changed = try await restarted.receivePlans([unrelated], stamp: stamp("list", revision: 6), for: query)
            XCTAssertEqual(changed?.plans, [unrelated])
            let oldPlan = try await restarted.receivePlans([base], stamp: stamp("list", revision: 6), for: query)
            XCTAssertNil(oldPlan, "Changing the plan never discharges the original scope's floor")
            let replacement = disposition == .committed ? try workingPlan(["b"]) : base
            try await owner.receive([replacement], stamp: stamp("list", revision: 7), for: query)
            XCTAssertEqual(frames.last??.first?.taskCount, disposition == .committed ? 1 : 2)
            owner.close(reopened)
        }
    }

    func testReusableWorkingSetEvictsOldAnswersButPinsIndependentLeases() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let disk = store(directory)
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { stop(owner) }
        let base = try workingPlan(["a", "b"])
        let pinned = ServedRead.viewSelect(expression: "pinned")
        var pinnedFrames = 0
        let first = owner.open(pinned) { _ in pinnedFrames += 1 }
        let second = owner.open(pinned) { _ in pinnedFrames += 1 }
        try await owner.receive([base], stamp: stamp("pinned", revision: 6), for: pinned)
        owner.close(first)
        var oldest: ServedRead?
        var newest: ServedRead?
        for index in 0..<(WorkInteraction.reusableQueryLimit + 3) {
            let query = ServedRead.viewSelect(expression: "query-\(index)")
            if oldest == nil { oldest = query }
            newest = query
            let lease = owner.open(query) { _ in }
            try await owner.receive([base], stamp: stamp("q-\(index)", revision: 6), for: query)
            owner.close(lease)
        }
        var warm: [ProjectedWorkPlan]?
        _ = owner.open(try XCTUnwrap(newest)) { warm = $0 }
        XCTAssertEqual(warm?.first?.taskCount, 2, "Recent unleased answers remain immediately usable")
        var coldFrames = 0
        _ = owner.open(try XCTUnwrap(oldest)) { if $0 != nil { coldFrames += 1 } }
        XCTAssertEqual(coldFrames, 0, "An evicted answer must be reconciled before reuse")
        let before = pinnedFrames
        await owner.refreshFromStore()
        XCTAssertGreaterThan(pinnedFrames, before, "Closing one lease leaves its other window live")
        XCTAssertGreaterThan(coldFrames, 0)
        owner.close(second)
    }

    func testRepeatedSettlementBoundsEffectsAndCompactsClosedReloadRequirements() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let database = try ClientWorkDatabase(directory: directory)
        let query = ServedRead.viewSelect(expression: "state:working")
        let closed = ServedRead.viewSelect(expression: "closed")
        let base = try workingPlan(["a", "b"])
        _ = try database.cache(ClientWorkQuery(stamp: stamp("list", revision: 6), plans: [base]), key: key(query))
        _ = try database.cache(ClientWorkQuery(stamp: stamp("closed", revision: 6), plans: [base]), key: key(closed))
        try database.retainQueries(machine: "machine", keys: [key(query)])
        for index in 0..<(ClientWorkDatabase.retainedEffectLimit + 3) {
            let id = "completion-\(index)"
            try database.save(SubmittedOperation(requestID: id, destination: "machine",
                payload: JSONEncoder().encode(ServedWrite.taskComplete(directory: "/repo", issue: "a")),
                version: ServerProtocol.revision, submittedAt: Date()))
            XCTAssertTrue(try database.observeEpoch("epoch", id: id))
            var wire = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(
                receipt(kind: "task.complete", stage: "task.completed"))) as? [String: Any])
            wire["request_id"] = id
            try database.receive(JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: wire)), epoch: "epoch", id: id)
            try database.retireIfConverged(id)
            XCTAssertTrue(try database.pending().isEmpty)
            let held = try database.cached(machine: "machine", key: key(query))
            XCTAssertLessThanOrEqual(held?.effects?.count ?? 0, ClientWorkDatabase.retainedEffectLimit)
            XCTAssertNil(try database.cached(machine: "machine", key: key(closed)))
        }
        var connection: OpaquePointer?
        XCTAssertEqual(sqlite3_open(directory.appendingPathComponent("work.sqlite").path, &connection), SQLITE_OK)
        defer { sqlite3_close(connection) }
        var statement: OpaquePointer?
        XCTAssertEqual(sqlite3_prepare_v2(connection,
            "SELECT json_array_length(CAST(state AS TEXT)) FROM work_requirements WHERE id='machine'", -1, &statement, nil), SQLITE_OK)
        defer { sqlite3_finalize(statement) }
        XCTAssertEqual(sqlite3_step(statement), SQLITE_ROW)
        XCTAssertEqual(sqlite3_column_int(statement, 0), 1, "Reload metadata grows by scope, never by operation")
        XCTAssertNil(try database.cached(machine: "machine", key: key(query)), "A held answer exceeding the effect budget becomes cold")
        let restarted = try ClientWorkDatabase(directory: directory)
        XCTAssertTrue(try restarted.pending().isEmpty)
        XCTAssertNil(try restarted.cached(machine: "machine", key: key(closed)))
        let covering = try restarted.cache(ClientWorkQuery(stamp: stamp("closed", revision: 7), plans: [workingPlan(["b"])]), key: key(closed))
        XCTAssertEqual(covering?.projection.first?.taskCount, 1)
        // The compact frontier applies even to expressions created after retirement.
        let future = try restarted.cache(ClientWorkQuery(stamp: stamp("future", revision: 6), plans: [base]), key: "future")
        XCTAssertNil(future)
    }

    func testObservationAndFieldUpgradeDropOldBasesAndRetainSavedCommands() throws {
        for missing in ["$.stamp.observations", "$.plans[0].membership.facts[0].parent",
                        "$.plans[0].membership.facts[0].lifecycle", "$.plans[0].membership.facts[0].activity"] {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
            let query = ServedRead.viewSelect(expression: "state:working")
            do {
                let database = try complete(directory)
                _ = try database.cache(ClientWorkQuery(stamp: stamp("list", revision: 6), plans: [workingPlan(["a", "b"])]), key: key(query))
            }
            do {
                var connection: OpaquePointer?
                XCTAssertEqual(sqlite3_open(directory.appendingPathComponent("work.sqlite").path, &connection), SQLITE_OK)
                defer { sqlite3_close(connection) }
                XCTAssertEqual(sqlite3_exec(connection,
                    "UPDATE work_queries SET state=CAST(json_remove(CAST(state AS TEXT),'\(missing)') AS BLOB)",
                    nil, nil, nil), SQLITE_OK)
            }
            let restarted = try ClientWorkDatabase(directory: directory)
            XCTAssertNil(try restarted.cached(machine: "machine", key: key(query)))
            XCTAssertEqual(try restarted.pending().map(\.id), ["saved"])
            XCTAssertEqual(try restarted.state("saved")?.record.destination, "machine")
        }
    }

    func testLaterSettlementCannotReviveAnInvalidatedBaseWithAnUnrelatedEffect() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let query = ServedRead.viewSelect(expression: "state:working")
        let database = try complete(directory)
        _ = try database.cache(ClientWorkQuery(stamp: stamp("list", revision: 6), plans: [workingPlan(["a", "b"])]), key: key(query))
        let first = try receipt(kind: "task.complete", stage: "task.completed")
        try database.receive(first, epoch: "epoch", id: "saved")
        try database.retireIfConverged("saved")
        XCTAssertNil(try database.cached(machine: "machine", key: key(query)))
        try database.retainQueries(machine: "machine", keys: [key(query)])
        try database.save(SubmittedOperation(requestID: "later", destination: "machine",
            payload: JSONEncoder().encode(ServedWrite.taskComplete(directory: "/repo", issue: "b")),
            version: ServerProtocol.revision, submittedAt: Date()))
        XCTAssertTrue(try database.observeEpoch("epoch", id: "later"))
        var wire = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(first)) as? [String: Any])
        wire["request_id"] = "later"
        try database.receive(JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: wire)), epoch: "epoch", id: "later")
        try database.retireIfConverged("later")
        XCTAssertTrue(try database.pending().isEmpty)
        XCTAssertNil(try database.cached(machine: "machine", key: key(query)), "B's fence must not substitute for A's missing effect")
    }

    func testHealthyCompletionWaitsAndReconnectsAfterRestartWithoutAnotherGesture() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let machine = HeldCompletionMachine()
        let carrier = CompletionCarrier()
        let connection = MachineConnection {
            if machine.isWaiting { throw ServerConnectionUnavailable(RegistryQueryError("Offline")) }
            return try await ServerLink.open(ServerEndpoint(client: .init(kind: "test", platform: "test", name: "test", version: "test"),
                control: { carrier }, ticketed: { carrier }))
        }
        func client() -> ClientWorkStore {
            let fixture = ClientWorkStore(directory: directory, connection: connection, machine: { XCTFail("Saved work needs no discovery"); return "machine" },
                read: { _ in "null" }, observe: { machine.observe($0) }, send: { machine.send($0) })
            addTeardownBlock { await fixture.close() }
            return fixture
        }
        let disk = client()
        let owner = WorkInteraction(machine: "machine", store: disk)
        let a = WorkWorkspaceKey(repository: "repo", kind: .task, id: "a")
        let b = WorkWorkspaceKey(repository: "repo", kind: .task, id: "b")
        var selected: WorkWorkspaceKey? = a
        let id = owner.complete(a, issue: "a", directory: "/repo", selected: a, visible: [a, b]) { selected = $0 }
        XCTAssertEqual(selected, b)
        XCTAssertTrue(owner.listedOperations.isEmpty, "A healthy change shows only its projection")
        await owner.persistPending()
        var watches = machine.observations.makeAsyncIterator()
        _ = await watches.next()
        machine.setWaiting(true)
        _ = try? await connection.link()
        await owner.refreshFromStore()
        let operation = try XCTUnwrap(owner.listedOperations.first)
        XCTAssertEqual(operation.presentation().status, .waitingForConnection)
        XCTAssertTrue(operation.presentation().actions.isEmpty)
        XCTAssertTrue(owner.isCompleting(issue: "a", directory: "/repo"))
        XCTAssertTrue(machine.submissions.isEmpty)
        await owner.shutdown().value
        let recovering = await disk.isRecovering(id)
        XCTAssertFalse(recovering, "Shutdown releases recovery before a replacement owner starts")
        let restartedDisk = client()
        let restarted = WorkInteraction(machine: "machine", store: restartedDisk)
        defer { stop(restarted) }
        restarted.start()
        _ = await watches.next()
        await restarted.refreshFromStore()
        XCTAssertEqual(restarted.listedOperations.map(\.id), [id])
        XCTAssertEqual(restarted.listedOperations.first?.presentation().status, .waitingForConnection)
        machine.setWaiting(false)
        let link = try await connection.link()
        defer { Task { await link.close() } }
        machine.publish(try completionFrame(nil, epoch: "epoch"))
        var sends = machine.sends.makeAsyncIterator()
        _ = await sends.next()
        await restarted.refreshFromStore()
        XCTAssertTrue(restarted.listedOperations.isEmpty)
        XCTAssertEqual(machine.submissions.count, 1)
        let envelope = try XCTUnwrap(JSONSerialization.jsonObject(with: machine.submissions[0]) as? [String: Any])
        XCTAssertEqual(envelope["request_id"] as? String, id)
        XCTAssertEqual(selected, b)
    }

    func testReconnectReadsTheLostSendOutcomeWithoutExecutingAgain() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let machine = HeldCompletionMachine()
        let disk = ClientWorkStore(directory: directory, machine: { "machine" }, read: { _ in "null" },
            observe: { machine.observe($0) }, send: {
                machine.send($0)
                throw ServerConnectionUnavailable(RegistryQueryError("Answer lost"))
            })
        let record = try await disk.record(.taskComplete(directory: "/repo", issue: "a"), requestID: "saved")
        let delivery = Task { try await disk.reconcile(record) }
        var observations = machine.observations.makeAsyncIterator()
        _ = await observations.next()
        machine.publish(try completionFrame(nil, epoch: "epoch"))
        var sends = machine.sends.makeAsyncIterator()
        _ = await sends.next()
        machine.setWaiting(true)
        let completed = try receipt(kind: "task.complete", stage: "task.completed")
        machine.setWaiting(false)
        machine.publish(try completionFrame(completed, epoch: "epoch"))
        let result = try await delivery.value
        XCTAssertEqual(result?.delivery, .finished)
        XCTAssertEqual(machine.submissions.count, 1)
        let pending = try await disk.pending()
        XCTAssertTrue(pending.isEmpty)
    }

    func testAbandonSharesRemovalNavigationAndRefusalWithoutStealingLaterSelection() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let machine = HeldCompletionMachine()
        let disk = ClientWorkStore(directory: directory, machine: { "machine" }, read: { _ in "null" },
            observe: { machine.observe($0) }, send: { machine.send($0) })
        addTeardownBlock { await disk.close() }
        let query = ServedRead.viewSelect(expression: "state:working")
        let base = try workingPlan(["a", "b"])
        _ = try await disk.receivePlans([base], stamp: stamp("list", revision: 6), for: query)
        let prepared = try await disk.presentation(machine: "machine", queries: [key(query): query])
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { stop(owner) }
        var shown: ProjectedWorkPlan?
        _ = owner.open(query) { shown = $0?.first }
        owner.apply(prepared)
        let a = base.contents[0].work, b = base.contents[1].work
        var selected: WorkWorkspaceKey? = a
        let id = owner.abandon(a, issue: "a", directory: "/repo", selected: a, visible: [a, a, b]) { selected = $0 }
        XCTAssertEqual(selected, b)
        XCTAssertEqual(shown?.projection.visible.map(\.work), [b])
        XCTAssertTrue(owner.listedOperations.isEmpty)
        selected = a // A deliberate later visit must survive a refusal.
        await owner.persistPending()
        var observations = machine.observations.makeAsyncIterator()
        _ = await observations.next()
        var wire = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(
            receipt(kind: "task.abandon", stage: "task.abandoned", diagnostic: "Cannot abandon", resolution: .definitelyRefused))) as? [String: Any])
        wire["request_id"] = id
        machine.publish(try completionFrame(JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: wire)), epoch: "epoch"))
        for await _ in await disk.changes() {
            if try await disk.receipt(id)?.error != nil { break }
        }
        await owner.refreshFromStore()
        XCTAssertEqual(shown?.projection.visible.map(\.work), [a, b])
        XCTAssertEqual(selected, a)
        XCTAssertTrue(owner.listedOperations.first?.presentation().status.text?.contains("Cannot abandon") == true)
        XCTAssertTrue(machine.submissions.isEmpty)
    }

    func completionFrame(_ receipt: OperationReceipt?, epoch: String) throws -> SubscriptionReading<OperationReceipt> {
        let wire: [String: Any] = ["body": try receipt.map { try JSONSerialization.jsonObject(with: JSONEncoder().encode($0)) } ?? NSNull(),
            "stamp": try JSONSerialization.jsonObject(with: JSONEncoder().encode(stamp("receipt", revision: 7, epoch: epoch))),
            "unavailable": NSNull(), "answers": NSNull()]
        return try JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self, from: JSONSerialization.data(withJSONObject: wire))
    }

}

final class HeldIntentDisk: @unchecked Sendable {
    private let semaphore = DispatchSemaphore(value: 0)
    let entered: AsyncStream<Void>
    private let entering: AsyncStream<Void>.Continuation
    init() { (entered, entering) = AsyncStream.makeStream() }
    func enter() { entering.yield(()); semaphore.wait() }
    func release() { semaphore.signal() }
}

final class IntentDiskFault: @unchecked Sendable {
    private let lock = NSLock()
    private var failing = true
    func check() throws {
        if lock.withLock({ failing }) { throw RegistryQueryError("Disk unavailable") }
    }
    func recover() { lock.withLock { failing = false } }
}

private final class HeldCompletionMachine: @unchecked Sendable {
    private let lock = NSLock()
    private var watches: [UUID: AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error>.Continuation] = [:]
    private var current: SubscriptionReading<OperationReceipt>?
    private var sent: [Data] = []
    private var waiting = false
    var isWaiting: Bool { lock.withLock { waiting } }
    func setWaiting(_ value: Bool) {
        lock.withLock { waiting = value }
    }
    var submissions: [Data] { lock.withLock { sent } }
    let observations: AsyncStream<Void>
    private let observing: AsyncStream<Void>.Continuation
    let sends: AsyncStream<Void>
    private let sending: AsyncStream<Void>.Continuation
    init() {
        (observations, observing) = AsyncStream.makeStream()
        (sends, sending) = AsyncStream.makeStream()
    }
    func observe(_ id: String) -> ClientWorkStore.ReceiptWatch {
        let key = UUID()
        let (stream, continuation) = AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error>.makeStream()
        lock.withLock { watches[key] = continuation; if let current { continuation.yield(current) } }
        observing.yield(())
        return ServerWatch(updates: stream, checkpoint: { _ in }, cancel: { [self] in
            lock.withLock { watches.removeValue(forKey: key)?.finish() }
        })
    }
    func publish(_ frame: SubscriptionReading<OperationReceipt>) {
        lock.withLock { current = frame; for watch in watches.values { watch.yield(frame) } }
    }
    func send(_ data: Data) { lock.withLock { sent.append(data) }; sending.yield(()) }
}

private actor HeldIntentMachine {
    nonisolated let entered: AsyncStream<Void>
    private let entering: AsyncStream<Void>.Continuation
    private var continuation: CheckedContinuation<Void, Never>?
    private var ready = false
    init() { (entered, entering) = AsyncStream.makeStream() }
    func read() async -> String {
        if !ready { await withCheckedContinuation { continuation = $0; entering.yield(()) } }
        return "machine"
    }
    func release() { ready = true; continuation?.resume(); continuation = nil }
}

private actor CompletionCarrier: ServerCarrier {
    private var welcomed = false
    private var closed = false
    private var waiting: CheckedContinuation<Data?, Never>?
    func send(_ message: Data) {}
    func receive() async -> Data? {
        if !welcomed {
            welcomed = true
            return Data(#"{"type":"welcome","observations":"fixture-observer","client_id":"fixture","revision":6,"server_revision":6,"min_revision":6,"instance":"fixture","build":"test","executable_sha256":"hash","machine":null,"limits":{"max_message":1048576,"max_connections":64,"max_open":256,"max_retained_bytes":67108864}}"#.utf8)
        }
        if closed { return nil }
        return await withCheckedContinuation { waiting = $0 }
    }
    func close() { closed = true; waiting?.resume(returning: nil); waiting = nil }
}
