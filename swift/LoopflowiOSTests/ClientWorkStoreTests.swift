import Foundation
import XCTest
@testable import Loopflow

final class ClientWorkStoreTests: XCTestCase {
    func testLivePlanningStatusOutlivesTheSavedResultAndPhoneRestart() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "operation_planning", withExtension: "json", subdirectory: "server"))
        let result = try XCTUnwrap(try RawJSON(Data(contentsOf: url)).fields()["result"])
        let historical = try result.decode(OperationReceipt.self)
        var current = try XCTUnwrap(JSONSerialization.jsonObject(with: result.bytes) as? [String: Any])
        current["revision"] = historical.revision + 1
        var planning = try XCTUnwrap(current["planning"] as? [String: Any])
        var changes = try XCTUnwrap(planning["changes"] as? [[String: Any]])
        for index in changes.indices {
            changes[index]["state"] = "delivered"
            changes[index]["error"] = NSNull()
        }
        planning["changes"] = changes
        current["planning"] = planning
        let newer = try RawJSON(JSONSerialization.data(withJSONObject: current)).decode(OperationReceipt.self)
        let frames = try [historical, newer, historical].map { receipt in
            let value: [String: Any] = ["answers": NSNull(), "unavailable": NSNull(),
                "body": try JSONSerialization.jsonObject(with: JSONEncoder().encode(receipt)),
                "stamp": ["machine": "machine_fixture", "epoch": "ab0123456789ab0123456789ab01234567",
                          "schema": ServerProtocol.revision, "query": "operation-query", "complete": true,
                          "revisions": ["planning": 12, "sessions": 1, "graphs": 1, "processes": 1, "usage": 1]]]
            return try JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self,
                from: JSONSerialization.data(withJSONObject: value))
        }
        func store() -> ClientWorkStore {
            let fixture = ClientWorkStore(directory: directory, machine: { "machine_fixture" }, read: { _ in "null" }, observe: { _ in
                let stream = AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error> { continuation in
                    for frame in frames { continuation.yield(frame) }
                    continuation.finish()
                }
                return ServerWatch(updates: stream, checkpoint: { _ in }, cancel: {})
            }, send: { _ in throw RegistryQueryError("Observation must never submit the saved edit") })
            addTeardownBlock { await fixture.close() }
            return fixture
        }
        let client = store()
        let record = try await client.record(.taskUpdate(directory: "/repo", issue: "LOO-1", notes: "Original phone edit"),
            requestID: historical.requestID)
        do {
            try await client.observeSaved(record)
            XCTFail("Ended observation must report the missing live evidence")
        } catch { }
        try await client.presented(record.id)
        let restarted = store()
        let recovery = try await restarted.recovery(record.id)
        XCTAssertTrue(recovery?.resultPresented == true)
        XCTAssertEqual(recovery?.planning, .resolved)
        XCTAssertTrue(recovery?.needsInspection == true) // unclassified command diagnostic remains
        XCTAssertEqual(recovery?.diagnostic, historical.error)
        let saved = try await restarted.receipt(record.id)
        let status = try await restarted.planning(record.id)
        XCTAssertEqual(saved?.result, historical.result)
        XCTAssertEqual(saved?.error, historical.error)
        XCTAssertEqual(status?.unresolved.count, 0)
        let recovered = try await restarted.reconcile(record)
        XCTAssertEqual(recovered?.planning, newer.planning)
        let pending = try await restarted.pending()
        XCTAssertEqual(pending.first?.payload, record.payload)

        // A replacement lifetime and delayed old receipt cannot turn the saved
        // result into current confirmation, even after the phone restarts.
        var replacement = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(try XCTUnwrap(frames[0].stamp))) as? [String: Any])
        replacement["epoch"] = "replacement"
        let missing = try JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self,
            from: JSONSerialization.data(withJSONObject: ["answers": NSNull(), "unavailable": NSNull(),
                "body": NSNull(), "stamp": replacement]))
        let resetFrames = [missing, frames[0]]
        let resetClient = ClientWorkStore(directory: directory, machine: { "machine_fixture" }, read: { _ in "null" }, observe: { _ in
            let stream = AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error> { continuation in
                for frame in resetFrames { continuation.yield(frame) }
                continuation.finish()
            }
            return ServerWatch(updates: stream, checkpoint: { _ in }, cancel: {})
        }, send: { _ in XCTFail("Epoch replacement cannot replay the saved edit") })
        addTeardownBlock { await resetClient.close() }
        do { try await resetClient.observeSaved(record) } catch { }
        // The first receipt from this new observer belongs to the departed epoch.
        do { try await store().observeSaved(record) } catch { }
        let afterReset = try await store().recovery(record.id)
        XCTAssertTrue(afterReset?.resultPresented == true)
        XCTAssertTrue(afterReset?.needsInspection == true)
        XCTAssertEqual(afterReset?.planning, .unavailable)
        XCTAssertTrue(afterReset?.receiptAvailable == false)
    }

    func testSubmittedWordsSurvivePhoneRestartWithoutSending() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        func store() -> ClientWorkStore {
            let fixture = ClientWorkStore(directory: directory, machine: { "original-machine" }, read: { _ in "null" }, observe: { _ in
                let stream = AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error> {
                    $0.finish(throwing: RegistryQueryError("Offline"))
                }
                return ServerWatch(updates: stream, checkpoint: { _ in }, cancel: {})
            }, send: { _ in throw RegistryQueryError("No submission before durable recovery") })
            addTeardownBlock { await fixture.close() }
            return fixture
        }
        let original = try await store().record(.taskCreate(directory: "/original-repo", requestID: "same", title: nil,
            request: "Exact phone request", wave: nil, graph: nil, session: nil, run: true), requestID: "same")
        let restarted = store()
        let pending = try await restarted.pending()
        XCTAssertEqual(pending.first?.payload, original.payload)
        XCTAssertEqual(pending.first?.destination, "original-machine")
        do {
            _ = try await restarted.reconcile(original)
            XCTFail("An offline lookup cannot authorize a send")
        } catch { }
        let retained = try await restarted.pending()
        XCTAssertEqual(retained.first?.requestID, "same")
    }
}
