import AppKit
import Testing
@testable import Concerto

@MainActor
@Suite("Keyboard Router")
struct KeyboardRouterTests {
    @Test("Slash and question resolve to different actions by modifiers")
    func slashAndQuestionActions() {
        let router = KeyboardRouter()
        var actions: [ShortcutAction] = []

        let slashHandled = router.routeEvent(
            key: .character("/"),
            modifiers: [],
            isRepeat: false,
            mode: .global
        ) { actions.append($0) }

        let questionHandled = router.routeEvent(
            key: .character("/"),
            modifiers: [.shift],
            isRepeat: false,
            mode: .global
        ) { actions.append($0) }

        #expect(slashHandled)
        #expect(questionHandled)
        #expect(actions == [.focusSessionComposer, .showHelp])
    }

    @Test("Command-K still opens command palette")
    func commandKOpensPalette() {
        let router = KeyboardRouter()
        var actions: [ShortcutAction] = []

        let handled = router.routeEvent(
            key: .character("k"),
            modifiers: [.command],
            isRepeat: false,
            mode: .global
        ) { actions.append($0) }

        #expect(handled)
        #expect(actions == [.openCommandPalette])
    }

    @Test("Normalize modifiers keeps only supported shortcut modifiers")
    func normalizeModifiers() {
        let router = KeyboardRouter()

        let normalized = router.normalizeModifiers([.shift, .command, .capsLock, .numericPad, .function])

        #expect(normalized == [.shift, .command])
    }

    @Test("Slash keycode fallback handles non-US layouts")
    func slashKeyFallback() {
        let router = KeyboardRouter()

        let normalized = router.normalizeKey(
            charactersIgnoringModifiers: "ß",
            specialKey: nil,
            keyCode: ShortcutCatalog.slashKeyCode
        )

        #expect(normalized == .keyCode(ShortcutCatalog.slashKeyCode))
    }

    @Test("Repeat is ignored for non-navigation actions")
    func repeatIgnoredForNonNavigationAction() {
        let router = KeyboardRouter()
        var actions: [ShortcutAction] = []

        let handled = router.routeEvent(
            key: .character("c"),
            modifiers: [],
            isRepeat: true,
            mode: .global
        ) { actions.append($0) }

        #expect(handled)
        #expect(actions.isEmpty)
    }

    @Test("Repeat is allowed for navigation actions")
    func repeatAllowedForNavigationAction() {
        let router = KeyboardRouter()
        var actions: [ShortcutAction] = []

        let handled = router.routeEvent(
            key: .character("j"),
            modifiers: [],
            isRepeat: true,
            mode: .global
        ) { actions.append($0) }

        #expect(handled)
        #expect(actions == [.moveDown])
    }

    @Test("g h chord executes go-to-first action")
    func chordTriggersAction() {
        let router = KeyboardRouter(chordTimeout: .milliseconds(100))
        var actions: [ShortcutAction] = []

        _ = router.routeEvent(
            key: .character("g"),
            modifiers: [],
            isRepeat: false,
            mode: .global
        ) { actions.append($0) }

        _ = router.routeEvent(
            key: .character("h"),
            modifiers: [],
            isRepeat: false,
            mode: .global
        ) { actions.append($0) }

        #expect(actions == [.goToFirst])
        #expect(router.chordPrefix == nil)
    }

    @Test("Chord timeout cancels pending chord")
    func chordTimeoutCancels() async {
        let router = KeyboardRouter(chordTimeout: .milliseconds(40))
        var actions: [ShortcutAction] = []

        _ = router.routeEvent(
            key: .character("g"),
            modifiers: [],
            isRepeat: false,
            mode: .global
        ) { actions.append($0) }

        // Poll until the timer's MainActor continuation clears the chord.
        // A single sleep+yield is unreliable on loaded CI runners.
        let deadline = ContinuousClock.now + .seconds(2)
        while router.chordPrefix != nil && ContinuousClock.now < deadline {
            try? await Task.sleep(for: .milliseconds(10))
        }

        #expect(router.chordPrefix == nil)

        _ = router.routeEvent(
            key: .character("h"),
            modifiers: [],
            isRepeat: false,
            mode: .global
        ) { actions.append($0) }

        #expect(actions.isEmpty)
    }

    @Test("Escape cancels pending chord")
    func escapeCancelsChord() {
        let router = KeyboardRouter(chordTimeout: .milliseconds(100))

        _ = router.routeEvent(
            key: .character("g"),
            modifiers: [],
            isRepeat: false,
            mode: .global
        ) { _ in }

        let handled = router.routeEvent(
            key: .keyCode(ShortcutCatalog.escapeKeyCode),
            modifiers: [],
            isRepeat: false,
            mode: .global
        ) { _ in }

        #expect(handled)
        #expect(router.chordPrefix == nil)
    }

    @Test("Mode resolver prioritizes text editing")
    func textEditingMode() {
        let router = KeyboardRouter()

        let mode = router.resolveMode(
            firstResponder: NSTextView(),
            isCommandPaletteVisible: false,
            isHelpOverlayVisible: false
        )

        #expect(mode == .textEditing)
    }

    @Test("Mode resolver recognizes terminal responder")
    func terminalMode() {
        let router = KeyboardRouter()

        let mode = router.resolveMode(
            firstResponder: GhosttyMetalViewStub(),
            isCommandPaletteVisible: false,
            isHelpOverlayVisible: false
        )

        #expect(mode == .terminal)
    }

    @Test("terminal mode still routes multiplexer shortcuts")
    func terminalModeRoutesMultiplexerShortcuts() {
        let router = KeyboardRouter()
        var actions: [ShortcutAction] = []

        let handled = router.routeEvent(
            key: .character("d"),
            modifiers: [.command],
            isRepeat: false,
            mode: .terminal
        ) { actions.append($0) }

        #expect(handled)
        #expect(actions == [.splitVertical])
    }

    @Test("terminal mode ignores non-multiplexer shortcuts")
    func terminalModeIgnoresNonMultiplexerShortcuts() {
        let router = KeyboardRouter()
        var actions: [ShortcutAction] = []

        let handled = router.routeEvent(
            key: .character("k"),
            modifiers: [.command],
            isRepeat: false,
            mode: .terminal
        ) { actions.append($0) }

        #expect(!handled)
        #expect(actions.isEmpty)
    }

    @Test("Help overlay mode only handles dismiss keys")
    func helpOverlayDismissKeys() {
        let router = KeyboardRouter()
        router.isHelpOverlayVisible = true

        let ignored = router.routeEvent(
            key: .character("j"),
            modifiers: [],
            isRepeat: false,
            mode: .helpOverlay
        ) { _ in }

        #expect(!ignored)

        let handled = router.routeEvent(
            key: .keyCode(ShortcutCatalog.escapeKeyCode),
            modifiers: [],
            isRepeat: false,
            mode: .helpOverlay
        ) { _ in }

        #expect(handled)
        #expect(router.isHelpOverlayVisible == false)
    }
}

private final class GhosttyMetalViewStub: NSObject {}
