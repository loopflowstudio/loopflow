import Foundation
import XCTest
import Loopflow
@testable import LoopflowiOS

final class ReadingStampTests: XCTestCase {
    func testTypedFieldSupportKeepsSQLiteIndependentOfUnavailableActivityOnPhone() throws {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "membership_fields", withExtension: "json"))
        let data = try Data(contentsOf: url)
        let facts = try JSONDecoder().decode([WorkMembershipPlan.Facts].self, from: data)
        XCTAssertEqual(facts[0].parent.support?.revisions, ["planning": 7])
        XCTAssertEqual(facts[0].parent.support?.observations, .sqlite)
        XCTAssertEqual(facts[0].activity.support?.observations, .unavailable)
        guard case .captured(let evidence) = facts[1].activity.support?.observations else {
            XCTFail("Empty validated capture must remain distinct from unavailable"); return
        }
        XCTAssertTrue(evidence.dependencies.isEmpty)
        XCTAssertEqual(try JSONDecoder().decode([WorkMembershipPlan.Facts].self, from: JSONEncoder().encode(facts)), facts)
        for field in ["parent", "lifecycle", "activity"] {
            var raw = try XCTUnwrap((JSONSerialization.jsonObject(with: data) as? [[String: Any]])?.first)
            var value = try XCTUnwrap(raw[field] as? [String: Any])
            value.removeValue(forKey: "support"); raw[field] = value
            XCTAssertThrowsError(try JSONDecoder().decode(WorkMembershipPlan.Facts.self,
                from: JSONSerialization.data(withJSONObject: raw)))
        }
    }

    func testCompletionPlanningEvidenceUsesTheSharedStateDeliveryOnPhone() throws {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "operation_state_planning", withExtension: "json", subdirectory: "server"))
        let result = try XCTUnwrap(try RawJSON(Data(contentsOf: url)).fields()["result"])
        let receipt = try result.decode(OperationReceipt.self)
        let current = try XCTUnwrap(receipt.planning?.changes.first)
        XCTAssertEqual(current.change.kind, .taskState)
        XCTAssertEqual(current.change, receipt.commits.first?.planningChanges.first)
        XCTAssertEqual(current.state, .uncertain)
        XCTAssertEqual(receipt.delivery, .finished)
        XCTAssertEqual(receipt.result, receipt.commits.first?.result)
    }
    func testOperationReceiptUsesTheSharedDecoderOnPhone() throws {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "operation_receipt", withExtension: "json", subdirectory: "server"))
        let result = try XCTUnwrap(try RawJSON(Data(contentsOf: url)).fields()["result"])
        let receipt = try result.decode(OperationReceipt.self)
        XCTAssertEqual(receipt.delivery, .uncertain)
        XCTAssertEqual(receipt.validation, .bound)
        XCTAssertNil(receipt.fingerprint)
        XCTAssertNotNil(receipt.submittedFingerprint)
        XCTAssertEqual(receipt.commits.first?.fence.revisions["planning"], 7)
        XCTAssertEqual(receipt.commits.map(\.resolution), [.committed, .superseded])
        XCTAssertEqual(receipt.commits.last?.stage, "task.completed")
        XCTAssertEqual(receipt.commits.last?.fence.revisions["planning"], 8)
        XCTAssertNil(receipt.result)
        XCTAssertNotNil(receipt.error)
    }

    func testCurrentPlanningEvidenceUsesTheSharedDecoderOnPhone() throws {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "operation_planning", withExtension: "json", subdirectory: "server"))
        let result = try XCTUnwrap(try RawJSON(Data(contentsOf: url)).fields()["result"])
        let receipt = try result.decode(OperationReceipt.self)
        let current = try XCTUnwrap(receipt.planning)
        XCTAssertEqual(current.changes.map(\.state), [.pending, .uncertain, .delivered, .superseded, .conflict, .unavailable])
        XCTAssertEqual(current.changes.map(\.change), receipt.commits.flatMap(\.planningChanges))
        XCTAssertEqual(current.fence.revisions["planning"], 12)
        XCTAssertEqual(receipt.commits.first?.fence.revisions["planning"], 7)
        XCTAssertEqual(receipt.error, "planning response lost")
        XCTAssertTrue(current.changes.dropFirst(2).allSatisfy { $0.error == nil })
    }

    func testAdmissionAndCandidateValidationStayDistinctOnPhone() throws {
        for (name, state) in [("operation_pending", OperationReceipt.Validation.pending), ("operation_candidate", .candidate), ("operation_conflict", .conflict)] {
            let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: name, withExtension: "json", subdirectory: "server"))
            let result = try XCTUnwrap(try RawJSON(Data(contentsOf: url)).fields()["result"])
            let receipt = try result.decode(OperationReceipt.self)
            XCTAssertEqual(receipt.validation, state)
            XCTAssertEqual(receipt.fingerprint == nil, state == .pending)
            XCTAssertNotNil(receipt.submittedFingerprint)
            XCTAssertTrue(receipt.commits.isEmpty)
        }
    }

    func testStampedFrameUsesTheSharedDecoderOnPhone() throws {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "reading_stamped", withExtension: "json", subdirectory: "server"))
        let message = RawJSON(try Data(contentsOf: url))
        let frame = try XCTUnwrap(try message.fields()["frame"])
        let reading = try frame.decode(SubscriptionReading<[WorkViewPlan]>.self)
        let stamp = try XCTUnwrap(reading.stamp)
        XCTAssertEqual(stamp.schema, UInt32(ServerProtocol.revision))
        XCTAssertEqual(stamp.revisions.sessions, 11)
        XCTAssertEqual(stamp.observations?.source, "fixture-observer")
        XCTAssertEqual(stamp.observations?.validatedAt, 19)
        XCTAssertEqual(stamp.observations?.dependencies.first?.token, 17)
        XCTAssertTrue(stamp.complete)
        let decoded = try RemoteTaskObservation.decode(frame: frame)
        XCTAssertTrue(try decoded.reading.get().isEmpty)
        var missing = try XCTUnwrap(JSONSerialization.jsonObject(with: frame.bytes) as? [String: Any])
        missing.removeValue(forKey: "stamp")
        XCTAssertThrowsError(try JSONDecoder().decode(SubscriptionReading<[WorkViewPlan]>.self,
            from: JSONSerialization.data(withJSONObject: missing)))
    }
}
