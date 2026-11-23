import Foundation
import Testing
import SwiftUI
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

actor CreationTestMachine {
    private var receipts: [String: String] = [:]
    private(set) var creations = 0
    private(set) var submissions: [Data] = []
    func lookup(_ id: String) -> ReadOutput { ReadOutput(receipts[id] ?? "null") }
    func send(_ payload: Data) throws -> String {
        let fields = try #require(JSONSerialization.jsonObject(with: payload) as? [String: Any])
        let id = try #require(fields["request_id"] as? String)
        submissions.append(payload)
        if receipts[id] == nil { creations += 1 }
        receipts[id] = Self.receipt(id, disposition: "complete")
        return #"{"task":{"id":"original"}}"#
    }
    private static func receipt(_ id: String, disposition: String) -> String {
        // Deliberately identical milestones: only the server's disposition
        // determines whether the client continues.
        #"{"request_id":"\#(id)","kind":"task.create","milestone":"run_requested","task_id":"original","session_id":null,"result":{"task":{"id":"original"}},"disposition":"\#(disposition)","explanation":"Server explanation","run":true,"artifact_key":null,"launch":null,"created_at":1,"updated_at":2}"#
    }
    func continuation(_ id: String) { receipts[id] = Self.receipt(id, disposition: "continue") }
    func unknown(_ id: String) {
        receipts[id] = Self.receipt(id, disposition: "inspect")
    }
}

struct CreationSubmissionTests {
    @Test func restartAtEachSubmissionWindowRecoversTheOriginalResource() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationTestMachine()
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
        #expect(await server.submissions.count == 1)
        try await client().presented(saved.id)
        #expect(try await client().pending().isEmpty)
        #expect(try await client().reconcile(saved) == nil)
    }

    @Test func unknownLaunchAndAnotherMachineNeverAuthorizeResubmission() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationTestMachine()
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

    @Test func aRecordFromAnEarlierRevisionReadsItsReceiptAndNeverResends() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationTestMachine()
        let client = CreationSubmissions(directory: root, machine: { "original-machine" },
            lookup: { await server.lookup($0) }, send: { try await server.send($0) })
        let older = SubmittedCreation(requestID: "older", destination: "original-machine", payload: Data(),
            version: ServerProtocol.revision - 1, submittedAt: Date())
        try await client.save(older)
        await server.unknown("older")
        #expect(try await client.reconcile(older)?.disposition == .inspect)
        await server.continuation("older")
        await #expect(throws: (any Error).self) { try await client.reconcile(older) }
        #expect(await server.submissions.count == 0)
    }

    @Test func storageFailurePrecedesSendingAndRecordsHaveNoOutcomeFields() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data().write(to: root) // a file cannot hold submitted requests
        let server = CreationTestMachine()
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

    @Test func anUnreadableUnrelatedDraftStillAllowsANewSubmission() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data().write(to: root.appendingPathComponent("damaged.json"))
        let machine = CreationTestMachine()
        let client = CreationSubmissions(directory: root, machine: { "original" },
            lookup: { await machine.lookup($0) }, send: { try await machine.send($0) })
        _ = try await client.submit(.taskCreate(directory: "/repo", requestID: "fresh", title: nil,
            request: "new words", wave: nil, graph: nil, session: nil, run: false), requestID: "fresh")
        #expect(await machine.creations == 1)
        await #expect(throws: DecodingError.self) { try await client.pending() }
    }

    @Test func aRefusedCreationReturnsToTheDraftOnlyAfterConfirmedAbsence() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationTestMachine()
        let refusal = ServerError(code: "invalid", message: "Wave unavailable", data: .null)
        let client = CreationSubmissions(directory: root, machine: { "original-machine" },
            lookup: { await server.lookup($0) }, send: { _ in throw refusal })
        let change = ServedWrite.taskCreate(directory: "/repo", requestID: "file", title: nil,
            request: "an idea", wave: .name("product"), graph: nil, session: nil, run: false)
        await #expect(throws: refusal) { try await client.submit(change, requestID: "file") }
        #expect(try await client.pending().isEmpty)
        let stale = SubmittedCreation(requestID: "file", destination: "original-machine",
            payload: try JSONEncoder().encode(change), version: ServerProtocol.revision, submittedAt: Date())
        #expect(try await client.reconcile(stale) == nil)

        await server.unknown("file")
        await #expect(throws: SubmittedCreationError.self) { try await client.submit(change, requestID: "file") }
        #expect(try await client.pending().first?.id == "file")
    }

    @Test("Only a confirmed absent receipt after replay returns a draft", arguments: [true, false])
    func refusalRequiresAnotherReceiptLookup(receiptAvailable: Bool) async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let machine = CreationTestMachine()
        let client = CreationSubmissions(directory: root, machine: { "original" }, lookup: { id in
            if !receiptAvailable, await machine.creations > 0 { throw RegistryQueryError("Machine disconnected") }
            return await machine.lookup(id)
        }, send: { payload in
            _ = try await machine.send(payload)
            throw ServerError(code: "invalid", message: "Start refused", data: .null)
        })
        let record = try await client.record(.taskCreate(directory: "/repo", requestID: "original", title: nil,
            request: "my words", wave: nil, graph: nil, session: nil, run: true), requestID: "original")
        if receiptAvailable {
            #expect(try await client.reconcile(record, keepingRefusedDraft: true)?.taskId == "original")
        } else {
            await #expect(throws: RegistryQueryError.self) { try await client.reconcile(record, keepingRefusedDraft: true) }
        }
        let pending = try #require(await client.pending().first)
        #expect(pending.id == record.id && pending.refusedDraft == nil)
        #expect(await machine.submissions == [record.payload])
    }

    @MainActor @Test func unsuccessfulReplacementKeepsTheParkedDraftUntilDiscard() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationTestMachine()
        func client(on machine: String) -> CreationSubmissions {
            CreationSubmissions(directory: root, machine: { machine }, lookup: { await server.lookup($0) },
                send: { _ in throw ServerError(code: "invalid", message: "Workflow removed", data: .null) })
        }
        func start(_ id: String, graph: String?) -> ServedWrite {
            .taskCreate(directory: "/repo", requestID: id, title: nil, request: "original words",
                        wave: .name("product"), graph: graph, session: nil, run: true)
        }
        let original = client(on: "original")
        let record = try await original.record(start("one", graph: "removed"), requestID: "one")
        let recovery = CreationRecovery(submissions: original)
        await recovery.refresh()
        let draft = try #require(await original.pending().first?.refusedDraft)
        #expect(draft.wave == .name("product") && draft.graph == "removed")

        let correction = start("two", graph: nil)
        await #expect(throws: RegistryQueryError.self) {
            try await client(on: "another").submit(correction, requestID: "two", replacing: draft.id)
        }
        let file = try #require(FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil).first)
        try FileManager.default.setAttributes([.immutable: true], ofItemAtPath: file.path)
        defer { try? FileManager.default.setAttributes([.immutable: false], ofItemAtPath: file.path) }
        await #expect(throws: (any Error).self) {
            try await original.submit(correction, requestID: "two", replacing: draft.id)
        }
        try FileManager.default.setAttributes([.immutable: false], ofItemAtPath: file.path)
        let saved = try #require(await original.pending().first)
        #expect(saved.refusedDraft == draft && saved.payload == record.payload)

        // The next refusal belongs to the saved correction. A stale window
        // can neither discard nor replace it by acting on the first ID.
        await #expect(throws: RefusedTaskDraft.self) {
            try await original.submit(correction, requestID: "two", replacing: draft.id)
        }
        try await recovery.discardDraft(draft.id)
        await #expect(throws: RegistryQueryError.self) {
            try await original.submit(start("three", graph: nil), requestID: "three", replacing: draft.id)
        }
        await recovery.refresh()
        let corrected = try #require(await original.pending().first?.refusedDraft)
        #expect(corrected.id == "two" && corrected.graph == nil)
        try await recovery.discardDraft(corrected.id)
        #expect(try await original.pending().isEmpty && recovery.items.isEmpty)
    }

    @Test func simultaneousWindowsShareOneRecoveryAndUnavailableLookupNeverSends() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationTestMachine()
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
        #expect(await server.submissions.count == 1)

        let unavailable = CreationSubmissions(directory: root, machine: { "machine" },
            lookup: { _ in throw RegistryQueryError("unavailable") }, send: { try await server.send($0) })
        await #expect(throws: (any Error).self) { try await unavailable.reconcile(record) }
        #expect(await server.submissions.count == 1)
        #expect(try await unavailable.pending().count == 1)
    }
    @Test func aReceiptForAnotherRequestIsRefusedAndNothingIsResent() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let receipts = try #require(JSONSerialization.jsonObject(with: loadFixtureData("creation_lookup.json")) as? [Any])
        let other = String(decoding: try JSONSerialization.data(withJSONObject: receipts[0]), as: UTF8.self)
        actor Sends { var count = 0; func add() { count += 1 } }
        let sent = Sends()
        let client = CreationSubmissions(directory: root, machine: { "machine" },
            lookup: { _ in ReadOutput(other) }, send: { _ in await sent.add(); return "{}" })
        let record = try await client.record(.taskCreate(directory: "/repo", requestID: "not-the-fixture", title: nil,
            request: "work", wave: nil, graph: nil, session: nil, run: true), requestID: "not-the-fixture")
        await #expect(throws: (any Error).self) { try await client.reconcile(record) }
        #expect(await sent.count == 0)
        #expect(try await client.pending().count == 1)
    }

    @Test func lookupDispositionsRoundTrip() throws {
        let data = try loadFixtureData("creation_lookup.json")
        let receipts = try JSONDecoder().decode([CreationLookup].self, from: data)
        let encoded = try JSONSerialization.jsonObject(with: JSONEncoder().encode(receipts)) as? NSArray
        #expect(encoded == (try JSONSerialization.jsonObject(with: data) as? NSArray))
        #expect(receipts.map(\.disposition) == [.complete, .continue, .inspect, .inspect, .inspect, .inspect])
    }

    @Test func continuationUsesTheSavedPayloadWithoutInterpretingMilestones() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationTestMachine()
        await server.continuation("same")
        let client = CreationSubmissions(directory: root, machine: { "machine" },
            lookup: { await server.lookup($0) }, send: { try await server.send($0) })
        let record = SubmittedCreation(requestID: "same", destination: "machine",
            payload: Data(#"{"kind":"task.create","request_id":"same","directory":"/repo"}"#.utf8),
            version: ServerProtocol.revision, submittedAt: Date())
        try await client.save(record)
        #expect(try await client.reconcile(record)?.disposition == .complete)
        #expect(await server.submissions.count == 1)
        #expect(await server.creations == 0)
        #expect(try await client.pending().first?.payload == record.payload)
    }

    @MainActor @Test(arguments: [CreationRecovery.Presentation.task("original"), .session("original")])
    func mountedPresentationRetiresNormalAndPromotedResults(resource: CreationRecovery.Presentation) async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationTestMachine()
        let client = CreationSubmissions(directory: root, machine: { "machine" },
            lookup: { await server.lookup($0) }, send: { try await server.send($0) })
        _ = try await client.submit(.taskCreate(directory: "/repo", requestID: "same", title: nil,
            request: nil, wave: nil, graph: nil, session: nil, run: false), requestID: "same")
        let recovery = CreationRecovery(submissions: client)
        recovery.awaitingPresentation("same", of: resource)
        #expect(try await client.pending().count == 1) // reply and navigation aren't presentation
        let hidden = Text("Result").modifier(CreationPresentation(recovery: recovery, resource: resource, visible: false))
        let (mounts, mounted) = AsyncStream<Void>.makeStream()
        let hiddenPane = ImageRenderer(content: hidden.task { mounted.yield(()) })
        defer { withExtendedLifetime(hiddenPane) {} }
        _ = try #require(hiddenPane.cgImage)
        for await _ in mounts { break }
        #expect(try await client.pending().count == 1)
        let shown = Text("Result").modifier(CreationPresentation(recovery: recovery, resource: resource))
        let visiblePane = ImageRenderer(content: shown)
        defer { withExtendedLifetime(visiblePane) {} }
        _ = try #require(visiblePane.cgImage)
        for await retired in Observations({ recovery.requests(for: resource).isEmpty }) where retired { break }
        #expect(try await client.pending().isEmpty)
        await recovery.refresh()
        #expect(recovery.items.isEmpty)
        #expect(await server.submissions.count == 1)
    }

    @MainActor @Test func coldRecoveryAndFailedRemovalRepeatOnlyPresentationAndLookup() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationTestMachine()
        let client = CreationSubmissions(directory: root, machine: { "machine" },
            lookup: { await server.lookup($0) }, send: { try await server.send($0) })
        _ = try await client.submit(.taskCreate(directory: "/repo", requestID: "same", title: nil,
            request: nil, wave: nil, graph: nil, session: nil, run: false), requestID: "same")
        let recovery = CreationRecovery(submissions: client)
        let notices = NoticeCenter()
        recovery.report(to: notices) { _ in Issue.record("Expected a receipt") }
        await recovery.refresh()
        let item = try #require(recovery.items.first)
        #expect(item.text == "Server explanation")
        // The window's Notice stands for the saved creation; its action opens the result.
        #expect(notices.current?.text == "Server explanation")
        #expect(notices.current?.action?.title == "Show result")
        #expect(notices.current?.kind == .condition)
        notices.current?.action?.perform()
        #expect(recovery.inspecting?.id == item.id)
        // An immutable file fails actual removal on macOS, without mocking storage.
        let file = try #require(FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil).first)
        try FileManager.default.setAttributes([.immutable: true], ofItemAtPath: file.path)
        await recovery.presented([item.id])
        try FileManager.default.setAttributes([.immutable: false], ofItemAtPath: file.path)
        #expect(try await client.pending().count == 1)
        await recovery.refresh()
        #expect(notices.current?.action?.title == "Show result")
        #expect(await server.submissions.count == 1)
        await recovery.presented([item.id])
        #expect(try await client.pending().isEmpty)
        #expect(notices.current == nil)
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
        let reading = ServerConnectionReading(generation: 2, state: .connected, nativeReady: true, message: nil, transition: transition)
        #expect(reading.title == transition.explanation)
        #expect(reading.acceptsInput)
    }
}
