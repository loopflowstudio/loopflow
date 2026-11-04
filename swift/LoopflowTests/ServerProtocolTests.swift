import Foundation
import Testing
@testable import Loopflow

struct ServerProtocolTests {
    private func fixture(_ name: String) throws -> Data {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        return try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/server/\(name).json"))
    }

    @Test func sharedSubscriptionShapeAndUnavailableReading() throws {
        let request = ServerRequest(version: 1, id: "tasks-1", operation: .subscribe(
            .roadmap(directory: "/", wave: nil, task: nil, all: true), generation: 7))
        let encoded = try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request))
        #expect(encoded == (try JSONDecoder().decode(JSONValue.self, from: fixture("subscribe"))))
        let response = try JSONDecoder().decode(ServerResponse.self, from: fixture("reading"))
        #expect(response.id == "tasks-1")
        guard case .reading(let generation, let frame) = response.result else { Issue.record("Missing reading"); return }
        #expect(generation == 7)
        guard case .object(let fields) = frame else { Issue.record("Missing watch frame"); return }
        #expect(fields["body"] == .null)
        #expect(fields["unavailable"] == .string("repository is unavailable"))
    }

    @Test func commandTraceAndExistingOwnerShareRustFixtures() throws {
        let bootstrap = try JSONDecoder().decode(ServerResponse.self, from: fixture("bootstrap"))
        guard case .bootstrap(let version, _, let executable, _, let capabilities, let maxFrame, let maxConnections) = bootstrap.result else {
            Issue.record("Missing bootstrap"); return
        }
        #expect(version == 1)
        #expect(executable.count == 64)
        #expect(capabilities.contains("session_owner"))
        #expect(capabilities.contains("view_plan"))
        #expect(capabilities.contains("reads"))
        #expect(capabilities.contains("subscriptions"))
        #expect(capabilities.contains("commands"))
        #expect(maxFrame == 33554432)
        #expect(maxConnections == 64)
        let command = try JSONDecoder().decode(ServerResponse.self, from: fixture("command"))
        guard case .command(let trace, _, _, let code) = command.result else { Issue.record("Missing command"); return }
        #expect(trace == "85c31c1e-850c-4821-878d-55527b6dba46")
        #expect(code == 0)
        let owner = try JSONDecoder().decode(ServerResponse.self, from: fixture("owner"))
        guard case .sessionOwner(let process) = owner.result else { Issue.record("Missing owner"); return }
        #expect(process == trace)
        let request = ServerRequest(version: 1, id: "owner-1", operation: .sessionOwner(session: "session_5e55105e000040008000000000000001"))
        #expect(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)) == JSONDecoder().decode(JSONValue.self, from: fixture("session_owner")))
        let attach = ServerRequest(version: 1, id: "terminal-1", operation: .attach(owner: trace))
        #expect(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(attach)) == JSONDecoder().decode(JSONValue.self, from: fixture("attach")))
    }

    @Test func primarySessionSelectionUsesTheSharedViewPlanRead() throws {
        let request = ServerRequest(version: 1, id: "view-1", operation: .read(
            .viewPlan(directory: "/repo", expression: "task:INF-123")))
        #expect(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)) == JSONDecoder().decode(JSONValue.self, from: fixture("view_plan")))
    }

    @Test func missingRequiredReplyDoesNotBecomeAnEmptyReading() {
        #expect(throws: (any Error).self) {
            try JSONDecoder().decode(ServerResponse.self, from: Data(#"{"id":"x","instance":"i","result":{"kind":"reading","generation":1}}"#.utf8))
        }
    }
}
