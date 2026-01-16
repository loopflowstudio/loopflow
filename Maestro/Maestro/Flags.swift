import Foundation

enum Flags {
    static var beta: Bool {
        UserDefaults.standard.bool(forKey: "beta")
    }

    static func setBeta(_ enabled: Bool) {
        UserDefaults.standard.set(enabled, forKey: "beta")
    }
}
