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
        let set = ConfigOperation.set(destination: .object(["kind": .string("personal_global")]), key: "theme", value: "Hearth")
        #expect(try encoded(.request(id: "config-1", operation: .config(set))) == value("configuration"))
    }

    @Test func sessionAdoptionSharesTheRustWriteContract() throws {
        let request = ServerClientMessage.request(id: "adopt-1", operation: .command(
            .taskFromSession(directory: "/repo", id: "session_5e55105e000040008000000000000001",
                             title: "Keep the conversation")))
        #expect(try encoded(request) == value("task_from_session"))
    }

    @Test func coreLoopWritesShareTheRustWriteContract() throws {
        let writes: [(String, String, ServedWrite)] = [
            ("task_run", "run-1", .taskRun(directory: "/repo", issue: "LOO-1", from: "implement", agent: "claude", approve: false)),
            ("task_ship", "ship-1", .taskShip(directory: "/repo", issue: "LOO-1")),
            ("task_hold", "hold-1", .taskHold(directory: "/repo", issue: "LOO-1")),
            ("task_steer", "steer-1", .taskSteer(directory: "/repo", issue: "LOO-1", message: "Keep the API small")),
            ("task_report", "report-1", .taskReport(directory: "/repo", issue: "LOO-1", message: "Demo recorded")),
            ("task_interrupt", "interrupt-1", .taskInterrupt(directory: "/repo", issue: "LOO-1")),
            ("session_steer", "steer-2", .sessionSteer(directory: "/repo", session: nil, wave: "product",
                                                       message: "Ship the smallest slice first")),
            ("wave_planning_bind", "bind-1", .wavePlanningBind(directory: "/repo", wave: "product",
                                                               project: "5e55105e-0000-4000-8000-000000000042")),
            ("wave_planning_connect", "connect-1", .wavePlanningConnect(directory: "/repo", wave: "product", all: false,
                                                                        teamKey: "LOO", teamName: nil)),
            ("wave_planning_reteam", "reteam-1", .wavePlanningReteam(directory: "/repo", apply: false)),
            ("repository_release", "release-1", .repositoryRelease(directory: "/repo", version: "minor", target: nil)),
            ("checkout_commit", "commit-1", .checkoutCommit(directory: "/repo", message: "Serve the checkout namespace",
                                                            push: false, noAdd: false, paths: [], agent: nil)),
            ("checkout_sync", "sync-1", .checkoutSync(directory: "/repo", onto: nil, preview: true, manual: false,
                                                      continueSync: false, abort: false, adopt: false)),
            ("checkout_create", "create-1", .checkoutCreate(directory: "/repo", name: "spike", preview: false,
                                                            persistent: false)),
        ]
        for (name, id, write) in writes {
            #expect(try encoded(.request(id: id, operation: .command(write))) == value(name), "\(name)")
        }
        #expect(ServedWrite.taskRun(directory: "/repo", issue: "LOO-1", from: nil, agent: nil, approve: true).argv
            == ["--batch", "task", "run", "LOO-1", "--json"])
        guard case .response(_, _, .object(let running)) = try message("task_running") else {
            Issue.record("Missing task.run result"); return
        }
        #expect(running["state"] == .string("running"))
        #expect(running["graph_process"] == .string("85c31c1e-850c-4821-878d-55527b6dba46"))
        guard case .response(_, _, .object(let shipping)) = try message("task_shipping") else {
            Issue.record("Missing task.ship result"); return
        }
        #expect(shipping["state"] == .string("shipping"))
        guard case .response(_, _, .object(let held)) = try message("task_held") else {
            Issue.record("Missing task.hold result"); return
        }
        #expect(held["stopped"] == .bool(true))
        guard case .response(_, _, .object(let steered)) = try message("session_steered") else {
            Issue.record("Missing session.steer result"); return
        }
        #expect(steered["text"] == .string("Ship the smallest slice first"))
        guard case .response(_, _, .array(let connected)) = try message("wave_planning_connected") else {
            Issue.record("Missing wave.planning.connect result"); return
        }
        #expect(connected.count == 1)
        guard case .response(_, _, .object(let reteamed)) = try message("wave_planning_reteamed") else {
            Issue.record("Missing wave.planning.reteam result"); return
        }
        #expect(reteamed["applied"] == .bool(false))
        guard case .response(_, _, .object(let launched)) = try message("launched") else {
            Issue.record("Missing detached result"); return
        }
        #expect(launched["operation"] == .string("repository.release"))
        guard case .response(_, _, .object(let planned)) = try message("checkout_sync_planned") else {
            Issue.record("Missing checkout.sync plan"); return
        }
        #expect(planned["state"] == .string("planned"))
        let ci = ServerClientMessage.request(id: "ci-1", operation: .read(.ci(since: "7d", wave: "product", repo: nil)))
        #expect(try encoded(ci) == value("repository_ci"))
        #expect(ServedRead.ci(since: "7d", wave: nil, repo: nil).argv == ["repository", "ci", "--since", "7d", "--json"])
    }

    @Test func aWaitEndsAfterTheReadingThatReachedIt() throws {
        let wait = ServerClientMessage.subscribe(id: "wait-1", operation: .taskWait(issue: "LOO-1", until: "submitted"))
        #expect(try encoded(wait) == value("task_wait"))
        guard case .ended(let id) = try message("ended") else { Issue.record("Missing ended"); return }
        #expect(id == "wait-1")
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
