import Testing
<<<<<<< HEAD
<<<<<<< HEAD
import AppKit
=======
>>>>>>> 55cd605c (lf commit: implement)
=======
import AppKit
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
@testable import Concerto

@Suite("Ghostty terminal command")
struct GhosttyTerminalViewTests {
    @Test("wraps environment assignments with env before the shell command")
    func wrapsEnvironmentAssignmentsWithEnv() {
        let command = buildGhosttyShellCommand(
            argv: ["/bin/zsh", "-lc", "echo hi"],
            env: ["RLM_DEPTH": "1"]
        )

        #expect(command == "env RLM_DEPTH='1' '/bin/zsh' '-lc' 'echo hi'")
    }

    @Test("returns the raw command when no environment is provided")
    func returnsRawCommandWithoutEnvironment() {
        let command = buildGhosttyShellCommand(
            argv: ["/bin/zsh", "-lc", "echo hi"],
            env: [:]
        )

        #expect(command == "'/bin/zsh' '-lc' 'echo hi'")
    }

    @Test("returns nil when there is no command to run")
    func returnsNilWithoutCommand() {
        #expect(buildGhosttyShellCommand(argv: [], env: ["RLM_DEPTH": "1"]) == nil)
    }
<<<<<<< HEAD
<<<<<<< HEAD
=======
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)

    @Test("control-modified control characters fall through to key handling")
    func controlCharactersFallThroughToKeyHandling() {
        #expect(ghosttyShouldHandleTextAsKeyEvent("\u{02}", modifiers: [.control]))
        #expect(!ghosttyShouldHandleTextAsKeyEvent("\n", modifiers: []))
        #expect(!ghosttyShouldHandleTextAsKeyEvent("a", modifiers: [.control]))
    }

    @Test("control and command shortcuts bypass interpretKeyEvents")
    func controlAndCommandBypassInterpretKeyEvents() {
        #expect(ghosttyShouldBypassInterpretKeyEvents(modifiers: [.control]))
        #expect(ghosttyShouldBypassInterpretKeyEvents(modifiers: [.command]))
        #expect(!ghosttyShouldBypassInterpretKeyEvents(modifiers: [.option]))
        #expect(!ghosttyShouldBypassInterpretKeyEvents(modifiers: []))
    }

<<<<<<< HEAD
    @Test("ordinary printable typing goes straight to key events")
    func ordinaryPrintableTypingUsesDirectKeyEvents() {
        #expect(
            ghosttyShouldHandleKeyDownDirectly(
                characters: "a",
                modifiers: [],
                hasMarkedText: false,
                selectedInputSource: "com.apple.keylayout.US"
            )
        )
    }

    @Test("composing input keeps printable keys on the text input path")
    func composingInputUsesTextInputPath() {
        #expect(
            !ghosttyShouldHandleKeyDownDirectly(
                characters: "a",
                modifiers: [],
                hasMarkedText: true,
                selectedInputSource: "com.apple.keylayout.US"
            )
        )
        #expect(
            !ghosttyShouldHandleKeyDownDirectly(
                characters: "a",
                modifiers: [],
                hasMarkedText: false,
                selectedInputSource: "com.apple.inputmethod.Kotoeri.Japanese"
            )
        )
        #expect(
            !ghosttyShouldHandleKeyDownDirectly(
                characters: "´",
                modifiers: [.option],
                hasMarkedText: false,
                selectedInputSource: "com.apple.keylayout.US"
            )
        )
    }

    @Test("printable key text is only attached for printable characters without command or control")
    func printableKeyTextRespectsModifiers() {
        #expect(ghosttyKeyText(characters: "a", modifiers: []) == "a")
        #expect(ghosttyKeyText(characters: " ", modifiers: []) == " ")
        #expect(ghosttyKeyText(characters: "\n", modifiers: []) == nil)
=======
    @Test("printable key text is only attached without command or control")
    func printableKeyTextRespectsModifiers() {
        #expect(ghosttyKeyText(characters: "a", modifiers: []) == "a")
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
        #expect(ghosttyKeyText(characters: "a", modifiers: [.control]) == nil)
        #expect(ghosttyKeyText(characters: "a", modifiers: [.command]) == nil)
        #expect(ghosttyKeyText(characters: nil, modifiers: []) == nil)
    }
<<<<<<< HEAD
=======
>>>>>>> 55cd605c (lf commit: implement)
=======
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
}
