import Foundation
import XCTest
@testable import Loopflow

final class ClientWorkStoreTests: XCTestCase {
    func testSubmittedWordsSurvivePhoneRestartWithoutSending() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        func store() -> ClientWorkStore {
            ClientWorkStore(directory: directory, machine: { "original-machine" }, read: { _ in "null" }, observe: { _ in
                let stream = AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error> {
                    $0.finish(throwing: RegistryQueryError("Offline"))
                }
                return ServerWatch(updates: stream, checkpoint: { _ in }, cancel: {})
            }, send: { _ in throw RegistryQueryError("No submission before durable recovery") })
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
