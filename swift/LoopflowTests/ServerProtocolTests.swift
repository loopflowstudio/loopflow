import Foundation
import Testing
@testable import Loopflow

struct ServerProtocolTests {
    private func fixture(_ name: String) throws -> Data {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        return try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/server/\(name).json"))
    }

    private func encoded(_ message: ServerClientMessage) throws -> JSONValue {
        try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(message))
    }

    private func value(_ name: String) throws -> JSONValue {
        try JSONDecoder().decode(JSONValue.self, from: fixture(name))
    }

    private func message(_ name: String) throws -> ServerMessage {
        try JSONDecoder().decode(ServerMessage.self, from: fixture(name))
    }

    @Test func helloAndWelcomeNegotiateTheRevision() throws {
        let hello = ServerClientMessage.hello(ServerHello(revision: 2, minRevision: 2, client: ServerClientIdentity(
            kind: "desktop", platform: "macos", name: "Loopflow", version: "0.15.1")))
        #expect(try encoded(hello) == value("hello"))
        guard case .welcome(let welcome) = try message("welcome") else { Issue.record("Missing welcome"); return }
        #expect(welcome.clientID == "6f1c2d3e-4b5a-4c6d-8e7f-0a1b2c3d4e5f")
        #expect(welcome.revision == 2)
        #expect(welcome.serverRevision == 2)
        #expect(welcome.minRevision == 2)
        #expect(welcome.instance == "service-incarnation")
        #expect(welcome.executableSHA256.count == 64)
        #expect(welcome.capabilities.contains("session_owner"))
        #expect(welcome.capabilities.contains("subscriptions"))
        #expect(welcome.limits.maxMessage == 33554432)
        #expect(welcome.limits.maxConnections == 64)
        #expect(welcome.machine == .object([:]))
    }

    @Test func readinessProbesWithoutHello() throws {
        #expect(try encoded(.status) == value("status_request"))
        guard case .status(let status) = try message("status") else { Issue.record("Missing status"); return }
        #expect(status.revision == 2)
        #expect(status.minRevision == 2)
        #expect(status.instance == "server-1")
        #expect(status.executableSHA256 == String(repeating: "a", count: 64))
        #expect(status.clients.map(\.id) == ["6f1c2d3e-4b5a-4c6d-8e7f-0a1b2c3d4e5f"])
        #expect(status.clients.first?.client == ServerClientIdentity(kind: "desktop", platform: "macos", name: "Loopflow", version: "0.15.1"))
        #expect(status.clients.first?.requests == 12)
    }

    @Test func subscriptionsCarryTheirRequestId() throws {
        let request = ServerClientMessage.subscribe(id: "tasks-1", operation:
            .roadmap(directory: "/", wave: nil, task: nil, all: true))
        #expect(try encoded(request) == value("subscribe"))
        guard case .reading(let id, let frame) = try message("reading") else { Issue.record("Missing reading"); return }
        #expect(id == "tasks-1")
        guard case .object(let fields) = frame else { Issue.record("Missing watch frame"); return }
        #expect(fields["body"] == .null)
        #expect(fields["unavailable"] == .string("repository is unavailable"))
        #expect(try encoded(.checkpoint(id: "tasks-1", checkpoint: 3)) == value("checkpoint"))
        #expect(try encoded(.cancel(id: "tasks-1")) == value("cancel"))
    }

    @Test func commandTraceAndExistingOwnerShareRustFixtures() throws {
        guard case .response(let id, let trace, .object(let renamed)) = try message("response") else {
            Issue.record("Missing response"); return
        }
        #expect(id == "rename-1")
        #expect(trace == "85c31c1e-850c-4821-878d-55527b6dba46")
        #expect(renamed["title"] == .string("Served name"))
        guard case .response(_, _, .object(let owned)) = try message("owner"), case .string(let process)? = owned["owner"] else {
            Issue.record("Missing owner"); return
        }
        #expect(process == trace)
        let owner = ServerClientMessage.request(id: "owner-1", operation: .sessionOwner(session: "session_5e55105e000040008000000000000001"))
        #expect(try encoded(owner) == self.value("session_owner"))
        let attach = ServerClientMessage.request(id: "terminal-1", operation: .attach(owner: trace))
        #expect(try encoded(attach) == self.value("attach"))
    }

    @Test func primarySessionSelectionUsesTheSharedViewPlanRead() throws {
        let request = ServerClientMessage.request(id: "view-1", operation: .read(
            .viewPlan(directory: "/repo", expression: "task:INF-123")))
        #expect(try encoded(request) == value("view_plan"))
    }

    @Test func configurationUsesTheSharedDestinationContract() throws {
        let raw = try value("configuration")
        guard case .object(let root) = raw, case .object(let operation) = root["operation"],
              let request = operation["request"] else { Issue.record("Missing configuration request"); return }
        #expect(try encoded(.request(id: "config-1", operation: .configuration(request))) == raw)
    }

    @Test func sessionAdoptionSharesTheRustWriteContract() throws {
        let request = ServerClientMessage.request(id: "adopt-1", operation: .command(
            .taskFromSession(directory: "/repo", id: "session_5e55105e000040008000000000000001",
                             title: "Keep the conversation")))
        #expect(try encoded(request) == value("task_from_session"))
    }

    @Test func connectionErrorsHaveNoRequestId() throws {
        guard case .error(let id, let code, let message, let data) = try message("error") else { Issue.record("Missing error"); return }
        #expect(id == nil)
        #expect(code == "incompatible")
        #expect(message.contains("update the selected installation"))
        guard case .object(let detail) = data else { Issue.record("Missing error data"); return }
        #expect(detail["update"] == .string("server"))
        guard case .error(let missing, let absent, _, .object(let named)) = try self.message("not_found") else {
            Issue.record("Missing not_found"); return
        }
        #expect(missing == "view-1")
        #expect(absent == "not_found")
        #expect(named["expression"] == .string("task:INF-404"))
    }

    @Test func missingRequiredFieldsDoNotBecomeDefaults() {
        #expect(throws: (any Error).self) {
            try JSONDecoder().decode(ServerMessage.self, from: Data(#"{"type":"reading","id":"x"}"#.utf8))
        }
        #expect(throws: (any Error).self) {
            try JSONDecoder().decode(ServerMessage.self, from: Data(#"{"type":"welcome","revision":2}"#.utf8))
        }
        #expect(throws: (any Error).self) {
            try JSONDecoder().decode(ServerMessage.self, from: Data(#"{"type":"response","id":"x","result":{"kind":"bootstrap"}}"#.utf8))
        }
    }
}
