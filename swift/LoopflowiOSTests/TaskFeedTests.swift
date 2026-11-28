import Foundation
import Loopflow
import ViewInspector
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
    /// Each write's `children`, `"null"` when it sent none.
    private(set) var children: [String] = []
    private var refusal: String?
    private(set) var expression: String?

    /// `openChildren[i]` open children under `LOO-<i + 1>`.
    init(machineID: String, paused: Bool = false, openChildren: [Int: Int] = [:]) throws {
        self.machineID = machineID
        let url = try XCTUnwrap(Bundle(for: TaskFeedTests.self).url(forResource: "task-reading", withExtension: "json"))
        let response = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        var plan = try XCTUnwrap(response["result"] as? [String: Any])
        var contents = try XCTUnwrap(plan["contents"] as? [[String: Any]])
        for index in contents.indices {
            contents[index]["identifier"] = "LOO-\(index + 1)"
            contents[index]["working_group"] = paused ? "paused" : "active"
            contents[index]["states"] = paused ? ["open", "paused", "working"] : ["open", "working", "idle", "available"]
            contents[index]["open_children"] = openChildren[index] ?? 0
        }
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
            expression = (json["operation"] as? [String: Any])?["expression"] as? String
            deliver(reading(id, answers: nil))
        case "checkpoint":
            deliver(reading(id, answers: json["checkpoint"]))
        case "request" where json["request_id"] is String:
            writes.append(try XCTUnwrap(json["request_id"] as? String))
            children.append(((json["operation"] as? [String: Any])?["children"] as? String) ?? "null")
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
    private func client(_ server: HeldServer, machineID: String) -> RemoteMobileClient {
        let endpoint = ServerEndpoint(
            client: ServerClientIdentity(kind: "mobile", platform: "ios", name: "Loopflow", version: "test"),
            control: { server }, ticketed: { throw RegistryQueryError("No ticketed reads here") })
        return RemoteMobileClient(
            savedMachine: SavedMachine(endpoint: URL(string: "ws://fixture.invalid")!, machineID: machineID),
            endpoint: endpoint)
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

    func testUnpauseStaysInWorkingAndARefusalRestoresItsState() async throws {
        let machineID = "HOME_\(UUID().uuidString.lowercased())"
        let server = try HeldServer(machineID: machineID, paused: true)
        let feed = TaskFeed(client: client(server, machineID: machineID), notices: NoticeCenter())
        defer { feed.forget() }
        await feed.start()
        try await until { identifiers(feed).count == 3 }
        let expression = await server.expression
        XCTAssertEqual(expression, "kind:task state:working")
        await server.refuse("land is still finishing")
        feed.pendingChanges.setPause(.unpause, issue: "LOO-2", directory: "/repo", notices: feed.notices, show: nil) {}
        XCTAssertEqual(feed.pendingChanges.controlRequest(for: "LOO-2"), "Unpausing…")
        XCTAssertEqual(identifiers(feed), ["LOO-1", "LOO-2", "LOO-3"])
        XCTAssertTrue(feed.shownRepositories.first?.tasks[1].states.contains(.idle) == true)
        try await until { feed.notices.current != nil }
        XCTAssertEqual(feed.shownRepositories.first?.tasks[1].workingGroup, .paused)
        XCTAssertEqual(feed.shownRepositories.first?.tasks[1].status?.word, "Paused")
        XCTAssertEqual(identifiers(feed), ["LOO-1", "LOO-2", "LOO-3"])
    }

    func testOpeningAPausedTaskShowsItsSessionTerminal() async throws {
        let id = "HOME_\(UUID().uuidString.lowercased())"
        let server = try HeldServer(machineID: id, paused: true)
        let client = client(server, machineID: id)
        let feed = TaskFeed(client: client, notices: NoticeCenter())
        defer { feed.forget() }
        await feed.start()
        try await until { feed.loaded }
        let row = try XCTUnwrap(feed.shownRepositories.first?.tasks.first)
        let workspace = TaskWorkspaceView(client: client, feed: feed, task: MobileTask(row), draft: .constant(""))
        let terminal = try workspace.inspect().find(SessionTerminal.self).actualView()
        XCTAssertEqual(terminal.link.target, .session(try XCTUnwrap(row.primarySession)))
    }

    func testTaskRowShowsItsCurrentGraphStep() async throws {
        let id = "HOME_\(UUID().uuidString.lowercased())"
        let server = try HeldServer(machineID: id)
        let client = client(server, machineID: id)
        let feed = TaskFeed(client: client, notices: NoticeCenter())
        defer { feed.forget() }
        await feed.start()
        try await until { feed.loaded }
        let task = try XCTUnwrap(feed.shownRepositories.first?.tasks.first)
        let list = TaskListView(client: client, machineTheme: MachineTheme(), onForget: {}, onChangeAddress: {})
        XCTAssertEqual(try list.link(task).inspect().find(text: "review").string(), "review")
    }

    func testCompleteHidesTheRowBeforeTheMachineAnswers() async throws {
        let machineID = "HOME_\(UUID().uuidString.lowercased())"
        let server = try HeldServer(machineID: machineID)
        let feed = TaskFeed(client: client(server, machineID: machineID), notices: NoticeCenter())
        defer { feed.forget() }
        await feed.start()
        try await until { identifiers(feed) == ["LOO-1", "LOO-2", "LOO-3"] }
        feed.complete(feed.shownRepositories[0].tasks[1]) {}
        XCTAssertEqual(identifiers(feed), ["LOO-1", "LOO-3"], "gone in the swipe")
        XCTAssertEqual(feed.notices.current?.text, "LOO-2 completed")
        XCTAssertEqual(feed.notices.current?.action?.title, "Undo")
        try await until { await server.writes.count == 1 }
        XCTAssertEqual(identifiers(feed), ["LOO-1", "LOO-3"], "still gone while the Machine holds its answer")
        feed.notices.current?.action?.perform()
        try await until { await server.writes.count == 2 }
    }

    func testASwipedParentStaysUntilAskedWhatHappensToItsChildren() async throws {
        let machineID = "HOME_\(UUID().uuidString.lowercased())"
        let server = try HeldServer(machineID: machineID, openChildren: [1: 3])
        let feed = TaskFeed(client: client(server, machineID: machineID), notices: NoticeCenter())
        defer { feed.forget() }
        await feed.start()
        try await until { identifiers(feed) == ["LOO-1", "LOO-2", "LOO-3"] }
        let parent = feed.shownRepositories[0].tasks[1]
        feed.complete(parent) {}
        XCTAssertEqual(feed.completing?.identifier, "LOO-2")
        XCTAssertEqual(TaskChildPolicy.question(issue: "LOO-2", openChildren: parent.openChildren ?? 0),
                       "Complete LOO-2? 3 children are open")
        XCTAssertEqual(identifiers(feed), ["LOO-1", "LOO-2", "LOO-3"], "the row stays while the dialog asks")
        let unasked = await server.writes
        XCTAssertEqual(unasked, [], "nothing is written before the answer")

        // Completing from the workspace checkmark asks the same question.
        feed.completing = nil
        feed.complete(issue: "LOO-2", directory: "/repo")
        XCTAssertEqual(feed.completing?.identifier, "LOO-2")
        XCTAssertNil(feed.notices.current)
        let stillUnasked = await server.writes
        XCTAssertEqual(stillUnasked, [], "workspace checkmark writes nothing before the answer")

        feed.complete(parent, children: .rehome) {}
        XCTAssertNil(feed.completing)
        XCTAssertEqual(identifiers(feed), ["LOO-1", "LOO-3"])
        try await until { await server.writes.count == 1 }
        let sent = await server.children
        XCTAssertEqual(sent, ["rehome"])
    }

    func testASwipedLeafSendsNoChildren() async throws {
        let machineID = "HOME_\(UUID().uuidString.lowercased())"
        let server = try HeldServer(machineID: machineID, openChildren: [1: 3])
        let feed = TaskFeed(client: client(server, machineID: machineID), notices: NoticeCenter())
        defer { feed.forget() }
        await feed.start()
        try await until { identifiers(feed).count == 3 }
        feed.complete(feed.shownRepositories[0].tasks[0]) {}
        XCTAssertNil(feed.completing)
        try await until { await server.writes.count == 1 }
        let sent = await server.children
        XCTAssertEqual(sent, ["null"])
    }

    func testARefusedCompleteReturnsTheRowAndItsNoticeShowsTheTask() async throws {
        let machineID = "HOME_\(UUID().uuidString.lowercased())"
        let server = try HeldServer(machineID: machineID)
        await server.refuse("its PR #12 is open")
        let feed = TaskFeed(client: client(server, machineID: machineID), notices: NoticeCenter())
        defer { feed.forget() }
        await feed.start()
        try await until { identifiers(feed).count == 3 }
        var shown = false
        feed.complete(feed.shownRepositories[0].tasks[1]) { shown = true }
        try await until { feed.notices.current?.text.contains("couldn't be completed") == true }
        XCTAssertEqual(identifiers(feed), ["LOO-1", "LOO-2", "LOO-3"])
        XCTAssertEqual(feed.notices.current?.text, "LOO-2 couldn't be completed: its PR #12 is open")
        XCTAssertEqual(feed.notices.current?.action?.title, "Show")
        feed.notices.current?.action?.perform()
        XCTAssertTrue(shown)
    }
}
