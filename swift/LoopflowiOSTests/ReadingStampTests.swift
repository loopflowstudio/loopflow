import Foundation
import XCTest
import Loopflow
@testable import LoopflowiOS

final class ReadingStampTests: XCTestCase {
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
