import Foundation
import Testing
import ViewInspector
import Loopflow
@testable import LoopflowMac

struct CreationSubmissionTests {
    private actor Server {
        private var receipts: [String: String] = [:]
        private(set) var creations = 0
        private(set) var submissions = 0
        func lookup(_ id: String) -> String { receipts[id] ?? "null" }
        func send(_ payload: Data) throws -> String {
            let fields = try #require(JSONSerialization.jsonObject(with: payload) as? [String: Any])
            let id = try #require(fields["request_id"] as? String)
            submissions += 1
            if receipts[id] == nil {
                creations += 1
                receipts[id] = #"{"request_id":"\#(id)","kind":"task.create","milestone":"admitted","task_id":"original","session_id":null,"result":{"task":{"id":"original"}}}"#
            }
            return #"{"task":{"id":"original"}}"#
        }
        func unknown(_ id: String) {
            receipts[id] = #"{"request_id":"\#(id)","kind":"task.create","milestone":"run_requested","task_id":"original","session_id":null,"result":null}"#
        }
    }

    @Test func restartAtEachSubmissionWindowRecoversTheOriginalResource() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = Server()
        func client() -> CreationSubmissions {
            CreationSubmissions(directory: root, machine: { "original-machine" },
                lookup: { await server.lookup($0) }, send: { try await server.send($0) })
        }
        let payload = Data(#"{"kind":"task.create","request_id":"same","directory":"/original-repo"}"#.utf8)
        let record = SubmittedCreation(requestID: "same", destination: "original-machine", payload: payload,
            version: ServerProtocol.revision, submittedAt: Date())
        try await client().save(record) // app exits before first send
        let cold = client()
        let saved = try #require(await cold.pending().first)
        #expect(saved.payload == payload)
        #expect(try await cold.reconcile(saved)?.taskId == "original")
        // Reopening after the effect/reply but before presentation/removal only looks up.
        #expect(try await client().reconcile(saved)?.taskId == "original")
        #expect(await server.creations == 1)
        #expect(await server.submissions == 1)
        try await client().presented(saved.id)
        #expect(try await client().pending().isEmpty)
    }

    @Test func unknownLaunchAndAnotherMachineNeverAuthorizeResubmission() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = Server()
        await server.unknown("same")
        let record = SubmittedCreation(requestID: "same", destination: "original-machine", payload: Data(),
            version: ServerProtocol.revision, submittedAt: Date())
        let client = CreationSubmissions(directory: root, machine: { "original-machine" },
            lookup: { await server.lookup($0) }, send: { try await server.send($0) })
        try await client.save(record)
        #expect(try await client.reconcile(record)?.milestone == "run_requested")
        let other = CreationSubmissions(directory: root, machine: { "other-machine" },
            lookup: { await server.lookup($0) }, send: { try await server.send($0) })
        await #expect(throws: (any Error).self) { try await other.reconcile(record) }
        #expect(await server.creations == 0)
        #expect(try await client.pending().count == 1)
    }

    @Test func storageFailurePrecedesSendingAndRecordsHaveNoOutcomeFields() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data().write(to: root) // a file cannot hold submitted requests
        let server = Server()
        let client = CreationSubmissions(directory: root, machine: { "original-machine" },
            lookup: { await server.lookup($0) }, send: { try await server.send($0) })
        let change = ServedWrite.taskCreate(directory: "/repo", requestID: "same", title: "Task", request: nil,
            wave: nil, graph: nil, session: nil, run: false)
        await #expect(throws: (any Error).self) { try await client.submit(change, requestID: "same") }
        #expect(await server.creations == 0)
        let record = SubmittedCreation(requestID: "same", destination: "machine", payload: Data(), version: 3, submittedAt: Date())
        let fields = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as? [String: Any])
        #expect(Set(fields.keys) == Set(["requestID", "destination", "payload", "version", "submittedAt"]))
    }

    @Test func simultaneousWindowsShareOneRecoveryAndUnavailableLookupNeverSends() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = Server()
        let (entered, began) = AsyncStream<Void>.makeStream()
        let (release, finish) = AsyncStream<Void>.makeStream()
        let client = CreationSubmissions(directory: root, machine: {
                began.yield(())
                for await _ in release { break }
                return "machine"
            }, lookup: { await server.lookup($0) }, send: { try await server.send($0) })
        let record = SubmittedCreation(requestID: "same", destination: "machine",
            payload: Data(#"{"kind":"task.create","request_id":"same","directory":"/repo"}"#.utf8),
            version: ServerProtocol.revision, submittedAt: Date())
        try await client.save(record)
        let firstWindow = Task { try await client.reconcile(record) }
        for await _ in entered { break }
        #expect(try await client.reconcile(record) == nil)
        finish.finish()
        #expect(try await firstWindow.value?.taskId == "original")
        #expect(await server.submissions == 1)

        let unavailable = CreationSubmissions(directory: root, machine: { "machine" },
            lookup: { _ in throw RegistryQueryError("unavailable") }, send: { try await server.send($0) })
        await #expect(throws: (any Error).self) { try await unavailable.reconcile(record) }
        #expect(await server.submissions == 1)
        #expect(try await unavailable.pending().count == 1)
    }
}


@MainActor
struct ProviderTransitionTests {
    @Test func theConnectionRowNamesTheObservedForkAndDatesSourceLiveness() throws {
        let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto/provider_transition.json")
        let transition = try JSONDecoder().decode(ProviderTransition.self, from: Data(contentsOf: path))
        let row = ProviderTransitionRow(transition: transition)
        #expect(try row.inspect().find(text: "Forked from earlier; original was running at fork").string() == transition.explanation)
        let reading = ServerConnectionReading(generation: 2, state: .connected, input: false, message: nil, transition: transition)
        #expect(reading.title?.contains("Read-only") == true)
        #expect(!reading.acceptsInput)
    }
}
