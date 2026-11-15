import Foundation
import XCTest
@testable import Loopflow

/// The same persisted-query cases run in the Desktop and phone test targets.
@MainActor
final class ClientWorkQueryTests: XCTestCase {
    private func plan() throws -> WorkViewPlan {
        #if os(iOS)
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "view_plan", withExtension: "json"))
        #else
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/view_plan.json")
        #endif
        return try JSONDecoder().decode(WorkViewPlan.self, from: Data(contentsOf: url))
    }

    private func stamp(_ query: String, revision: Int64, epoch: String = "epoch") -> ReadingStamp {
        // All fields, including nullable query, exercise the wire decoder.
        try! JSONDecoder().decode(ReadingStamp.self, from: JSONSerialization.data(withJSONObject: [
            "machine": "machine", "epoch": epoch, "schema": ServerProtocol.revision, "query": query,
            "complete": true, "revisions": ["planning": revision, "sessions": revision, "graphs": revision,
                "processes": revision, "usage": revision]]))
    }

    private func receipt(kind: String = "session.new", stage: String = "session.input_replaced",
                         planning: OperationCommit.PlanningEffect = .none, diagnostic: String? = nil) throws -> OperationReceipt {
        let declaration: [String: Any] = ["resolution": "committed", "stage": stage, "resources": ["session"],
            "fence": ["machine": "machine", "epoch": "epoch", "schema": ServerProtocol.revision,
                      "revisions": ["planning": 7, "sessions": 7]],
            "result": ["id": "session"], "planning_effect": planning.rawValue, "planning_changes": []]
        return try JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: [
            "request_id": "saved", "kind": kind, "fingerprint": NSNull(), "submitted_fingerprint": "hash",
            "validation": "bound", "delivery": "finished", "revision": 2,
            "result": ["id": "session"], "error": diagnostic as Any? ?? NSNull(),
            "commits": [declaration], "planning": NSNull(), "created_at": 1, "updated_at": 2]))
    }

    private func store(_ directory: URL) -> ClientWorkStore {
        ClientWorkStore(directory: directory, machine: { "machine" }, read: { _ in "null" }, observe: { _ in
            ServerWatch(updates: AsyncThrowingStream { $0.finish() }, checkpoint: { _ in }, cancel: {})
        }, send: { _ in XCTFail("Cache and retirement must not send commands") })
    }

    private func save(_ directory: URL, receipt: OperationReceipt) throws {
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
        defer { try? FileManager.default.removeItem(at: directory) }
        #if os(iOS)
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "operation_creation_planning", withExtension: "json", subdirectory: "server"))
        #else
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("rust/loopflow/tests/fixtures/dto/server/operation_creation_planning.json")
        #endif
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
        defer { try? FileManager.default.removeItem(at: directory) }
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
        defer { try? FileManager.default.removeItem(at: directory) }
        try save(directory, receipt: receipt(kind: "task.update", stage: "task.updated"))
        let database = try ClientWorkDatabase(directory: directory)
        try database.retireIfConverged("saved")
        XCTAssertTrue(try database.pending().isEmpty)
    }

    func testMissingCorrelationAndUnclassifiedDiagnosticsRetainTheExactRequest() async throws {
        for (planning, diagnostic) in [(OperationCommit.PlanningEffect.correlated, nil), (.none, "Attachment interrupted")] {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            defer { try? FileManager.default.removeItem(at: directory) }
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
        defer { try? FileManager.default.removeItem(at: directory) }
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
        defer { try? FileManager.default.removeItem(at: directory) }
        let query = ServedRead.viewSelect(expression: "state:working")
        let base = try plan()
        _ = try await store(directory).receivePlans([base], stamp: stamp("list", revision: 12), for: query)
        let delayed = try await store(directory).receivePlans([], stamp: stamp("list", revision: 11), for: query)
        XCTAssertEqual(delayed?.plans, [base])
        XCTAssertEqual(delayed?.stamp.revisions.planning, 12)
    }
}
