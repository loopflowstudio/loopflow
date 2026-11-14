import Foundation
import Testing
@testable import Loopflow

struct ServerMessageOrderTests {
    private func line(_ fields: [String: Any]) throws -> Data {
        try JSONSerialization.data(withJSONObject: fields)
    }

    private func bulk(_ id: String) throws -> Data {
        try line(["type": "bulk", "id": id, "ticket": UUID().uuidString, "bytes": 100])
    }

    private func reading(_ id: String, _ value: String) throws -> Data {
        try line(["type": "reading", "id": id, "checkpoint": value, "frame": value])
    }

    @Test func consecutiveTicketsRetainOrderWhileOtherIDsFlow() throws {
        var order = ServerMessageOrder()
        _ = try order.receive(bulk("watch"))
        #expect(try order.receive(bulk("watch")) == nil)
        #expect(try order.receive(reading("watch", "third")) == nil)
        #expect(try order.receive(reading("other", "independent")) != nil)
        let completedFirst = try order.complete("watch", payload: reading("watch", "first"))
        let (first, queued) = try #require(completedFirst)
        guard case .reading(_, let checkpoint, _) = first else { Issue.record("Missing first reading"); return }
        #expect(checkpoint == "first")
        #expect(queued.count == 2)
        guard case .bulk = try order.resume(queued[0].0, bytes: queued[0].1) else { Issue.record("Missing second ticket"); return }
        #expect(try order.resume(queued[1].0, bytes: queued[1].1) == nil)
        let completedSecond = try order.complete("watch", payload: reading("watch", "second"))
        let (_, remaining) = try #require(completedSecond)
        guard case .reading(_, let last, _) = try order.resume(remaining[0].0, bytes: remaining[0].1) else { Issue.record("Missing last reading"); return }
        #expect(last == "third")
    }

    @Test func cancellationDropsDeferredFramesAndLatePayloads() throws {
        var order = ServerMessageOrder()
        _ = try order.receive(bulk("watch"))
        _ = try order.receive(reading("watch", "later"))
        order.discard("watch")
        #expect(try order.complete("watch", payload: reading("watch", "old")) == nil)
        #expect(try order.receive(reading("other", "current")) != nil)
    }

    @Test func wrongRequestAndNestedTicketsCannotReplaceThePayload() throws {
        var order = ServerMessageOrder()
        _ = try order.receive(bulk("watch"))
        #expect(throws: RegistryQueryError.self) { try order.complete("watch", payload: reading("other", "wrong")) }
        #expect(throws: RegistryQueryError.self) { try order.complete("watch", payload: bulk("watch")) }
    }

    @Test func stalledTransferHasABoundedQueue() throws {
        var order = ServerMessageOrder()
        _ = try order.receive(bulk("watch"))
        for index in 0..<64 { _ = try order.receive(reading("watch", String(index))) }
        #expect(throws: RegistryQueryError.self) { try order.receive(reading("watch", "overflow")) }
        order.discard("watch")
        _ = try order.receive(bulk("again"))
        #expect(try order.receive(reading("again", "room")) == nil)
    }
    @Test func eventsAreSequencedBeforeLargeReadingsDeferTheirDelivery() throws {
        var order = ServerMessageOrder()
        _ = try order.receive(bulk("slow"))
        let event: (String, Int) throws -> Data = { id, seq in
            try line(["type": "event", "id": id, "seq": seq, "event": ["kind": "future-event"]])
        }
        #expect(try order.receive(event("slow", 1)) == nil)
        guard case .event("fast", 2, _) = try order.receive(event("fast", 2)) else {
            Issue.record("Unrelated events must keep flowing"); return
        }
        let completed = try order.complete("slow", payload: reading("slow", "baseline"))
        let (_, queued) = try #require(completed)
        guard case .event("slow", 1, _) = try order.resume(queued[0].0, bytes: queued[0].1) else {
            Issue.record("Deferred event lost its baseline order"); return
        }
        #expect(try order.receive(event("fast", 3)) != nil)
        #expect(throws: ServerEventSequenceGap.self) { try order.receive(event("fast", 5)) }
    }

}
