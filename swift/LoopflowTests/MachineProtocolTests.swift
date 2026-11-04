import Foundation
import Testing
@testable import Loopflow

struct MachineProtocolTests {
    @Test func sessionOpeningDelegatesTheExactRecordedIdentity() throws {
        let id = "session_5e55105e000040008000000000000001"
        let change = ServedWrite.sessionStart(id: id)
        let request = MachineRequest(version: 1, id: "start-1", operation: .command(change))
        #expect(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)) == JSONDecoder().decode(JSONValue.self, from: fixture("session_start")))
        #expect(change.argv == ["session", "resume", "--start-only", "--json", "--", id])
        #expect(change.cwd == "/")
    }
    @Test func creationRetainsItsIdentityAndOutcomeAcrossCarriers() throws {
        let id = try #require(UUID(uuidString: "00000000-0000-4000-8000-000000000001"))
        let change = ServedWrite.taskCreate(directory: "/repo", requestID: id, request: "Fix the retry test", wave: nil, graph: nil, terminal: false)
        let request = MachineRequest(version: 1, id: "create-1", operation: .command(change))
        #expect(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)) == JSONDecoder().decode(JSONValue.self, from: fixture("task_create")))
        #expect(change.argv == ["task", "create", "--run", "--json", "--request-id", id.uuidString.lowercased(), "--no-wave", "--", "Fix the retry test"])
        #expect(change.cwd == "/repo")
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let outcomes = try JSONDecoder().decode([TaskCreateOutcome].self, from: Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/task_create_outcome.json")))
        #expect(outcomes.map(\.state) == [.inProgress, .started, .failed, .unknown])
        #expect(outcomes[1].receipt?.task.id == outcomes[1].taskId)
        #expect(outcomes[2].receipt != nil)
        #expect(outcomes[3].message != nil)
    }
    @Test func allRepositorySelectionUsesTheSharedWatch() throws {
        let query = ServedRead.allViewPlans(expression: "kind:task state:open")
        let request = MachineRequest(version: 1, id: "tasks-1", operation: .subscribe(query, generation: 1))
        #expect(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)) == JSONDecoder().decode(JSONValue.self, from: fixture("all_view_plans")))
        #expect(query.argv == ["desktop", "plans", "--all", "--json", "--expression", "kind:task state:open"])
        #expect(query.cwd == nil)
    }
    @Test func localMobileSetupUsesRustFixtures() throws {
        let configuration = try JSONDecoder().decode(MachineMobileConfiguration.self, from: fixture("mobile_configuration"))
        let request = MachineRequest(version: 1, id: "mobile-1", operation: .configureRemote(configuration: configuration))
        #expect(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)) == JSONDecoder().decode(JSONValue.self, from: fixture("configure_remote")))
        let response = try JSONDecoder().decode(MachineResponse.self, from: fixture("remote"))
        guard case .remote(let configuration) = response.result else { Issue.record("Missing remote configuration"); return }
        #expect(configuration?.endpoint == "https://host.tail.ts.net:8443")
    }
    @Test func mobileReadinessDoesNotTreatSavedApprovalAsALiveConnection() throws {
        let request = MachineRequest(version: 1, id: "mobile-status-1", operation: .mobileStatus)
        #expect(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)) == JSONDecoder().decode(JSONValue.self, from: fixture("mobile_status_request")))
        let response = try JSONDecoder().decode(MachineResponse.self, from: fixture("mobile_status"))
        guard case .mobileStatus(let status) = response.result else { Issue.record("Missing mobile readiness"); return }
        #expect(status.configuration?.enabled == true)
        #expect(!status.ready)
        #expect(status.error == "Sign in to Tailscale on this Machine")
    }

    private func fixture(_ name: String) throws -> Data {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        return try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/machine/\(name).json"))
    }

    @Test func sharedSubscriptionShapeAndUnavailableReading() throws {
        let request = MachineRequest(version: 1, id: "tasks-1", operation: .subscribe(
            .roadmap(directory: "/", wave: nil, task: nil, all: true), generation: 7))
        let encoded = try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request))
        #expect(encoded == (try JSONDecoder().decode(JSONValue.self, from: fixture("subscribe"))))
        let response = try JSONDecoder().decode(MachineResponse.self, from: fixture("reading"))
        #expect(response.id == "tasks-1")
        guard case .reading(let generation, let frame) = response.result else { Issue.record("Missing reading"); return }
        #expect(generation == 7)
        guard case .object(let fields) = frame else { Issue.record("Missing watch frame"); return }
        #expect(fields["body"] == .null)
        #expect(fields["unavailable"] == .string("repository is unavailable"))
    }

    @Test func commandTraceAndExistingOwnerShareRustFixtures() throws {
        let bootstrap = try JSONDecoder().decode(MachineResponse.self, from: fixture("bootstrap"))
        guard case .bootstrap(let version, _, let executable, _, let capabilities, let maxFrame, let maxConnections) = bootstrap.result else {
            Issue.record("Missing bootstrap"); return
        }
        #expect(version == 1)
        #expect(executable.count == 64)
        #expect(capabilities.contains("session_owner"))
        #expect(capabilities.contains("view_plan"))
        #expect(maxFrame == 33554432)
        #expect(maxConnections == 64)
        let command = try JSONDecoder().decode(MachineResponse.self, from: fixture("command"))
        guard case .command(let trace, _, _, let code) = command.result else { Issue.record("Missing command"); return }
        #expect(trace == "85c31c1e-850c-4821-878d-55527b6dba46")
        #expect(code == 0)
        let owner = try JSONDecoder().decode(MachineResponse.self, from: fixture("owner"))
        guard case .sessionOwner(let process) = owner.result else { Issue.record("Missing owner"); return }
        #expect(process == trace)
        let request = MachineRequest(version: 1, id: "owner-1", operation: .sessionOwner(session: "session_5e55105e000040008000000000000001"))
        #expect(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)) == JSONDecoder().decode(JSONValue.self, from: fixture("session_owner")))
        let attach = MachineRequest(version: 1, id: "terminal-1", operation: .attach(owner: trace))
        #expect(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(attach)) == JSONDecoder().decode(JSONValue.self, from: fixture("attach")))
    }

    @Test func primarySessionSelectionUsesTheSharedViewPlanRead() throws {
        let request = MachineRequest(version: 1, id: "view-1", operation: .read(
            .viewPlan(directory: "/repo", expression: "task:INF-123")))
        #expect(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)) == JSONDecoder().decode(JSONValue.self, from: fixture("view_plan")))
    }

    @Test func missingRequiredReplyDoesNotBecomeAnEmptyReading() {
        #expect(throws: (any Error).self) {
            try JSONDecoder().decode(MachineResponse.self, from: Data(#"{"id":"x","instance":"i","result":{"kind":"reading","generation":1}}"#.utf8))
        }
    }
}
