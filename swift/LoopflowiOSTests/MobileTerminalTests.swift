import Loopflow
import SwiftTerm
import XCTest
@testable import LoopflowiOS

@MainActor
final class MobileTerminalTests: XCTestCase {
    private var window: UIWindow?

    func testNativeEditingPasteSubmissionAndPermissionResponse() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        let view = controller.terminal
        view.insertText("ac")
        view.send(txt: "\u{1b}[D")
        view.insertText("b")
        XCTAssertEqual(client.terminal.draft, "abc")
        view.deleteBackward()
        XCTAssertEqual(client.terminal.draft, "ac")
        view.pasteText("one\ntwo")
        XCTAssertEqual(client.terminal.draft, "aone\ntwoc")
        XCTAssertTrue(client.terminal.submissions.isEmpty)
        view.insertText("\n")
        XCTAssertEqual(client.terminal.submissions, ["aone\ntwoc"])
        await settle()
        XCTAssertTrue(view.accessibilityPageContent()?.contains("Received") == true)

        view.insertText("permission")
        view.insertText("\n")
        await settle()
        XCTAssertTrue(view.accessibilityPageContent()?.contains("Allow fixture action?") == true)
        view.insertText("n")
        view.insertText("\n")
        await settle()
        XCTAssertTrue(view.accessibilityPageContent()?.contains("canceled") == true)
    }

    func testMarkedTextCommitsOnceAndUnicodeSurvives() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        let view = controller.terminal
        view.setMarkedText("ni", selectedRange: NSRange(location: 2, length: 0))
        XCTAssertEqual(client.terminal.draft, "")
        view.setMarkedText("你", selectedRange: NSRange(location: 1, length: 0))
        XCTAssertEqual(client.terminal.draft, "")
        view.unmarkText()
        XCTAssertEqual(client.terminal.draft, "你")
        view.insertText("e")
        view.insertText("\u{301}")
        view.insertText("🌱")
        XCTAssertEqual(client.terminal.draft, "你é🌱")
        view.deleteBackward()
        XCTAssertEqual(client.terminal.draft, "你é")
        view.deleteBackward()
        view.insertText("好")
        XCTAssertEqual(client.terminal.draft, "你好")
    }

    func testNativeTextReplacementEditsThePeerDraft() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        let view = controller.terminal
        view.insertText("hello world")
        let start = try XCTUnwrap(view.position(from: view.beginningOfDocument, offset: 6))
        let end = try XCTUnwrap(view.position(from: view.beginningOfDocument, offset: 11))
        let range = try XCTUnwrap(view.textRange(from: start, to: end))
        view.replace(range, withText: "phone")
        XCTAssertEqual(client.terminal.draft, "hello phone")
        XCTAssertTrue(client.terminal.submissions.isEmpty)
    }

    func testKeyboardCanHideWhileHardwareInputKeepsWorking() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        let bar = try XCTUnwrap(controller.view.subviews.first { $0 is UIStackView } as? UIStackView)
        let keyboard = try XCTUnwrap(bar.arrangedSubviews.last as? UIButton)
        keyboard.sendActions(for: .touchUpInside)
        XCTAssertNotNil(controller.terminal.inputView)
        XCTAssertTrue(controller.terminal.isFirstResponder)
        controller.terminal.insertText("hardware")
        XCTAssertEqual(client.terminal.draft, "hardware")
        XCTAssertEqual(keyboard.accessibilityLabel, "Show keyboard")
        keyboard.sendActions(for: .touchUpInside)
        XCTAssertNil(controller.terminal.inputView)
    }

    func testBackendOwnedFixturesDecodeWithoutDefaults() throws {
        let response: MachineResponse = try machineFixture("reading")
        guard case .reading(let generation, let frame) = response.result else { return XCTFail("Expected reading") }
        XCTAssertEqual(generation, 7)
        guard case .object(let fields) = frame else { return XCTFail("Expected watch frame") }
        XCTAssertEqual(fields["unavailable"], .string("repository is unavailable"))
        XCTAssertEqual(fields["body"], .null)
        XCTAssertThrowsError(try JSONDecoder().decode(MachineResponse.self,
            from: Data(#"{"id":"x","instance":"i","result":{"kind":"read"}}"#.utf8)))

        let request = MachineRequest(version: 1, id: "tasks-1", operation: .subscribe(
            .roadmap(directory: "/", wave: nil, task: nil, all: true), generation: 7))
        XCTAssertEqual(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)),
                       try machineFixture("subscribe"))

        let ownerResponse: MachineResponse = try machineFixture("owner")
        guard case .sessionOwner(let owner) = ownerResponse.result else { return XCTFail("Expected Session owner") }
        XCTAssertEqual(owner, "85c31c1e-850c-4821-878d-55527b6dba46")
        let commandResponse: MachineResponse = try machineFixture("command")
        guard case .command(let trace, _, _, let code) = commandResponse.result else { return XCTFail("Expected command result") }
        XCTAssertFalse(trace.isEmpty)
        XCTAssertEqual(code, 0)
    }

    func testTaskReadingRejectsStaleAndUnavailableReplies() async throws {
        let url = try XCTUnwrap(Bundle.main.url(forResource: "task-reading", withExtension: "json"))
        let data = try Data(contentsOf: url)
        XCTAssertThrowsError(try decodeMobileTasks(data, requestID: "old-request"))
        XCTAssertEqual(try decodeMobileTasks(data, requestID: "fixture-tasks").count, 3)
        XCTAssertThrowsError(try decodeMobileTasks(
            Data(#"{"id":"fixture-tasks","instance":"i","result":{"kind":"error","code":"unavailable","message":"Offline"}}"#.utf8),
            requestID: "fixture-tasks"))
    }

    func testNativeSelectionCopiesRenderedTextAndViewportResizes() async throws {
        let (workspace, _, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        controller.view.frame = CGRect(x: 0, y: 0, width: 390, height: 700)
        controller.view.layoutIfNeeded()
        let rows = controller.terminal.getTerminal().rows
        controller.view.frame.size.height = 400
        controller.view.layoutIfNeeded()
        XCTAssertLessThan(controller.terminal.getTerminal().rows, rows)
        controller.terminal.setSelectionRange(start: Position(col: 0, row: 0), end: Position(col: 8, row: 0))
        XCTAssertEqual(controller.terminal.getSelection(), "Loopflow")
        XCTAssertTrue(controller.terminal.accessibilityPageContent()?.contains("Offline input harness") == true)
    }

    func testViewingAndControlLossNeverSendOrResize() async throws {
        let client = TestClient()
        let workspace = MobileWorkspace(client: client)
        let task = try await FixtureClient().workingTasks()[0]
        await workspace.open(task)
        await settle()
        workspace.send(Data("ignored".utf8))
        workspace.resize(columns: 40, rows: 20)
        XCTAssertTrue(client.terminal.draft.isEmpty)
        XCTAssertTrue(client.link.sizes.isEmpty)
        workspace.requestControl()
        await settle()
        XCTAssertEqual(client.link.sizes.count, 1)
        workspace.send(Data("kept".utf8))
        client.link.loseControl()
        await settle()
        workspace.send(Data("ignored".utf8))
        workspace.resize(columns: 20, rows: 10)
        XCTAssertEqual(client.terminal.draft, "kept")
        XCTAssertEqual(client.link.sizes.count, 1)
        XCTAssertEqual(workspace.state, .viewing)
        workspace.leave()
    }

    func testReconnectPreservesSessionAndDoesNotReplayUncertainInput() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        workspace.send(Data("draft".utf8))
        client.link.drop()
        await settle()
        workspace.send(Data("must not queue".utf8))
        let id = workspace.selected?.primarySession
        await workspace.reconnect()
        await settle()
        XCTAssertEqual(workspace.selected?.primarySession, id)
        XCTAssertEqual(client.link.sessionID, id)
        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(client.terminal.draft, "draft")
        XCTAssertTrue(client.terminal.submissions.isEmpty)
        workspace.requestControl()
        await settle()
        XCTAssertEqual(client.terminal.draft, "draft")
    }

    func testEndedAndMissingSessionsRemainUnavailable() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        client.link.end()
        await settle()
        XCTAssertEqual(workspace.state, .ended)
        workspace.send(Data("no".utf8))
        XCTAssertTrue(client.terminal.draft.isEmpty)
        let rows = try await FixtureClient().workingTasks()
        await workspace.open(rows[2])
        XCTAssertEqual(workspace.state, .unavailable("No existing Session is recorded"))
    }

    func testMultilinePasteWithoutBracketModeDoesNotSubmit() {
        let view = SessionTerminalView(frame: CGRect(x: 0, y: 0, width: 390, height: 300))
        defer { view.updateUiClosed() }
        view.canWrite = true
        var explanation: String?
        view.pasteUnavailable = { explanation = $0 }
        view.pasteText("one\ntwo")
        XCTAssertEqual(explanation, "This program has not enabled multiline paste.")
    }

    private func machineFixture<T: Decodable>(_ name: String) throws -> T {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: name, withExtension: "json", subdirectory: "machine"))
        return try JSONDecoder().decode(T.self, from: Data(contentsOf: url))
    }

    private func connected() async throws -> (MobileWorkspace, TestClient, SessionTerminalController) {
        bootstrapLoopflowApp()
        let client = TestClient()
        let workspace = MobileWorkspace(client: client)
        let controller = SessionTerminalController(workspace: workspace)
        controller.loadViewIfNeeded()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 700))
        window.rootViewController = controller
        window.isHidden = false
        self.window = window
        controller.view.frame = window.bounds
        controller.view.layoutIfNeeded()
        let task = try await FixtureClient().workingTasks()[0]
        await workspace.open(task)
        await settle()
        workspace.requestControl()
        await settle()
        controller.update(theme: .light)
        XCTAssertEqual(workspace.state, .controlling)
        return (workspace, client, controller)
    }

    private func settle() async {
        for _ in 0..<20 { await Task.yield() }
    }
}

@MainActor
private final class TestClient: MobileClient {
    let terminal = FixtureTerminal()
    var link: FixtureAttachment!

    func workingTasks() async throws -> [WorkViewPlan.Content] { try await FixtureClient().workingTasks() }
    func attach(sessionID: String) async throws -> any MobileAttachment {
        link = FixtureAttachment(sessionID: sessionID, terminal: terminal)
        return link
    }
}
