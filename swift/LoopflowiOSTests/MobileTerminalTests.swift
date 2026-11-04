import Loopflow
import SwiftTerm
import ViewInspector
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

    func testFreshControlGrantRestoresKeyboardAfterHiddenAttachment() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        let bar = try XCTUnwrap(controller.view.subviews.first { $0 is UIStackView } as? UIStackView)
        let keyboard = try XCTUnwrap(bar.arrangedSubviews.last as? UIButton)
        keyboard.sendActions(for: .touchUpInside)
        XCTAssertNotNil(controller.terminal.inputView)
        workspace.send(Data("keep this draft".utf8))

        client.link.loseControl()
        await settle()
        controller.update(theme: .light)
        XCTAssertFalse(controller.terminal.canWrite)
        await workspace.reconnect()
        await settle()
        controller.update(theme: .light)
        XCTAssertEqual(workspace.state, .viewing)
        workspace.requestControl()
        await settle()
        controller.update(theme: .light)

        XCTAssertNil(controller.terminal.inputView)
        XCTAssertTrue(controller.terminal.isFirstResponder)
        XCTAssertEqual(keyboard.accessibilityLabel, "Hide keyboard")
        XCTAssertEqual(client.terminal.draft, "keep this draft")
        XCTAssertTrue(client.terminal.submissions.isEmpty)
    }

    func testObserverPreservesOwnerGridAcrossPhoneLayoutAndControlHandoff() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        client.link.loseControl()
        client.link.reportGeometry(columns: 120, rows: 40)
        await settle()
        controller.update(theme: .light)
        let sizeReports = client.link.sizes.count
        XCTAssertEqual(controller.terminal.getTerminal().cols, 120)
        XCTAssertEqual(controller.terminal.getTerminal().rows, 40)

        controller.view.frame.size = CGSize(width: 320, height: 400)
        controller.view.layoutIfNeeded()
        XCTAssertEqual(controller.terminal.getTerminal().cols, 120)
        XCTAssertEqual(controller.terminal.getTerminal().rows, 40)
        XCTAssertGreaterThan(controller.terminal.bounds.width, controller.view.bounds.width)
        let viewport = try XCTUnwrap(controller.terminal.superview as? UIScrollView)
        XCTAssertGreaterThan(viewport.contentSize.width, viewport.bounds.width)
        viewport.setContentOffset(CGPoint(x: 150, y: 0), animated: false)
        viewport.layoutIfNeeded()
        XCTAssertEqual(viewport.contentOffset.x, 150)
        XCTAssertEqual(controller.terminal.getTerminal().cols, 120)
        XCTAssertEqual(client.link.sizes.count, sizeReports)
        controller.terminal.insertText("observer must not type")
        XCTAssertTrue(client.terminal.draft.isEmpty)

        workspace.requestControl()
        await settle()
        controller.update(theme: .light)
        XCTAssertLessThan(controller.terminal.getTerminal().cols, 120)
        XCTAssertLessThan(controller.terminal.getTerminal().rows, 40)
        XCTAssertEqual(client.link.sizes.last?.0, controller.terminal.getTerminal().cols)
        XCTAssertEqual(client.link.sizes.last?.1, controller.terminal.getTerminal().rows)
        let grantedReports = client.link.sizes.count
        controller.view.setNeedsLayout()
        controller.view.layoutIfNeeded()
        XCTAssertEqual(client.link.sizes.count, grantedReports, "Unchanged layout must not send resize traffic")
        XCTAssertTrue(controller.terminal.getTerminal().bracketedPasteMode,
                      "Changing viewport must not soft-reset terminal input modes")
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

    func testReconnectPreservesSessionAfterConnectionDropOrStreamEnd() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        workspace.send(Data("draft".utf8))
        let id = workspace.selected?.primarySession
        for closeStream in [false, true] {
            if closeStream { client.link.close() }
            else { client.link.drop() }
            await settle()
            guard case .disconnected = workspace.state else { return XCTFail("Expected retryable disconnect") }
            workspace.send(Data("must not queue".utf8))
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
    }

    func testSessionScreenKeepsRetryAfterInitialAndRepeatedAttachmentFailure() async throws {
        let client = TestClient()
        client.attachFailures = 2
        let workspace = MobileWorkspace(client: client)
        defer { workspace.leave() }
        let task = try await client.workingTasks()[0]
        let screen = MobileSessionView(task: task, workspace: workspace, theme: .light)

        await workspace.open(task)
        for _ in 0..<2 {
            guard case .disconnected = workspace.state else { return XCTFail("Expected retryable failure") }
            XCTAssertEqual(workspace.selected?.primarySession, task.primarySession)
            XCTAssertThrowsError(try screen.inspect().find(button: "Use keyboard"))
            try screen.inspect().find(button: "Reconnect").tap()
            await settle()
        }

        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(client.link.sessionID, task.primarySession)
        XCTAssertThrowsError(try screen.inspect().find(button: "Reconnect"))
        workspace.send(Data("must not queue".utf8))
        workspace.resize(columns: 40, rows: 20)
        XCTAssertTrue(client.terminal.draft.isEmpty)
        XCTAssertTrue(client.link.sizes.isEmpty)
        try screen.inspect().find(button: "Use keyboard").tap()
        await settle()
        workspace.send(Data("same Session".utf8))
        XCTAssertEqual(client.terminal.draft, "same Session")
    }

    func testForegroundRestoresScreenWithoutInputOrGeometryTakeover() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        controller.terminal.insertText("phone draft")
        let session = workspace.selected?.primarySession
        let opens = client.attachments.count
        workspace.suspend()
        workspace.suspend()
        client.terminal.accept(Data(" + desktop".utf8))
        client.terminal.columns = 120
        client.terminal.rows = 40
        await workspace.restoreForegroundSession()
        await workspace.restoreForegroundSession()
        await settle()
        controller.update(theme: .light)
        XCTAssertEqual(client.attachments.count, opens + 1)
        XCTAssertEqual(client.attachments.last, session)
        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(controller.terminal.getTerminal().cols, 120)
        XCTAssertEqual(controller.terminal.getTerminal().rows, 40)
        XCTAssertTrue(controller.terminal.accessibilityPageContent()?.contains("phone draft + desktop") == true)
        XCTAssertEqual(client.transport.controlRequests, 0)
        XCTAssertTrue(client.link.sizes.isEmpty)
        controller.terminal.insertText("do not send")
        XCTAssertEqual(client.terminal.draft, "phone draft + desktop")
        XCTAssertTrue(client.terminal.submissions.isEmpty)
    }

    func testForegroundFailureWaitsForExplicitRetryAndLeaveCancelsRestoration() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        workspace.suspend()
        client.attachFailures = 1
        let count = client.attachments.count
        await workspace.restoreForegroundSession()
        guard case .disconnected = workspace.state else { return XCTFail("Expected a retryable connection failure") }
        await workspace.restoreForegroundSession()
        XCTAssertEqual(client.attachments.count, count + 1, "No foreground polling loop")
        await workspace.reconnect()
        await settle()
        XCTAssertEqual(workspace.state, .viewing)
        workspace.suspend()
        workspace.leave()
        let last = client.attachments.count
        await workspace.restoreForegroundSession()
        XCTAssertEqual(client.attachments.count, last, "Leaving the screen must cancel return to that Session")
        XCTAssertNil(workspace.selected)
    }

    func testBackgroundDuringOpeningDiscardsTheLateAttachment() async throws {
        let client = TestClient()
        client.delayNextAttachment = true
        let workspace = MobileWorkspace(client: client)
        defer { workspace.leave() }
        let task = try await client.workingTasks()[0]
        let opening = Task { await workspace.open(task) }
        let deadline = ContinuousClock.now.advanced(by: .seconds(2))
        while client.releaseAttachment == nil && ContinuousClock.now < deadline { await Task.yield() }
        let release = try XCTUnwrap(client.releaseAttachment)
        let lateLink = try XCTUnwrap(client.transport).link
        workspace.suspend()
        await workspace.restoreForegroundSession()
        await settle()
        XCTAssertEqual(workspace.state, .viewing)
        release.resume()
        await opening.value
        await settle()
        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(workspace.selected?.primarySession, task.primarySession)
        XCTAssertThrowsError(try lateLink.requestControl(size: TerminalGeometry(columns: 80, rows: 24)))
        workspace.requestControl()
        await settle()
        workspace.send(Data("new attachment".utf8))
        XCTAssertEqual(client.terminal.draft, "new attachment")
    }

    func testWrongSessionIsRejectedBeforeRenderingAndHasNoRetry() async throws {
        let client = TestClient()
        client.attachmentSessionID = "another-session"
        client.terminal.accept(Data("another Session's draft".utf8))
        let workspace = MobileWorkspace(client: client)
        defer { workspace.leave() }
        var output = Data()
        workspace.render = { bytes, _ in output.append(bytes) }
        let task = try await client.workingTasks()[0]
        let screen = MobileSessionView(task: task, workspace: workspace, theme: .light)
        await workspace.open(task)
        await settle()
        workspace.suspend()
        XCTAssertEqual(workspace.state, .unavailable("The attachment belongs to a different Session"))
        XCTAssertEqual(workspace.selected?.primarySession, task.primarySession)
        XCTAssertTrue(output.isEmpty)
        XCTAssertThrowsError(try client.link.requestControl(size: TerminalGeometry(columns: 80, rows: 24)), "Rejected attachment must be closed")
        XCTAssertThrowsError(try screen.inspect().find(button: "Reconnect"))
        XCTAssertThrowsError(try screen.inspect().find(button: "Use keyboard"))
    }

    func testDelayedKeyboardGrantDoesNotQueueTypingOrRepeatTakeover() async throws {
        let client = TestClient()
        let workspace = MobileWorkspace(client: client)
        defer { workspace.leave() }
        await workspace.open(try await client.workingTasks()[0])
        await settle()
        client.transport.delayControl = true
        workspace.requestControl()
        workspace.requestControl()
        workspace.send(Data("do not queue".utf8))
        XCTAssertEqual(workspace.state, .requestingControl)
        XCTAssertEqual(client.transport.controlRequests, 1)
        XCTAssertTrue(client.terminal.draft.isEmpty)
        try client.link.requestControl(size: TerminalGeometry(columns: 44, rows: 20))
        await settle()
        XCTAssertEqual(workspace.state, .controlling)
        XCTAssertTrue(client.terminal.draft.isEmpty)
        workspace.send(Data("deliberate input".utf8))
        XCTAssertEqual(client.terminal.draft, "deliberate input")
    }

    func testMissingOwnerRemainsUnavailableAfterBackgrounding() async throws {
        let client = TestClient()
        client.ownerUnavailable = true
        let workspace = MobileWorkspace(client: client)
        defer { workspace.leave() }
        let task = try await client.workingTasks()[0]
        await workspace.open(task)
        workspace.suspend()
        await workspace.restoreForegroundSession()
        XCTAssertEqual(workspace.state, .unavailable("No running owner"))
        let screen = MobileSessionView(task: task, workspace: workspace, theme: .light)
        XCTAssertThrowsError(try screen.inspect().find(button: "Reconnect"))
        XCTAssertThrowsError(try screen.inspect().find(button: "Use keyboard"))
        XCTAssertNil(client.transport)
    }

    func testAcceptedInputThenSendErrorRestoresDraftWithoutReplayOrDuplicateSubmission() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        let task = try XCTUnwrap(workspace.selected)
        let screen = MobileSessionView(task: task, workspace: workspace, theme: .light)

        client.transport.failNextSendAfterAccept = true
        controller.terminal.insertText("accepted once")
        guard case .disconnected = workspace.state else { return XCTFail("Expected uncertain delivery") }
        controller.terminal.insertText("must not queue")
        controller.terminal.insertText("\n")
        XCTAssertEqual(client.terminal.draft, "accepted once")
        XCTAssertTrue(client.terminal.submissions.isEmpty)
        try screen.inspect().find(button: "Reconnect").tap()
        await settle()
        XCTAssertEqual(client.link.sessionID, task.primarySession)
        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(client.terminal.draft, "accepted once")
        XCTAssertTrue(controller.terminal.accessibilityPageContent()?.contains("accepted once") == true)
        XCTAssertTrue(client.terminal.submissions.isEmpty)

        try screen.inspect().find(button: "Use keyboard").tap()
        await settle()
        client.transport.failNextSendAfterAccept = true
        controller.terminal.insertText("\n")
        XCTAssertEqual(client.terminal.submissions, ["accepted once"])
        try screen.inspect().find(button: "Reconnect").tap()
        await settle()
        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(client.link.sessionID, task.primarySession)
        try screen.inspect().find(button: "Use keyboard").tap()
        await settle()
        XCTAssertTrue(client.terminal.draft.isEmpty)
        XCTAssertEqual(client.terminal.submissions, ["accepted once"])
    }

    func testEndedAndMissingSessionsRemainUnavailable() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        client.link.end()
        await settle()
        workspace.suspend()
        await workspace.restoreForegroundSession()
        XCTAssertEqual(workspace.state, .ended)
        let endedScreen = MobileSessionView(task: try XCTUnwrap(workspace.selected), workspace: workspace, theme: .light)
        XCTAssertThrowsError(try endedScreen.inspect().find(button: "Reconnect"))
        workspace.send(Data("no".utf8))
        XCTAssertTrue(client.terminal.draft.isEmpty)
        let rows = try await FixtureClient().workingTasks()
        await workspace.open(rows[2])
        workspace.suspend()
        await workspace.restoreForegroundSession()
        XCTAssertEqual(workspace.state, .unavailable("No existing Session is recorded"))
        let missingScreen = MobileSessionView(task: rows[2], workspace: workspace, theme: .light)
        XCTAssertThrowsError(try missingScreen.inspect().find(button: "Reconnect"))
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
    var transport: TestAttachment!
    var link: FixtureAttachment { transport.link }
    var attachFailures = 0
    var attachmentSessionID: String?
    var ownerUnavailable = false
    var attachments: [String] = []
    var delayNextAttachment = false
    var releaseAttachment: CheckedContinuation<Void, Never>?

    func workingTasks() async throws -> [WorkViewPlan.Content] { try await FixtureClient().workingTasks() }
    func observeTasks() async throws -> any MobileTaskObservation { try await FixtureClient().observeTasks() }
    func attach(sessionID: String) async throws -> any MobileAttachment {
        attachments.append(sessionID)
        if ownerUnavailable { throw SessionUnavailable("No running owner") }
        if attachFailures > 0 {
            attachFailures -= 1
            throw URLError(.cannotConnectToHost)
        }
        let link = FixtureAttachment(sessionID: attachmentSessionID ?? sessionID, terminal: terminal)
        let transport = TestAttachment(link: link)
        self.transport = transport
        if delayNextAttachment {
            delayNextAttachment = false
            await withCheckedContinuation { releaseAttachment = $0 }
        }
        return transport
    }
}

@MainActor
private final class TestAttachment: MobileAttachment {
    let link: FixtureAttachment
    var failNextSendAfterAccept = false
    var delayControl = false
    var controlRequests = 0
    var sessionID: String { link.sessionID }
    var events: AsyncStream<MobileTerminalEvent> { link.events }

    init(link: FixtureAttachment) { self.link = link }

    func requestControl(size: TerminalGeometry) throws {
        controlRequests += 1
        if !delayControl { try link.requestControl(size: size) }
    }
    func send(_ bytes: Data) throws {
        try link.send(bytes)
        if failNextSendAfterAccept {
            failNextSendAfterAccept = false
            throw URLError(.networkConnectionLost)
        }
    }
    func resize(columns: Int, rows: Int) throws { try link.resize(columns: columns, rows: rows) }
    func close() { link.close() }
}
