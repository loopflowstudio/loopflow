import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Program Status")
struct ProgramStatusTests {
    private func report(_ state: ProgramStatusState, id: String? = nil,
                        app: String? = nil, msg: String? = nil) -> ProgramStatusReport {
        ProgramStatusReport(state: state, id: id, kind: state == .blocked ? .question : nil,
                            progress: nil, app: app, title: nil, msg: msg)
    }

    @Test func replacementAndChildLifetime() {
        var reducer = ProgramStatusReducer()
        reducer.apply(.report(report(.blocked, id: "flow/step", msg: "Proceed?")))
        reducer.apply(.report(report(.working, id: "flow")))
        #expect(reducer.snapshot.summary?.state == .blocked)
        reducer.apply(.report(report(.working, id: "flow/step")))
        #expect(reducer.snapshot.records.count == 2)
        #expect(reducer.snapshot.summary?.msg == nil)
        #expect(reducer.snapshot.summary?.kind == nil)
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

    @Test @MainActor func staleSurfaceAndClosedCallbacksCannotPublish() async throws {
        let surface = ProgramStatusSurface()
        surface.receive(.report(report(.blocked)), incarnation: UUID())
        try await Task.sleep(for: .milliseconds(300))
        #expect(!surface.snapshot.seen)
        surface.receive(.report(report(.working)), incarnation: surface.incarnation)
        try await Task.sleep(for: .milliseconds(300))
        #expect(surface.snapshot.summary?.state == .working)
        surface.close()
        surface.receive(.report(report(.blocked)), incarnation: surface.incarnation)
        try await Task.sleep(for: .milliseconds(300))
        #expect(surface.snapshot.records.isEmpty)
        #expect(surface.snapshot.seen)
    }

    @Test @MainActor func labelRendersProgramTextLiterally() throws {
        let value = report(.blocked, msg: "**Proceed?** [answer](https://example.com)")
        let view = ProgramStatusLabel(status: ProgramStatusRecords(seen: true, records: [value]))
        #expect(try view.inspect().find(text: value.displayText).string() == value.displayText)
    }
}
