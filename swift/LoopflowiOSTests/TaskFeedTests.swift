import Foundation
import Loopflow
import XCTest
@testable import LoopflowiOS

/// A Machine that speaks the server protocol from memory: it welcomes the phone,
/// answers the Task subscription with `LOO-1`…`LOO-3`, and holds every write
/// unanswered, or refuses it.
private actor HeldServer: ServerCarrier {
    private let machineID: String
    private let plan: Any
    private var outgoing: [Data] = []
    private var receiving: CheckedContinuation<Data?, any Error>?
    private(set) var writes: [String] = []
    private var refusal: String?

    init(machineID: String) throws {
        self.machineID = machineID
        let url = try XCTUnwrap(Bundle(for: TaskFeedTests.self).url(forResource: "task-reading", withExtension: "json"))
        let response = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        var plan = try XCTUnwrap(response["result"] as? [String: Any])
        var contents = try XCTUnwrap(plan["contents"] as? [[String: Any]])
        for index in contents.indices { contents[index]["identifier"] = "LOO-\(index + 1)" }
        plan["contents"] = contents
        self.plan = plan
    }

    func refuse(_ message: String) { refusal = message }

    func send(_ message: Data) async throws {
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: message) as? [String: Any])
        let id = json["id"] as? String
        switch json["type"] as? String {
        case "hello":
            deliver(["type": "welcome", "client_id": "phone", "revision": ServerProtocol.revision,
                     "server_revision": ServerProtocol.revision, "min_revision": ServerProtocol.minRevision,
                     "instance": "fixture", "build": "fixture", "executable_sha256": "fixture",
                     "machine": ["label": "studio", "id": machineID, "route": "local",
                                 "created_at": "2026-11-05T12:00:00Z", "observed_at": "2026-11-05T12:00:00Z"],
                     "limits": ["max_message": 1_048_576, "max_connections": 64, "max_open": 256,
                                "max_retained_bytes": 67_108_864]])
        case "subscribe":
            deliver(reading(id, answers: nil))
        case "checkpoint":
            deliver(reading(id, answers: json["checkpoint"]))
        case "request" where json["request_id"] is String:
            writes.append(try XCTUnwrap(json["request_id"] as? String))
            if let refusal {
                deliver(["type": "error", "id": id as Any, "code": "conflict", "message": refusal,
                         "data": NSNull(), "revisions": NSNull()])
            }
        case "request":
            deliver(["type": "error", "id": id as Any, "code": "not_found", "message": "not read here",
                     "data": NSNull(), "revisions": NSNull()])
        default:
            break
        }
    }

    func receive() async throws -> Data? {
        if !outgoing.isEmpty { return outgoing.removeFirst() }
        return try await withCheckedThrowingContinuation { receiving = $0 }
    }

    func close() async {
        receiving?.resume(returning: nil)
        receiving = nil
    }

    private func reading(_ id: String?, answers: Any?) -> [String: Any] {
        ["type": "reading", "id": id as Any, "checkpoint": "fixture",
         "frame": ["answers": answers ?? NSNull(), "unavailable": NSNull(), "body": [plan], "work": NSNull(),
                   "read_at": ["planning": 4]] as [String: Any]]
    }

    private func deliver(_ message: [String: Any]) {
        guard let line = try? JSONSerialization.data(withJSONObject: message) else { return }
        if let receiving {
            self.receiving = nil
            receiving.resume(returning: line)
        } else {
            outgoing.append(line)
        }
    }
}

@MainActor
final class TaskFeedTests: XCTestCase {
    private func feed(_ server: HeldServer, machineID: String) -> TaskFeed {
        let endpoint = ServerEndpoint(
            client: ServerClientIdentity(kind: "mobile", platform: "ios", name: "Loopflow", version: "test"),
            control: { server }, ticketed: { throw RegistryQueryError("No ticketed reads here") })
        let client = RemoteMobileClient(
            savedMachine: SavedMachine(endpoint: URL(string: "ws://fixture.invalid")!, machineID: machineID),
            endpoint: endpoint)
        return TaskFeed(client: client, notices: NoticeCenter())
    }

    private func identifiers(_ feed: TaskFeed) -> [String] {
        feed.shownRepositories.flatMap(\.tasks).compactMap(\.identifier)
    }

    private func until(_ condition: () async -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(10)
        while await !condition() {
            guard ContinuousClock.now < deadline else { return XCTFail("condition did not hold in time") }
            try await Task.sleep(for: .milliseconds(10))
        }
    }

    func testCompleteHidesTheRowBeforeTheMachineAnswers() async throws {
        let machineID = "HOME_\(UUID().uuidString.lowercased())"
        let server = try HeldServer(machineID: machineID)
        let feed = feed(server, machineID: machineID)
        defer { feed.forget() }
        await feed.start()
        try await until { identifiers(feed) == ["LOO-1", "LOO-2", "LOO-3"] }
        feed.complete(feed.shownRepositories[0].tasks[1]) {}
        XCTAssertEqual(identifiers(feed), ["LOO-1", "LOO-3"], "gone in the swipe")
        try await until { await server.writes.count == 1 }
        XCTAssertEqual(identifiers(feed), ["LOO-1", "LOO-3"], "still gone while the Machine holds its answer")
    }

    func testARefusedCompleteReturnsTheRowAndItsNoticeShowsTheTask() async throws {
        let machineID = "HOME_\(UUID().uuidString.lowercased())"
        let server = try HeldServer(machineID: machineID)
        await server.refuse("its PR #12 is open")
        let feed = feed(server, machineID: machineID)
        defer { feed.forget() }
        await feed.start()
        try await until { identifiers(feed).count == 3 }
        var shown = false
        feed.complete(feed.shownRepositories[0].tasks[1]) { shown = true }
        try await until { feed.notices.current != nil }
        XCTAssertEqual(identifiers(feed), ["LOO-1", "LOO-2", "LOO-3"])
        XCTAssertEqual(feed.notices.current?.text, "LOO-2 couldn't be completed: its PR #12 is open")
        XCTAssertEqual(feed.notices.current?.action?.title, "Show")
        feed.notices.current?.action?.perform()
        XCTAssertTrue(shown)
    }
}
