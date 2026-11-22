import Foundation
import Loopflow
import Observation

/// The paired Machine's `theme:`, as the phone last read it. Each Machine's last answer is kept
/// so launch draws it before the Machine answers again; before any Machine, Loopflow's own pair.
@Observable @MainActor
final class MachineTheme {
    private(set) var choice: ThemeChoice?
    private var machineID: String?

    /// Draw `machineID`'s last known choice, or Loopflow's pair when there is none.
    func show(machineID: String?) {
        self.machineID = machineID
        choice = machineID.flatMap { UserDefaults.standard.string(forKey: Self.key($0)) }.flatMap(ThemeChoice.parse)
    }

    /// Ask the Machine again; a failed read keeps what is drawn.
    func refresh(_ client: RemoteMobileClient) async {
        guard let raw = try? await client.themeChoice() else { return }
        remember(raw, machineID: client.savedMachine.machineID)
    }

    /// The Machine answered `raw` (nil: it names no theme).
    func remember(_ raw: String?, machineID: String) {
        UserDefaults.standard.set(raw, forKey: Self.key(machineID))
        if machineID == self.machineID { choice = raw.flatMap(ThemeChoice.parse) }
    }

    /// What the app draws in `dark` or light appearance. Names the phone doesn't ship (your own
    /// files, Ghostty themes) draw Loopflow's theme for the side.
    func theme(dark: Bool) -> ThemeDefinition {
        (choice ?? .pair(light: nil, dark: nil)).theme(dark: dark) { name in
            ThemeDefinition.builtins.first { $0.name == name }
        }
    }

    nonisolated static func declared(in answer: RawJSON) throws -> String? {
        struct Declaration: Decodable { let value: JSONValue }
        guard case .string(let value) = try answer.decode(Declaration.self).value else { return nil }
        return value
    }

    private static func key(_ machineID: String) -> String { "theme-\(machineID)" }
}
