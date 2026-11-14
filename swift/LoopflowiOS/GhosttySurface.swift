import GhosttyKit
import Loopflow
import UIKit

/// The process's one Ghostty app and the appearance its surfaces draw with.
@MainActor
final class GhosttyRuntime {
    static let shared = GhosttyRuntime()

    private(set) var app: ghostty_app_t?
    private var config: ghostty_config_t?
    private var appearance: Appearance?

    private struct Appearance: Equatable {
        let terminal: TerminalPalette
        let dark: Bool
        let fontSize: Double
    }

    private static let ready = ghostty_init(UInt(CommandLine.argc), CommandLine.unsafeArgv) == GHOSTTY_SUCCESS

    /// Ghostty's own font is JetBrains Mono. The Session view insets the terminal,
    /// and nothing runs here to confirm closing.
    static func configuration(_ theme: ThemeDefinition, fontSize: Double) -> String {
        """
        font-size = \(fontSize)
        window-padding-x = 0
        window-padding-y = 0
        window-padding-balance = false
        confirm-close-surface = false
        \(theme.ghosttyOverlay)
        """
    }

    /// A finalized configuration; the caller frees it.
    static func load(_ text: String) -> ghostty_config_t? {
        guard ready, let config = ghostty_config_new() else { return nil }
        let path = FileManager.default.temporaryDirectory.appendingPathComponent("loopflow-ghostty-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: path) }
        guard (try? Data(text.utf8).write(to: path)) != nil else {
            ghostty_config_free(config)
            return nil
        }
        path.path.withCString { ghostty_config_load_file(config, $0) }
        ghostty_config_finalize(config)
        return config
    }

    func apply(_ theme: ThemeDefinition, fontSize: Double) {
        let wanted = Appearance(terminal: theme.terminal, dark: theme.isDark, fontSize: fontSize)
        guard wanted != appearance, let next = Self.load(Self.configuration(theme, fontSize: fontSize)) else { return }
        if let app {
            ghostty_app_update_config(app, next)
        } else {
            app = Self.makeApp(next)
        }
        guard let app else {
            ghostty_config_free(next)
            return
        }
        ghostty_app_set_color_scheme(app, theme.isDark ? GHOSTTY_COLOR_SCHEME_DARK : GHOSTTY_COLOR_SCHEME_LIGHT)
        if let config { ghostty_config_free(config) }
        config = next
        appearance = wanted
    }

    private func tick() {
        if let app { ghostty_app_tick(app) }
    }

    private static func view(_ userdata: UnsafeMutableRawPointer?) -> GhosttySurfaceView? {
        guard let userdata else { return nil }
        return Unmanaged<SurfaceHandle>.fromOpaque(userdata).takeUnretainedValue().view
    }

    private static func makeApp(_ config: ghostty_config_t) -> ghostty_app_t? {
        var runtime = ghostty_runtime_config_s()
        runtime.supports_selection_clipboard = false
        runtime.wakeup_cb = { _ in
            DispatchQueue.main.async { MainActor.assumeIsolated { GhosttyRuntime.shared.tick() } }
        }
        // Ghostty performs actions on the thread that ticks it.
        runtime.action_cb = { _, target, action in
            guard target.tag == GHOSTTY_TARGET_SURFACE, let surface = target.target.surface else { return false }
            return MainActor.assumeIsolated {
                guard let view = GhosttyRuntime.view(ghostty_surface_userdata(surface)) else { return false }
                switch action.tag {
                case GHOSTTY_ACTION_SCROLLBAR:
                    let bar = action.action.scrollbar
                    view.onScroll?(.init(total: Int(bar.total), offset: Int(bar.offset), visible: Int(bar.len)))
                    return true
                case GHOSTTY_ACTION_CELL_SIZE:
                    view.setNeedsLayout()
                    return true
                default:
                    return false
                }
            }
        }
        runtime.read_clipboard_cb = { userdata, _, state, _, _, _ in
            MainActor.assumeIsolated {
                guard let surface = GhosttyRuntime.view(userdata)?.surface,
                      let value = UIPasteboard.general.string else { return GHOSTTY_CLIPBOARD_READ_UNAVAILABLE }
                value.withCString { text in
                    "text/plain".withCString { mime in
                        var content = ghostty_clipboard_content_s(mime: mime, data: text, len: value.utf8.count)
                        withUnsafePointer(to: &content) { contents in
                            var complete = ghostty_clipboard_complete_s(
                                contents: contents, contents_len: 1, available: nil, available_len: 0,
                                confirmed: false, remember: false
                            )
                            ghostty_surface_complete_clipboard_request(surface, &complete, state)
                        }
                    }
                }
                return GHOSTTY_CLIPBOARD_READ_STARTED
            }
        }
        // A paste the person asked for goes through; a program reading the clipboard does not.
        runtime.confirm_read_clipboard_cb = { userdata, content, state, request in
            MainActor.assumeIsolated {
                guard let surface = GhosttyRuntime.view(userdata)?.surface else { return }
                guard request == GHOSTTY_CLIPBOARD_REQUEST_PASTE, let content else {
                    ghostty_surface_deny_clipboard_request(surface, state)
                    return
                }
                var complete = ghostty_clipboard_complete_s(
                    contents: content.pointee.contents, contents_len: content.pointee.contents_len,
                    available: content.pointee.available, available_len: content.pointee.available_len,
                    confirmed: true, remember: false
                )
                ghostty_surface_complete_clipboard_request(surface, &complete, state)
            }
        }
        runtime.write_clipboard_cb = { _, _, content, len, _ in
            guard let content, len > 0, let data = content.pointee.data else { return }
            let bytes = UnsafeRawBufferPointer(start: data, count: content.pointee.len)
            UIPasteboard.general.string = String(decoding: bytes, as: UTF8.self)
        }
        runtime.close_surface_cb = { _, _ in }
        return ghostty_app_new(&runtime, config)
    }
}

/// What Ghostty's callbacks hold. Its IO thread hands bytes over here, so the
/// handle outlives a callback that is still on its way to the main thread.
private final class SurfaceHandle: @unchecked Sendable {
    @MainActor weak var view: GhosttySurfaceView?
}

/// A Ghostty terminal with no process behind it: `feed` supplies what the
/// Session printed, and `onWrite` receives what the terminal sends back —
/// keys and the answers to a program's queries.
@MainActor
final class GhosttySurfaceView: UIView {
    /// Rows of history and screen, and which of them are showing.
    struct Scroll: Equatable {
        var total = 0
        var offset = 0
        var visible = 0
        var lastOffset: Int { max(0, total - visible) }
        var readingEarlier: Bool { offset < lastOffset }
    }

    var onWrite: ((Data) -> Void)?
    var onGrid: ((_ columns: Int, _ rows: Int) -> Void)?
    var onScroll: ((Scroll) -> Void)?

    private(set) var surface: ghostty_surface_t?
    private let handle = SurfaceHandle()
    /// Output that arrived before the first layout gave the terminal a size.
    private var early = Data()
    private var held: (key: ghostty_input_key_s, text: String?, timer: Timer)?

    override init(frame: CGRect) {
        super.init(frame: frame)
        handle.view = self
        clipsToBounds = true
    }

    required init?(coder: NSCoder) { fatalError("Use init(frame:)") }

    private var scale: CGFloat { traitCollection.displayScale > 0 ? traitCollection.displayScale : 2 }

    /// Points per row; zero until the terminal has a size.
    var rowHeight: CGFloat {
        guard let surface else { return 0 }
        return CGFloat(ghostty_surface_size(surface).cell_height_px) / scale
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        guard bounds.width > 0, bounds.height > 0 else { return }
        if surface == nil { open() }
        guard let surface else { return }
        // Ghostty draws into a sublayer it adds; UIKit does not resize sublayers.
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        for sublayer in layer.sublayers ?? [] {
            sublayer.frame = bounds
            sublayer.contentsScale = scale
        }
        CATransaction.commit()
        ghostty_surface_set_content_scale(surface, scale, scale)
        ghostty_surface_set_size(surface, UInt32(bounds.width * scale), UInt32(bounds.height * scale))
        let size = ghostty_surface_size(surface)
        onGrid?(Int(size.columns), Int(size.rows))
    }

    private func open() {
        guard let app = GhosttyRuntime.shared.app else { return }
        var config = ghostty_surface_config_new()
        config.userdata = Unmanaged.passUnretained(handle).toOpaque()
        config.platform_tag = GHOSTTY_PLATFORM_IOS
        config.platform = ghostty_platform_u(
            ios: ghostty_platform_ios_s(uiview: Unmanaged.passUnretained(self).toOpaque())
        )
        config.scale_factor = scale
        config.io_write_cb = { userdata, bytes, count in
            guard let userdata, let bytes, count > 0 else { return }
            let handle = Unmanaged<SurfaceHandle>.fromOpaque(userdata).takeUnretainedValue()
            let data = Data(bytes: bytes, count: count)
            DispatchQueue.main.async { MainActor.assumeIsolated { handle.view?.onWrite?(data) } }
        }
        surface = ghostty_surface_new(app, &config)
        guard let surface else { return }
        ghostty_surface_set_focus(surface, true)
        if !early.isEmpty {
            feed(early)
            early = Data()
        }
    }

    /// Frees the terminal. Ghostty stops its threads before this returns.
    func close() {
        held?.timer.invalidate()
        held = nil
        guard let surface else { return }
        self.surface = nil
        ghostty_surface_free(surface)
    }

    func feed(_ bytes: Data) {
        guard !bytes.isEmpty else { return }
        guard let surface else {
            early.append(bytes)
            return
        }
        bytes.withUnsafeBytes { buffer in
            ghostty_surface_process_output(surface, buffer.bindMemory(to: CChar.self).baseAddress, UInt(buffer.count))
        }
    }

    /// Whether the program asked for pastes to arrive bracketed.
    var bracketedPaste: Bool {
        guard let surface else { return false }
        return ghostty_surface_mode(surface, 2004, false)
    }

    func scroll(toRow row: Int) { perform("scroll_to_row:\(row)") }
    func scrollToLatest() { perform("scroll_to_bottom") }

    private func perform(_ action: String) {
        guard let surface else { return }
        action.withCString { _ = ghostty_surface_binding_action(surface, $0, UInt(action.utf8.count)) }
    }

    // MARK: Keys

    /// A key by its USB keyboard usage; Ghostty encodes it for the modes the program set.
    func press(usage: UInt32) {
        var key = ghostty_input_key_s()
        key.mods = GHOSTTY_MODS_NONE
        key.consumed_mods = GHOSTTY_MODS_NONE
        key.keycode = 0x07_0000 | usage
        _ = send(key, text: nil, action: GHOSTTY_ACTION_PRESS)
        _ = send(key, text: nil, action: GHOSTTY_ACTION_RELEASE)
    }

    private func send(_ key: ghostty_input_key_s, text: String?, action: ghostty_input_action_e) -> Bool {
        guard let surface else { return false }
        var key = key
        key.action = action
        guard let text, action != GHOSTTY_ACTION_RELEASE else { return ghostty_surface_key(surface, key) }
        return text.withCString {
            key.text = $0
            return ghostty_surface_key(surface, key)
        }
    }

    override var canBecomeFirstResponder: Bool { true }

    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        var handled = false
        for press in presses {
            guard let pressed = press.key else { continue }
            let (key, text) = Self.translate(pressed)
            handled = send(key, text: text, action: GHOSTTY_ACTION_PRESS) || handled
            // UIKit reports a held key once; the terminal repeats it.
            held?.timer.invalidate()
            let timer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { [weak self] _ in
                MainActor.assumeIsolated {
                    guard let self, let held = self.held else { return }
                    _ = self.send(held.key, text: held.text, action: GHOSTTY_ACTION_REPEAT)
                }
            }
            timer.fireDate = Date().addingTimeInterval(0.4)
            held = (key, text, timer)
        }
        if !handled { super.pressesBegan(presses, with: event) }
    }

    override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        release(presses)
        super.pressesEnded(presses, with: event)
    }

    override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        release(presses)
        super.pressesCancelled(presses, with: event)
    }

    private func release(_ presses: Set<UIPress>) {
        for press in presses {
            guard let pressed = press.key else { continue }
            let (key, _) = Self.translate(pressed)
            if held?.key.keycode == key.keycode {
                held?.timer.invalidate()
                held = nil
            }
            _ = send(key, text: nil, action: GHOSTTY_ACTION_RELEASE)
        }
    }

    /// A hardware key as Ghostty reads it. Text is what the key types; keys that
    /// type nothing (arrows, Control chords) are encoded from the key itself.
    static func translate(_ pressed: UIKey) -> (ghostty_input_key_s, String?) {
        var key = ghostty_input_key_s()
        key.keycode = 0x07_0000 | UInt32(pressed.keyCode.rawValue)
        key.mods = mods(pressed.modifierFlags)
        key.consumed_mods = GHOSTTY_MODS_NONE
        key.unshifted_codepoint = pressed.charactersIgnoringModifiers.unicodeScalars.first.map(\.value) ?? 0
        return (key, typed(pressed.characters))
    }

    static func typed(_ characters: String) -> String? {
        guard let first = characters.unicodeScalars.first, first.value >= 0x20, first.value != 0x7F,
              !characters.hasPrefix("UIKeyInput") else { return nil }
        return characters
    }

    static func mods(_ flags: UIKeyModifierFlags) -> ghostty_input_mods_e {
        var mods = GHOSTTY_MODS_NONE.rawValue
        if flags.contains(.shift) { mods |= GHOSTTY_MODS_SHIFT.rawValue }
        if flags.contains(.control) { mods |= GHOSTTY_MODS_CTRL.rawValue }
        if flags.contains(.alternate) { mods |= GHOSTTY_MODS_ALT.rawValue }
        if flags.contains(.command) { mods |= GHOSTTY_MODS_SUPER.rawValue }
        if flags.contains(.alphaShift) { mods |= GHOSTTY_MODS_CAPS.rawValue }
        return ghostty_input_mods_e(rawValue: mods)
    }
}
