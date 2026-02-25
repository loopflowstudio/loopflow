#if os(iOS)
import Foundation
import LoopflowCore

@MainActor
@Observable
final class MobileConnectionProfilesStore {
    private let defaults: UserDefaults
    private let defaultsKey = "concerto.mobile.connectionProfiles.v1"

    private(set) var profiles: [ConnectionProfile] = []

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        profiles = loadProfiles()
    }

    func upsert(
        name: String,
        connection: ServerConnection
    ) -> ConnectionProfile {
        if let index = profiles.firstIndex(where: { $0.name.caseInsensitiveCompare(name) == .orderedSame }) {
            profiles[index].connection = connection
            profiles[index].lastConnectedAt = Date()
            persist()
            return profiles[index]
        }

        let profile = ConnectionProfile(
            name: name,
            connection: connection,
            lastConnectedAt: Date()
        )
        profiles.insert(profile, at: 0)
        persist()
        return profile
    }

    func delete(_ profile: ConnectionProfile) {
        profiles.removeAll { $0.id == profile.id }
        persist()
    }

    private func loadProfiles() -> [ConnectionProfile] {
        guard let data = defaults.data(forKey: defaultsKey) else { return [] }
        return (try? JSONDecoder().decode([ConnectionProfile].self, from: data)) ?? []
    }

    private func persist() {
        guard let data = try? JSONEncoder().encode(profiles) else { return }
        defaults.set(data, forKey: defaultsKey)
    }
}

#endif
