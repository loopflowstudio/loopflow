import Foundation

enum Flags {
    static var beta: Bool {
        UserDefaults.standard.bool(forKey: "beta")
    }

    static func setBeta(_ enabled: Bool) {
        UserDefaults.standard.set(enabled, forKey: "beta")
    }

    static var embeddedTerminal: Bool {
        UserDefaults.standard.bool(forKey: "enableEmbeddedTerminal")
    }

    static func setEmbeddedTerminal(_ enabled: Bool) {
        UserDefaults.standard.set(enabled, forKey: "enableEmbeddedTerminal")
    }
}
