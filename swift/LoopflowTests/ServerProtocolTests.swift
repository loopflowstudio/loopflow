import Foundation
import Testing
@testable import Loopflow

struct ServerProtocolTests {
    private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()

    private func fixture(_ name: String) throws -> Data {
        try Data(contentsOf: root.appendingPathComponent("rust/loopflow/tests/fixtures/dto/server/\(name).json"))
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
        try ServerMessage(line: fixture(name))
    }

    /// An answer with its payload decoded, as the caller who asked would.
    private enum Answer { case response(String, String, JSONValue), reading(String, String, JSONValue) }
    private func answer(_ name: String) throws -> Answer? {
        switch try message(name) {
        case .response(let id, let trace, let result): .response(id, trace, try result.decode(JSONValue.self))
        case .reading(let id, let checkpoint, let frame): .reading(id, checkpoint, try frame.decode(JSONValue.self))
        default: nil
        }
    }

    @Test func helloAndWelcomeNegotiateTheRevision() throws {
        let hello = ServerClientMessage.hello(ServerHello(revision: ServerProtocol.revision, minRevision: ServerProtocol.minRevision, client: ServerClientIdentity(
            kind: "desktop", platform: "macos", name: "Loopflow", version: "0.15.1")))
        #expect(try encoded(hello) == value("hello"))
        guard case .welcome(let welcome) = try message("welcome") else { Issue.record("Missing welcome"); return }
        #expect(welcome.clientID == "6f1c2d3e-4b5a-4c6d-8e7f-0a1b2c3d4e5f")
        #expect(welcome.revision == ServerProtocol.revision)
        #expect(welcome.serverRevision == ServerProtocol.revision)
        #expect(welcome.minRevision == ServerProtocol.minRevision)
        #expect(welcome.instance == "service-incarnation")
        #expect(welcome.executableSHA256.count == 64)
        #expect(welcome.limits.maxMessage == 1_048_576)
        #expect(welcome.limits.maxConnections == 64)
        guard case .object(let machine) = welcome.machine else { Issue.record("Missing Machine"); return }
        #expect(machine["label"] == .string("studio"))
    }

    @Test func readinessProbesWithoutHello() throws {
        #expect(try encoded(.status) == value("status_request"))
        guard case .status(let status) = try message("status") else { Issue.record("Missing status"); return }
        #expect(status.revision == ServerProtocol.revision)
        #expect(status.minRevision == ServerProtocol.minRevision)
        #expect(status.instance == "server-1")
        #expect(status.executableSHA256 == String(repeating: "a", count: 64))
        #expect(status.clients.map(\.id) == ["6f1c2d3e-4b5a-4c6d-8e7f-0a1b2c3d4e5f"])
        #expect(status.clients.first?.client == ServerClientIdentity(kind: "desktop", platform: "macos", name: "Loopflow", version: "0.15.1"))
        #expect(status.clients.first?.requests == 12)
    }

    @Test func errorsBranchOnTheirCode() {
        let unknown = ServerError(code: "outcome_unknown", message: "launch ended without a result", data: .null)
        #expect(unknown.code == .outcomeUnknown)
        #expect(unknown.mayHaveHappened)
        #expect(unknown.localizedDescription == "launch ended without a result. It may have happened; refresh before retrying.")
        let busy = ServerError(code: "busy", message: "too many connections", data: .null)
        #expect(busy.code == .busy && !busy.mayHaveHappened)
        // A newer server's code is still an error this client can show.
        #expect(ServerError(code: "throttled", message: "later", data: .null).code == nil)
    }

    @Test func subscriptionsCarryTheirRequestId() throws {
        let request = ServerClientMessage.subscribe(id: "tasks-1", operation:
            .planList(directory: "/", wave: nil, task: nil, all: true), select: ["tasks.items.task.identifier", "tasks.items.task.title", "tasks.items.section"], resume: "fixture-checkpoint")
        #expect(try encoded(request) == value("subscribe"))
        guard case .reading(let id, let checkpoint, let frame) = try answer("reading") else { Issue.record("Missing reading"); return }
        #expect(id == "tasks-1")
        #expect(checkpoint == "fixture-checkpoint")
        guard case .object(let fields) = frame else { Issue.record("Missing subscription frame"); return }
        #expect(fields["body"] == .null)
        #expect(fields["unavailable"] == .string("repository is unavailable"))
        #expect(try encoded(.checkpoint(id: "tasks-1", checkpoint: 3)) == value("checkpoint"))
        #expect(try encoded(.cancel(id: "tasks-1")) == value("cancel"))
    }

    @Test func commandTraceAndHeldTerminalShareRustFixtures() throws {
        guard case .response(let id, let trace, .object(let renamed)) = try answer("response") else {
            Issue.record("Missing response"); return
        }
        #expect(id == "rename-1")
        #expect(trace == "85c31c1e-850c-4821-878d-55527b6dba46")
        #expect(renamed["title"] == .string("Served name"))
        let terminal = "7c1d9a40-e5b2-4f6a-8d3e-2b1c0f9a8e7d"
        guard case .response(_, _, .object(let issued)) = try answer("attached"),
              case .string(let attached)? = issued["terminal_id"], case .string(let ticket)? = issued["ticket"] else {
            Issue.record("Missing ticket"); return
        }
        #expect(attached == terminal)
        #expect(issued["expires_in_ms"] == .integer(60000))
        let attach = ServerClientMessage.request(id: "terminal-1", operation: .sessionAttach(
            session: "session_5e55105e000040008000000000000001"), select: nil)
        #expect(try encoded(attach) == self.value("attach"))
        // The bridge presents the ticket on a connection of its own.
        #expect(try encoded(.attach(ticket: ticket)) == value("attach_ticket"))
        // A held terminal outlives server restarts and is named by its terminal.
        guard case .bridged(let held) = try message("bridged") else { Issue.record("Missing bridge"); return }
        #expect(held == terminal)
        let start = ServerClientMessage.request(id: "start-1", operation: .sessionStart(
            session: "session_5e55105e000040008000000000000001", request: "0b9c1f0e-6d1c-4a43-9f0e-2f6c8f0f5a11",
            argv: ["/usr/local/bin/lf", "session", "resume", "session_5e55105e000040008000000000000001"], cwd: "/Users/me/src/app",
            env: [("PATH", "/usr/bin:/bin"), ("HOME", "/Users/me")], rows: 24, cols: 80, placement: "desktop-shell"), select: nil)
        #expect(try encoded(start) == value("session_start"))
    }

    @Test func ticketsAdmitTerminalAndBulkConnections() throws {
        guard case .response("terminal-1", _, let result) = try answer("ticket_reply"),
              case .string(let ticket) = result["ticket"] else {
            Issue.record("Missing ticket"); return
        }
        #expect(ticket == "f06b5872-b113-47e5-be82-4320e33d7a3d")
        #expect(try encoded(.attach(ticket: ticket)) == value("ticket"))
        #expect(try encoded(.request(id: "save-1", operation: .requestUpload(bytes: 2_097_152), select: nil)) == value("upload"))
        guard case .bulk(let id, let bulkTicket, let bytes) = try message("bulk") else { Issue.record("Missing bulk"); return }
        #expect(id == "file-1")
        #expect(bulkTicket == ticket)
        #expect(bytes == 2_097_152)
        guard case .admitted = try message("admitted") else { Issue.record("Missing admitted"); return }
    }

    @Test func mobileOperationsUseTheSharedFixtures() throws {
        let task = ServedWrite.taskCreate(directory: "/repo", requestID: "00000000-0000-4000-8000-000000000001", title: nil, request: "Fix the retry test", wave: nil, graph: nil, session: nil, run: true)
        #expect(try encoded(.request(id: "create-1", operation: .command(task), select: nil)) == value("task_create"))
        #expect(try encoded(.request(id: "sessions-1", operation: .read(.sessionList(
            directory: "/repo", headless: false, after: nil, limit: 100, task: "task_00000000000000000000000000000001")), select: nil)) == value("session_page"))
        #expect(try encoded(.request(id: "continue-1", operation: .sessionContinue(
            session: "session_5e55105e000040008000000000000001", request: "3f2a7c1e-9b4d-4e8a-a1c6-5d0e7f9b2c34",
            rows: 40, cols: 60), select: nil)) == value("session_continue"))
        #expect(try encoded(.subscribe(id: "tasks-1", operation: .viewSelect(expression: "kind:task state:open"), select: nil, resume: nil)) == value("view_select"))
    }

    @Test func hookBoundarySharesTheRustServerContract() throws {
        let boundary = ServerClientMessage.request(id: "stop", operation: .sessionBoundary(
            moment: "stop", delivery: "/Users/maya/.lf/context-deliveries/0b9f2c1e.json",
            adapter: .object(["session": .string("session_5e55105e000040008000000000000001"),
                              "agent_process_id": .string("85c31c1e-850c-4821-878d-55527b6dba46"),
                              "capability": .string("4b0e1f2a-9c3d-4e5f-8a6b-7c8d9e0f1a2b")]),
            callback: .object(["hook_event_name": .string("Stop"), "session_id": .string("5f1c0d2e-native"),
                               "prompt_id": .string("p-12")]),
            gap: .object(["first": .integer(1794358000), "last": .integer(1794358020)]),
            printed: ["54dc6432-26f1-43fe-ad9d-20b7e91bad69"],
            inbox: .string("/Users/maya/.claude/sessions/4242.9c1d.key")), select: nil)
        #expect(try encoded(boundary) == value("session_boundary"))
        guard case .response(_, _, .object(let answer)) = try answer("session_boundary_answer") else {
            Issue.record("Missing boundary answer"); return
        }
        #expect(answer["steers"] == .null)
        #expect(answer["update"] == .null)
        #expect(answer["waiting"] == .bool(false))
    }

    @Test func primarySessionSelectionUsesTheSharedViewPlanRead() throws {
        let request = ServerClientMessage.request(id: "view-1", operation: .read(
            .desktopViewGet(directory: "/repo", expression: "task:INF-123")), select: nil)
        #expect(try encoded(request) == value("view_plan"))
    }

    @Test func configurationUsesTheSharedDestinationContract() throws {
        let set = ConfigOperation.configSet(destination: .object(["kind": .string("personal_global")]), key: "theme", value: "Hearth")
        #expect(try encoded(.request(id: "config-1", operation: .config(set), select: nil)) == value("configuration"))
    }

    @Test func configurationEncodingsKeepTheirKindsAndFields() throws {
        let destination = JSONValue.object(["kind": .string("personal_global")])
        let operations: [(ConfigOperation, String)] = [
            (.configCatalog, #"{"kind":"config.catalog"}"#),
            (.configGet(destination: destination, key: "theme"),
             #"{"kind":"config.get","destination":{"kind":"personal_global"},"key":"theme"}"#),
            (.configExplain(destination: destination, key: "theme"),
             #"{"kind":"config.explain","destination":{"kind":"personal_global"},"key":"theme"}"#),
            (.configUnset(destination: destination, key: "theme"),
             #"{"kind":"config.unset","destination":{"kind":"personal_global"},"key":"theme"}"#),
            (.configTheme(path: "/theme.yaml", value: nil),
             #"{"kind":"config.theme","path":"/theme.yaml","value":null}"#),
            (.configTheme(path: "/theme.yaml", value: "Hearth"),
             #"{"kind":"config.theme","path":"/theme.yaml","value":"Hearth"}"#),
        ]
        for (operation, json) in operations {
            let encoded = try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(operation))
            #expect(try encoded == JSONDecoder().decode(JSONValue.self, from: Data(json.utf8)))
        }
    }

    @Test func sessionAdoptionSharesTheRustWriteContract() throws {
        let request = ServerClientMessage.request(id: "adopt-1", operation: .command(
            .taskCreate(directory: "/repo", requestID: "fixture-creation", title: "Keep the conversation", request: nil, wave: nil, graph: nil, session: "session_5e55105e000040008000000000000001", run: false)), select: nil)
        #expect(try encoded(request) == value("task_from_session"))
    }

    @Test func coreLoopWritesShareTheRustWriteContract() throws {
        let writes: [(String, String, ServedWrite)] = [
            ("task_run", "run-1", .taskRun(directory: "/repo", issue: "LOO-1", graph: nil, from: "implement", agent: "claude", approve: false)),
            ("task_ship", "ship-1", .taskShip(directory: "/repo", issue: "LOO-1")),
            ("task_hold", "hold-1", .taskHold(directory: "/repo", issue: "LOO-1")),
            ("task_steer", "steer-1", .taskSteer(directory: "/repo", issue: "LOO-1", message: "Keep the API small")),
            ("task_report", "report-1", .taskReport(directory: "/repo", issue: "LOO-1", message: "Demo recorded")),
            ("task_interrupt", "interrupt-1", .taskInterrupt(directory: "/repo", issue: "LOO-1")),
            ("session_steer", "steer-2", .sessionSteer(directory: "/repo", session: nil, wave: "product",
                                                       message: "Ship the smallest slice first")),
            ("session_status", "status-1", .sessionStatus(
                directory: "/", id: "session_5e55105e000040008000000000000001", terminal: "pane-1",
                agentProcess: "5e55105e-0000-4000-8000-000000000007", stream: "5e55105e-0000-4000-8000-000000000008",
                sequence: 1, records: ProgramStatusRecords(seen: true, records: [
                    ProgramStatusReport(state: .blocked, id: nil, kind: .question, progress: nil, app: nil,
                                        title: nil, msg: "Proceed?")]))),
            ("wave_plan_bind", "bind-1", .wavePlanBind(directory: "/repo", wave: "product",
                                                               project: "5e55105e-0000-4000-8000-000000000042")),
            ("wave_plan_connect", "connect-1", .wavePlanConnect(directory: "/repo", wave: "product", all: false,
                                                                        teamKey: "LOO", teamName: nil)),
            ("wave_plan_reteam", "reteam-1", .wavePlanReteam(directory: "/repo", apply: false)),
            ("repo_release", "release-1", .repoRelease(directory: "/repo", version: "minor", target: nil)),
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
                                                 plan: nil, clearPlan: false, paidThrough: nil,
                                                 clearPaidThrough: false, clearCooldown: true, chromeProfiles: [],
                                                 clearChromeProfiles: false)),
            ("account_redeem_reset", "redeem-1", .accountRedeemReset(provider: "codex", email: "maya@example.com",
                                                                     idempotencyKey: "4b0e1f2a-9c3d-4e5f-8a6b-7c8d9e0f1a2b",
                                                                     creditId: nil)),
            ("account_use", "use-1", .accountUse(login: "claude=maya@example.com")),
            ("account_pause", "pause-1", .accountPause(login: "claude=maya@example.com")),
            ("account_unpause", "unpause-1", .accountUnpause(login: "claude=maya@example.com")),
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
            do {
                #expect(try encoded(.request(id: id, operation: .command(write), select: nil)) == value(name), "\(name)")
            } catch { Issue.record("\(name): \(error)") }
        }
        #expect(ServedWrite.taskRun(directory: "/repo", issue: "LOO-1", graph: nil, from: nil, agent: nil, approve: true).argv
            == ["--batch", "task", "run", "LOO-1", "--json"])
        guard case .response(_, _, .object(let running)) = try answer("task_running") else {
            Issue.record("Missing task.run result"); return
        }
        #expect(running["state"] == .string("running"))
        #expect(running["graph_process"] == .string("85c31c1e-850c-4821-878d-55527b6dba46"))
        guard case .response(_, _, .object(let shipping)) = try answer("task_shipping") else {
            Issue.record("Missing task.ship result"); return
        }
        #expect(shipping["state"] == .string("shipping"))
        guard case .response(_, _, .object(let held)) = try answer("task_held") else {
            Issue.record("Missing task.hold result"); return
        }
        #expect(held["stopped"] == .bool(true))
        guard case .response(_, _, .object(let steered)) = try answer("session_steered") else {
            Issue.record("Missing session.steer result"); return
        }
        #expect(steered["text"] == .string("Ship the smallest slice first"))
        guard case .response(_, _, .array(let connected)) = try answer("wave_plan_connected") else {
            Issue.record("Missing wave.plan.connect result"); return
        }
        #expect(connected.count == 1)
        guard case .response(_, _, .object(let reteamed)) = try answer("wave_plan_reteamed") else {
            Issue.record("Missing wave.plan.reteam result"); return
        }
        #expect(reteamed["applied"] == .bool(false))
        guard case .response(_, _, .object(let launched)) = try answer("launched") else {
            Issue.record("Missing detached result"); return
        }
        #expect(launched["operation"] == .string("repo.release"))
        guard case .response(_, _, .object(let planned)) = try answer("checkout_sync_planned") else {
            Issue.record("Missing checkout.sync plan"); return
        }
        #expect(planned["state"] == .string("planned"))
        let ci = ServerClientMessage.request(id: "ci-1", operation: .read(.repoCi(since: "7d", wave: "product", repo: nil)), select: nil)
        #expect(try encoded(ci) == value("repo_ci"))
        #expect(ServedRead.repoCi(since: "7d", wave: nil, repo: nil).argv == ["repo", "ci", "--since", "7d", "--json"])
        #expect(ServedWrite.prLand(directory: "/repo", waitAndFix: true, strict: false, local: false, worktree: nil,
                                   message: nil, title: "Serve the pr namespace", body: nil, agent: nil).argv
            == ["pr", "land", "--wait-and-fix", "--title", "Serve the pr namespace", "--json"])
        guard case .response(_, _, .object(let published)) = try answer("pr_published") else {
            Issue.record("Missing pr.publish result"); return
        }
        #expect(published["created"] == .bool(false))
        guard case .response(_, _, .object(let checks)) = try answer("pr_checks_report") else {
            Issue.record("Missing pr.checks result"); return
        }
        #expect(checks["state"] == .string("pending"))
        let checksRead = ServerClientMessage.request(id: "checks-1",
                                                     operation: .read(.prChecks(directory: "/repo", required: true)), select: nil)
        #expect(try encoded(checksRead) == value("pr_checks"))
        let reviewsRead = ServerClientMessage.request(id: "reviews-1",
                                                      operation: .read(.prReviews(directory: "/repo", pr: nil)), select: nil)
        #expect(try encoded(reviewsRead) == value("pr_reviews"))
        #expect(ServedRead.prReviews(directory: "/repo", pr: "1813").argv == ["pr", "reviews", "1813", "--json"])
        #expect(ServedWrite.graphRun(directory: "/repo", name: "code", message: nil, agent: nil).argv
            == ["--batch", "graph", "code"])
        let list = ServerClientMessage.request(id: "list-1", operation: .read(.skillList(directory: "/repo", path: [])), select: nil)
        #expect(try encoded(list) == value("skill_list"))
        guard case .response(_, _, .array(let entries)) = try answer("skill_entries") else {
            Issue.record("Missing skill.list result"); return
        }
        #expect(entries.count == 2)
        #expect(ServedWrite.accountUse(login: "claude=maya@example.com").cwd == "/")
        let accounts = ServerClientMessage.request(id: "accounts-1",
                                                   operation: .read(.accountList(target: "claude", cached: true)), select: nil)
        #expect(try encoded(accounts) == value("account_list"))
        let route = ServerClientMessage.request(id: "route-1",
                                                operation: .read(.accountNext(directory: "/repo", repo: nil,
                                                                               default: false)), select: nil)
        #expect(try encoded(route) == value("account_next_request"))
        guard case .response(_, _, .object(let report)) = try answer("account_report") else {
            Issue.record("Missing account.list result"); return
        }
        #expect(report["browser"] == .null)
        guard case .response(_, _, .object(let redeemed)) = try answer("account_redeemed") else {
            Issue.record("Missing account.redeem_reset result"); return
        }
        #expect(redeemed["after"] == .null)
    }

    @Test func aWaitEndsAfterTheReadingThatReachedIt() throws {
        let wait = ServerClientMessage.subscribe(id: "wait-1", operation: .taskWait(issue: "LOO-1", until: "submitted"), select: nil, resume: nil)
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
        let protocolSource = try source("swift/Loopflow/Services/ServerProtocol.swift")
        let operations = try source("swift/Loopflow/Services/RegistryQuery.swift") + protocolSource
        let call = #"case \.(\w+)(?:\([^)]*\))?:\s*TaggedFields\("([a-z_.]+)"(?:,\s*\[((?:[^\[\]]|\[[^\]]*\])*)\])?\s*\)"#
        let calls = try matches(call, in: operations)
        #expect(calls.count == operations.components(separatedBy: "TaggedFields(").count - 1, "A TaggedFields call has a shape this test cannot read")
        for groups in calls {
            let fields = try Set(matches(#""(\w+)":"#, in: groups[3] ?? "").compactMap { $0[1] })
            let kind = try #require(groups[2])
            let words = kind.split { $0 == "." || $0 == "_" }
            let name = String(try #require(words.first)) + words.dropFirst().map { $0.capitalized }.joined()
            #expect(groups[1] == name, "\(kind) must be named \(name) in Swift")
            #expect(described[kind] == fields, "\(kind): `lf server describe --json` has \(described[kind].map { $0.sorted() } ?? []), Swift encodes \(fields.sorted())")
        }
        let events = try arms("SubscriptionEvent", tag: "kind", of: schema)
        for groups in try matches(#""([a-z]+(?:\.[a-z_]+)+)""#, in: protocolSource) {
            let kind = try #require(groups[1])
            #expect(described[kind] != nil || events[kind] != nil, "\(kind) is not in `lf server describe --json`")
        }
    }

    @Test func typedEventsCarryTheirPayloadAndUnknownKindsStayLocal() throws {
        guard case .event(let id, let seq, .taskShipped(let task, _, let number, _, _)) = try message("event_shipped") else {
            Issue.record("Missing ship event"); return
        }
        #expect(id == "task" && seq == 1 && number == 42)
        #expect(task == "task_11111111111111111111111111111111")
        guard case .event(_, _, .taskSteer(_, let steer, let comment)) = try message("event_steer") else {
            Issue.record("Missing steer event"); return
        }
        #expect(steer.text == "Keep the examples.")
        #expect(comment?.body == "Keep the examples.")
        guard case .event(_, _, .graphStepFinished(_, _, let step)) = try message("event_step_finished") else {
            Issue.record("Missing step event"); return
        }
        #expect(step.label == "implement" && step.outcome == "succeeded")
        let unknown = Data(#"{"type":"event","id":"task","seq":2,"event":{"kind":"future.occurrence","extra":true}}"#.utf8)
        guard case .event(let unknownID, 2, .unknown(let kind)) = try ServerMessage(line: unknown) else {
            Issue.record("Unknown event closed the connection"); return
        }
        #expect(unknownID == "task" && kind == "future.occurrence")
    }

    /// A fixture the mirror neither encodes nor decodes is one it can drift from.
    @Test func everyFixtureIsMirrored() throws {
        let client = try arms("ClientMessage", tag: "type", of: schema())
        let tests = try source("swift/LoopflowTests/ServerProtocolTests.swift")
        let directory = root.appendingPathComponent("rust/loopflow/tests/fixtures/dto/server")
        for url in try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil) {
            let name = url.deletingPathExtension().lastPathComponent
            let object = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
            let type = try #require(object["type"] as? String)
            // Mobile setup is exposed to Desktop by the CLI; Swift has no encoder for it.
            if ["mobile_status_request", "configure_remote"].contains(name) { continue }
            if client[type] == Set(object.keys).subtracting(["type"]) {
                let encodedHere = tests.contains("\"\(name)\"")
                #expect(encodedHere, "No test here encodes the client fixture \(name)")
            } else {
                #expect(throws: Never.self, "\(name)") { try message(name) }
            }
        }
    }

    @Test func missingRequiredFieldsDoNotBecomeDefaults() {
        #expect(throws: (any Error).self) {
            try ServerMessage(line: Data(#"{"type":"reading","id":"x"}"#.utf8))
        }
        #expect(throws: (any Error).self) {
            try ServerMessage(line: Data(#"{"type":"welcome","revision":5}"#.utf8))
        }
        #expect(throws: (any Error).self) {
            try ServerMessage(line: Data(#"{"type":"response","id":"x","result":{"kind":"bootstrap"}}"#.utf8))
        }
        #expect(throws: (any Error).self) {
            try ServerMessage(line: Data(#"{"type":"bridged"}"#.utf8))
        }
    }
}
