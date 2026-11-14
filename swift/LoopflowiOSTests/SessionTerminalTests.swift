import GhosttyKit
import Loopflow
import XCTest
@testable import LoopflowiOS

/// A real Ghostty terminal with no Session behind it: bytes in, bytes out.
@MainActor
final class SessionTerminalTests: XCTestCase {
    private var terminal: GhosttySurfaceView!
    private var written = Data()
    private var scroll = GhosttySurfaceView.Scroll()
    private var grid: (columns: Int, rows: Int)?

    override func setUp() async throws {
        GhosttyRuntime.shared.apply(.dark, fontSize: 12)
        terminal = GhosttySurfaceView(frame: CGRect(x: 0, y: 0, width: 390, height: 600))
        terminal.onWrite = { [unowned self] in written.append($0) }
        terminal.onScroll = { [unowned self] in scroll = $0 }
        terminal.onGrid = { [unowned self] in grid = ($0, $1) }
        terminal.layoutIfNeeded()
        XCTAssertNotNil(terminal.surface, "Ghostty made no surface")
    }

    override func tearDown() async throws {
        terminal.close()
        terminal = nil
        written = Data()
        scroll = .init()
        grid = nil
    }

    private func feed(_ text: String) { terminal.feed(Data(text.utf8)) }

    private func wait(_ what: String, until done: () -> Bool) {
        let deadline = Date().addingTimeInterval(5)
        while !done(), Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.01)) }
        XCTAssertTrue(done(), what)
    }

    private func sent(_ key: DesktopKey) -> String {
        written = Data()
        terminal.press(usage: key.usage)
        wait("\(key.title) sent nothing") { !written.isEmpty }
        return String(decoding: written, as: UTF8.self)
    }

    func testReportsTheGridThatFitsTheView() throws {
        let grid = try XCTUnwrap(grid)
        let size = ghostty_surface_size(try XCTUnwrap(terminal.surface))
        XCTAssertEqual(grid.columns, Int(size.width_px / size.cell_width_px))
        XCTAssertEqual(grid.rows, Int(size.height_px / size.cell_height_px))
        XCTAssertGreaterThan(grid.columns, 20)
        XCTAssertGreaterThan(grid.rows, 10)
    }

    func testAnswersTheProgramStatusQuery() {
        feed("\u{1b}]7501;?\u{7}")
        wait("no answer to OSC 7501") { !written.isEmpty }
        XCTAssertEqual(String(decoding: written, as: UTF8.self), "\u{1b}]7501;?\u{7}")
    }

    func testKeyRowHonoursApplicationCursorMode() {
        XCTAssertEqual(sent(.up), "\u{1b}[A")
        XCTAssertEqual(sent(.escape), "\u{1b}")
        XCTAssertEqual(sent(.tab), "\t")
        XCTAssertEqual(sent(.enter), "\r")
        feed("\u{1b}[?1h")
        XCTAssertEqual(sent(.up), "\u{1b}OA")
        XCTAssertEqual(sent(.down), "\u{1b}OB")
        XCTAssertEqual(sent(.right), "\u{1b}OC")
        XCTAssertEqual(sent(.left), "\u{1b}OD")
    }

    func testReplyIsOnePasteThatFollowsTheProgramsPasteMode() {
        XCTAssertFalse(terminal.bracketedPaste)
        feed("\u{1b}[?2004h")
        XCTAssertTrue(terminal.bracketedPaste)
        XCTAssertEqual(SessionTerminalController.replyBytes("one\ntwo\u{1b}", bracketed: true),
                       Data("\u{1b}[200~one\ntwo\u{1b}[201~".utf8))
        XCTAssertEqual(SessionTerminalController.replyBytes("one\ntwo", bracketed: false), Data("one two".utf8))
        XCTAssertNil(SessionTerminalController.replyBytes("\u{1b}\u{7}", bracketed: true))
    }

    func testEarlierLinesStayPutWhileOutputArrives() {
        feed((1...300).map { "line \($0)\r\n" }.joined())
        wait("no scrollback reported") { scroll.total > scroll.visible }
        XCTAssertFalse(scroll.readingEarlier)

        terminal.scroll(toRow: 40)
        wait("did not scroll up") { scroll.offset == 40 }
        XCTAssertTrue(scroll.readingEarlier)

        let before = scroll.total
        feed((301...400).map { "line \($0)\r\n" }.joined())
        wait("new output was not counted") { scroll.total > before }
        XCTAssertEqual(scroll.offset, 40)

        terminal.scrollToLatest()
        wait("Latest did not return to the end") { !scroll.readingEarlier }
    }

    func testAClearedScreenDropsItsHistory() {
        feed((1...300).map { "line \($0)\r\n" }.joined())
        wait("no scrollback reported") { scroll.total > scroll.visible }
        feed("\u{1b}c")
        wait("history survived the reset") { scroll.total == scroll.visible }
    }

    func testEveryBundledThemeLoadsWithoutComplaint() throws {
        for theme in [ThemeDefinition.light, .dark] {
            let config = try XCTUnwrap(GhosttyRuntime.load(GhosttyRuntime.configuration(theme, fontSize: 14.5)))
            defer { ghostty_config_free(config) }
            XCTAssertEqual(ghostty_config_diagnostics_count(config), 0, theme.id)
            var color = ghostty_config_color_s()
            XCTAssertTrue(ghostty_config_get(config, &color, "background", 10))
            XCTAssertEqual(UInt32(color.r) << 16 | UInt32(color.g) << 8 | UInt32(color.b), theme.terminal.background.hex)
        }
    }

    func testHardwareKeysTypeOnlyWhatTheyPrint() {
        XCTAssertEqual(GhosttySurfaceView.typed("a"), "a")
        XCTAssertEqual(GhosttySurfaceView.typed("é"), "é")
        XCTAssertNil(GhosttySurfaceView.typed(UIKeyCommand.inputUpArrow))
        XCTAssertNil(GhosttySurfaceView.typed("\u{3}"))
        XCTAssertNil(GhosttySurfaceView.typed(""))
        XCTAssertEqual(GhosttySurfaceView.mods([.control, .shift]).rawValue,
                       GHOSTTY_MODS_CTRL.rawValue | GHOSTTY_MODS_SHIFT.rawValue)
    }
}
