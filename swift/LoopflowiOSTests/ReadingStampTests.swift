import Foundation
import XCTest
import Loopflow
@testable import LoopflowiOS

final class ReadingStampTests: XCTestCase {
    func testOperationReceiptUsesTheSharedDecoderOnPhone() throws {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "operation_receipt", withExtension: "json", subdirectory: "server"))
        let result = try XCTUnwrap(try RawJSON(Data(contentsOf: url)).fields()["result"])
        let receipt = try result.decode(OperationReceipt.self)
        XCTAssertEqual(receipt.delivery, .uncertain)
        XCTAssertEqual(receipt.commits.first?.fence.revisions["planning"], 7)
        XCTAssertNil(receipt.result)
        XCTAssertNotNil(receipt.error)
    }

    func testStampedFrameUsesTheSharedDecoderOnPhone() throws {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "reading_stamped", withExtension: "json", subdirectory: "server"))
        let message = RawJSON(try Data(contentsOf: url))
        let frame = try XCTUnwrap(try message.fields()["frame"])
        let reading = try frame.decode(SubscriptionReading<[WorkViewPlan]>.self)
        let stamp = try XCTUnwrap(reading.stamp)
        XCTAssertEqual(stamp.schema, UInt32(ServerProtocol.revision))
        XCTAssertEqual(stamp.revisions.sessions, 11)
        XCTAssertTrue(stamp.complete)
        let decoded = try RemoteTaskObservation.decode(frame: frame)
        XCTAssertTrue(try decoded.reading.get().isEmpty)
        var missing = try XCTUnwrap(JSONSerialization.jsonObject(with: frame.bytes) as? [String: Any])
        missing.removeValue(forKey: "stamp")
        XCTAssertThrowsError(try JSONDecoder().decode(SubscriptionReading<[WorkViewPlan]>.self,
            from: JSONSerialization.data(withJSONObject: missing)))
    }
}
