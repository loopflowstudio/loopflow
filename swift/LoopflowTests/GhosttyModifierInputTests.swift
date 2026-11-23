#if os(macOS) && canImport(GhosttyKit)
import AppKit
import GhosttyKit
import IOKit
import Synchronization
import Testing
@testable import LoopflowMac

@MainActor
@Suite("Desktop terminal modifier input")
struct GhosttyModifierInputTests {
    private final class Output: Sendable {
        let bytes = Mutex(Data())
    }

    @Test("Option-B follows the terminal's Option-as-Alt setting", arguments: [
        ("true", "\u{1b}b\u{1b}b"), ("left", "\u{1b}b∫"), ("right", "∫\u{1b}b"), ("false", "∫∫"),
    ])
    func optionB(setting: String, expected: String) async throws {
        _ = NSApplication.shared
        try #require(GhosttyManager.libraryReady)
        let config = try #require(ghostty_config_new())
        defer { ghostty_config_free(config) }
        try GhosttyThemeConfiguration.load("macos-option-as-alt = \(setting)\nwindow-vsync = false", into: config)
        ghostty_config_finalize(config)
        var runtime = ghostty_runtime_config_s()
        runtime.wakeup_cb = { @Sendable _ in }
        runtime.action_cb = { @Sendable _, _, _ in false }
        runtime.read_clipboard_cb = { @Sendable _, _, _, _, _, _ in GHOSTTY_CLIPBOARD_READ_UNAVAILABLE }
        runtime.confirm_read_clipboard_cb = { @Sendable _, _, _, _ in }
        runtime.write_clipboard_cb = { @Sendable _, _, _, _, _ in }
        let app = try #require(ghostty_app_new(&runtime, config))
        defer { ghostty_app_free(app) }

        let view = GhosttyMetalView(terminal: .shell("option-b", machineId: fixtureMachineId),
                                   frame: CGRect(x: 0, y: 0, width: 400, height: 200))
        let output = Output()
        var options = ghostty_surface_config_new()
        options.platform_tag = GHOSTTY_PLATFORM_MACOS
        options.platform.macos.nsview = Unmanaged.passUnretained(view).toOpaque()
        options.userdata = Unmanaged.passUnretained(output).toOpaque()
        options.io_write_cb = { @Sendable userdata, bytes, count in
            guard let userdata, let bytes else { return }
            let output = Unmanaged<Output>.fromOpaque(userdata).takeUnretainedValue()
            output.bytes.withLock { $0.append(Data(bytes: bytes, count: count)) }
        }
        let surface = try #require(ghostty_surface_new(app, &options))
        view.surface = surface
        defer {
            view.destroySurface()
            withExtendedLifetime(output) {}
        }
        // Paste wrappers make a mistaken trip through the clipboard API visible.
        ghostty_surface_process_output(surface, "\u{1b}[?2004h", 8)

        for side in [NX_DEVICELALTKEYMASK, NX_DEVICERALTKEYMASK] {
            let flags = NSEvent.ModifierFlags.option.union(.init(rawValue: UInt(side)))
            let event = try #require(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: flags,
                timestamp: 1, windowNumber: 0, context: nil, characters: "∫", charactersIgnoringModifiers: "b",
                isARepeat: false, keyCode: 0x0B))
            view.keyDown(with: event)
        }

        // Ghostty replies on the same IO queue, after both key events.
        ghostty_surface_process_output(surface, "\u{1b}[5n", 4)
        let reply = Data("\u{1b}[0n".utf8)
        let deadline = ContinuousClock.now + .seconds(5)
        while true {
            let bytes = output.bytes.withLock { $0 }
            if bytes.suffix(reply.count) == reply {
                #expect(Array(bytes.dropLast(reply.count)) == Array(expected.utf8))
                return
            }
            try #require(ContinuousClock.now < deadline, "Terminal wrote \(bytes as NSData)")
            try await Task.sleep(for: .milliseconds(10))
        }
    }
}
#endif
