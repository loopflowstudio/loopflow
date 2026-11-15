import Foundation
import XCTest
@testable import Loopflow

/// The same persisted-query cases run in the Desktop and phone test targets.
@MainActor
final class ClientWorkQueryTests: XCTestCase {
    private func plan() throws -> WorkViewPlan {
        let url = Bundle(for: Self.self).url(forResource: "view_plan", withExtension: "json")
            ?? URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/view_plan.json")
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

    private func workingPlan(_ ids: [String], explicit: Bool = false) throws -> WorkViewPlan {
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
        membership["facts"] = ids.map { ["work": key($0), "name": $0, "states": ["open", "working", "available"], "activity_known": true] as [String: Any] }
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

    private func complete(_ directory: URL) throws -> ClientWorkDatabase {
        let database = try ClientWorkDatabase(directory: directory)
        try database.save(SubmittedOperation(requestID: "saved", destination: "machine",
            payload: JSONEncoder().encode(ServedWrite.taskComplete(directory: "/repo", issue: "a")),
            version: ServerProtocol.revision, submittedAt: Date()))
        XCTAssertTrue(try database.observeEpoch("epoch", id: "saved"))
        return database
    }

    func testHeldListKeepsItsRemovalAfterCommandRetirementAndRestartRegardlessOfCountOrder() async throws {
        for countFirst in [false, true] {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            defer { try? FileManager.default.removeItem(at: directory) }
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
        defer { try? FileManager.default.removeItem(at: directory) }
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
        defer { try? FileManager.default.removeItem(at: directory) }
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
        defer { try? FileManager.default.removeItem(at: directory) }
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
        defer { try? FileManager.default.removeItem(at: directory) }
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
        defer { try? FileManager.default.removeItem(at: directory) }
        let query = ServedRead.viewSelect(expression: "state:working")
        let client = store(directory)
        _ = try await client.receivePlans([workingPlan(["a", "b"])], stamp: stamp("list", revision: 6), for: query)
        let database = try complete(directory)
        try database.save(SubmittedOperation(requestID: "later", destination: "machine",
            payload: JSONEncoder().encode(ServedWrite.taskComplete(directory: "/repo", issue: "b")),
            version: ServerProtocol.revision, submittedAt: Date()))
        let hidden = try await client.cachedPlans(machine: "machine", for: query)
        XCTAssertEqual(hidden?.projection.first?.taskCount, 0)
        try database.receive(receipt(kind: "task.complete", stage: "task.completed", resolution: .definitelyRefused), epoch: "epoch", id: "saved")
        try database.retireIfConverged("saved")
        let refused = try await store(directory).cachedPlans(machine: "machine", for: query)
        XCTAssertEqual(refused?.projection.first?.projection.visible.map(\.work.id), ["a"])
        XCTAssertEqual(refused?.projection.first?.taskCount, 1)
        XCTAssertEqual(try database.pending().map(\.id), ["later"])
    }

    func testUnknownActivityElsewhereDoesNotRetainACompletedOccurrenceOrInventAnExactCount() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        var body = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(workingPlan(["a", "b"]))) as? [String: Any])
        var membership = try XCTUnwrap(body["membership"] as? [String: Any])
        var facts = try XCTUnwrap(membership["facts"] as? [[String: Any]])
        facts[1]["activity_known"] = false
        facts[1]["states"] = ["open", "working"]
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
        defer { try? FileManager.default.removeItem(at: directory) }
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
        defer { try? FileManager.default.removeItem(at: directory) }
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
        defer { try? FileManager.default.removeItem(at: directory) }
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

}
