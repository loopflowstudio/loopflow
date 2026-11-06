import Foundation
import Testing
@testable import Loopflow

struct ServerProtocolTests {
    private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()

    private func fixture(_ name: String) throws -> Data {
        try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/server/\(name).json"))
    }

    /// What `lf server describe --json` prints; a Rust test holds it to the wire types.
    private func schema() throws -> [String: Any] {
        let data = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/server_schema.json"))
        return try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    /// The arms of a tagged definition, by the value of their `tag`: the fields each carries.
    private func arms(_ definition: String, tag: String, of schema: [String: Any]) throws -> [String: Set<String>] {
        let definitions = try #require(schema["$defs"] as? [String: Any])
        let arms = try #require((definitions[definition] as? [String: Any])?["oneOf"] as? [[String: Any]])
        return try Dictionary(uniqueKeysWithValues: arms.map { arm in
            let properties = try #require(arm["properties"] as? [String: [String: Any]])
            return (try #require(properties[tag]?["const"] as? String), Set(properties.keys).subtracting([tag]))
        })
    }

    private func source(_ path: String) throws -> String {
        try String(contentsOf: root.appendingPathComponent(path), encoding: .utf8)
    }

    private func matches(_ pattern: String, in text: String) throws -> [[String?]] {
        let whole = NSRange(text.startIndex..., in: text)
        return try NSRegularExpression(pattern: pattern).matches(in: text, range: whole).map { match in
            (0..<match.numberOfRanges).map { Range(match.range(at: $0), in: text).map { String(text[$0]) } }
        }
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
        guard case .object(let machine) = welcome.machine else { Issue.record("Missing Machine"); return }
        #expect(machine["label"] == .string("studio"))
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
            ("account_disconnect", "disconnect-1", .accountDisconnect(provider: "claude", email: "maya@example.com")),
            ("account_connect", "connect-1", .accountConnect(provider: "codex", email: "maya@example.com",
                                                             chromeProfile: "Profile 1", import: false, apiKey: false)),
            ("account_set", "set-1", .accountSet(provider: "codex", email: "maya@example.com", loginEmail: nil,
                                                 routing: "active", plan: nil, clearPlan: false, paidThrough: nil,
                                                 clearPaidThrough: false, clearCooldown: true, chromeProfiles: [],
                                                 clearChromeProfiles: false)),
            ("account_token", "token-1", .accountToken(provider: "claude", email: "maya@example.com",
                                                       file: "/Users/maya/.lf/tmp/token", chromeProfile: nil)),
            ("account_redeem_reset", "redeem-1", .accountRedeemReset(provider: "codex", email: "maya@example.com",
                                                                     idempotencyKey: "4b0e1f2a-9c3d-4e5f-8a6b-7c8d9e0f1a2b",
                                                                     creditId: nil)),
            ("account_use", "use-1", .accountUse(provider: "claude", email: "maya@example.com")),
            ("graph_run", "graph-1", .graphRun(directory: "/repo", name: "code", message: "Serve the graph namespace",
                                               agent: "claude")),
            ("skill_run", "skill-1", .skillRun(directory: "/repo", name: "debug", message: nil, agent: nil)),
            ("pr_publish", "publish-1", .prPublish(directory: "/repo", title: "Serve the pr namespace", body: nil,
                                                   agent: nil)),
            ("pr_open", "open-1", .prOpen(directory: "/repo", title: nil, body: nil, agent: "claude")),
            ("pr_submit", "submit-1", .prSubmit(directory: "/repo", strict: false, worktree: nil, message: nil,
                                                title: nil, body: nil, agent: nil)),
            ("pr_arm", "arm-1", .prArm(directory: "/repo", strict: true, local: false, worktree: nil, message: nil,
                                       title: nil, body: nil, agent: nil)),
            ("pr_land", "land-1", .prLand(directory: "/repo", waitAndFix: true, strict: false, local: false,
                                          worktree: nil, message: nil, title: "Serve the pr namespace", body: nil,
                                          agent: nil)),
            ("pr_abandon", "abandon-1", .prAbandon(directory: "/repo", branch: "spike", force: true)),
            ("pr_reconcile", "reconcile-1", .prReconcile(directory: "/repo")),
            ("pr_request_review", "review-1", .prRequestReview(directory: "/repo", pr: "1813",
                                                               reviewers: ["alice", "org/platform"])),
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
        #expect(ServedWrite.prLand(directory: "/repo", waitAndFix: true, strict: false, local: false, worktree: nil,
                                   message: nil, title: "Serve the pr namespace", body: nil, agent: nil).argv
            == ["pr", "land", "--wait-and-fix", "--title", "Serve the pr namespace", "--json"])
        guard case .response(_, _, .object(let published)) = try message("pr_published") else {
            Issue.record("Missing pr.publish result"); return
        }
        #expect(published["created"] == .bool(false))
        guard case .response(_, _, .object(let checks)) = try message("pr_checks_report") else {
            Issue.record("Missing pr.checks result"); return
        }
        #expect(checks["state"] == .string("pending"))
        let checksRead = ServerClientMessage.request(id: "checks-1",
                                                     operation: .read(.prChecks(directory: "/repo", required: true)))
        #expect(try encoded(checksRead) == value("pr_checks"))
        let reviewsRead = ServerClientMessage.request(id: "reviews-1",
                                                      operation: .read(.prReviews(directory: "/repo", pr: nil)))
        #expect(try encoded(reviewsRead) == value("pr_reviews"))
        #expect(ServedRead.prReviews(directory: "/repo", pr: "1813").argv == ["pr", "reviews", "1813", "--json"])
        #expect(ServedWrite.graphRun(directory: "/repo", name: "code", message: nil, agent: nil).argv
            == ["--batch", "graph", "code"])
        let list = ServerClientMessage.request(id: "list-1", operation: .read(.skillList(directory: "/repo", path: [])))
        #expect(try encoded(list) == value("skill_list"))
        guard case .response(_, _, .array(let entries)) = try message("skill_entries") else {
            Issue.record("Missing skill.list result"); return
        }
        #expect(entries.count == 2)
        #expect(ServedWrite.accountUse(provider: "claude", email: "maya@example.com").cwd == "/")
        let accounts = ServerClientMessage.request(id: "accounts-1",
                                                   operation: .read(.accountList(provider: "claude", cached: true,
                                                                                 details: false)))
        #expect(try encoded(accounts) == value("account_list"))
        let route = ServerClientMessage.request(id: "route-1",
                                                operation: .read(.accountRoute(directory: "/repo", repo: nil,
                                                                               default: false)))
        #expect(try encoded(route) == value("account_route"))
        guard case .response(_, _, .object(let report)) = try message("account_report") else {
            Issue.record("Missing account.list result"); return
        }
        #expect(report["browser"] == .null)
        guard case .response(_, _, .object(let redeemed)) = try message("account_redeemed") else {
            Issue.record("Missing account.redeem_reset result"); return
        }
        #expect(redeemed["after"] == .null)
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

    @Test func theMirrorSpeaksTheDescribedRevision() throws {
        let schema = try schema()
        #expect(schema["x-revision"] as? Int == ServerProtocol.revision)
        #expect(schema["x-min-revision"] as? Int == ServerProtocol.minRevision)
    }

    /// Every operation Desktop can encode, read from its source so that no
    /// case escapes: the kind exists and carries exactly the described fields.
    @Test func everyEncodedOperationIsTheDescribedOperation() throws {
        let schema = try schema()
        let described = try ["ServerOperation", "Read", "Write"].reduce(into: [String: Set<String>]()) { all, definition in
            all.merge(try arms(definition, tag: "kind", of: schema)) { first, _ in first }
        }
        let registry = try source("swift/Loopflow/Services/RegistryQuery.swift")
        let call = #"TaggedFields\(\s*(?:\w+ \? )?"([a-z_.]+)"(?: : "([a-z_.]+)")?(?:,\s*\[((?:[^\[\]]|\[[^\]]*\])*)\])?\s*\)"#
        let calls = try matches(call, in: registry)
        #expect(calls.count == registry.components(separatedBy: "TaggedFields(").count - 1, "A TaggedFields call has a shape this test cannot read")
        for groups in calls {
            let fields = try Set(matches(#""(\w+)":"#, in: groups[3] ?? "").compactMap { $0[1] })
            for kind in groups[1...2].compactMap({ $0 }) {
                #expect(described[kind] == fields, "\(kind): `lf server describe --json` has \(described[kind].map { $0.sorted() } ?? []), Swift encodes \(fields.sorted())")
            }
        }
        for groups in try matches(#""([a-z]+(?:\.[a-z_]+)+)""#, in: source("swift/Loopflow/Services/ServerProtocol.swift")) {
            let kind = try #require(groups[1])
            #expect(described[kind] != nil, "\(kind) is not in `lf server describe --json`")
        }
    }

    /// A fixture the mirror neither encodes nor decodes is one it can drift from.
    @Test func everyFixtureIsMirrored() throws {
        let client = try arms("ClientMessage", tag: "type", of: schema())
        let tests = try source("swift/LoopflowTests/ServerProtocolTests.swift")
        let directory = root.appendingPathComponent("tests/fixtures/dto/server")
        for url in try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil) {
            let name = url.deletingPathExtension().lastPathComponent
            let object = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
            let type = try #require(object["type"] as? String)
            if client[type] == Set(object.keys).subtracting(["type"]) {
                #expect(tests.contains("\"\(name)\""), "No test here encodes the client fixture \(name)")
            } else {
                #expect(throws: Never.self, "\(name)") { try message(name) }
            }
        }
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
