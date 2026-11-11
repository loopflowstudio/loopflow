import Foundation
import Observation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Program Status", .serialized)
struct ProgramStatusTests {
    private func report(_ state: ProgramStatusState, id: String? = nil,
                        app: String? = nil, msg: String? = nil) -> ProgramStatusReport {
        ProgramStatusReport(state: state, id: id, kind: state == .blocked ? .question : nil,
                            progress: nil, app: app, title: nil, msg: msg)
    }

    @Test func replacementAndChildLifetime() {
        var reducer = ProgramStatusReducer()
        reducer.apply(.report(report(.blocked, id: "graph/step", msg: "Proceed?")))
        reducer.apply(.report(report(.working, id: "graph")))
        #expect(reducer.snapshot.summary?.state == .blocked)
        reducer.apply(.report(report(.working, id: "graph/step")))
        #expect(reducer.snapshot.records.count == 2)
        #expect(reducer.snapshot.summary?.msg == nil)
        #expect(reducer.snapshot.summary?.kind == nil)
    }

    @Test("Native turn reports reach pane and Session labels without a launcher Working record")
    @MainActor func nativeTurnPresentation() async throws {
        let surface = ProgramStatusSurface()
        defer { surface.close() }
        let fixture = try renameFixtureRecord("native-status", title: "Native conversation")
        var session = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(fixture)) as? [String: Any])

        // Reproduce the old producer's extra record: an idle root cannot retire
        // another program's Working claim. The repair removes that producer.
        surface.receive(.report(report(.working, id: "lf/launcher")), incarnation: surface.incarnation)
        surface.receive(.report(report(.idle)), incarnation: surface.incarnation)
        try await eventually { surface.snapshot.records.count == 2 }
        #expect(surface.snapshot.summary?.state == .working)
        surface.receive(.report(report(.clear, id: "lf/launcher")), incarnation: surface.incarnation)
        try await eventually { surface.snapshot.summary?.state == .idle }

        for (raw, state) in [(1, ProgramStatusState.working), (2, .done), (0, .idle), (3, .blocked), (1, .working), (5, .clear)] {
            let event = try #require(copyProgramStatusEvent(event: 0, state: raw, kind: -1, progress: -1,
                                                           id: nil, app: nil, title: nil, msg: nil))
            surface.receive(event, incarnation: surface.incarnation)
            let expected: ProgramStatusState? = state == .clear ? nil : state
            try await eventually { surface.snapshot.summary?.state == expected }
            session["program_status"] = try JSONSerialization.jsonObject(with: JSONEncoder().encode(surface.snapshot))
            let record = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: session))
            #expect(SessionProgramStatus(sessions: [record], unavailable: nil).label == expected?.rawValue.capitalized)
            if let expected {
                let view = ProgramStatusLabel(status: surface.snapshot)
                #expect(try view.inspect().find(text: expected.rawValue.capitalized).string() == expected.rawValue.capitalized)
            } else {
                #expect(surface.snapshot.seen)
                #expect(surface.snapshot.records.isEmpty)
            }
        }
    }

    @Test func clearUsesPathComponents() {
        var reducer = ProgramStatusReducer()
        for id in ["a", "a/b", "ab"] { reducer.apply(.report(report(.blocked, id: id))) }
        reducer.apply(.report(report(.clear, id: "a")))
        #expect(reducer.snapshot.records.map(\.id) == ["ab"])
        reducer.apply(.report(report(.clear)))
        #expect(reducer.snapshot.records.isEmpty)
        #expect(reducer.snapshot.seen)
    }

    @Test func promptAndExitRetainOnlyResultsUntilInput() {
        for boundary in [ProgramStatusEvent.prompt, .exit] {
            var reducer = ProgramStatusReducer()
            for (index, state) in [ProgramStatusState.working, .blocked, .done, .error, .idle].enumerated() {
                reducer.apply(.report(report(state, id: "r\(index)")))
            }
            reducer.apply(boundary)
            #expect(reducer.snapshot.records.map(\.state) == [.done, .error])
            reducer.apply(.interaction)
            #expect(reducer.snapshot.records.isEmpty)
            #expect(reducer.snapshot.seen)
        }
    }

    @Test func resetPreservesReportPrecedence() {
        var reducer = ProgramStatusReducer()
        reducer.apply(.report(report(.blocked)))
        reducer.apply(.reset)
        #expect(reducer.snapshot.seen)
        #expect(reducer.snapshot.records.isEmpty)
    }

    @Test func boundedRecordsEvictLeastRecentlyUpdated() {
        var reducer = ProgramStatusReducer()
        for i in 0..<64 { reducer.apply(.report(report(.working, id: "r\(i)"))) }
        reducer.apply(.report(report(.blocked, id: "r0")))
        reducer.apply(.report(report(.working, id: "r64")))
        #expect(reducer.snapshot.records.count == 64)
        #expect(!reducer.snapshot.records.contains { $0.id == "r1" })
        #expect(reducer.snapshot.summary?.id == "r0")
    }

    @Test func inheritedAppIsResolvedWithoutChangingRecords() {
        var reducer = ProgramStatusReducer()
        reducer.apply(.report(report(.working, app: "root")))
        reducer.apply(.report(report(.working, id: "a", app: "parent")))
        let child = report(.blocked, id: "a/b/c")
        reducer.apply(.report(child))
        #expect(reducer.snapshot.app(for: child) == "parent")
        #expect(reducer.snapshot.records.last?.app == nil)
    }

    @Test func displayRemovesInvisibleFormattingAndKeepsLiteralMarkup() {
        let value = report(.blocked, msg: "**question** [click](https://example.com)\u{202E}\u{200B}\u{07}")
        #expect(value.displayText == "Blocked · Question · **question** [click](https://example.com)")
        #expect(value.msg?.contains("\u{202E}") == true)
    }

    @Test func snapshotsEncodeNullableFieldsAndRequireSeen() throws {
        let snapshot = ProgramStatusRecords(seen: true, records: [report(.working)])
        let data = try JSONEncoder().encode(snapshot)
        let object = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let records = try #require(object["records"] as? [[String: Any]])
        #expect(records[0]["msg"] is NSNull)
        #expect(records[0]["kind"] is NSNull)
        #expect(try JSONDecoder().decode(ProgramStatusRecords.self, from: data) == snapshot)
        #expect(throws: (any Error).self) {
            try JSONDecoder().decode(ProgramStatusRecords.self, from: Data(#"{"records":[]}"#.utf8))
        }
    }

    @Test func callbackCopiesBorrowedStrings() throws {
        let pointer = strdup("Continue?")!
        let event = copyProgramStatusEvent(event: 0, state: 3, kind: 1, progress: -1,
                                          id: nil, app: nil, title: nil, msg: pointer)
        pointer[0] = 88
        free(pointer)
        guard case .report(let value) = try #require(event) else {
            Issue.record("Expected copied report")
            return
        }
        #expect(value.msg == "Continue?")
        #expect(value.kind == .question)
        #expect(value.progress == nil)
        #expect(copyProgramStatusEvent(event: 2, state: -1, kind: -1, progress: -1,
                                       id: nil, app: nil, title: nil, msg: nil) == .reset)
    }

    @Test(.timeLimit(.minutes(1))) @MainActor func staleSurfaceAndClosedCallbacksCannotPublish() async throws {
        let surface = ProgramStatusSurface()
        surface.receive(.report(report(.blocked)), incarnation: UUID())
        try await Task.sleep(for: .milliseconds(300))
        #expect(!surface.snapshot.seen)
        await withCheckedContinuation { (publication: CheckedContinuation<Void, Never>) in
            withObservationTracking {
                _ = surface.snapshot
            } onChange: {
                publication.resume()
            }
            surface.receive(.report(report(.working)), incarnation: surface.incarnation)
        }
        #expect(surface.snapshot.summary?.state == .working)
        surface.close()
        surface.receive(.report(report(.blocked)), incarnation: surface.incarnation)
        try await Task.sleep(for: .milliseconds(300))
        #expect(surface.snapshot.records.isEmpty)
        #expect(surface.snapshot.seen)
    }

    /// What the server would have recorded, with the first sends refused.
    private final class StatusSink: @unchecked Sendable {
        struct Reading { let stream: String; let sequence: Int; let records: ProgramStatusRecords }
        private let lock = NSLock()
        private var refusals: Int
        private var readings: [Reading] = []

        init(refusals: Int = 0) { self.refusals = refusals }

        func send(_ write: ServedWrite) throws {
            try lock.withLock {
                if refusals > 0 {
                    refusals -= 1
                    throw RegistryQueryError("Status observation unavailable")
                }
                guard case .sessionStatus(_, _, _, _, let stream, let sequence, let records) = write else { return }
                readings.append(Reading(stream: stream, sequence: sequence, records: records))
            }
        }

        var received: [Reading] { lock.withLock { readings } }
    }

    @Test("A later status event recovers a refused observer without replacing the Session")
    @MainActor func observationFailureRecovers() async throws {
        let sink = StatusSink(refusals: 1)
        let surface = ProgramStatusSurface(send: { try sink.send($0) })
        defer { surface.close() }
        surface.associate(sessionId: "retained", terminalId: "terminal", provider: .init(agentProcessId: AgentProcessId(rawValue: "agent")))
        surface.receive(.report(report(.working, msg: "First turn")), incarnation: surface.incarnation)
        try await eventually { surface.observationError != nil }
        #expect(surface.sessionId == "retained")
        #expect(surface.observationError == "Status observation unavailable")
        // The new turn clears the previous report before its fresh result arrives.
        surface.receive(.reset, incarnation: surface.incarnation)
        try await eventually {
            guard let last = sink.received.last else { return false }
            return last.records.seen && last.records.records.isEmpty && surface.observationError == nil
        }
        surface.receive(.report(report(.blocked, msg: "New result")), incarnation: surface.incarnation)
        try await eventually { sink.received.last?.records.summary?.msg == "New result" }
        #expect(surface.sessionId == "retained")
        surface.close()
        // The closing snapshot still reaches the server, in order, on one stream.
        try await eventually { sink.received.count == 3 }
        #expect(sink.received.map(\.sequence) == [1, 2, 3])
        #expect(Set(sink.received.map(\.stream)).count == 1)
        #expect(sink.received.last?.records.records.isEmpty == true)
    }

    @Test("A restored question reaches the first Session reading without leaking to its replacement")
    @MainActor func restoredStatusSurvivesLateAssociation() async throws {
        let sink = StatusSink()
        let surface = ProgramStatusSurface(send: { try sink.send($0) })
        defer { surface.close() }
        func received() -> ProgramStatusRecords? { sink.received.last?.records }
        surface.receive(.report(report(.blocked, msg: "Original question")), incarnation: surface.incarnation)
        try await eventually { surface.snapshot.summary?.state == .blocked }
        surface.associate(sessionId: "retained", terminalId: "terminal", provider: .init(agentProcessId: AgentProcessId(rawValue: "first")))
        try await eventually { received()?.summary?.msg == "Original question" }
        surface.associate(sessionId: "retained", terminalId: "terminal", provider: .init(agentProcessId: AgentProcessId(rawValue: "replacement")))
        surface.receive(.report(report(.idle, id: "new")), incarnation: surface.incarnation)
        try await eventually { received()?.summary?.state == .idle }
        #expect(received()?.records.count == 1)
        // The replacement provider is a new observer, counted from one.
        #expect(sink.received.last?.sequence == 1)
        #expect(Set(sink.received.map(\.stream)).count == 2)
    }

    @MainActor private func eventually(_ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(5)
        while !condition() {
            try #require(ContinuousClock.now < deadline, "Status observation did not settle")
            try await Task.sleep(for: .milliseconds(10))
        }
    }

    @Test @MainActor func labelRendersProgramTextLiterally() throws {
        let value = report(.blocked, msg: "**Proceed?** [answer](https://example.com)")
        let view = ProgramStatusLabel(status: ProgramStatusRecords(seen: true, records: [value]))
        #expect(try view.inspect().find(text: value.displayText).string() == value.displayText)
    }
}
