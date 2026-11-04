import Loopflow
import SafariServices
import SwiftTerm
import SwiftUI
import UIKit
import ViewInspector
import XCTest
@testable import LoopflowiOS

@MainActor
final class MobileTerminalTests: XCTestCase {
    private var window: UIWindow?

    func testFindMovesBetweenUnicodeMatchesWithoutSendingInputOrChangingTheGrid() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        let between = (0..<100).map { "Line \($0)\r\n" }.joined()
        workspace.render?(Data("\r\nFirst 你好\r\n\(between)Second 你好\r\n".utf8), false)
        let sent = client.transport.sent.count
        let sizes = client.link.sizes.count
        let grid = controller.terminal.getTerminal()
        let columns = grid.cols, rows = grid.rows
        workspace.findInTerminal?()
        let finder = try XCTUnwrap(controller.view.subviews.compactMap { $0 as? UIStackView }.first {
            $0.arrangedSubviews.contains { $0.accessibilityIdentifier == "terminal-find-query" }
        })
        let field = try XCTUnwrap(finder.arrangedSubviews.first as? UITextField)
        XCTAssertTrue(field.isFirstResponder)
        XCTAssertFalse(controller.terminal.canBecomeFirstResponder)
        XCTAssertFalse(controller.terminal.allowMouseReporting)
        controller.terminal.insertText("not remote input")
        controller.terminal.deleteBackward()
        controller.terminal.pasteText("not a remote paste")
        XCTAssertEqual(client.transport.sent.count, sent)
        field.text = "你好"
        field.sendActions(for: .editingChanged)
        XCTAssertEqual(controller.terminal.getSelection(), "你好")
        XCTAssertLessThan(controller.terminal.scrollPosition, 1, "The first match is in local scrollback")
        let first = controller.terminal.selection.start
        _ = controller.textFieldShouldReturn(field)
        XCTAssertEqual(controller.terminal.getSelection(), "你好")
        XCTAssertNotEqual(controller.terminal.selection.start, first)
        _ = controller.textFieldShouldReturn(field)
        XCTAssertEqual(controller.terminal.selection.start, first, "Find wraps through loaded history")
        let searchBar = try XCTUnwrap(controller.view.subviews.compactMap { $0 as? UIStackView }.first {
            $0.arrangedSubviews.contains { $0.accessibilityIdentifier == "terminal-find-feedback" }
        })
        let feedback = try XCTUnwrap(searchBar.arrangedSubviews.first { $0 is UILabel } as? UILabel)
        field.text = "missing phrase"
        field.sendActions(for: .editingChanged)
        XCTAssertEqual(feedback.text, "No matches")
        XCTAssertFalse(controller.terminal.hasActiveSelection)
        controller.view.frame.size.height -= 120
        controller.view.layoutIfNeeded()
        XCTAssertEqual(grid.cols, columns)
        XCTAssertEqual(grid.rows, rows)
        XCTAssertEqual(client.link.sizes.count, sizes)
        controller.view.frame.size.height += 120
        controller.view.layoutIfNeeded()
        let done = try XCTUnwrap(finder.arrangedSubviews.last as? UIButton)
        done.sendActions(for: .touchUpInside)
        XCTAssertTrue(finder.isHidden)
        XCTAssertEqual(workspace.state, .controlling)
        XCTAssertTrue(controller.terminal.allowMouseReporting)
        XCTAssertEqual(client.transport.sent.count, sent)
        XCTAssertEqual(client.link.sizes.count, sizes)
    }

    func testFindRevealsAnOffscreenObserverMatchWithoutTakingControl() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        client.link.loseControl()
        client.link.reportGeometry(columns: 120, rows: 80)
        await settle()
        controller.update(theme: .light)
        workspace.render?(Data("\u{1b}[2J\u{1b}[70;101HNeedle".utf8), false)
        let sent = client.transport.sent.count, sizes = client.link.sizes.count
        let controls = client.transport.controlRequests
        workspace.findInTerminal?()
        let finder = try XCTUnwrap(controller.view.subviews.compactMap { $0 as? UIStackView }.first {
            $0.arrangedSubviews.contains { $0.accessibilityIdentifier == "terminal-find-query" }
        })
        let field = try XCTUnwrap(finder.arrangedSubviews.first as? UITextField)
        field.text = "needle"
        field.sendActions(for: .editingChanged)
        XCTAssertEqual(controller.terminal.getSelection(), "Needle")
        let viewport = try XCTUnwrap(controller.view.subviews.first { $0.accessibilityIdentifier == "terminal-viewport" } as? UIScrollView)
        XCTAssertGreaterThan(viewport.contentOffset.x, 0)
        XCTAssertGreaterThan(viewport.contentOffset.y, 0)
        XCTAssertEqual(controller.terminal.getTerminal().cols, 120)
        XCTAssertEqual(controller.terminal.getTerminal().rows, 80)
        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(client.transport.sent.count, sent)
        XCTAssertEqual(client.link.sizes.count, sizes)
        XCTAssertEqual(client.transport.controlRequests, controls)
    }

    func testFindRefreshesAfterNewOutputAndReconnectBeforeExplicitKeyboardTakeover() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        workspace.findInTerminal?()
        let finder = try XCTUnwrap(controller.view.subviews.compactMap { $0 as? UIStackView }.first {
            $0.arrangedSubviews.contains { $0.accessibilityIdentifier == "terminal-find-query" }
        })
        let field = try XCTUnwrap(finder.arrangedSubviews.first as? UITextField)
        field.text = "arrived later"
        field.sendActions(for: .editingChanged)
        XCTAssertFalse(controller.terminal.hasActiveSelection)
        workspace.render?(Data("\r\narrived later\r\n".utf8), false)
        _ = controller.textFieldShouldReturn(field)
        XCTAssertEqual(controller.terminal.getSelection(), "arrived later")

        workspace.suspend()
        controller.update(theme: .light)
        await workspace.restoreForegroundSession()
        await settle()
        controller.update(theme: .light)
        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(field.text, "arrived later")
        XCTAssertFalse(controller.terminal.hasActiveSelection, "A replacement screen must not keep a stale highlight")
        XCTAssertFalse(finder.isHidden)
        XCTAssertEqual(client.transport.controlRequests, 0)
        XCTAssertTrue(client.transport.sent.isEmpty)
        XCTAssertTrue(client.link.sizes.isEmpty)
        field.text = "Loopflow"
        field.sendActions(for: .editingChanged)
        XCTAssertEqual(controller.terminal.getSelection(), "Loopflow")
        workspace.requestControl()
        await settle()
        controller.update(theme: .light)
        XCTAssertTrue(finder.isHidden)
        XCTAssertFalse(controller.terminal.hasActiveSelection)
        XCTAssertTrue(controller.terminal.isFirstResponder)
        XCTAssertEqual(workspace.state, .controlling)
        controller.terminal.insertText("actual input")
        XCTAssertEqual(client.terminal.draft, "actual input")
    }

    func testFindLayouts() async throws {
        for (name, theme, size) in [("light", ThemeDefinition.light, UIContentSizeCategory.large),
                                    ("dark", .dark, .large), ("large-text", .light, .accessibilityExtraLarge)] {
            let (workspace, client, controller) = try await connected()
            defer { workspace.leave(); controller.terminal.updateUiClosed() }
            client.link.loseControl()
            await settle()
            controller.update(theme: theme)
            controller.overrideUserInterfaceStyle = theme.isDark ? .dark : .light
            controller.traitOverrides.preferredContentSizeCategory = size
            controller.view.layoutIfNeeded()
            let window = try XCTUnwrap(window)
            let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
            let previous = scene.keyWindow
            window.windowScene = scene
            window.makeKeyAndVisible()
            defer { window.isHidden = true; previous?.makeKey() }
            workspace.findInTerminal?()
            try await Task.sleep(for: .milliseconds(200))
            controller.view.layoutIfNeeded()
            let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
                window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
            }
            let attachment = XCTAttachment(image: image)
            attachment.name = "terminal-find-\(name)"
            attachment.lifetime = .keepAlways
            add(attachment)
        }
    }

    func testFindKeyboardDoesNotResizeAControllingTerminalWithItsKeyboardHidden() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        let window = try XCTUnwrap(window)
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let previous = scene.keyWindow
        window.windowScene = scene
        window.makeKeyAndVisible()
        defer { window.isHidden = true; previous?.makeKey() }
        let keyboard = try XCTUnwrap(controller.view.subviews.compactMap { $0 as? UIStackView }
            .flatMap(\.arrangedSubviews).first { $0.accessibilityLabel == "Hide keyboard" } as? UIButton)
        keyboard.sendActions(for: .touchUpInside)
        try await Task.sleep(for: .milliseconds(500))
        let sizes = client.link.sizes.count
        workspace.findInTerminal?()
        try await Task.sleep(for: .milliseconds(500))
        XCTAssertEqual(client.link.sizes.count, sizes)
        let done = try XCTUnwrap(controller.view.subviews.compactMap { $0 as? UIStackView }
            .flatMap(\.arrangedSubviews).first { $0.accessibilityLabel == "Close Find" } as? UIButton)
        done.sendActions(for: .touchUpInside)
        try await Task.sleep(for: .milliseconds(500))
        XCTAssertEqual(client.link.sizes.count, sizes)
    }

    func testReadingHistoryLayouts() async throws {
        for (name, theme, size) in [("light", ThemeDefinition.light, UIContentSizeCategory.large),
                                    ("dark", .dark, .large), ("large-text", .light, .accessibilityExtraLarge)] {
            let (workspace, client, controller) = try await connected()
            defer { workspace.leave(); controller.terminal.updateUiClosed() }
            client.link.loseControl()
            await settle()
            controller.update(theme: theme)
            controller.overrideUserInterfaceStyle = theme.isDark ? .dark : .light
            controller.traitOverrides.preferredContentSizeCategory = size
            controller.view.layoutIfNeeded()
            workspace.render?(Data((0..<100).map { "Reading line \($0)\r\n" }.joined().utf8), false)
            controller.terminal.scroll(toPosition: 0.3)
            controller.view.layoutIfNeeded()
            let window = try XCTUnwrap(window)
            let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
            let previous = scene.keyWindow
            window.windowScene = scene
            window.makeKeyAndVisible()
            defer { window.isHidden = true; previous?.makeKey() }
            try await Task.sleep(for: .milliseconds(150))
            let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
                window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
            }
            let attachment = XCTAttachment(image: image)
            attachment.name = "terminal-reading-\(name)"
            attachment.lifetime = .keepAlways
            add(attachment)
        }
    }

    func testLatestReturnsFromScrollbackAndOversizedViewportWithoutTakingControl() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        client.link.loseControl()
        await settle()
        controller.update(theme: .light)
        let view = controller.terminal
        let sent = client.transport.sent.count
        XCTAssertEqual(view.getTerminal().link(at: .buffer(Position(col: 2, row: 0)), mode: .explicitOnly), "https://loopflow.studio")
        let sizes = client.link.sizes
        let controls = client.transport.controlRequests
        workspace.render?(Data((0..<100).map { "Reading line \($0)\r\n" }.joined().utf8), false)
        view.scroll(toPosition: 0.3)
        view.setSelectionRange(start: Position(col: 0, row: 20), end: Position(col: 7, row: 20))
        let selection = try XCTUnwrap(view.getSelection())
        XCTAssertFalse(selection.isEmpty)
        let latest = try XCTUnwrap(controller.view.subviews.first { $0.accessibilityIdentifier == "terminal-latest" } as? UIButton)
        XCTAssertFalse(latest.isHidden)
        controller.view.layoutIfNeeded()
        XCTAssertEqual(latest.configuration?.title, "Latest")
        XCTAssertGreaterThanOrEqual(latest.bounds.width, 44)
        let offset = view.contentOffset.y
        workspace.render?(Data("More output while reading\r\n".utf8), false)
        XCTAssertEqual(view.contentOffset.y, offset, accuracy: 1, "New output must not interrupt reading")
        latest.sendActions(for: .touchUpInside)
        XCTAssertEqual(view.scrollPosition, 1)
        XCTAssertEqual(view.getSelection(), selection)
        XCTAssertTrue(latest.isHidden)

        workspace.setGeometry?(TerminalGeometry(columns: 120, rows: 80))
        controller.view.layoutIfNeeded()
        let viewport = try XCTUnwrap(controller.view.subviews.first { $0.accessibilityIdentifier == "terminal-viewport" } as? UIScrollView)
        viewport.setContentOffset(CGPoint(x: 100, y: 0), animated: false)
        XCTAssertFalse(latest.isHidden)
        latest.sendActions(for: .touchUpInside)
        XCTAssertEqual(viewport.contentOffset.x, 100)
        XCTAssertEqual(viewport.contentOffset.y, viewport.contentSize.height - viewport.bounds.height, accuracy: 1)
        XCTAssertTrue(latest.isHidden)
        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(client.transport.sent.count, sent)
        XCTAssertTrue(client.link.sizes.elementsEqual(sizes, by: { $0.0 == $1.0 && $0.1 == $1.1 }))
        XCTAssertEqual(client.transport.controlRequests, controls)
    }

    func testObserverCanSelectAndOpenWebLinksWithoutAKeyboardOrRemoteInput() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        client.link.loseControl()
        await settle()
        controller.update(theme: .light)
        let view = controller.terminal
        let sent = client.transport.sent.count
        XCTAssertTrue(view.becomeFirstResponder(), "Reading gestures need a local responder")
        XCTAssertNotNil(view.inputView, "Observation must not show the software keyboard")
        XCTAssertFalse(view.allowMouseReporting)
        view.setSelectionRange(start: Position(col: 0, row: 0), end: Position(col: 8, row: 0))
        XCTAssertEqual(view.getSelection(), "Loopflow")
        view.insertText("must not send")
        view.deleteBackward()
        view.pasteText("must not paste")
        controller.requestOpenLink(source: view, link: "file:///tmp/local-on-another-machine", params: [:])
        XCTAssertNil(controller.presentedViewController)
        XCTAssertTrue(controller.view.subviews.contains { ($0 as? UILabel)?.text == "This link cannot be opened on this device." })
        controller.requestOpenLink(source: view, link: "https://example.invalid/review", params: [:])
        XCTAssertTrue(controller.presentedViewController is SFSafariViewController)
        controller.dismiss(animated: false)
        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(client.transport.sent.count, sent)
    }

    func testMobileTextColorsRemainReadableInBothThemes() {
        for theme in [ThemeDefinition.light, .dark] {
            let text = [theme.palette.text, theme.palette.textSecondary, theme.palette.accentInk,
                        theme.palette.available, theme.palette.working, theme.palette.warning]
            for background in [theme.palette.background, theme.palette.surface] {
                for foreground in text {
                    let first = luminance(foreground)
                    let second = luminance(background)
                    // WCAG's small-text contrast ratio, independent of font rasterization.
                    let ratio = (max(first, second) + 0.05) / (min(first, second) + 0.05)
                    XCTAssertGreaterThanOrEqual(ratio, 4.5, "\(theme.id): text contrast is \(ratio)")
                }
            }
        }
    }

    private func luminance(_ color: SwiftUI.Color) -> Double {
        var red: CGFloat = 0, green: CGFloat = 0, blue: CGFloat = 0, alpha: CGFloat = 0
        XCTAssertTrue(UIColor(color).getRed(&red, green: &green, blue: &blue, alpha: &alpha))
        XCTAssertEqual(alpha, 1)
        let linear = [red, green, blue].map { channel -> Double in
            let value = Double(channel)
            return value <= 0.04045 ? value / 12.92 : pow((value + 0.055) / 1.055, 2.4)
        }
        return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
    }

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

    func testProgramStatusDetectionAnswersOnlyLiveControlledOutput() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        let before = client.transport.sent.count
        let query = Data("\u{1b}]7501;?\u{1b}\\".utf8)
        workspace.render?(query, false)
        XCTAssertEqual(Array(client.transport.sent.dropFirst(before)), [Data("\u{1b}]7501;?\u{7}".utf8)])
        client.link.replaceScreen(query)
        await settle()
        XCTAssertEqual(client.transport.sent.count, before + 1, "Snapshot queries must never answer again")
        client.link.loseControl()
        await settle()
        workspace.render?(query, false)
        XCTAssertEqual(client.transport.sent.count, before + 1, "Only the controlling viewer answers")
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
        let keyboard = try XCTUnwrap(controller.view.subviews.compactMap { $0 as? UIStackView }
            .flatMap(\.arrangedSubviews).first { $0.accessibilityLabel == "Hide keyboard" } as? UIButton)
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
        let keyboard = try XCTUnwrap(controller.view.subviews.compactMap { $0 as? UIStackView }
            .flatMap(\.arrangedSubviews).first { $0.accessibilityLabel == "Hide keyboard" } as? UIButton)
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

    func testLargerTextPreservesObserverCellsAndOnlyResizesAfterTakeover() async throws {
        let (workspace, client, controller) = try await connected()
        defer { workspace.leave(); controller.terminal.updateUiClosed() }
        client.link.loseControl()
        client.link.reportGeometry(columns: 120, rows: 40)
        await settle()
        controller.update(theme: .light)
        // Alternate screens do not reflow. A temporary narrower resize would
        // lose these cells even if the final grid went back to 120 columns.
        controller.terminal.feed(text: "\u{1b}[?1049h\u{1b}[2J\u{1b}[1;110HRIGHT EDGE\u{1b}[39;1HBOTTOM EDGE")
        let before = controller.terminal.accessibilityPageContent()
        let pointSize = controller.terminal.font.pointSize
        let reports = client.link.sizes.count
        controller.traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraLarge
        controller.view.layoutIfNeeded()
        await settle()
        XCTAssertGreaterThan(controller.terminal.font.pointSize, pointSize)
        XCTAssertEqual(controller.terminal.getTerminal().cols, 120)
        XCTAssertEqual(controller.terminal.getTerminal().rows, 40)
        XCTAssertEqual(controller.terminal.accessibilityPageContent(), before)
        XCTAssertEqual(client.link.sizes.count, reports)
        workspace.requestControl()
        await settle()
        controller.update(theme: .light)
        XCTAssertLessThan(controller.terminal.getTerminal().cols, 120)
        XCTAssertEqual(client.link.sizes.last?.0, controller.terminal.getTerminal().cols)
        XCTAssertEqual(client.link.sizes.last?.1, controller.terminal.getTerminal().rows)
        workspace.send(Data("large text".utf8))
        XCTAssertEqual(client.terminal.draft, "large text")
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

    func testSavedSessionOpensOnlyOnExplicitActionAndStaysAnObserver() async throws {
        let client = TestClient()
        client.ownerNotRunning = true
        let workspace = MobileWorkspace(client: client)
        defer { workspace.leave() }
        let task = try await client.workingTasks()[0]
        await workspace.open(task)
        workspace.suspend()
        await workspace.restoreForegroundSession()
        XCTAssertEqual(workspace.state, .notRunning)
        XCTAssertTrue(client.starts.isEmpty)
        let screen = MobileSessionView(task: task, workspace: workspace, theme: .light)
        XCTAssertThrowsError(try screen.inspect().find(button: "Use keyboard"))
        try screen.inspect().find(button: "Open Session").tap()
        await settle()
        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(client.starts, [try XCTUnwrap(task.primarySession)])
        XCTAssertEqual(client.transport.controlRequests, 0)
        XCTAssertTrue(client.link.sizes.isEmpty)
        workspace.send(Data("not until keyboard takeover".utf8))
        XCTAssertTrue(client.terminal.draft.isEmpty)
        workspace.requestControl()
        await settle()
        workspace.send(Data("continued conversation".utf8))
        XCTAssertEqual(client.terminal.draft, "continued conversation")
    }

    func testSavedSessionOpeningLayouts() async throws {
        bootstrapLoopflowApp()
        for (name, theme, textSize) in [("light", ThemeDefinition.light, DynamicTypeSize.large),
                                      ("dark", .dark, .large), ("large-text", .light, .accessibility3)] {
            let client = TestClient()
            client.ownerNotRunning = true
            let workspace = MobileWorkspace(client: client)
            defer { workspace.leave() }
            let task = try await client.workingTasks()[0]
            let view = NavigationStack { MobileSessionView(task: task, workspace: workspace, theme: theme) }
                .environment(\.dynamicTypeSize, textSize)
                .preferredColorScheme(name == "dark" ? .dark : .light)
            let controller = UIHostingController(rootView: view)
            let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
            let previous = scene.keyWindow
            let window = UIWindow(windowScene: scene)
            window.frame = CGRect(x: 0, y: 0, width: 390, height: 844)
            window.rootViewController = controller
            window.makeKeyAndVisible()
            defer { window.isHidden = true; previous?.makeKey() }
            controller.view.frame = window.bounds
            controller.view.layoutIfNeeded()
            try await Task.sleep(for: .milliseconds(250))
            XCTAssertEqual(workspace.state, .notRunning)
            let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
                XCTAssertTrue(window.drawHierarchy(in: window.bounds, afterScreenUpdates: true))
            }
            let attachment = XCTAttachment(image: image)
            attachment.name = "open-session-\(name)"
            attachment.lifetime = .keepAlways
            add(attachment)
        }
    }

    func testLostOpeningReplyReconnectsWithoutStartingAgain() async throws {
        let client = TestClient()
        client.ownerNotRunning = true
        client.loseStartReply = true
        let workspace = MobileWorkspace(client: client)
        defer { workspace.leave() }
        await workspace.open(try await client.workingTasks()[0])
        await workspace.startSession()
        guard case .disconnected(let reason) = workspace.state else { return XCTFail("Opening must remain uncertain") }
        XCTAssertTrue(reason.contains("Reconnect to inspect"))
        XCTAssertFalse(workspace.canStartSession)
        await workspace.startSession()
        workspace.suspend()
        await workspace.restoreForegroundSession()
        await settle()
        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(client.starts.count, 1)
        XCTAssertEqual(client.transport.controlRequests, 0)
    }

    func testLateSessionOpeningCannotReopenAfterLeavingOrBackgrounding() async throws {
        for leave in [false, true] {
            let client = TestClient()
            client.ownerNotRunning = true
            client.delayNextStart = true
            let workspace = MobileWorkspace(client: client)
            defer { workspace.leave() }
            await workspace.open(try await client.workingTasks()[0])
            let opening = Task { await workspace.startSession() }
            let deadline = ContinuousClock.now.advanced(by: .seconds(2))
            while client.releaseStart == nil && ContinuousClock.now < deadline { await Task.yield() }
            let release = try XCTUnwrap(client.releaseStart)
            await workspace.startSession() // A second tap cannot overlap the first.
            if leave { workspace.leave() } else { workspace.suspend() }
            release.resume()
            await opening.value
            XCTAssertEqual(client.starts.count, 1)
            XCTAssertEqual(client.attachments.count, 1, "A late acknowledgement cannot attach")
            await workspace.restoreForegroundSession()
            await settle()
            if leave {
                XCTAssertNil(workspace.selected)
                XCTAssertEqual(client.attachments.count, 1)
            } else {
                XCTAssertEqual(workspace.state, .viewing)
                XCTAssertEqual(client.transport.controlRequests, 0)
            }
        }
    }

    func testCanceledOpeningOffersInspectionInsteadOfStayingLoading() async throws {
        let client = TestClient()
        client.ownerNotRunning = true
        client.delayNextStart = true
        let workspace = MobileWorkspace(client: client)
        defer { workspace.leave() }
        await workspace.open(try await client.workingTasks()[0])
        let opening = Task { await workspace.startSession() }
        let deadline = ContinuousClock.now.advanced(by: .seconds(2))
        while client.releaseStart == nil && ContinuousClock.now < deadline { await Task.yield() }
        let release = try XCTUnwrap(client.releaseStart)
        opening.cancel()
        release.resume()
        await opening.value
        guard case .disconnected = workspace.state else { return XCTFail("Canceled opening needs inspection") }
        XCTAssertEqual(client.attachments.count, 1)
        await workspace.reconnect()
        await settle()
        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(client.starts.count, 1)
    }

    func testUnavailableSessionRemainsUnavailableAfterBackgrounding() async throws {
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
        try endedScreen.inspect().find(button: "Open Session").tap()
        await settle()
        XCTAssertEqual(workspace.state, .viewing)
        XCTAssertEqual(client.starts.count, 1)
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
        // Fixed viewport/text size for line-selection fixtures; the font-change
        // test explicitly grows this after the observer screen is populated.
        controller.traitOverrides.preferredContentSizeCategory = .large
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
private final class TestClient: MobileClient, MobileSessionOpeningClient {
    let terminal = FixtureTerminal()
    var transport: TestAttachment!
    var link: FixtureAttachment { transport.link }
    var attachFailures = 0
    var attachmentSessionID: String?
    var ownerUnavailable = false
    var ownerNotRunning = false
    var starts: [String] = []
    var loseStartReply = false
    var delayNextStart = false
    var releaseStart: CheckedContinuation<Void, Never>?
    var attachments: [String] = []
    var delayNextAttachment = false
    var releaseAttachment: CheckedContinuation<Void, Never>?

    func workingTasks() async throws -> [WorkViewPlan.Content] { try await FixtureClient().workingTasks() }
    func observeTasks() async throws -> any MobileTaskObservation { try await FixtureClient().observeTasks() }
    func startSession(sessionID: String) async throws {
        starts.append(sessionID)
        ownerNotRunning = false
        if delayNextStart {
            delayNextStart = false
            await withCheckedContinuation { releaseStart = $0 }
        }
        if loseStartReply { throw URLError(.networkConnectionLost) }
    }
    func attach(sessionID: String) async throws -> any MobileAttachment {
        attachments.append(sessionID)
        if ownerNotRunning { throw SessionNotRunning() }
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
    private(set) var sent: [Data] = []
    var sessionID: String { link.sessionID }
    var events: AsyncStream<MobileTerminalEvent> { link.events }

    init(link: FixtureAttachment) { self.link = link }

    func requestControl(size: TerminalGeometry) throws {
        controlRequests += 1
        if !delayControl { try link.requestControl(size: size) }
    }
    func send(_ bytes: Data) throws {
        sent.append(bytes)
        try link.send(bytes)
        if failNextSendAfterAccept {
            failNextSendAfterAccept = false
            throw URLError(.networkConnectionLost)
        }
    }
    func resize(columns: Int, rows: Int) throws { try link.resize(columns: columns, rows: rows) }
    func close() { link.close() }
}
