#if GHOSTTY_ENABLED
import AppKit
import GhosttyKit
import IOKit

private let modifiers: [(flag: NSEvent.ModifierFlags, ghostty: ghostty_input_mods_e,
                         rightMask: UInt, right: ghostty_input_mods_e)] = [
    (.shift, GHOSTTY_MODS_SHIFT, UInt(NX_DEVICERSHIFTKEYMASK), GHOSTTY_MODS_SHIFT_RIGHT),
    (.control, GHOSTTY_MODS_CTRL, UInt(NX_DEVICERCTLKEYMASK), GHOSTTY_MODS_CTRL_RIGHT),
    (.option, GHOSTTY_MODS_ALT, UInt(NX_DEVICERALTKEYMASK), GHOSTTY_MODS_ALT_RIGHT),
    (.command, GHOSTTY_MODS_SUPER, UInt(NX_DEVICERCMDKEYMASK), GHOSTTY_MODS_SUPER_RIGHT),
]

extension NSEvent.ModifierFlags {
    var ghosttyMods: ghostty_input_mods_e {
        var bits = contains(.capsLock) ? GHOSTTY_MODS_CAPS.rawValue : GHOSTTY_MODS_NONE.rawValue
        for modifier in modifiers where contains(modifier.flag) {
            bits |= modifier.ghostty.rawValue
            if rawValue & modifier.rightMask != 0 { bits |= modifier.right.rawValue }
        }
        return ghostty_input_mods_e(rawValue: bits)
    }
}

extension NSEvent {
    func translatedForText(in surface: ghostty_surface_t) -> NSEvent {
        let translated = ghostty_surface_key_translation_mods(surface, modifierFlags.ghosttyMods)
        var flags = modifierFlags
        for modifier in modifiers {
            if translated.rawValue & modifier.ghostty.rawValue != 0 {
                flags.insert(modifier.flag)
            } else {
                flags.remove(modifier.flag)
            }
        }
        // Input methods rely on the original event when no translation is needed.
        guard flags != modifierFlags else { return self }
        return NSEvent.keyEvent(with: type, location: locationInWindow, modifierFlags: flags,
            timestamp: timestamp, windowNumber: windowNumber, context: nil,
            characters: characters(byApplyingModifiers: flags) ?? "",
            charactersIgnoringModifiers: charactersIgnoringModifiers ?? "",
            isARepeat: isARepeat, keyCode: keyCode) ?? self
    }

    func ghosttyKey(translationFlags: ModifierFlags? = nil, composing: Bool = false) -> ghostty_input_key_s {
        var key = ghostty_input_key_s()
        if type == .keyUp {
            key.action = GHOSTTY_ACTION_RELEASE
        } else {
            key.action = isARepeat ? GHOSTTY_ACTION_REPEAT : GHOSTTY_ACTION_PRESS
        }
        key.mods = modifierFlags.ghosttyMods
        key.consumed_mods = (translationFlags ?? modifierFlags).subtracting([.control, .command]).ghosttyMods
        key.keycode = UInt32(keyCode)
        key.unshifted_codepoint = characters(byApplyingModifiers: [])?.unicodeScalars.first?.value ?? 0
        key.composing = composing
        return key
    }

    func hasGhosttyBinding(in surface: ghostty_surface_t) -> Bool {
        var key = ghosttyKey()
        return (characters ?? "").withCString {
            key.text = $0
            return ghostty_surface_key_is_binding(surface, key, nil)
        }
    }

    var ghosttyModifierKey: ghostty_input_key_s? {
        // The aggregate Shift flag stays set when just one of two held Shifts lifts.
        let mask: UInt
        switch keyCode {
        case 0x38: mask = UInt(NX_DEVICELSHIFTKEYMASK)
        case 0x3C: mask = UInt(NX_DEVICERSHIFTKEYMASK)
        case 0x3B: mask = UInt(NX_DEVICELCTLKEYMASK)
        case 0x3E: mask = UInt(NX_DEVICERCTLKEYMASK)
        case 0x3A: mask = UInt(NX_DEVICELALTKEYMASK)
        case 0x3D: mask = UInt(NX_DEVICERALTKEYMASK)
        case 0x37: mask = UInt(NX_DEVICELCMDKEYMASK)
        case 0x36: mask = UInt(NX_DEVICERCMDKEYMASK)
        case 0x39: mask = NSEvent.ModifierFlags.capsLock.rawValue
        case 0x3F: mask = NSEvent.ModifierFlags.function.rawValue
        default: return nil
        }
        var key = ghostty_input_key_s()
        key.mods = modifierFlags.ghosttyMods
        key.keycode = UInt32(keyCode)
        key.action = modifierFlags.rawValue & mask != 0 ? GHOSTTY_ACTION_PRESS : GHOSTTY_ACTION_RELEASE
        return key
    }
}

extension ghostty_input_key_s {
    func send(to surface: ghostty_surface_t, text: String) {
        text.withCString { pointer in
            var key = self
            key.text = pointer
            _ = ghostty_surface_key(surface, key)
        }
    }
}
#endif
